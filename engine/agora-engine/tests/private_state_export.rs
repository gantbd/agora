use agora_engine::export_source::{
    PrivateParticipantStateSourceRefusalV1, PrivateParticipantStateSourceV1,
};
use agora_engine::persistence::{
    CheckpointError, RecoveryLimits, prepare_private_participant_state_checkpoint_v1,
    recover_private_participant_state_checkpoint_v1,
};
use agora_engine::retention::{
    ContentDigestV1, ExportRequestIdV1, ExportScopeBindingV1, RecordClassV1, RetentionBoundV1,
    RetentionPolicyRevisionV1, RetentionPolicyV1,
};
use agora_engine::{
    AdmissionPolicy, AttentionPreferenceV1, AttentionScopeV1, CommandV2, Engine, Limits,
    LogicalTime, ModerationCausationV1, ModerationReasonV1, NotificationIntentV1, OutcomeV2,
    Participant, ParticipantId, ParticipantKind, ReadScopeV1, RoomId,
};

const MAX_BYTES: usize = 128 * 1_024;

fn policy(engine: &Engine, bound: RetentionBoundV1) -> RetentionPolicyV1 {
    let mut bounds = [RetentionBoundV1::entries(8).unwrap(); 6];
    bounds[RecordClassV1::PrivateParticipantState as usize - 1] = bound;
    RetentionPolicyV1::new(
        RetentionPolicyRevisionV1::new(13).unwrap(),
        engine.revision().0,
        bounds,
    )
}

fn setup() -> (
    Engine,
    RoomId,
    ParticipantId,
    ParticipantId,
    agora_engine::MessageId,
) {
    let mut engine = Engine::new();
    let human = ParticipantId::from("human");
    let agent = ParticipantId::from("agent");
    let room = RoomId::from("room");
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
    engine
        .apply(CommandV2::CreateRoom {
            room_id: room.clone(),
            name: "Room".to_owned(),
            owner: human.clone(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(CommandV2::Join {
            room_id: room.clone(),
            participant: agent.clone(),
        })
        .unwrap();
    engine
        .apply(CommandV2::Speak {
            room_id: room.clone(),
            author: human.clone(),
            at: LogicalTime(1),
            body: "private source anchor".to_owned(),
        })
        .unwrap();
    let message_id = engine.message(&room, 0).unwrap().id();
    (engine, room, human, agent, message_id)
}

fn rich_private_engine() -> Engine {
    let (mut engine, room, human, agent, message_id) = setup();
    engine
        .advance_read_position(
            &human,
            &room,
            ReadScopeV1::Room,
            engine.revision(),
            message_id,
        )
        .unwrap();
    engine
        .set_attention_preference(
            &agent,
            &room,
            AttentionScopeV1::Room,
            engine.revision(),
            AttentionPreferenceV1::new(true, false, NotificationIntentV1::AddressedOnly),
        )
        .unwrap();
    engine
        .set_saved_reference(&human, &room, message_id, engine.revision(), true)
        .unwrap();
    let OutcomeV2::PollCreated { poll_id, .. } = engine
        .apply(CommandV2::CreatePoll {
            room_id: room.clone(),
            actor: human.clone(),
            prompt: "private choice".to_owned(),
            options: vec!["one".to_owned(), "two".to_owned()],
            eligible_participants: vec![agent.clone(), human.clone()],
            max_choices: 1,
        })
        .unwrap()
    else {
        panic!("poll expected");
    };
    engine
        .apply(CommandV2::SetPollBallot {
            room_id: room.clone(),
            actor: agent,
            poll_id,
            selections: vec![1],
        })
        .unwrap();
    let report = engine
        .submit_message_report(
            &human,
            &room,
            message_id,
            engine.revision(),
            ModerationReasonV1::new("private report"),
            ModerationCausationV1::new([9; 16]),
        )
        .unwrap();
    engine
        .review_message_report(
            &human,
            &room,
            report.report_id,
            agora_engine::MessageReportDispositionV1::Escalated,
            ModerationReasonV1::new("private review"),
            ModerationCausationV1::new([10; 16]),
        )
        .unwrap();
    engine
}

#[test]
fn private_source_contains_only_the_canonical_private_generation() {
    let engine = rich_private_engine();
    let source = PrivateParticipantStateSourceV1::capture(
        &engine,
        policy(&engine, RetentionBoundV1::entries(1).unwrap()),
        MAX_BYTES,
    )
    .unwrap();
    let fragment = source.fragment();
    assert_eq!(fragment.class(), RecordClassV1::PrivateParticipantState);
    assert_eq!(fragment.source_engine_revision(), engine.revision().0);
    assert_eq!(fragment.records()[0].id.ordinal, 6);
    assert_eq!(fragment.frontier().discarded_through, 5);
    assert_eq!(fragment.frontier().oldest_retained, Some(6));
    assert!(!format!("{source:?}").contains("private report"));
    assert!(!format!("{source:?}").contains("private review"));

    let recovery = RecoveryLimits {
        max_snapshot_bytes: MAX_BYTES,
        maximum_engine_limits: Limits::default(),
    };
    let bytes = fragment.records()[0].payload();
    let recovered = recover_private_participant_state_checkpoint_v1(bytes, recovery).unwrap();
    assert_eq!(recovered.source_engine_revision(), engine.revision().0);
    assert_eq!(recovered.private_state_revision(), 6);
    assert_eq!(recovered.private_record_count(), 5);
    assert_eq!(recovered.bytes(), bytes);
    assert!(!format!("{recovered:?}").contains("private report"));
    assert!(!format!("{recovered:?}").contains("private review"));
    assert_eq!(
        prepare_private_participant_state_checkpoint_v1(&engine, MAX_BYTES)
            .unwrap()
            .bytes(),
        bytes
    );

    let digest = |value: &[u8]| {
        ContentDigestV1::new([value.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte)); 32])
    };
    let materialized = source
        .materialize(
            ExportRequestIdV1::new([6; 16]).unwrap(),
            ExportScopeBindingV1::new([7; 32]).unwrap(),
            2,
            MAX_BYTES * 2,
            digest,
        )
        .unwrap();
    assert_eq!(
        materialized.manifest.included_classes,
        [false, false, false, true, false, false]
    );

    let mut refused = 0;
    for bit in 0..bytes.len() * 8 {
        let mut mutated = bytes.to_vec();
        mutated[bit / 8] ^= 1 << (bit % 8);
        match recover_private_participant_state_checkpoint_v1(&mutated, recovery) {
            Ok(checkpoint) => assert_eq!(checkpoint.bytes(), mutated),
            Err(_) => refused += 1,
        }
    }
    assert!(refused > 0);
}

#[test]
fn empty_private_generation_is_frozen_and_source_preflights_identity_policy_and_saturation() {
    let (mut engine, room, human, _, _) = setup();
    assert_eq!(
        PrivateParticipantStateSourceV1::capture(
            &engine,
            policy(&engine, RetentionBoundV1::entries(1).unwrap()),
            MAX_BYTES,
        ),
        Err(PrivateParticipantStateSourceRefusalV1::LegacyIdentityUnavailable)
    );
    let preference = AttentionPreferenceV1::new(true, false, NotificationIntentV1::Default);
    engine
        .set_attention_preference(
            &human,
            &room,
            AttentionScopeV1::Room,
            engine.revision(),
            preference,
        )
        .unwrap();
    engine
        .set_attention_preference(
            &human,
            &room,
            AttentionScopeV1::Room,
            engine.revision(),
            AttentionPreferenceV1::default(),
        )
        .unwrap();
    let source = PrivateParticipantStateSourceV1::capture(
        &engine,
        policy(&engine, RetentionBoundV1::entries(1).unwrap()),
        MAX_BYTES,
    )
    .unwrap();
    let expected = [
        b"AGAPRV1\0".as_slice(),
        &engine.revision().0.to_be_bytes(),
        &2_u64.to_be_bytes(),
        &1_u64.to_be_bytes(),
        &0_u64.to_be_bytes(),
        &0_u64.to_be_bytes(),
        &0_u64.to_be_bytes(),
        &0_u64.to_be_bytes(),
        &0_u64.to_be_bytes(),
    ]
    .concat();
    assert_eq!(source.fragment().records()[0].payload(), expected);

    let mut stale = policy(&engine, RetentionBoundV1::entries(1).unwrap());
    stale.effective_engine_revision -= 1;
    assert_eq!(
        PrivateParticipantStateSourceV1::capture(&engine, stale, MAX_BYTES),
        Err(PrivateParticipantStateSourceRefusalV1::StalePolicy)
    );
    assert_eq!(
        PrivateParticipantStateSourceV1::capture(
            &engine,
            policy(&engine, RetentionBoundV1::Disabled),
            MAX_BYTES,
        ),
        Err(PrivateParticipantStateSourceRefusalV1::ClassDisabled)
    );

    let recovery = RecoveryLimits {
        max_snapshot_bytes: 4 * 1024 * 1024,
        maximum_engine_limits: Limits::default(),
    };
    let checkpoint = engine.prepare_checkpoint(4 * 1024 * 1024).unwrap();
    let mut saturated = checkpoint.bytes().to_vec();
    saturated[14..22].copy_from_slice(&u64::MAX.to_be_bytes());
    let mut saturated = Engine::recover_checkpoint(&saturated, recovery).unwrap();
    saturated
        .set_attention_preference(
            &human,
            &room,
            AttentionScopeV1::Room,
            saturated.revision(),
            preference,
        )
        .unwrap();
    assert_eq!(
        PrivateParticipantStateSourceV1::capture(
            &saturated,
            policy(&saturated, RetentionBoundV1::entries(1).unwrap()),
            MAX_BYTES,
        ),
        Err(PrivateParticipantStateSourceRefusalV1::IdentityExhausted)
    );
    assert_eq!(
        recover_private_participant_state_checkpoint_v1(&expected[..expected.len() - 1], recovery)
            .unwrap_err(),
        CheckpointError::Truncated
    );
}
