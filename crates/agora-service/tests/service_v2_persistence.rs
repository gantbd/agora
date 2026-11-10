use agora_engine::{
    AdmissionPolicy, CommandV2 as EngineCommand, Engine, Limits, Participant, ParticipantKind,
};
use agora_protocol::v2::{
    ClientIdV2, CommandV2, OutcomeV2, RefusalV2, ReplyV2, RequestIdV2, RequestV2,
};
use agora_service::v2::{
    ServiceLimitsV2, ServiceV2,
    persistence::{ServiceCheckpointErrorV2, ServiceCheckpointScopeV1, ServiceRecoveryLimitsV2},
};

const MAX_CHECKPOINT: usize = 2 * 1_024 * 1_024;
const SCOPE: ServiceCheckpointScopeV1 = ServiceCheckpointScopeV1::new(*b"agora-test-scope");

fn bounds() -> ServiceRecoveryLimitsV2 {
    ServiceRecoveryLimitsV2 {
        max_snapshot_bytes: MAX_CHECKPOINT,
        max_engine_snapshot_bytes: MAX_CHECKPOINT,
        maximum_service_limits: ServiceLimitsV2::default(),
        maximum_engine_limits: Limits::default(),
    }
}

fn request(request_id: u64, command: CommandV2) -> RequestV2 {
    RequestV2 {
        request_id: RequestIdV2(request_id),
        command,
    }
}

fn populated() -> (
    ServiceV2,
    ClientIdV2,
    ReplyV2,
    agora_protocol::v2::PageCursorV2,
) {
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
    for room in ["alpha", "beta", "gamma"] {
        engine
            .apply(EngineCommand::CreateRoom {
                room_id: room.into(),
                name: room.to_owned(),
                owner: "alice".into(),
                admission: AdmissionPolicy::Open,
            })
            .unwrap();
    }
    let mut service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    let _bot = service.connect("bot").unwrap();
    let mutation = service.execute(
        alice,
        request(
            10,
            CommandV2::Speak {
                room_id: "alpha".to_owned(),
                expected_revision: 5,
                logical_time: 9,
                body: "checkpoint me".to_owned(),
                addressed_to: vec!["bot".to_owned()],
            },
        ),
    );
    let ReplyV2::Accepted(OutcomeV2::Rooms(page)) = service.execute(
        alice,
        request(
            11,
            CommandV2::Rooms {
                cursor: None,
                limit: 2,
            },
        ),
    ) else {
        panic!("room page should be accepted");
    };
    (service, alice, mutation, page.next.unwrap())
}

#[test]
fn complete_service_state_round_trips_and_continues_exactly() {
    let (service, alice, mutation, cursor) = populated();
    let revision = service.engine().revision();
    let pending = service
        .engine()
        .pending_events()
        .cloned()
        .collect::<Vec<_>>();
    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let fixture = include_str!("fixtures/service-checkpoint-v1.hex").trim();
    let actual = checkpoint
        .bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual.len(), fixture.len());
    assert_eq!(actual, fixture);
    let mut recovered = ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, bounds()).unwrap();

    assert_eq!(recovered.engine().revision(), revision);
    assert_eq!(
        recovered
            .engine()
            .pending_events()
            .cloned()
            .collect::<Vec<_>>(),
        pending
    );
    assert_eq!(
        recovered
            .prepare_checkpoint(SCOPE, MAX_CHECKPOINT)
            .unwrap()
            .bytes(),
        checkpoint.bytes()
    );

    assert_eq!(
        recovered.execute(
            alice,
            request(
                10,
                CommandV2::Speak {
                    room_id: "alpha".to_owned(),
                    expected_revision: 5,
                    logical_time: 9,
                    body: "checkpoint me".to_owned(),
                    addressed_to: vec!["bot".to_owned()],
                },
            ),
        ),
        mutation
    );
    assert_eq!(recovered.engine().revision(), revision);
    assert_eq!(
        recovered.execute(
            alice,
            request(
                10,
                CommandV2::Speak {
                    room_id: "alpha".to_owned(),
                    expected_revision: 5,
                    logical_time: 9,
                    body: "changed".to_owned(),
                    addressed_to: vec![],
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::RequestConflict)
    );

    let ReplyV2::Accepted(OutcomeV2::Rooms(page)) = recovered.execute(
        alice,
        request(
            12,
            CommandV2::Rooms {
                cursor: Some(cursor),
                limit: 2,
            },
        ),
    ) else {
        panic!("recovered cursor should continue");
    };
    assert_eq!(page.rooms.len(), 1);
    assert!(page.next.is_none());
}

#[test]
fn recovery_is_bounded_scope_bound_and_fail_closed() {
    let (service, _, _, _) = populated();
    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let bytes = checkpoint.bytes();

    assert_eq!(
        ServiceV2::recover_checkpoint(
            bytes,
            ServiceCheckpointScopeV1::new(*b"foreign-scope-v1"),
            bounds(),
        )
        .unwrap_err(),
        ServiceCheckpointErrorV2::Scope
    );
    let mut too_small = bounds();
    too_small.max_snapshot_bytes = bytes.len() - 1;
    assert_eq!(
        ServiceV2::recover_checkpoint(bytes, SCOPE, too_small).unwrap_err(),
        ServiceCheckpointErrorV2::Limit
    );

    for end in 0..bytes.len() {
        assert!(ServiceV2::recover_checkpoint(&bytes[..end], SCOPE, bounds()).is_err());
    }
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert_eq!(
        ServiceV2::recover_checkpoint(&trailing, SCOPE, bounds()).unwrap_err(),
        ServiceCheckpointErrorV2::Trailing
    );
    let mut future = bytes.to_vec();
    future[8] = 2;
    assert_eq!(
        ServiceV2::recover_checkpoint(&future, SCOPE, bounds()).unwrap_err(),
        ServiceCheckpointErrorV2::Profile
    );
    assert_eq!(format!("{checkpoint:?}"), "ServiceCheckpointV2(<redacted>)");
}

#[test]
fn any_accepted_mutation_after_recovery_advances_from_the_recovered_revision() {
    let (service, alice, _, _) = populated();
    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let mut recovered = ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, bounds()).unwrap();
    let before = recovered.engine().revision().0;

    assert_eq!(
        recovered.execute(
            alice,
            request(
                13,
                CommandV2::CreateRoom {
                    room_id: "delta".to_owned(),
                    name: "Delta".to_owned(),
                    admission: agora_protocol::v2::AdmissionPolicyV2::Open,
                    expected_revision: before,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::RoomCreated {
            revision: before + 1
        })
    );
}
