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
    let fixture = include_str!("fixtures/service-checkpoint-v5.hex").trim();
    let actual = checkpoint
        .bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
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
    future[8] = 6;
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

#[test]
fn moderation_state_and_receipt_survive_recovery() {
    let mut engine = Engine::new();
    for id in ["alice", "bot"] {
        engine
            .apply(EngineCommand::RegisterParticipant(Participant::new(
                id,
                id,
                ParticipantKind::Human,
            )))
            .unwrap();
    }
    engine
        .apply(EngineCommand::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
            owner: "alice".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    let mut service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    let bot = service.connect("bot").unwrap();
    let ban = request(
        1,
        CommandV2::Ban {
            room_id: "room".to_owned(),
            participant_id: "bot".to_owned(),
            reason: "policy".to_owned(),
            causation: [7; 16],
            expected_revision: 3,
        },
    );
    let accepted = service.execute(alice, ban.clone());
    assert_eq!(
        accepted,
        ReplyV2::Accepted(OutcomeV2::Banned { revision: 4 })
    );

    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let mut recovered = ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, bounds()).unwrap();
    assert_eq!(recovered.execute(alice, ban), accepted);
    assert_eq!(
        recovered.execute(
            bot,
            request(
                1,
                CommandV2::Join {
                    room_id: "room".to_owned(),
                    expected_revision: 4,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::Banned)
    );
}

#[test]
fn room_policy_event_and_receipt_survive_recovery() {
    let mut engine = Engine::new();
    for id in ["alice", "bot"] {
        engine
            .apply(EngineCommand::RegisterParticipant(Participant::new(
                id,
                id,
                ParticipantKind::Human,
            )))
            .unwrap();
    }
    engine
        .apply(EngineCommand::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
            owner: "alice".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    let mut service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    let bot = service.connect("bot").unwrap();
    let policy = request(
        1,
        CommandV2::SetAdmissionPolicy {
            room_id: "room".to_owned(),
            admission: agora_protocol::v2::AdmissionPolicyV2::InviteOnly,
            expected_revision: 3,
        },
    );
    let accepted = service.execute(alice, policy.clone());
    assert_eq!(
        accepted,
        ReplyV2::Accepted(OutcomeV2::RoomPolicyChanged { revision: 4 })
    );

    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let mut recovered = ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, bounds()).unwrap();
    assert_eq!(recovered.execute(alice, policy), accepted);
    assert_eq!(
        recovered.execute(
            bot,
            request(
                1,
                CommandV2::Join {
                    room_id: "room".to_owned(),
                    expected_revision: 4,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::NotAuthorized)
    );
}

#[test]
fn ban_page_cursor_survives_recovery_without_widening_authority() {
    let mut engine = Engine::new();
    for (id, kind) in [
        ("alice", ParticipantKind::Human),
        ("bot", ParticipantKind::Agent),
        ("carol", ParticipantKind::Human),
    ] {
        engine
            .apply(EngineCommand::RegisterParticipant(Participant::new(
                id, id, kind,
            )))
            .unwrap();
    }
    engine
        .apply(EngineCommand::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
            owner: "alice".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    let mut service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    for (request_id, subject, revision) in [(1, "bot", 4), (2, "carol", 5)] {
        assert!(matches!(
            service.execute(
                alice,
                request(
                    request_id,
                    CommandV2::Ban {
                        room_id: "room".to_owned(),
                        participant_id: subject.to_owned(),
                        reason: "policy".to_owned(),
                        causation: [request_id as u8; 16],
                        expected_revision: revision,
                    },
                ),
            ),
            ReplyV2::Accepted(OutcomeV2::Banned { .. })
        ));
    }
    let ReplyV2::Accepted(OutcomeV2::Bans(first)) = service.execute(
        alice,
        request(
            3,
            CommandV2::Bans {
                room_id: "room".to_owned(),
                cursor: None,
                limit: 1,
            },
        ),
    ) else {
        panic!("first ban page expected");
    };
    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let mut recovered = ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, bounds()).unwrap();
    let ReplyV2::Accepted(OutcomeV2::Bans(second)) = recovered.execute(
        alice,
        request(
            4,
            CommandV2::Bans {
                room_id: "room".to_owned(),
                cursor: first.next,
                limit: 1,
            },
        ),
    ) else {
        panic!("continued ban page expected");
    };
    assert_eq!(second.banned.len(), 1);
    assert_eq!(second.banned[0].participant_id, "carol");
    assert!(second.next.is_none());
}

#[test]
fn recovered_change_position_continues_without_snapshot_gap() {
    let (mut service, alice, _, _) = populated();
    let revision = service.engine().revision().0;
    let ReplyV2::Accepted(OutcomeV2::ChangesOpened { cursor, .. }) = service.execute(
        alice,
        request(
            20,
            CommandV2::OpenChanges {
                room_id: "alpha".to_owned(),
                observed_revision: revision,
            },
        ),
    ) else {
        panic!("change position expected");
    };
    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let mut recovered = ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, bounds()).unwrap();
    assert_eq!(
        recovered.execute(
            alice,
            request(
                21,
                CommandV2::Speak {
                    room_id: "alpha".to_owned(),
                    expected_revision: revision,
                    logical_time: 10,
                    body: "after recovery".to_owned(),
                    addressed_to: vec![],
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::MessageAccepted {
            revision: revision + 1,
            sequence: 0,
        })
    );
    let ReplyV2::Accepted(OutcomeV2::Changes(page)) = recovered.execute(
        alice,
        request(
            22,
            CommandV2::Changes {
                room_id: "alpha".to_owned(),
                cursor,
                limit: 4,
            },
        ),
    ) else {
        panic!("recovered position must continue");
    };
    assert_eq!(page.changes.len(), 1);
    assert_eq!(page.changes[0].revision, revision + 1);
}
