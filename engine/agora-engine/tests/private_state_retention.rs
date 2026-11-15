use agora_engine::export_source::PrivateParticipantStateSourceV1;
use agora_engine::persistence::RecoveryLimits;
use agora_engine::retention::{
    PrivateParticipantStateRetentionRefusalV1, RecordClassV1, RetentionBoundV1,
    RetentionPolicyRevisionV1, RetentionPolicyV1, apply_private_participant_state_retention_v1,
};
use agora_engine::{
    AdmissionPolicy, CommandV2, Engine, Limits, LogicalTime, OutcomeV2, Participant,
    ParticipantKind, RoomId,
};

const MAX_RECORD_BYTES: usize = 4 * 1024 * 1024;

fn policy(engine: &Engine, bound: RetentionBoundV1) -> RetentionPolicyV1 {
    let mut bounds = [RetentionBoundV1::Disabled; 6];
    bounds[RecordClassV1::PrivateParticipantState as usize - 1] = bound;
    RetentionPolicyV1::new(
        RetentionPolicyRevisionV1::new(29).unwrap(),
        engine.revision().0,
        bounds,
    )
}

fn engine_with_private_ballot() -> Engine {
    let mut engine = Engine::new();
    for (id, kind) in [
        ("human", ParticipantKind::Human),
        ("agent", ParticipantKind::Agent),
    ] {
        engine
            .apply(CommandV2::RegisterParticipant(Participant::new(
                id, id, kind,
            )))
            .unwrap();
    }
    let room_id = RoomId::from("room");
    engine
        .apply(CommandV2::CreateRoom {
            room_id: room_id.clone(),
            name: "Room".to_owned(),
            owner: "human".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(CommandV2::Join {
            room_id: room_id.clone(),
            participant: "agent".into(),
        })
        .unwrap();
    engine
        .apply(CommandV2::Speak {
            room_id: room_id.clone(),
            author: "agent".into(),
            at: LogicalTime(1),
            body: "public".to_owned(),
        })
        .unwrap();
    let OutcomeV2::PollCreated { poll_id, .. } = engine
        .apply(CommandV2::CreatePoll {
            room_id: room_id.clone(),
            actor: "human".into(),
            prompt: "Choose".to_owned(),
            options: vec!["one".to_owned(), "two".to_owned()],
            eligible_participants: vec!["agent".into(), "human".into()],
            max_choices: 1,
        })
        .unwrap()
    else {
        panic!("poll expected");
    };
    engine
        .apply(CommandV2::SetPollBallot {
            room_id,
            actor: "agent".into(),
            poll_id,
            selections: vec![1],
        })
        .unwrap();
    engine
}

#[test]
fn positive_bound_identifies_only_the_live_private_generation() {
    let engine = engine_with_private_ballot();
    let before = engine.prepare_checkpoint(MAX_RECORD_BYTES).unwrap();
    let exact = policy(&engine, RetentionBoundV1::entries(1).unwrap());
    let applied = apply_private_participant_state_retention_v1(&engine, exact).unwrap();
    let source =
        PrivateParticipantStateSourceV1::capture(&engine, exact, MAX_RECORD_BYTES).unwrap();

    assert_eq!(applied.source_engine_revision, engine.revision().0);
    assert_eq!(applied.retained_records, 1);
    assert_eq!(applied.discarded_records, 0);
    assert_eq!(
        applied.current_generation,
        source.fragment().records()[0].id
    );
    assert_eq!(applied.frontier, source.fragment().frontier());

    let wider = apply_private_participant_state_retention_v1(
        &engine,
        policy(&engine, RetentionBoundV1::entries(8).unwrap()),
    )
    .unwrap();
    assert_eq!(wider.current_generation, applied.current_generation);
    assert_eq!(wider.retained_records, 1);
    assert_eq!(wider.discarded_records, 0);
    assert_eq!(
        engine.prepare_checkpoint(MAX_RECORD_BYTES).unwrap().bytes(),
        before.bytes()
    );
}

#[test]
fn disabled_future_and_legacy_policies_cannot_erase_private_or_public_state() {
    let engine = engine_with_private_ballot();
    let before = engine.prepare_checkpoint(MAX_RECORD_BYTES).unwrap();
    assert_eq!(
        apply_private_participant_state_retention_v1(
            &engine,
            policy(&engine, RetentionBoundV1::Disabled),
        ),
        Err(PrivateParticipantStateRetentionRefusalV1::ClassDisabled)
    );
    let mut future = policy(&engine, RetentionBoundV1::entries(1).unwrap());
    future.effective_engine_revision += 1;
    assert_eq!(
        apply_private_participant_state_retention_v1(&engine, future),
        Err(PrivateParticipantStateRetentionRefusalV1::StalePolicy)
    );
    assert_eq!(
        engine.prepare_checkpoint(MAX_RECORD_BYTES).unwrap().bytes(),
        before.bytes()
    );

    let legacy = Engine::new();
    assert_eq!(
        apply_private_participant_state_retention_v1(
            &legacy,
            policy(&legacy, RetentionBoundV1::entries(1).unwrap()),
        ),
        Err(PrivateParticipantStateRetentionRefusalV1::LegacyIdentityUnavailable)
    );

    let mut saturated = before.bytes().to_vec();
    assert_eq!(saturated[8], 151);
    saturated[13] = 2;
    saturated[14..22].copy_from_slice(&u64::MAX.to_be_bytes());
    let saturated = Engine::recover_checkpoint(
        &saturated,
        RecoveryLimits {
            max_snapshot_bytes: MAX_RECORD_BYTES,
            maximum_engine_limits: Limits::default(),
        },
    )
    .unwrap();
    assert_eq!(
        apply_private_participant_state_retention_v1(
            &saturated,
            policy(&saturated, RetentionBoundV1::entries(1).unwrap()),
        ),
        Err(PrivateParticipantStateRetentionRefusalV1::IdentityExhausted)
    );
}
