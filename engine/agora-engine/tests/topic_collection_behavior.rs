use agora_engine::{
    AdmissionPolicy, CommandV2, Engine, ObservationRefusal, OutcomeV2, Participant,
    ParticipantKind, RoomId, TopicCollectionFilterV1, TopicCollectionLifecycleV1,
};

fn setup() -> (Engine, RoomId) {
    let mut engine = Engine::new();
    for (id, kind) in [
        ("owner", ParticipantKind::Human),
        ("agent", ParticipantKind::Agent),
    ] {
        engine
            .apply(CommandV2::RegisterParticipant(Participant::new(
                id, id, kind,
            )))
            .unwrap();
    }
    let room: RoomId = "room".into();
    engine
        .apply(CommandV2::CreateRoom {
            room_id: room.clone(),
            name: "Room".to_owned(),
            owner: "owner".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(CommandV2::Join {
            room_id: room.clone(),
            participant: "agent".into(),
        })
        .unwrap();
    (engine, room)
}

fn topic(engine: &mut Engine, room: &RoomId, name: &str) -> agora_engine::TopicId {
    let OutcomeV2::TopicCreated { topic_id, .. } = engine
        .apply(CommandV2::CreateTopic {
            room_id: room.clone(),
            actor: "owner".into(),
            name: name.to_owned(),
        })
        .unwrap()
    else {
        unreachable!()
    };
    topic_id
}

#[test]
fn collection_filters_project_complete_current_metadata_without_side_effects() {
    let (mut engine, room) = setup();
    let first = topic(&mut engine, &room, "first");
    let second = topic(&mut engine, &room, "second");
    let third = topic(&mut engine, &room, "third");
    for (topic_id, tag) in [(first, "blue"), (second, "blue"), (second, "urgent")] {
        engine
            .apply(CommandV2::SetTopicTag {
                room_id: room.clone(),
                actor: "owner".into(),
                topic_id,
                tag: tag.to_owned(),
                added: true,
            })
            .unwrap();
    }
    engine
        .apply(CommandV2::SetTopicAlias {
            room_id: room.clone(),
            actor: "owner".into(),
            topic_id: second,
            alias: "prior-name".to_owned(),
            added: true,
        })
        .unwrap();
    engine
        .apply(CommandV2::SetTopicResolved {
            room_id: room.clone(),
            actor: "owner".into(),
            topic_id: first,
            resolved: true,
        })
        .unwrap();
    engine
        .apply(CommandV2::SetTopicSuccessor {
            room_id: room.clone(),
            actor: "owner".into(),
            topic_id: first,
            successor_topic_id: Some(third),
        })
        .unwrap();
    let revision = engine.revision();
    let events = engine.pending_events().count();
    let observer = "agent".into();
    let filter = TopicCollectionFilterV1 {
        lifecycle: TopicCollectionLifecycleV1::Active,
        tags: vec!["blue".to_owned(), "urgent".to_owned()],
    };
    let page = engine
        .topic_collection_page(&observer, &room, &filter, None, 8)
        .unwrap();
    assert_eq!(page.revision, revision);
    assert_eq!(page.filter, filter);
    assert_eq!(page.topics.len(), 1);
    let entry = &page.topics[0];
    assert_eq!(entry.topic_id, second);
    assert_eq!(entry.name, "second");
    assert_eq!(entry.aliases, vec!["prior-name"]);
    assert_eq!(entry.tags, vec!["blue", "urgent"]);
    assert!(!entry.resolved);
    assert_eq!(entry.successor_topic_id, None);
    assert_eq!(engine.revision(), revision);
    assert_eq!(engine.pending_events().count(), events);

    let resolved = engine
        .topic_collection_page(
            &observer,
            &room,
            &TopicCollectionFilterV1 {
                lifecycle: TopicCollectionLifecycleV1::Resolved,
                tags: vec![],
            },
            None,
            8,
        )
        .unwrap();
    assert_eq!(resolved.topics.len(), 1);
    assert_eq!(resolved.topics[0].topic_id, first);
    assert_eq!(resolved.topics[0].successor_topic_id, Some(third));
}

#[test]
fn collection_cursors_bind_filter_observer_room_and_revision() {
    let (mut engine, room) = setup();
    for name in ["one", "two", "three"] {
        topic(&mut engine, &room, name);
    }
    let owner = "owner".into();
    let agent = "agent".into();
    let filter = TopicCollectionFilterV1 {
        lifecycle: TopicCollectionLifecycleV1::Any,
        tags: vec![],
    };
    let first = engine
        .topic_collection_page(&owner, &room, &filter, None, 1)
        .unwrap();
    let cursor = first.next.unwrap();
    assert!(matches!(
        engine.topic_collection_page(&agent, &room, &filter, Some(&cursor), 1),
        Err(ObservationRefusal::ForeignCursor)
    ));
    let changed = TopicCollectionFilterV1 {
        lifecycle: TopicCollectionLifecycleV1::Active,
        tags: vec![],
    };
    assert!(matches!(
        engine.topic_collection_page(&owner, &room, &changed, Some(&cursor), 1),
        Err(ObservationRefusal::ForeignCursor)
    ));
    topic(&mut engine, &room, "four");
    assert!(matches!(
        engine.topic_collection_page(&owner, &room, &filter, Some(&cursor), 1),
        Err(ObservationRefusal::Stale { .. })
    ));
}

#[test]
fn collection_filter_shape_is_canonical_and_bounded() {
    let (engine, room) = setup();
    let observer = "owner".into();
    for tags in [
        vec!["b".to_owned(), "a".to_owned()],
        vec!["a".to_owned(), "a".to_owned()],
    ] {
        assert!(matches!(
            engine.topic_collection_page(
                &observer,
                &room,
                &TopicCollectionFilterV1 {
                    lifecycle: TopicCollectionLifecycleV1::Any,
                    tags
                },
                None,
                1,
            ),
            Err(ObservationRefusal::InvalidTarget)
        ));
    }
}
