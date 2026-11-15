use agora_engine::{
    AdmissionPolicy, CommandV2, Engine, Limits, LogicalTime, Participant, ParticipantId,
    ParticipantKind, PublicationCommandV2, PublicationDomainV2, PublicationKeyV2,
    PublicationRefusalV2, RoomId,
};

fn setup(limits: Limits) -> Engine {
    let mut engine = Engine::with_limits(limits);
    engine
        .apply(CommandV2::RegisterParticipant(Participant::new(
            "human",
            "Human",
            ParticipantKind::Human,
        )))
        .unwrap();
    engine
        .apply(CommandV2::RegisterParticipant(Participant::new(
            "agent",
            "Agent",
            ParticipantKind::Agent,
        )))
        .unwrap();
    engine
        .apply(CommandV2::CreateRoom {
            room_id: RoomId::from("room"),
            name: "Room".into(),
            owner: ParticipantId::from("human"),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(CommandV2::Join {
            room_id: RoomId::from("room"),
            participant: ParticipantId::from("agent"),
        })
        .unwrap();
    engine
}

fn namespaced_publication(
    engine: &Engine,
    domain: PublicationDomainV2,
    key: [u8; 16],
    body: &str,
) -> PublicationCommandV2 {
    PublicationCommandV2::new(
        engine.revision(),
        PublicationKeyV2::namespaced(domain, key),
        RoomId::from("room"),
        ParticipantId::from("agent"),
        LogicalTime(20),
        body,
    )
}

fn publication(engine: &Engine, key: [u8; 16], body: &str) -> PublicationCommandV2 {
    PublicationCommandV2::new(
        engine.revision(),
        PublicationKeyV2::new(key),
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
        .apply(CommandV2::RegisterParticipant(Participant::new(
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
        Err(PublicationRefusalV2::IdempotencyConflict)
    );
    assert_eq!(engine.revision(), revision);
    assert_eq!(
        engine.room(&RoomId::from("room")).unwrap().messages.len(),
        1
    );
}

#[test]
fn equal_bytes_in_distinct_source_domains_do_not_collide() {
    let mut engine = setup(Limits::default());
    engine
        .publish(namespaced_publication(
            &engine,
            PublicationDomainV2::Scheduled,
            [5; 16],
            "scheduled",
        ))
        .unwrap();
    engine
        .publish(namespaced_publication(
            &engine,
            PublicationDomainV2::Interaction,
            [5; 16],
            "interaction",
        ))
        .unwrap();
    assert_eq!(
        engine.room(&RoomId::from("room")).unwrap().messages.len(),
        2
    );
}

#[test]
fn namespaced_request_replays_an_equal_legacy_receipt_during_migration() {
    let mut engine = setup(Limits::default());
    let legacy = publication(&engine, [6; 16], "legacy");
    let first = engine.publish(legacy).unwrap();
    let migrated =
        namespaced_publication(&engine, PublicationDomainV2::Scheduled, [6; 16], "legacy");
    assert_eq!(engine.publish(migrated).unwrap(), first);
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
        Err(PublicationRefusalV2::PublicationReceiptLimit)
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
