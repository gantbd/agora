use agora_durable::{
    DurableEvidenceV2, DurableHostConfigV2, DurableHostPublicationV2, DurableHostV2,
};
use agora_engine::{CommandV2 as EngineCommand, Engine, Limits, Participant, ParticipantKind};
use agora_host::v2::HostV2;
use agora_host::v2::persistence::{HostCheckpointScopeV1, HostRecoveryLimitsV2};
use agora_host::{HostLimits, HostTimeV1, PrincipalVerifier, VerifiedSubjectV1};
use agora_protocol::v2::{
    AdmissionPolicyV2, CommandV2, OutcomeV2, ReplyV2, RequestIdV2, RequestV2,
};
use agora_service::v2::{ServiceLimitsV2, ServiceV2, persistence::ServiceRecoveryLimitsV2};
use agora_storage_directory::{DirectoryCheckpointStoreV1, DirectoryLimitsV1};
use agora_wire::WireLimits;
use agora_wire::v2::{WireReplyV2, WireRequestV2, decode_reply_v2, encode_request_v2};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const MAX: usize = 2 * 1_024 * 1_024;
const HOST_SCOPE: HostCheckpointScopeV1 = HostCheckpointScopeV1::new(*b"directory-host!!");
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "agora-directory-host-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone)]
struct Context;

struct Verifier;

impl PrincipalVerifier for Verifier {
    type Context = Context;
    type Error = ();

    fn authenticate(
        &mut self,
        participant: &str,
        _context: &Self::Context,
        _now: HostTimeV1,
    ) -> Result<VerifiedSubjectV1, Self::Error> {
        (participant == "alice")
            .then(|| VerifiedSubjectV1::from_verifier([7; 16]))
            .ok_or(())
    }

    fn revalidate(
        &mut self,
        subject: VerifiedSubjectV1,
        participant: &str,
        _context: &Self::Context,
        _now: HostTimeV1,
    ) -> Result<(), Self::Error> {
        (participant == "alice" && subject == VerifiedSubjectV1::from_verifier([7; 16]))
            .then_some(())
            .ok_or(())
    }
}

fn host_limits() -> HostLimits {
    HostLimits {
        max_sessions: 4,
        max_retired_sessions: 4,
        wire: WireLimits {
            max_frame_bytes: 8_192,
            max_text_bytes: 256,
            max_fields: 512,
            max_list_items: 64,
        },
    }
}

fn config() -> DurableHostConfigV2 {
    DurableHostConfigV2::new(
        "agora-host",
        HOST_SCOPE,
        MAX,
        HostRecoveryLimitsV2 {
            max_snapshot_bytes: MAX,
            max_service_snapshot_bytes: MAX,
            maximum_host_limits: host_limits(),
            service: ServiceRecoveryLimitsV2 {
                max_snapshot_bytes: MAX,
                max_engine_snapshot_bytes: MAX,
                maximum_service_limits: ServiceLimitsV2::default(),
                maximum_engine_limits: Limits::default(),
            },
        },
    )
}

fn service() -> ServiceV2 {
    let mut engine = Engine::new();
    engine
        .apply(EngineCommand::RegisterParticipant(Participant::new(
            "alice",
            "Alice",
            ParticipantKind::Human,
        )))
        .unwrap();
    ServiceV2::new(engine, ServiceLimitsV2::default())
}

fn connect_frame() -> Vec<u8> {
    encode_request_v2(
        &WireRequestV2::Connect {
            participant_id: "alice".to_owned(),
        },
        host_limits().wire,
    )
    .unwrap()
}

fn create_frame(client: agora_protocol::v2::ClientIdV2) -> Vec<u8> {
    encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(7),
                command: CommandV2::CreateRoom {
                    room_id: "room".to_owned(),
                    name: "Room".to_owned(),
                    admission: AdmissionPolicyV2::Open,
                    expected_revision: 1,
                },
            },
        },
        host_limits().wire,
    )
    .unwrap()
}

#[test]
fn authenticated_mutation_recovers_and_replays_across_physical_restart() {
    let temp = TempDirectory::new();
    let store_path = temp.0.join("checkpoint");
    let store =
        DirectoryCheckpointStoreV1::create_new(&store_path, DirectoryLimitsV1::default()).unwrap();
    let host = HostV2::new(service(), Verifier, host_limits());
    let mut durable = DurableHostV2::initialize(host, store, config()).unwrap();
    let DurableHostPublicationV2::Published {
        value: Ok(opened), ..
    } = durable.open(&connect_frame(), &Context, HostTimeV1(1))
    else {
        panic!("authenticated open must be durable");
    };
    let session = opened.session().unwrap();
    let WireReplyV2::Connected(client) =
        decode_reply_v2(opened.frame(), host_limits().wire).unwrap()
    else {
        panic!("connect reply expected");
    };
    let frame = create_frame(client);
    let DurableHostPublicationV2::Published {
        value: Ok(reply),
        evidence: DurableEvidenceV2::Committed { .. },
    } = durable.handle(session, &frame, &Context, HostTimeV1(2))
    else {
        panic!("room creation must commit before publication");
    };
    assert!(matches!(
        decode_reply_v2(&reply, host_limits().wire).unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::RoomCreated { revision: 2 }))
    ));
    let event_count = durable
        .host()
        .unwrap()
        .service()
        .engine()
        .pending_events()
        .len();
    drop(durable.into_store());

    let reopened =
        DirectoryCheckpointStoreV1::open_existing(&store_path, DirectoryLimitsV1::default())
            .unwrap();
    let offers_before = reopened.signals().offers;
    let mut recovered = DurableHostV2::recover(reopened, config(), Verifier).unwrap();
    assert_eq!(
        recovered
            .host()
            .unwrap()
            .service()
            .engine()
            .pending_events()
            .len(),
        event_count
    );
    assert!(matches!(
        recovered.handle(session, &frame, &Context, HostTimeV1(2)),
        DurableHostPublicationV2::Published {
            value: Ok(_),
            evidence: DurableEvidenceV2::PreviouslyCommitted { .. }
        }
    ));
    assert_eq!(
        recovered
            .host()
            .unwrap()
            .service()
            .engine()
            .pending_events()
            .len(),
        event_count,
        "exact replay must not duplicate the engine event"
    );
    let reopened = recovered.into_store();
    assert_eq!(
        reopened.signals().offers,
        offers_before,
        "exact replay must not offer another checkpoint"
    );
}
