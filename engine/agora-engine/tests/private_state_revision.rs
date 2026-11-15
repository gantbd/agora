use agora_engine::persistence::RecoveryLimits;
use agora_engine::{
    AdmissionPolicy, AttentionPreferenceV1, AttentionScopeV1, CommandV2, Engine, Limits,
    LogicalTime, MessageReportDispositionV1, ModerationCausationV1, ModerationReasonV1,
    NotificationIntentV1, OutcomeV2, Participant, ParticipantId, ParticipantKind, ReadScopeV1,
    RoomId,
};

fn setup() -> (Engine, RoomId, ParticipantId, agora_engine::MessageId) {
    let mut engine = Engine::new();
    let human = ParticipantId::from("human");
    let room = RoomId::from("room");
    engine
        .apply(CommandV2::RegisterParticipant(Participant::new(
            "human",
            "Human",
            ParticipantKind::Human,
        )))
        .unwrap();
    engine
        .apply(CommandV2::CreateRoom {
            room_id: room.clone(),
            name: "Room".to_owned(),
            owner: human.clone(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(CommandV2::Speak {
            room_id: room.clone(),
            author: human.clone(),
            at: LogicalTime(1),
            body: "message".to_owned(),
        })
        .unwrap();
    let message_id = engine.message(&room, 0).unwrap().id();
    (engine, room, human, message_id)
}

#[test]
fn private_generation_advances_only_for_effective_private_mutations_and_recovers() {
    let (mut engine, room, human, message_id) = setup();
    assert_eq!(engine.private_state_source_revision(), None);
    engine
        .advance_read_position(
            &human,
            &room,
            ReadScopeV1::Room,
            engine.revision(),
            message_id,
        )
        .unwrap();
    assert_eq!(engine.private_state_source_revision(), Some(1));
    engine
        .advance_read_position(
            &human,
            &room,
            ReadScopeV1::Room,
            engine.revision(),
            message_id,
        )
        .unwrap();
    assert_eq!(engine.private_state_source_revision(), Some(1));

    let preference = AttentionPreferenceV1::new(true, false, NotificationIntentV1::AddressedOnly);
    engine
        .set_attention_preference(
            &human,
            &room,
            AttentionScopeV1::Room,
            engine.revision(),
            preference,
        )
        .unwrap();
    assert_eq!(engine.private_state_source_revision(), Some(2));
    engine
        .set_attention_preference(
            &human,
            &room,
            AttentionScopeV1::Room,
            engine.revision(),
            preference,
        )
        .unwrap();
    assert_eq!(engine.private_state_source_revision(), Some(2));

    engine
        .set_saved_reference(&human, &room, message_id, engine.revision(), true)
        .unwrap();
    assert_eq!(engine.private_state_source_revision(), Some(3));
    let report = engine
        .submit_message_report(
            &human,
            &room,
            message_id,
            engine.revision(),
            ModerationReasonV1::new("reason"),
            ModerationCausationV1::new([1; 16]),
        )
        .unwrap();
    assert_eq!(engine.private_state_source_revision(), Some(4));
    engine
        .review_message_report(
            &human,
            &room,
            report.report_id,
            MessageReportDispositionV1::Dismissed,
            ModerationReasonV1::new("review"),
            ModerationCausationV1::new([2; 16]),
        )
        .unwrap();
    assert_eq!(engine.private_state_source_revision(), Some(5));

    let before = engine.private_state_source_revision();
    let OutcomeV2::MessageAccepted { .. } = engine
        .apply(CommandV2::Speak {
            room_id: room,
            author: human,
            at: LogicalTime(2),
            body: "domain".to_owned(),
        })
        .unwrap()
    else {
        panic!("message expected");
    };
    assert_eq!(engine.private_state_source_revision(), before);

    let checkpoint = engine.prepare_checkpoint(4 * 1024 * 1024).unwrap();
    assert_eq!(checkpoint.bytes()[8], 151);
    let recovered = Engine::recover_checkpoint(
        checkpoint.bytes(),
        RecoveryLimits {
            max_snapshot_bytes: 4 * 1024 * 1024,
            maximum_engine_limits: Limits::default(),
        },
    )
    .unwrap();
    assert_eq!(recovered.private_state_source_revision(), Some(5));
    assert!(!recovered.private_state_source_exhausted());
    assert_eq!(
        recovered
            .prepare_checkpoint(4 * 1024 * 1024)
            .unwrap()
            .bytes(),
        checkpoint.bytes()
    );
}

#[test]
fn legacy_recovery_starts_identity_on_next_private_change_and_saturation_fences() {
    let (engine, room, human, message_id) = setup();
    let legacy = engine.prepare_checkpoint(4 * 1024 * 1024).unwrap();
    assert_ne!(legacy.bytes()[8], 151);
    let recovery = RecoveryLimits {
        max_snapshot_bytes: 4 * 1024 * 1024,
        maximum_engine_limits: Limits::default(),
    };
    let mut recovered = Engine::recover_checkpoint(legacy.bytes(), recovery).unwrap();
    assert_eq!(recovered.private_state_source_revision(), None);
    recovered
        .advance_read_position(
            &human,
            &room,
            ReadScopeV1::Room,
            recovered.revision(),
            message_id,
        )
        .unwrap();
    assert_eq!(recovered.private_state_source_revision(), Some(1));

    let checkpoint = recovered.prepare_checkpoint(4 * 1024 * 1024).unwrap();
    let mut saturated = checkpoint.bytes().to_vec();
    assert_eq!(&saturated[..8], b"AGORACP\0");
    assert_eq!(saturated[8], 151);
    saturated[13] = 1;
    saturated[14..22].copy_from_slice(&u64::MAX.to_be_bytes());
    let mut saturated = Engine::recover_checkpoint(&saturated, recovery).unwrap();
    assert_eq!(saturated.private_state_source_revision(), Some(u64::MAX));
    saturated
        .set_saved_reference(&human, &room, message_id, saturated.revision(), true)
        .unwrap();
    assert!(saturated.private_state_source_exhausted());
    assert_eq!(saturated.private_state_source_revision(), Some(u64::MAX));
    let exhausted = saturated.prepare_checkpoint(4 * 1024 * 1024).unwrap();
    assert_eq!(exhausted.bytes()[13], 2);
    assert!(
        Engine::recover_checkpoint(exhausted.bytes(), recovery)
            .unwrap()
            .private_state_source_exhausted()
    );
}
