use agora_engine::{
    AdmissionPolicy, CommandV2 as EngineCommand, Engine, Limits, Participant, ParticipantKind,
};
use agora_protocol::v2::{CommandV2, OutcomeV2, ReplyV2, RequestIdV2, RequestV2};
use agora_service::v2::{
    ServiceLimitsV2, ServiceV2,
    persistence::{ServiceCheckpointScopeV1, ServiceRecoveryLimitsV2},
};

const SCOPE: ServiceCheckpointScopeV1 = ServiceCheckpointScopeV1::new(*b"agora-soak-v1!!!");
const MAX_CHECKPOINT: usize = 64 * 1_024 * 1_024;

fn request(id: u64, command: CommandV2) -> RequestV2 {
    RequestV2 {
        request_id: RequestIdV2(id),
        command,
    }
}

fn cycles() -> usize {
    std::env::var("AGORA_SOAK_CYCLES")
        .ok()
        .map(|value| value.parse().expect("AGORA_SOAK_CYCLES must be an integer"))
        .unwrap_or(5_000)
}

fn engine_limits(cycles: usize) -> Limits {
    Limits {
        max_messages_per_room: 32,
        max_pending_events: cycles.saturating_mul(4).saturating_add(32),
        ..Limits::default()
    }
}

fn service_limits(cycles: usize) -> ServiceLimitsV2 {
    ServiceLimitsV2 {
        max_retired_clients: cycles.saturating_add(32),
        max_retained_requests: cycles.saturating_mul(5).saturating_add(32),
        max_retained_cursors: cycles.saturating_add(32),
        ..ServiceLimitsV2::default()
    }
}

fn recovery_limits(cycles: usize) -> ServiceRecoveryLimitsV2 {
    ServiceRecoveryLimitsV2 {
        max_snapshot_bytes: MAX_CHECKPOINT,
        max_engine_snapshot_bytes: MAX_CHECKPOINT,
        maximum_service_limits: service_limits(cycles),
        maximum_engine_limits: engine_limits(cycles),
    }
}

#[test]
#[ignore = "opt-in deterministic lifecycle/replay/recovery soak"]
fn repeated_lifecycle_replay_page_reconnect_and_recovery_remain_exact() {
    let cycles = cycles();
    assert!(cycles > 0);
    let mut engine = Engine::with_limits(engine_limits(cycles));
    engine
        .apply(EngineCommand::RegisterParticipant(Participant::new(
            "owner",
            "Owner",
            ParticipantKind::Human,
        )))
        .unwrap();
    engine
        .apply(EngineCommand::RegisterParticipant(Participant::new(
            "actor",
            "Actor",
            ParticipantKind::Agent,
        )))
        .unwrap();
    engine
        .apply(EngineCommand::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
            owner: "owner".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(EngineCommand::Join {
            room_id: "room".into(),
            participant: "actor".into(),
        })
        .unwrap();
    let mut service = ServiceV2::new(engine, service_limits(cycles));
    let owner = service.connect("owner").unwrap();
    let mut actor = service.connect("actor").unwrap();
    let mut owner_request = 0_u64;
    let mut actor_request = 0_u64;

    for cycle in 0..cycles {
        let revision = service.engine().revision().0;
        let speak = request(
            owner_request,
            CommandV2::SpeakIdentified {
                room_id: "room".to_owned(),
                expected_revision: revision,
                logical_time: cycle as u64,
                body: "bounded soak".to_owned(),
                addressed_to: vec!["actor".to_owned()],
            },
        );
        owner_request += 1;
        let accepted = service.execute(owner, speak.clone());
        assert!(matches!(
            accepted,
            ReplyV2::Accepted(OutcomeV2::IdentifiedMessageAccepted {
                revision: accepted_revision,
                ..
            }) if accepted_revision == revision + 1
        ));
        assert_eq!(service.execute(owner, speak), accepted);

        let ReplyV2::Accepted(OutcomeV2::IdentifiedMessages(page)) = service.execute(
            actor,
            request(
                actor_request,
                CommandV2::IdentifiedMessages {
                    room_id: "room".to_owned(),
                    cursor: None,
                    limit: 7,
                },
            ),
        ) else {
            panic!("bounded history page expected at cycle {cycle}");
        };
        actor_request += 1;
        assert!(!page.messages.is_empty());
        assert!(page.messages.len() <= 7);
        assert!(page.next_message_sequence - page.oldest_retained_sequence <= 32);

        let revision = service.engine().revision().0;
        let lifecycle = if cycle % 2 == 0 {
            service.execute(
                actor,
                request(
                    actor_request,
                    CommandV2::Leave {
                        room_id: "room".to_owned(),
                        expected_revision: revision,
                    },
                ),
            )
        } else {
            service.execute(
                owner,
                request(
                    owner_request,
                    CommandV2::Kick {
                        room_id: "room".to_owned(),
                        participant_id: "actor".to_owned(),
                        reason: "soak cycle".to_owned(),
                        causation: (cycle as u128).to_be_bytes(),
                        expected_revision: revision,
                    },
                ),
            )
        };
        if cycle % 2 == 0 {
            actor_request += 1;
            assert!(matches!(
                lifecycle,
                ReplyV2::Accepted(OutcomeV2::Left { .. })
            ));
        } else {
            owner_request += 1;
            assert!(matches!(
                lifecycle,
                ReplyV2::Accepted(OutcomeV2::Kicked { .. })
            ));
        }
        let revision = service.engine().revision().0;
        assert!(matches!(
            service.execute(
                actor,
                request(
                    actor_request,
                    CommandV2::Join {
                        room_id: "room".to_owned(),
                        expected_revision: revision,
                    },
                ),
            ),
            ReplyV2::Accepted(OutcomeV2::Joined { .. })
        ));
        actor_request += 1;

        if cycle % 7 == 6 {
            assert_eq!(service.disconnect(actor), Ok(true));
            actor = service.connect("actor").unwrap();
            actor_request = 0;
        }

        if cycle % 17 == 16 {
            let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
            let recovered =
                ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, recovery_limits(cycles))
                    .unwrap();
            assert_eq!(
                recovered
                    .prepare_checkpoint(SCOPE, MAX_CHECKPOINT)
                    .unwrap()
                    .bytes(),
                checkpoint.bytes()
            );
            service = recovered;
        }
    }

    assert_eq!(service.engine().revision().0, 4 + 3 * cycles as u64);
    assert!(service.engine().pending_events().count() <= 4 + 3 * cycles);
}
