#![cfg(unix)]

use aegis_client::ServicePort;
use aegis_protocol::{
    ActionV1, CommandV1, DecisionHandleV1, DecisionSummaryV1, EngineNamespaceV1, OutcomeV1,
    ReplyV1, SessionHandleV1,
};
use agora_aegis_verifier::{AegisContextV1, AegisVerifierV1};
use agora_engine::{CommandV2 as EngineCommand, Engine, Participant, ParticipantKind};
use agora_host::v2::HostV2;
use agora_host::{HostLimits, HostTimeV1, VerifiedSubjectV1};
use agora_local_daemon::{AuthenticatorV1, DaemonErrorV1, DaemonLimitsV1, DaemonV1, HostClockV1};
use agora_protocol::v2::{
    AdmissionPolicyV2, CommandV2 as ProtocolCommand, OutcomeV2, RefusalV2, ReplyV2, RequestIdV2,
    RequestV2,
};
use agora_service::v2::{ServiceLimitsV2, ServiceV2};
use agora_wire::WireLimits;
use agora_wire::v2::{WireReplyV2, WireRequestV2, decode_reply_v2, encode_request_v2};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

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
    let mut daemon =
        DaemonV1::bind_new(&path, host(port), Authentication, Clock(1), daemon_limits()).unwrap();

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
    let daemon =
        DaemonV1::bind_new(&path, host(port), Authentication, Clock(1), daemon_limits()).unwrap();
    let metadata = fs::symlink_metadata(&path).unwrap();
    assert!(metadata.file_type().is_socket());
    assert_eq!(metadata.permissions().mode() & 0o777, 0o600);

    let server = thread::spawn(move || {
        let mut daemon = daemon;
        let result = daemon.serve_next();
        (result, daemon)
    });
    let mut stream = UnixStream::connect(&path).unwrap();
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
fn participant_mismatch_returns_shared_refusal_without_authority_io() {
    let path = socket_path();
    let port = Port::default();
    let calls = port.calls.clone();
    let daemon =
        DaemonV1::bind_new(&path, host(port), Authentication, Clock(1), daemon_limits()).unwrap();
    let server = thread::spawn(move || {
        let mut daemon = daemon;
        let result = daemon.serve_next();
        (result, daemon)
    });
    let mut stream = UnixStream::connect(&path).unwrap();
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
        daemon_limits(),
    )
    .unwrap();
    let server = thread::spawn(move || {
        let mut daemon = daemon;
        let result = daemon.serve_next();
        (result, daemon)
    });
    let stream = UnixStream::connect(&path).unwrap();
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
fn request_budget_closes_the_admitted_session_after_its_last_reply() {
    let path = socket_path();
    let daemon = DaemonV1::bind_new(
        &path,
        host(Port::default()),
        Authentication,
        Clock(1),
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
        daemon_limits(),
    );
    assert!(matches!(occupied, Err(DaemonErrorV1::Io(_))));
    assert_eq!(fs::read(&path).unwrap(), b"replacement");
    fs::remove_file(&path).unwrap();
    remove_parent(&path);

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
            daemon_limits(),
        ),
        Err(DaemonErrorV1::UnsafeEndpoint)
    ));
    remove_parent(&unsafe_path);
}
