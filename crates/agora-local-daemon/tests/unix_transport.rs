#![cfg(unix)]

use aegis_client::ServicePort;
use aegis_protocol::{
    ActionV1, CommandV1, DecisionHandleV1, DecisionSummaryV1, EngineNamespaceV1, OutcomeV1,
    ReplyV1, SessionHandleV1,
};
use agora_aegis_verifier::{AegisContextV1, AegisVerifierV1};
use agora_durable::{DurableHostConfigV2, DurableHostV2, DurableTransientHostV1};
use agora_engine::{CommandV2 as EngineCommand, Engine, Limits, Participant, ParticipantKind};
use agora_host::transient::TransientHostV1;
use agora_host::v2::HostV2;
use agora_host::v2::persistence::{HostCheckpointScopeV1, HostRecoveryLimitsV2};
use agora_host::{HostLimits, HostTimeV1, VerifiedSubjectV1};
use agora_local_daemon::{
    AuthenticatorV1, COLLABORATION_SERVER_HELLO_BYTES_V1, CollaborationServerHelloV1,
    DaemonErrorV1, DaemonLimitsV1, DaemonV1, HostClockV1, LOCAL_SESSION_HELLO_BYTES_V1,
    LocalReplayEpochV1, LocalRouteV1, SERVER_HELLO_BYTES_V1, ServerHelloV1, ServerIncarnationV1,
    decode_collaboration_server_hello_v1, decode_local_routed_frame_v1,
    decode_local_session_hello_v1, decode_server_hello_v1, encode_client_hello_v1,
    encode_collaboration_client_hello_v1, encode_local_routed_frame_v1,
};
use agora_protocol::transient::{
    PresenceStateV1, PresenceViewStateV1, TransientCommandV1, TransientOutcomeV1, TransientReplyV1,
    TransientRequestIdV1, TransientRequestV1, TypingTargetV1, TypingViewStateV1,
};
use agora_protocol::v2::{
    AdmissionPolicyV2, CommandV2 as ProtocolCommand, OutcomeV2, RefusalV2, ReplyV2, RequestIdV2,
    RequestV2,
};
use agora_service::transient::TransientLimitsV1;
use agora_service::typing::TypingLimitsV1;
use agora_service::v2::persistence::ServiceRecoveryLimitsV2;
use agora_service::v2::{ServiceLimitsV2, ServiceV2};
use agora_storage_directory::{DirectoryCheckpointStoreV1, DirectoryLimitsV1};
use agora_wire::WireLimits;
use agora_wire::transient::{decode_transient_reply_v1, encode_transient_request_v1};
use agora_wire::v2::{WireReplyV2, WireRequestV2, decode_reply_v2, encode_request_v2};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const MAX_DURABLE_SNAPSHOT_BYTES: usize = 2 * 1_024 * 1_024;
const DURABLE_HOST_SCOPE: HostCheckpointScopeV1 = HostCheckpointScopeV1::new(*b"daemon-host-v1!!");

#[derive(Clone, Default)]
struct Port {
    calls: Arc<Mutex<Vec<(SessionHandleV1, CommandV1)>>>,
}

impl ServicePort for Port {
    type Error = ();

    fn execute(
        &mut self,
        session: SessionHandleV1,
        command: CommandV1,
    ) -> Result<ReplyV1, Self::Error> {
        self.calls.lock().unwrap().push((session, command));
        Ok(ReplyV1::Accepted(OutcomeV1::Authorized(
            DecisionSummaryV1 {
                handle: DecisionHandleV1(1),
                epoch: 0,
                expires_at: 10_000,
                engine: EngineNamespaceV1::Agora,
                action: ActionV1::Submit,
            },
        )))
    }
}

struct Authentication;

impl AuthenticatorV1 for Authentication {
    type Error = ();

    fn authenticate(&mut self, _: &UnixStream) -> Result<AegisContextV1, Self::Error> {
        Ok(AegisContextV1::new(
            "alice",
            VerifiedSubjectV1::from_verifier([7; 16]),
            SessionHandleV1(3),
            aegis_protocol::GrantHandleV1(4),
        ))
    }
}

struct RejectingAuthentication;

struct CountingAuthentication(Arc<AtomicU64>);

impl AuthenticatorV1 for CountingAuthentication {
    type Error = ();

    fn authenticate(&mut self, _: &UnixStream) -> Result<AegisContextV1, Self::Error> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Ok(AegisContextV1::new(
            "alice",
            VerifiedSubjectV1::from_verifier([7; 16]),
            SessionHandleV1(3),
            aegis_protocol::GrantHandleV1(4),
        ))
    }
}

struct SecretAuthenticationError;

impl std::fmt::Debug for SecretAuthenticationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("credential-secret")
    }
}

impl AuthenticatorV1 for RejectingAuthentication {
    type Error = SecretAuthenticationError;

    fn authenticate(&mut self, _: &UnixStream) -> Result<AegisContextV1, Self::Error> {
        Err(SecretAuthenticationError)
    }
}

struct Clock(u64);

impl HostClockV1 for Clock {
    fn now(&mut self) -> HostTimeV1 {
        let now = self.0;
        self.0 += 1;
        HostTimeV1(now)
    }
}

fn wire_limits() -> WireLimits {
    WireLimits {
        max_frame_bytes: 8_192,
        max_text_bytes: 128,
        max_fields: 512,
        max_list_items: 32,
    }
}

fn host(port: Port) -> HostV2<AegisVerifierV1<Port>> {
    let mut engine = Engine::new();
    for (id, kind) in [
        ("alice", ParticipantKind::Human),
        ("bot", ParticipantKind::Agent),
    ] {
        engine
            .apply(EngineCommand::RegisterParticipant(Participant::new(
                id, id, kind,
            )))
            .unwrap();
    }
    HostV2::new(
        ServiceV2::new(engine, ServiceLimitsV2::default()),
        AegisVerifierV1::new(port, [0x55; 16]),
        HostLimits {
            max_sessions: 4,
            max_retired_sessions: 8,
            wire: wire_limits(),
        },
    )
}

fn daemon_limits() -> DaemonLimitsV1 {
    DaemonLimitsV1 {
        max_frame_bytes: 8_192,
        max_frames_after_connect: 8,
        io_timeout: Duration::from_secs(2),
    }
}

fn durable_config() -> DurableHostConfigV2 {
    DurableHostConfigV2::new(
        "agora-daemon-host",
        DURABLE_HOST_SCOPE,
        MAX_DURABLE_SNAPSHOT_BYTES,
        HostRecoveryLimitsV2 {
            max_snapshot_bytes: MAX_DURABLE_SNAPSHOT_BYTES,
            max_service_snapshot_bytes: MAX_DURABLE_SNAPSHOT_BYTES,
            maximum_host_limits: HostLimits {
                max_sessions: 4,
                max_retired_sessions: 8,
                wire: wire_limits(),
            },
            service: ServiceRecoveryLimitsV2 {
                max_snapshot_bytes: MAX_DURABLE_SNAPSHOT_BYTES,
                max_engine_snapshot_bytes: MAX_DURABLE_SNAPSHOT_BYTES,
                maximum_service_limits: ServiceLimitsV2::default(),
                maximum_engine_limits: Limits::default(),
            },
        },
    )
}

fn incarnation() -> ServerIncarnationV1 {
    ServerIncarnationV1::new([0xa5; 16])
}

fn next_incarnation() -> ServerIncarnationV1 {
    ServerIncarnationV1::new([0xb6; 16])
}

fn socket_path() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let directory = std::env::temp_dir().join(format!(
        "agora-local-daemon-{}-{}.sock",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    directory.join("agora.sock")
}

fn remove_parent(path: &Path) {
    fs::remove_dir(path.parent().unwrap()).unwrap();
}

fn lifetime_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap().to_os_string();
    name.push(".owner.lock");
    path.parent().unwrap().join(name)
}

fn fixture(input: &str) -> Vec<u8> {
    let compact: String = input
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    compact
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            u8::from_str_radix(std::str::from_utf8(pair).expect("ASCII fixture"), 16)
                .expect("hex fixture")
        })
        .collect()
}

fn write_frame(stream: &mut UnixStream, frame: &[u8]) {
    stream
        .write_all(&(frame.len() as u32).to_be_bytes())
        .unwrap();
    stream.write_all(frame).unwrap();
}

fn read_frame(stream: &mut UnixStream) -> Vec<u8> {
    let mut length = [0; 4];
    stream.read_exact(&mut length).unwrap();
    let mut frame = vec![0; u32::from_be_bytes(length) as usize];
    stream.read_exact(&mut frame).unwrap();
    frame
}

fn handshake(stream: &mut UnixStream) -> ServerHelloV1 {
    stream.write_all(&encode_client_hello_v1()).unwrap();
    let mut reply = [0_u8; SERVER_HELLO_BYTES_V1];
    stream.read_exact(&mut reply).unwrap();
    decode_server_hello_v1(&reply).unwrap()
}

fn collaboration_handshake(stream: &mut UnixStream) -> CollaborationServerHelloV1 {
    stream
        .write_all(&encode_collaboration_client_hello_v1())
        .unwrap();
    let mut reply = [0_u8; COLLABORATION_SERVER_HELLO_BYTES_V1];
    stream.read_exact(&mut reply).unwrap();
    decode_collaboration_server_hello_v1(&reply).unwrap()
}

fn write_routed(stream: &mut UnixStream, route: LocalRouteV1, payload: &[u8]) {
    let frame =
        encode_local_routed_frame_v1(route, payload, daemon_limits().max_frame_bytes).unwrap();
    write_frame(stream, &frame);
}

fn read_routed(stream: &mut UnixStream, expected: LocalRouteV1) -> Vec<u8> {
    let frame = read_frame(stream);
    let decoded = decode_local_routed_frame_v1(&frame, daemon_limits().max_frame_bytes).unwrap();
    assert_eq!(decoded.route(), expected);
    decoded.payload().to_vec()
}

fn collaboration_connect(
    stream: &mut UnixStream,
) -> (agora_protocol::v2::ClientIdV2, LocalReplayEpochV1) {
    write_routed(stream, LocalRouteV1::Conversation, &connect_frame("alice"));
    let WireReplyV2::Connected(client) = decode_reply_v2(
        &read_routed(stream, LocalRouteV1::Conversation),
        wire_limits(),
    )
    .unwrap() else {
        panic!("connected reply expected");
    };
    let control = read_routed(stream, LocalRouteV1::Control);
    assert_eq!(control.len(), LOCAL_SESSION_HELLO_BYTES_V1);
    (client, decode_local_session_hello_v1(&control).unwrap())
}

fn transient_frame(request_id: u64, command: TransientCommandV1) -> Vec<u8> {
    encode_transient_request_v1(
        &TransientRequestV1 {
            request_id: TransientRequestIdV1(request_id),
            command,
        },
        wire_limits(),
    )
    .unwrap()
}

fn connect_frame(participant_id: &str) -> Vec<u8> {
    encode_request_v2(
        &WireRequestV2::Connect {
            participant_id: participant_id.to_owned(),
        },
        wire_limits(),
    )
    .unwrap()
}

#[test]
fn idle_drain_is_irreversible_and_refuses_before_accept_or_authority() {
    let path = socket_path();
    let port = Port::default();
    let calls = port.calls.clone();
    let mut daemon = DaemonV1::bind_new(
        &path,
        host(port),
        Authentication,
        Clock(1),
        incarnation(),
        daemon_limits(),
    )
    .unwrap();

    assert!(daemon.health().ready);
    assert!(!daemon.health().draining);
    assert!(daemon.begin_drain());
    assert!(!daemon.begin_drain(), "drain transition must be idempotent");
    assert!(!daemon.health().ready);
    assert!(daemon.health().draining);
    assert!(matches!(daemon.serve_next(), Err(DaemonErrorV1::Draining)));

    let observation = daemon.observation();
    assert_eq!(observation.transport.accepted_connections, 0);
    assert_eq!(observation.host.open_offers, 0);
    assert_eq!(observation.service.connect_offers, 0);
    assert!(calls.lock().unwrap().is_empty());
    assert!(path.exists(), "drain retains endpoint ownership until drop");

    drop(daemon);
    assert!(!path.exists());
    remove_parent(&path);
}

#[test]
fn private_socket_runs_aegis_authorized_frames_and_cleans_up_on_eof() {
    let path = socket_path();
    let port = Port::default();
    let calls = port.calls.clone();
    let daemon = DaemonV1::bind_new(
        &path,
        host(port),
        Authentication,
        Clock(1),
        incarnation(),
        daemon_limits(),
    )
    .unwrap();
    let metadata = fs::symlink_metadata(&path).unwrap();
    assert!(metadata.file_type().is_socket());
    assert_eq!(metadata.permissions().mode() & 0o777, 0o600);

    let server = thread::spawn(move || {
        let mut daemon = daemon;
        let result = daemon.serve_next();
        (result, daemon)
    });
    let mut stream = UnixStream::connect(&path).unwrap();
    let hello = handshake(&mut stream);
    assert_eq!(hello.max_frame_bytes(), 8_192);
    assert_eq!(hello.max_frames_after_connect(), 8);
    assert_eq!(hello.server_incarnation(), incarnation());
    assert!(hello.supports_capability_discovery());
    assert!(!format!("{hello:?}").contains("a5"));
    let connect = connect_frame("alice");
    write_frame(&mut stream, &connect);
    let WireReplyV2::Connected(client) =
        decode_reply_v2(&read_frame(&mut stream), wire_limits()).unwrap()
    else {
        panic!("connected reply expected");
    };

    let create = encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(0),
                command: ProtocolCommand::CreateRoom {
                    room_id: "room".to_owned(),
                    name: "Room".to_owned(),
                    admission: AdmissionPolicyV2::Open,
                    expected_revision: 2,
                },
            },
        },
        wire_limits(),
    )
    .unwrap();
    write_frame(&mut stream, &create);
    let first = read_frame(&mut stream);
    assert!(matches!(
        decode_reply_v2(&first, wire_limits()).unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::RoomCreated { revision: 3 }))
    ));
    write_frame(&mut stream, &create);
    assert_eq!(
        read_frame(&mut stream),
        first,
        "exact replay must be stable"
    );
    drop(stream);

    let (result, daemon) = server.join().unwrap();
    result.unwrap();
    assert_eq!(daemon.host().service().active_client_count(), 0);
    assert_eq!(daemon.health().accepted_connections, 1);
    assert_eq!(daemon.health().handshake_accepted, 1);
    assert_eq!(daemon.health().handshake_refused, 0);
    assert_eq!(daemon.health().completed_connections, 1);
    assert_eq!(daemon.health().active_connections, 0);
    let observation = daemon.observation();
    assert_eq!(
        observation,
        daemon.observation(),
        "observation must be passive"
    );
    assert_eq!(observation.transport, daemon.health());
    assert_eq!(observation.host.open_offers, 1);
    assert_eq!(observation.host.authenticated, 1);
    assert_eq!(observation.host.frame_offers, 2);
    assert_eq!(observation.host.executed, 2);
    assert_eq!(observation.service.connect_offers, 1);
    assert_eq!(observation.service.connected, 1);
    assert_eq!(observation.service.request_offers, 2);
    assert_eq!(observation.service.accepted, 1);
    assert_eq!(observation.service.replayed, 1);
    assert_eq!(observation.service.disconnected, 1);
    assert!(!observation.fidelity_lost);
    let diagnostic = format!("{observation:?}");
    assert!(!diagnostic.contains("alice"));
    assert!(!diagnostic.contains("room"));
    assert!(!diagnostic.contains("Room"));
    assert_eq!(calls.lock().unwrap().len(), 3);
    drop(daemon);
    assert!(!path.exists());
    remove_parent(&path);
}

#[test]
fn collaboration_profile_multiplexes_transient_frames_and_rotates_connection_epochs() {
    let path = socket_path();
    let transient_host = TransientHostV1::new_with_typing(
        host(Port::default()),
        TransientLimitsV1::default(),
        TypingLimitsV1::default(),
        10,
    )
    .unwrap();
    let daemon = DaemonV1::bind_new_collaboration(
        &path,
        transient_host,
        Authentication,
        Clock(1),
        incarnation(),
        daemon_limits(),
    )
    .unwrap();
    let server = thread::spawn(move || {
        let mut daemon = daemon;
        let first = daemon.serve_next();
        let second = daemon.serve_next();
        (first, second, daemon)
    });

    let mut first = UnixStream::connect(&path).unwrap();
    let hello = collaboration_handshake(&mut first);
    assert_eq!(hello.server_incarnation(), incarnation());
    assert_eq!(hello.max_frame_bytes(), 8_192);
    assert_eq!(hello.max_frames_after_connect(), 8);
    assert!(hello.supports_capability_discovery());
    assert!(hello.supports_transient_collaboration());
    assert!(!format!("{hello:?}").contains("a5"));
    let (client, first_epoch) = collaboration_connect(&mut first);
    let first_epoch_bytes = first_epoch.into_bytes();
    assert_eq!(&first_epoch_bytes[..16], &incarnation().as_bytes());
    assert_ne!(&first_epoch_bytes[16..], &[0; 16]);
    assert!(!format!("{first_epoch:?}").contains("a5"));

    let create = encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(0),
                command: ProtocolCommand::CreateRoom {
                    room_id: "room".to_owned(),
                    name: "Room".to_owned(),
                    admission: AdmissionPolicyV2::Open,
                    expected_revision: 2,
                },
            },
        },
        wire_limits(),
    )
    .unwrap();
    write_routed(&mut first, LocalRouteV1::Conversation, &create);
    assert!(matches!(
        decode_reply_v2(
            &read_routed(&mut first, LocalRouteV1::Conversation),
            wire_limits()
        )
        .unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::RoomCreated { revision: 3 }))
    ));

    let publish_presence = transient_frame(
        0,
        TransientCommandV1::PublishPresence {
            room_id: "room".to_owned(),
            state: PresenceStateV1::Available,
            lifetime: 20,
        },
    );
    write_routed(&mut first, LocalRouteV1::Transient, &publish_presence);
    assert_eq!(
        decode_transient_reply_v1(
            &read_routed(&mut first, LocalRouteV1::Transient),
            wire_limits()
        )
        .unwrap(),
        TransientReplyV1::Accepted(TransientOutcomeV1::PresencePublished {
            accepted_lifetime: 20,
        })
    );
    let publish_typing = transient_frame(
        1,
        TransientCommandV1::PublishTyping {
            room_id: "room".to_owned(),
            target: TypingTargetV1::Room,
            lifetime: 20,
        },
    );
    write_routed(&mut first, LocalRouteV1::Transient, &publish_typing);
    assert_eq!(
        decode_transient_reply_v1(
            &read_routed(&mut first, LocalRouteV1::Transient),
            wire_limits()
        )
        .unwrap(),
        TransientReplyV1::Accepted(TransientOutcomeV1::TypingPublished {
            accepted_lifetime: 20,
        })
    );
    drop(first);

    let mut second = UnixStream::connect(&path).unwrap();
    let second_hello = collaboration_handshake(&mut second);
    assert_eq!(second_hello.server_incarnation(), incarnation());
    let (_, second_epoch) = collaboration_connect(&mut second);
    assert_ne!(second_epoch, first_epoch);
    assert_eq!(&second_epoch.into_bytes()[..16], &incarnation().as_bytes());

    let presence_page = transient_frame(
        0,
        TransientCommandV1::PresencePage {
            room_id: "room".to_owned(),
            cursor: None,
            limit: 8,
        },
    );
    write_routed(&mut second, LocalRouteV1::Transient, &presence_page);
    let TransientReplyV1::Accepted(TransientOutcomeV1::PresencePage(page)) =
        decode_transient_reply_v1(
            &read_routed(&mut second, LocalRouteV1::Transient),
            wire_limits(),
        )
        .unwrap()
    else {
        panic!("presence page expected");
    };
    assert_eq!(page.entries.len(), 1);
    assert_eq!(page.entries[0].participant_id, "alice");
    assert_eq!(page.entries[0].state, PresenceViewStateV1::Unknown);

    let typing_page = transient_frame(
        1,
        TransientCommandV1::TypingPage {
            room_id: "room".to_owned(),
            target: TypingTargetV1::Room,
            cursor: None,
            limit: 8,
        },
    );
    write_routed(&mut second, LocalRouteV1::Transient, &typing_page);
    let TransientReplyV1::Accepted(TransientOutcomeV1::TypingPage(page)) =
        decode_transient_reply_v1(
            &read_routed(&mut second, LocalRouteV1::Transient),
            wire_limits(),
        )
        .unwrap()
    else {
        panic!("typing page expected");
    };
    assert_eq!(page.entries.len(), 1);
    assert_eq!(page.entries[0].participant_id, "alice");
    assert_eq!(page.entries[0].state, TypingViewStateV1::Inactive);
    drop(second);

    let (first_result, second_result, daemon) = server.join().unwrap();
    first_result.unwrap();
    second_result.unwrap();
    assert_eq!(daemon.health().handshake_accepted, 2);
    assert_eq!(daemon.health().completed_connections, 2);
    assert_eq!(daemon.host().host().service().active_client_count(), 0);
    assert!(!format!("{:?}", daemon.host()).contains("alice"));
    drop(daemon);
    assert!(!path.exists());

    let restarted_host = TransientHostV1::new_with_typing(
        host(Port::default()),
        TransientLimitsV1::default(),
        TypingLimitsV1::default(),
        10,
    )
    .unwrap();
    let restarted = DaemonV1::bind_new_collaboration(
        &path,
        restarted_host,
        Authentication,
        Clock(100),
        next_incarnation(),
        daemon_limits(),
    )
    .unwrap();
    let restarted_server = thread::spawn(move || {
        let mut daemon = restarted;
        let result = daemon.serve_next();
        (result, daemon)
    });
    let mut restarted_stream = UnixStream::connect(&path).unwrap();
    let restarted_hello = collaboration_handshake(&mut restarted_stream);
    assert_eq!(restarted_hello.server_incarnation(), next_incarnation());
    let (_, restarted_epoch) = collaboration_connect(&mut restarted_stream);
    assert_ne!(restarted_epoch, first_epoch);
    assert_ne!(restarted_epoch, second_epoch);
    drop(restarted_stream);
    let (restarted_result, restarted) = restarted_server.join().unwrap();
    restarted_result.unwrap();
    let diagnostic = format!("{:?}", restarted.host());
    assert!(diagnostic.contains("presence_signal_count: 0"));
    assert!(diagnostic.contains("typing_signal_count: 0"));
    drop(restarted);
    assert!(!path.exists());
    remove_parent(&path);
}

#[test]
fn durable_daemon_reopens_conversation_state_through_the_same_transport_profile() {
    let path = socket_path();
    let store_path = path.parent().unwrap().join("checkpoint");
    let store =
        DirectoryCheckpointStoreV1::create_new(&store_path, DirectoryLimitsV1::default()).unwrap();
    let durable =
        DurableHostV2::initialize(host(Port::default()), store, durable_config()).unwrap();
    let daemon = DaemonV1::bind_new(
        &path,
        durable,
        Authentication,
        Clock(1),
        incarnation(),
        daemon_limits(),
    )
    .unwrap();
    let server = thread::spawn(move || {
        let mut daemon = daemon;
        let result = daemon.serve_next();
        (result, daemon)
    });
    let mut stream = UnixStream::connect(&path).unwrap();
    handshake(&mut stream);
    write_frame(&mut stream, &connect_frame("alice"));
    let WireReplyV2::Connected(client) =
        decode_reply_v2(&read_frame(&mut stream), wire_limits()).unwrap()
    else {
        panic!("connected reply expected");
    };
    let create = encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(0),
                command: ProtocolCommand::CreateRoom {
                    room_id: "durable-room".to_owned(),
                    name: "Durable Room".to_owned(),
                    admission: AdmissionPolicyV2::Open,
                    expected_revision: 2,
                },
            },
        },
        wire_limits(),
    )
    .unwrap();
    write_frame(&mut stream, &create);
    assert!(matches!(
        decode_reply_v2(&read_frame(&mut stream), wire_limits()).unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::RoomCreated { revision: 3 }))
    ));
    drop(stream);
    let (result, daemon) = server.join().unwrap();
    result.unwrap();
    assert!(daemon.observation().is_some());
    assert!(
        daemon
            .host()
            .host()
            .unwrap()
            .service()
            .engine()
            .room(&"durable-room".into())
            .is_some()
    );
    drop(daemon);
    assert!(!path.exists());

    let reopened =
        DirectoryCheckpointStoreV1::open_existing(&store_path, DirectoryLimitsV1::default())
            .unwrap();
    let durable = DurableHostV2::recover(
        reopened,
        durable_config(),
        AegisVerifierV1::new(Port::default(), [0x55; 16]),
    )
    .unwrap();
    let daemon = DaemonV1::bind_new(
        &path,
        durable,
        Authentication,
        Clock(10),
        next_incarnation(),
        daemon_limits(),
    )
    .unwrap();
    let server = thread::spawn(move || {
        let mut daemon = daemon;
        let result = daemon.serve_next();
        (result, daemon)
    });
    let mut stream = UnixStream::connect(&path).unwrap();
    assert_eq!(
        handshake(&mut stream).server_incarnation(),
        next_incarnation()
    );
    write_frame(&mut stream, &connect_frame("alice"));
    let WireReplyV2::Connected(client) =
        decode_reply_v2(&read_frame(&mut stream), wire_limits()).unwrap()
    else {
        panic!("recovered connection expected");
    };
    let rooms = encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(0),
                command: ProtocolCommand::Rooms {
                    cursor: None,
                    limit: 4,
                },
            },
        },
        wire_limits(),
    )
    .unwrap();
    write_frame(&mut stream, &rooms);
    let WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::Rooms(page))) =
        decode_reply_v2(&read_frame(&mut stream), wire_limits()).unwrap()
    else {
        panic!("recovered room page expected");
    };
    assert_eq!(page.rooms.len(), 1);
    assert_eq!(page.rooms[0].room_id, "durable-room");
    drop(stream);
    let (result, daemon) = server.join().unwrap();
    result.unwrap();
    drop(daemon);
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn durable_collaboration_recovers_from_exact_residue_with_transients_empty() {
    let path = socket_path();
    let lock_path = lifetime_path(&path);
    let store_path = path.parent().unwrap().join("collaboration-checkpoint");
    let store =
        DirectoryCheckpointStoreV1::create_new(&store_path, DirectoryLimitsV1::default()).unwrap();
    let transient = TransientHostV1::new_with_typing(
        host(Port::default()),
        TransientLimitsV1::default(),
        TypingLimitsV1::default(),
        10,
    )
    .unwrap();
    let durable = DurableTransientHostV1::initialize(transient, store, durable_config()).unwrap();
    let daemon = DaemonV1::bind_new_durable_collaboration(
        &path,
        durable,
        Authentication,
        Clock(1),
        incarnation(),
        daemon_limits(),
    )
    .unwrap();
    let server = thread::spawn(move || {
        let mut daemon = daemon;
        let result = daemon.serve_next();
        (result, daemon)
    });
    let mut stream = UnixStream::connect(&path).unwrap();
    collaboration_handshake(&mut stream);
    let (client, first_epoch) = collaboration_connect(&mut stream);
    let create = encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(0),
                command: ProtocolCommand::CreateRoom {
                    room_id: "durable-collaboration".to_owned(),
                    name: "Durable Collaboration".to_owned(),
                    admission: AdmissionPolicyV2::Open,
                    expected_revision: 2,
                },
            },
        },
        wire_limits(),
    )
    .unwrap();
    write_routed(&mut stream, LocalRouteV1::Conversation, &create);
    assert!(matches!(
        decode_reply_v2(
            &read_routed(&mut stream, LocalRouteV1::Conversation),
            wire_limits(),
        )
        .unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::RoomCreated { revision: 3 }))
    ));
    for (request_id, command, expected) in [
        (
            0,
            TransientCommandV1::PublishPresence {
                room_id: "durable-collaboration".to_owned(),
                state: PresenceStateV1::Available,
                lifetime: 20,
            },
            TransientOutcomeV1::PresencePublished {
                accepted_lifetime: 20,
            },
        ),
        (
            1,
            TransientCommandV1::PublishTyping {
                room_id: "durable-collaboration".to_owned(),
                target: TypingTargetV1::Room,
                lifetime: 20,
            },
            TransientOutcomeV1::TypingPublished {
                accepted_lifetime: 20,
            },
        ),
    ] {
        write_routed(
            &mut stream,
            LocalRouteV1::Transient,
            &transient_frame(request_id, command),
        );
        assert_eq!(
            decode_transient_reply_v1(
                &read_routed(&mut stream, LocalRouteV1::Transient),
                wire_limits(),
            )
            .unwrap(),
            TransientReplyV1::Accepted(expected)
        );
    }
    drop(stream);
    let (result, daemon) = server.join().unwrap();
    result.unwrap();
    assert!(daemon.observation().is_some());
    drop(daemon);
    assert!(!path.exists());

    let stale_listener = UnixListener::bind(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    drop(stale_listener);
    fs::write(
        &lock_path,
        fixture(include_str!("fixtures/lifetime-record-v1.hex")),
    )
    .unwrap();
    fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o600)).unwrap();

    let reopened =
        DirectoryCheckpointStoreV1::open_existing(&store_path, DirectoryLimitsV1::default())
            .unwrap();
    let durable = DurableTransientHostV1::recover(
        reopened,
        durable_config(),
        AegisVerifierV1::new(Port::default(), [0x55; 16]),
        TransientLimitsV1::default(),
        TypingLimitsV1::default(),
        10,
    )
    .unwrap();
    let daemon = DaemonV1::recover_stale_durable_collaboration(
        &path,
        durable,
        Authentication,
        Clock(100),
        incarnation(),
        next_incarnation(),
        daemon_limits(),
    )
    .unwrap();
    let server = thread::spawn(move || {
        let mut daemon = daemon;
        let result = daemon.serve_next();
        (result, daemon)
    });
    let mut stream = UnixStream::connect(&path).unwrap();
    assert_eq!(
        collaboration_handshake(&mut stream).server_incarnation(),
        next_incarnation()
    );
    let (client, restarted_epoch) = collaboration_connect(&mut stream);
    assert_ne!(restarted_epoch, first_epoch);

    let rooms = encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(0),
                command: ProtocolCommand::Rooms {
                    cursor: None,
                    limit: 4,
                },
            },
        },
        wire_limits(),
    )
    .unwrap();
    write_routed(&mut stream, LocalRouteV1::Conversation, &rooms);
    let WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::Rooms(page))) = decode_reply_v2(
        &read_routed(&mut stream, LocalRouteV1::Conversation),
        wire_limits(),
    )
    .unwrap() else {
        panic!("recovered room page expected");
    };
    assert_eq!(page.rooms.len(), 1);
    assert_eq!(page.rooms[0].room_id, "durable-collaboration");

    for (request_id, command) in [
        (
            0,
            TransientCommandV1::PresencePage {
                room_id: "durable-collaboration".to_owned(),
                cursor: None,
                limit: 8,
            },
        ),
        (
            1,
            TransientCommandV1::TypingPage {
                room_id: "durable-collaboration".to_owned(),
                target: TypingTargetV1::Room,
                cursor: None,
                limit: 8,
            },
        ),
    ] {
        write_routed(
            &mut stream,
            LocalRouteV1::Transient,
            &transient_frame(request_id, command),
        );
        let reply = decode_transient_reply_v1(
            &read_routed(&mut stream, LocalRouteV1::Transient),
            wire_limits(),
        )
        .unwrap();
        match reply {
            TransientReplyV1::Accepted(TransientOutcomeV1::PresencePage(page)) => {
                assert_eq!(page.entries.len(), 1);
                assert_eq!(page.entries[0].state, PresenceViewStateV1::Unknown);
            }
            TransientReplyV1::Accepted(TransientOutcomeV1::TypingPage(page)) => {
                assert_eq!(page.entries.len(), 1);
                assert_eq!(page.entries[0].state, TypingViewStateV1::Inactive);
            }
            other => panic!("empty recovered transient page expected: {other:?}"),
        }
    }
    drop(stream);
    let (result, daemon) = server.join().unwrap();
    result.unwrap();
    drop(daemon);
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn participant_mismatch_returns_shared_refusal_without_authority_io() {
    let path = socket_path();
    let port = Port::default();
    let calls = port.calls.clone();
    let daemon = DaemonV1::bind_new(
        &path,
        host(port),
        Authentication,
        Clock(1),
        incarnation(),
        daemon_limits(),
    )
    .unwrap();
    let server = thread::spawn(move || {
        let mut daemon = daemon;
        let result = daemon.serve_next();
        (result, daemon)
    });
    let mut stream = UnixStream::connect(&path).unwrap();
    handshake(&mut stream);
    write_frame(&mut stream, &connect_frame("mallory"));
    assert_eq!(
        decode_reply_v2(&read_frame(&mut stream), wire_limits()).unwrap(),
        WireReplyV2::ConnectRefused(RefusalV2::Unavailable)
    );

    let (result, daemon) = server.join().unwrap();
    assert!(matches!(result, Err(DaemonErrorV1::ConnectionRefused)));
    assert_eq!(calls.lock().unwrap().len(), 0);
    assert_eq!(daemon.host().service().active_client_count(), 0);
    assert_eq!(daemon.health().refused_connections, 1);
    assert_eq!(daemon.health().handshake_accepted, 1);
    assert_eq!(daemon.health().handshake_refused, 0);
    assert_eq!(daemon.health().completed_connections, 0);
    drop(daemon);
    remove_parent(&path);
}

#[test]
fn authentication_failure_erases_adapter_detail() {
    let path = socket_path();
    let daemon = DaemonV1::bind_new(
        &path,
        host(Port::default()),
        RejectingAuthentication,
        Clock(1),
        incarnation(),
        daemon_limits(),
    )
    .unwrap();
    let server = thread::spawn(move || {
        let mut daemon = daemon;
        let result = daemon.serve_next();
        (result, daemon)
    });
    let mut stream = UnixStream::connect(&path).unwrap();
    handshake(&mut stream);
    let (result, daemon) = server.join().unwrap();
    drop(stream);
    assert!(matches!(result, Err(DaemonErrorV1::Authentication)));
    assert!(!format!("{result:?}").contains("credential-secret"));
    assert_eq!(daemon.health().refused_connections, 1);
    assert_eq!(daemon.health().completed_connections, 0);
    drop(daemon);
    remove_parent(&path);
}

#[test]
fn incompatible_handshake_refuses_before_authentication_authority_or_host() {
    let path = socket_path();
    let port = Port::default();
    let calls = port.calls.clone();
    let authentication_calls = Arc::new(AtomicU64::new(0));
    let daemon = DaemonV1::bind_new(
        &path,
        host(port),
        CountingAuthentication(authentication_calls.clone()),
        Clock(1),
        incarnation(),
        daemon_limits(),
    )
    .unwrap();
    let server = thread::spawn(move || {
        let mut daemon = daemon;
        let result = daemon.serve_next();
        (result, daemon)
    });
    let mut stream = UnixStream::connect(&path).unwrap();
    let mut incompatible = encode_client_hello_v1();
    incompatible[11] = 3;
    stream.write_all(&incompatible).unwrap();

    let (result, daemon) = server.join().unwrap();
    assert!(matches!(result, Err(DaemonErrorV1::Handshake)));
    assert_eq!(authentication_calls.load(Ordering::Relaxed), 0);
    assert!(calls.lock().unwrap().is_empty());
    assert_eq!(daemon.observation().host.open_offers, 0);
    assert_eq!(daemon.observation().service.connect_offers, 0);
    assert_eq!(daemon.health().refused_connections, 1);
    assert_eq!(daemon.health().handshake_accepted, 0);
    assert_eq!(daemon.health().handshake_refused, 1);
    drop(stream);
    drop(daemon);
    remove_parent(&path);
}

#[test]
fn lifetime_record_excludes_live_and_stale_owners_and_fences_replacement() {
    let path = socket_path();
    let lock_path = lifetime_path(&path);
    let daemon = DaemonV1::bind_new(
        &path,
        host(Port::default()),
        Authentication,
        Clock(1),
        incarnation(),
        daemon_limits(),
    )
    .unwrap();
    let lock_metadata = fs::symlink_metadata(&lock_path).unwrap();
    assert!(lock_metadata.file_type().is_file());
    assert_eq!(lock_metadata.permissions().mode() & 0o777, 0o600);
    assert_eq!(
        fs::read(&lock_path).unwrap(),
        fixture(include_str!("fixtures/lifetime-record-v1.hex"))
    );
    assert!(matches!(
        DaemonV1::bind_new(
            &path,
            host(Port::default()),
            Authentication,
            Clock(1),
            incarnation(),
            daemon_limits(),
        ),
        Err(DaemonErrorV1::LifetimeBusy)
    ));
    drop(daemon);
    assert!(!path.exists());
    assert!(!lock_path.exists());

    fs::write(
        &lock_path,
        fixture(include_str!("fixtures/lifetime-record-v1.hex")),
    )
    .unwrap();
    fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(matches!(
        DaemonV1::bind_new(
            &path,
            host(Port::default()),
            Authentication,
            Clock(1),
            incarnation(),
            daemon_limits(),
        ),
        Err(DaemonErrorV1::CrashResidue)
    ));
    assert!(
        lock_path.exists(),
        "residue refusal must not repair implicitly"
    );
    fs::remove_file(&lock_path).unwrap();

    let authentication_calls = Arc::new(AtomicU64::new(0));
    let daemon = DaemonV1::bind_new(
        &path,
        host(Port::default()),
        CountingAuthentication(authentication_calls.clone()),
        Clock(1),
        incarnation(),
        daemon_limits(),
    )
    .unwrap();
    let displaced_lock = path.parent().unwrap().join("displaced-owner.lock");
    fs::rename(&lock_path, &displaced_lock).unwrap();
    fs::write(&lock_path, b"replacement").unwrap();
    fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o600)).unwrap();

    let server = thread::spawn(move || {
        let mut daemon = daemon;
        let result = daemon.serve_next();
        (result, daemon)
    });
    let stream = UnixStream::connect(&path).unwrap();
    let (result, daemon) = server.join().unwrap();
    assert!(matches!(result, Err(DaemonErrorV1::Fenced)));
    assert_eq!(authentication_calls.load(Ordering::Relaxed), 0);
    assert!(daemon.health().fenced);
    assert!(!daemon.health().ready);
    assert_eq!(daemon.health().accepted_connections, 0);
    assert_eq!(daemon.health().refused_connections, 0);
    drop(stream);
    drop(daemon);
    assert!(
        path.exists(),
        "fenced teardown must preserve endpoint residue"
    );
    assert!(lock_path.exists(), "replacement lock must be preserved");
    assert!(
        displaced_lock.exists(),
        "displaced owned lock must be preserved"
    );
    fs::remove_file(&path).unwrap();
    fs::remove_file(&lock_path).unwrap();
    fs::remove_file(&displaced_lock).unwrap();
    remove_parent(&path);
}

#[test]
fn explicit_residue_recovery_rotates_incarnation_without_adopting_runtime_state() {
    let path = socket_path();
    let lock_path = lifetime_path(&path);
    let daemon = DaemonV1::bind_new(
        &path,
        host(Port::default()),
        Authentication,
        Clock(1),
        incarnation(),
        daemon_limits(),
    )
    .unwrap();
    assert!(matches!(
        DaemonV1::recover_stale(
            &path,
            host(Port::default()),
            Authentication,
            Clock(1),
            incarnation(),
            next_incarnation(),
            daemon_limits(),
        ),
        Err(DaemonErrorV1::LifetimeBusy)
    ));
    drop(daemon);

    let stale_listener = UnixListener::bind(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    drop(stale_listener);
    fs::write(
        &lock_path,
        fixture(include_str!("fixtures/lifetime-record-v1.hex")),
    )
    .unwrap();
    fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o600)).unwrap();

    assert!(matches!(
        DaemonV1::recover_stale(
            &path,
            host(Port::default()),
            Authentication,
            Clock(1),
            incarnation(),
            incarnation(),
            daemon_limits(),
        ),
        Err(DaemonErrorV1::ResidueMismatch)
    ));

    assert!(matches!(
        DaemonV1::recover_stale(
            &path,
            host(Port::default()),
            Authentication,
            Clock(1),
            ServerIncarnationV1::new([0xc7; 16]),
            next_incarnation(),
            daemon_limits(),
        ),
        Err(DaemonErrorV1::ResidueMismatch)
    ));
    assert!(path.exists());
    assert_eq!(
        fs::read(&lock_path).unwrap(),
        fixture(include_str!("fixtures/lifetime-record-v1.hex"))
    );

    let mut malformed = fixture(include_str!("fixtures/lifetime-record-v1.hex"));
    malformed[0] ^= 1;
    fs::write(&lock_path, &malformed).unwrap();
    assert!(matches!(
        DaemonV1::recover_stale(
            &path,
            host(Port::default()),
            Authentication,
            Clock(1),
            incarnation(),
            next_incarnation(),
            daemon_limits(),
        ),
        Err(DaemonErrorV1::ResidueMismatch)
    ));
    fs::write(
        &lock_path,
        fixture(include_str!("fixtures/lifetime-record-v1.hex")),
    )
    .unwrap();

    let daemon = DaemonV1::recover_stale(
        &path,
        host(Port::default()),
        Authentication,
        Clock(1),
        incarnation(),
        next_incarnation(),
        daemon_limits(),
    )
    .unwrap();
    let mut expected = b"AGORALF1".to_vec();
    expected.extend_from_slice(&[0xb6; 16]);
    assert_eq!(fs::read(&lock_path).unwrap(), expected);
    assert_eq!(daemon.observation().host.open_offers, 0);
    assert_eq!(daemon.observation().service.connect_offers, 0);

    let server = thread::spawn(move || {
        let mut daemon = daemon;
        let result = daemon.serve_next();
        (result, daemon)
    });
    let mut stream = UnixStream::connect(&path).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    assert_eq!(
        handshake(&mut stream).server_incarnation(),
        next_incarnation()
    );
    drop(stream);
    let (result, daemon) = server.join().unwrap();
    assert!(matches!(result, Err(DaemonErrorV1::Io(_))));
    drop(daemon);
    assert!(!path.exists());
    assert!(!lock_path.exists());
    remove_parent(&path);
}

#[test]
fn request_budget_closes_the_admitted_session_after_its_last_reply() {
    let path = socket_path();
    let daemon = DaemonV1::bind_new(
        &path,
        host(Port::default()),
        Authentication,
        Clock(1),
        incarnation(),
        DaemonLimitsV1 {
            max_frames_after_connect: 1,
            ..daemon_limits()
        },
    )
    .unwrap();
    let server = thread::spawn(move || {
        let mut daemon = daemon;
        let result = daemon.serve_next();
        (result, daemon)
    });
    let mut stream = UnixStream::connect(&path).unwrap();
    handshake(&mut stream);
    write_frame(&mut stream, &connect_frame("alice"));
    let WireReplyV2::Connected(client) =
        decode_reply_v2(&read_frame(&mut stream), wire_limits()).unwrap()
    else {
        panic!("connected reply expected");
    };
    let capabilities = encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(0),
                command: ProtocolCommand::Capabilities,
            },
        },
        wire_limits(),
    )
    .unwrap();
    write_frame(&mut stream, &capabilities);
    assert!(matches!(
        decode_reply_v2(&read_frame(&mut stream), wire_limits()).unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::Capabilities(_)))
    ));

    let (result, daemon) = server.join().unwrap();
    assert!(matches!(result, Err(DaemonErrorV1::RequestLimit)));
    assert_eq!(daemon.host().service().active_client_count(), 0);
    assert_eq!(daemon.health().transport_errors, 1);
    assert_eq!(daemon.health().completed_connections, 0);
    drop(daemon);
    remove_parent(&path);
}

#[test]
fn oversized_frame_refuses_before_host_session_and_replacement_path_is_preserved() {
    let path = socket_path();
    let daemon = DaemonV1::bind_new(
        &path,
        host(Port::default()),
        Authentication,
        Clock(1),
        incarnation(),
        DaemonLimitsV1 {
            max_frame_bytes: 32,
            ..daemon_limits()
        },
    )
    .unwrap();
    let server = thread::spawn(move || {
        let mut daemon = daemon;
        let result = daemon.serve_next();
        (result, daemon)
    });
    let mut stream = UnixStream::connect(&path).unwrap();
    handshake(&mut stream);
    stream.write_all(&33_u32.to_be_bytes()).unwrap();
    let (result, daemon) = server.join().unwrap();
    drop(stream);
    assert!(
        matches!(result, Err(DaemonErrorV1::FrameLimit)),
        "unexpected result: {result:?}"
    );
    assert_eq!(daemon.host().service().active_client_count(), 0);
    assert_eq!(daemon.health().transport_errors, 1);
    assert_eq!(daemon.health().completed_connections, 0);
    drop(daemon);

    fs::write(&path, b"replacement").unwrap();
    let occupied = DaemonV1::bind_new(
        &path,
        host(Port::default()),
        Authentication,
        Clock(1),
        incarnation(),
        daemon_limits(),
    );
    assert!(matches!(occupied, Err(DaemonErrorV1::Io(_))));
    assert_eq!(fs::read(&path).unwrap(), b"replacement");
    fs::remove_file(&path).unwrap();
    remove_parent(&path);

    let invalid_incarnation_path = socket_path();
    assert!(matches!(
        DaemonV1::bind_new(
            &invalid_incarnation_path,
            host(Port::default()),
            Authentication,
            Clock(1),
            ServerIncarnationV1::new([0; 16]),
            daemon_limits(),
        ),
        Err(DaemonErrorV1::InvalidLimits)
    ));
    assert!(!invalid_incarnation_path.exists());
    remove_parent(&invalid_incarnation_path);

    let unsafe_path = socket_path();
    fs::set_permissions(
        unsafe_path.parent().unwrap(),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    assert!(matches!(
        DaemonV1::bind_new(
            &unsafe_path,
            host(Port::default()),
            Authentication,
            Clock(1),
            incarnation(),
            daemon_limits(),
        ),
        Err(DaemonErrorV1::UnsafeEndpoint)
    ));
    remove_parent(&unsafe_path);
}
