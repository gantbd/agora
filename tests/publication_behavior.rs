use agora_engine::{
    AdmissionPolicy, CommandV1, Engine, Limits, LogicalTime, Participant, ParticipantId,
    ParticipantKind, PublicationCommandV1, PublicationKeyV1, PublicationRefusalV1, RoomId,
};

fn setup(limits: Limits) -> Engine {
    let mut engine = Engine::with_limits(limits);
    engine
        .apply(CommandV1::RegisterParticipant(Participant::new(
            "human",
            "Human",
            ParticipantKind::Human,
        )))
        .unwrap();
    engine
        .apply(CommandV1::RegisterParticipant(Participant::new(
            "agent",
            "Agent",
            ParticipantKind::Agent,
        )))
        .unwrap();
    engine
        .apply(CommandV1::CreateRoom {
            room_id: RoomId::from("room"),
            name: "Room".into(),
            owner: ParticipantId::from("human"),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(CommandV1::Join {
            room_id: RoomId::from("room"),
            participant: ParticipantId::from("agent"),
        })
        .unwrap();
    engine
}

fn publication(engine: &Engine, key: [u8; 16], body: &str) -> PublicationCommandV1 {
    PublicationCommandV1::new(
        engine.revision(),
        PublicationKeyV1::new(key),
        RoomId::from("room"),
        ParticipantId::from("agent"),
        LogicalTime(20),
        body,
    )
}

#[test]
fn matching_publication_replay_returns_the_original_transition_without_a_second_message() {
    let mut engine = setup(Limits::default());
    let command = publication(&engine, [1; 16], "answer");
    let first = engine.publish(command.clone()).unwrap();
    let accepted_revision = engine.revision();

    engine
        .apply(CommandV1::RegisterParticipant(Participant::new(
            "observer",
            "Observer",
            ParticipantKind::Human,
        )))
        .unwrap();
    assert_eq!(engine.publish(command).unwrap(), first);
    assert_ne!(engine.revision(), accepted_revision);
    assert_eq!(
        engine.room(&RoomId::from("room")).unwrap().messages.len(),
        1
    );
}

#[test]
fn publication_key_reuse_with_changed_content_is_a_conflict() {
    let mut engine = setup(Limits::default());
    engine
        .publish(publication(&engine, [2; 16], "first"))
        .unwrap();
    let revision = engine.revision();

    assert_eq!(
        engine.publish(publication(&engine, [2; 16], "changed")),
        Err(PublicationRefusalV1::IdempotencyConflict)
    );
    assert_eq!(engine.revision(), revision);
    assert_eq!(
        engine.room(&RoomId::from("room")).unwrap().messages.len(),
        1
    );
}

#[test]
fn full_publication_receipt_index_backpressures_before_message_mutation() {
    let mut engine = setup(Limits {
        max_publication_receipts: 1,
        ..Limits::default()
    });
    engine
        .publish(publication(&engine, [3; 16], "first"))
        .unwrap();
    let revision = engine.revision();

    assert_eq!(
        engine.publish(publication(&engine, [4; 16], "second")),
        Err(PublicationRefusalV1::PublicationReceiptLimit)
    );
    assert_eq!(engine.revision(), revision);
    assert_eq!(
        engine.room(&RoomId::from("room")).unwrap().messages.len(),
        1
    );
}

#[test]
fn publication_debug_surfaces_redact_message_content_and_keys() {
    let engine = setup(Limits::default());
    let command = publication(&engine, [0x88; 16], "secret response");

    let debug = format!("{command:?}");
    assert!(!debug.contains("secret response"));
    assert!(!debug.contains("136"));
    assert!(debug.contains("body_bytes"));
}
