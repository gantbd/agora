use agora_engine::persistence::{CursorRecoveryLimitsV1, RecoveryLimits};
use agora_engine::{
    AdmissionPolicy, CommandV2, Engine, Limits, LogicalTime, ObservationRefusal, OutcomeV2,
    Participant, ParticipantId, ParticipantKind, Refusal, Role, RoomId,
};

fn setup(limits: Limits) -> Engine {
    let mut engine = Engine::with_limits(limits);
    for (id, kind) in [
        ("owner", ParticipantKind::Human),
        ("agent", ParticipantKind::Agent),
        ("member", ParticipantKind::Human),
    ] {
        engine
            .apply(CommandV2::RegisterParticipant(Participant::new(
                id, id, kind,
            )))
            .unwrap();
    }
    engine
        .apply(CommandV2::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
            owner: "owner".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    for participant in ["agent", "member"] {
        engine
            .apply(CommandV2::Join {
                room_id: "room".into(),
                participant: participant.into(),
            })
            .unwrap();
    }
    engine
}

fn create_topic(engine: &mut Engine, actor: &str, name: &str) -> agora_engine::TopicId {
    let OutcomeV2::TopicCreated { topic_id, .. } = engine
        .apply(CommandV2::CreateTopic {
            room_id: "room".into(),
            actor: actor.into(),
            name: name.to_owned(),
        })
        .unwrap()
    else {
        panic!("topic creation expected");
    };
    topic_id
}

#[test]
fn topic_identity_and_creation_authority_are_bounded_kind_neutral_and_atomic() {
    let mut engine = setup(Limits {
        max_topics_per_room: 2,
        max_topic_name_bytes: 8,
        ..Limits::default()
    });
    assert!(matches!(
        engine.apply(CommandV2::CreateTopic {
            room_id: "room".into(),
            actor: "member".into(),
            name: "general".to_owned(),
        }),
        Err(Refusal::NotAuthorized(_))
    ));
    let first = create_topic(&mut engine, "owner", "general");
    engine
        .apply(CommandV2::SetRole {
            room_id: "room".into(),
            actor: "owner".into(),
            participant: "agent".into(),
            role: Role::Moderator,
        })
        .unwrap();
    let second = create_topic(&mut engine, "agent", "ops");
    assert_ne!(first, second);
    assert_eq!(
        engine.topic(&RoomId::new("room"), first).unwrap().name(),
        "general"
    );

    let revision = engine.revision();
    let events = engine.pending_events().count();
    for (name, refusal) in [
        ("general", Refusal::DuplicateTopicName),
        ("too-long-name", Refusal::InvalidTopicName),
        ("third", Refusal::TopicLimit),
    ] {
        assert_eq!(
            engine.apply(CommandV2::CreateTopic {
                room_id: "room".into(),
                actor: "owner".into(),
                name: name.to_owned(),
            }),
            Err(refusal)
        );
        assert_eq!(engine.revision(), revision);
        assert_eq!(engine.pending_events().count(), events);
    }
}

#[test]
fn topic_rename_preserves_identity_and_relations_with_bounded_authority() {
    let mut engine = setup(Limits {
        max_topic_name_bytes: 8,
        ..Limits::default()
    });
    let topic = create_topic(&mut engine, "owner", "general");
    create_topic(&mut engine, "owner", "ops");
    let OutcomeV2::TopicMessageAccepted { sequence, .. } = engine
        .apply(CommandV2::SpeakInTopic {
            room_id: "room".into(),
            author: "agent".into(),
            topic_id: topic,
            at: LogicalTime(1),
            body: "retained".to_owned(),
            addressed_to: vec![],
        })
        .unwrap()
    else {
        panic!("topic message expected");
    };
    assert!(matches!(
        engine.apply(CommandV2::RenameTopic {
            room_id: "room".into(),
            actor: "member".into(),
            topic_id: topic,
            name: "chat".to_owned(),
        }),
        Err(Refusal::NotAuthorized(_))
    ));
    engine
        .apply(CommandV2::SetRole {
            room_id: "room".into(),
            actor: "owner".into(),
            participant: "agent".into(),
            role: Role::Moderator,
        })
        .unwrap();

    let renamed = engine
        .apply(CommandV2::RenameTopic {
            room_id: "room".into(),
            actor: "agent".into(),
            topic_id: topic,
            name: "chat".to_owned(),
        })
        .unwrap();
    assert!(matches!(
        renamed,
        OutcomeV2::TopicRenamed {
            topic_id,
            ref previous,
            ref name,
            ..
        } if topic_id == topic && previous == "general" && name == "chat"
    ));
    assert_eq!(engine.topic(&"room".into(), topic).unwrap().name(), "chat");
    assert_eq!(
        engine.message(&"room".into(), sequence).unwrap().topic_id(),
        Some(topic)
    );

    let revision = engine.revision();
    let event_count = engine.pending_events().count();
    for (topic_id, name, refusal) in [
        (topic, "chat", Refusal::DuplicateTopicName),
        (topic, "ops", Refusal::DuplicateTopicName),
        (topic, "too-long-name", Refusal::InvalidTopicName),
        (
            agora_engine::TopicId::from_bytes([0xff; 16]),
            "other",
            Refusal::UnknownTopic,
        ),
    ] {
        assert_eq!(
            engine.apply(CommandV2::RenameTopic {
                room_id: "room".into(),
                actor: "owner".into(),
                topic_id,
                name: name.to_owned(),
            }),
            Err(refusal)
        );
        assert_eq!(engine.revision(), revision);
        assert_eq!(engine.pending_events().count(), event_count);
    }
}

#[test]
fn topic_messages_preserve_relation_through_replies_edits_reactions_and_tombstones() {
    let mut engine = setup(Limits::default());
    let topic = create_topic(&mut engine, "owner", "general");
    let OutcomeV2::TopicMessageAccepted { sequence, .. } = engine
        .apply(CommandV2::SpeakInTopic {
            room_id: "room".into(),
            author: "agent".into(),
            topic_id: topic,
            at: LogicalTime(10),
            body: "root".to_owned(),
            addressed_to: vec!["owner".into()],
        })
        .unwrap()
    else {
        panic!("topic message expected");
    };
    let root = engine.message(&RoomId::new("room"), sequence).unwrap().id();
    let OutcomeV2::ReplyAccepted {
        sequence: reply_sequence,
        ..
    } = engine
        .apply(CommandV2::SpeakReply {
            room_id: "room".into(),
            author: "owner".into(),
            root_message_id: root,
            at: LogicalTime(11),
            body: "reply".to_owned(),
            addressed_to: vec![],
        })
        .unwrap()
    else {
        panic!("reply expected");
    };
    let reply = engine
        .message(&RoomId::new("room"), reply_sequence)
        .unwrap();
    assert_eq!(reply.topic_id(), Some(topic));
    engine
        .apply(CommandV2::SetReaction {
            room_id: "room".into(),
            actor: "member".into(),
            message_id: root,
            key: "spark".to_owned(),
            reacted: true,
        })
        .unwrap();
    engine
        .apply(CommandV2::EditMessage {
            room_id: "room".into(),
            actor: "agent".into(),
            message_id: root,
            at: LogicalTime(12),
            body: "revised".to_owned(),
        })
        .unwrap();
    engine
        .apply(CommandV2::SetThreadArchived {
            room_id: "room".into(),
            actor: "agent".into(),
            root_message_id: root,
            archived: true,
            reason: agora_engine::ModerationReasonV1::new("pause"),
            causation: agora_engine::ModerationCausationV1::new([7; 16]),
        })
        .unwrap();
    assert_eq!(
        engine
            .message_by_id(&RoomId::new("room"), root)
            .unwrap()
            .topic_id(),
        Some(topic)
    );
    engine
        .apply(CommandV2::WithdrawMessage {
            room_id: "room".into(),
            actor: "agent".into(),
            message_id: root,
        })
        .unwrap();
    assert_eq!(
        engine
            .message_by_id(&RoomId::new("room"), root)
            .unwrap()
            .topic_id(),
        Some(topic)
    );
    assert_eq!(
        engine
            .reaction_status(&ParticipantId::new("member"), &RoomId::new("room"), root)
            .unwrap()
            .reactions[0]
            .count,
        1
    );
}

#[test]
fn topic_registry_outlives_message_retention_and_unknown_topics_are_atomic() {
    let mut engine = setup(Limits {
        max_messages_per_room: 1,
        ..Limits::default()
    });
    let topic = create_topic(&mut engine, "owner", "general");
    for body in ["first", "second"] {
        engine
            .apply(CommandV2::SpeakInTopic {
                room_id: "room".into(),
                author: "agent".into(),
                topic_id: topic,
                at: LogicalTime(1),
                body: body.to_owned(),
                addressed_to: vec![],
            })
            .unwrap();
    }
    let page = engine
        .topic_message_page(
            &ParticipantId::new("member"),
            &RoomId::new("room"),
            topic,
            None,
            1,
        )
        .unwrap();
    assert_eq!(page.messages[0].body(), "second");
    assert_eq!(page.oldest_retained_sequence, 1);
    assert!(engine.topic(&RoomId::new("room"), topic).is_some());

    let revision = engine.revision();
    let events = engine.pending_events().count();
    assert_eq!(
        engine.apply(CommandV2::SpeakInTopic {
            room_id: "room".into(),
            author: "agent".into(),
            topic_id: agora_engine::TopicId::from_bytes([0xff; 16]),
            at: LogicalTime(2),
            body: "missing".to_owned(),
            addressed_to: vec![],
        }),
        Err(Refusal::UnknownTopic)
    );
    assert_eq!(engine.revision(), revision);
    assert_eq!(engine.pending_events().count(), events);
}

#[test]
fn topic_and_topic_message_pages_are_bounded_bound_and_revision_stale() {
    let mut engine = setup(Limits::default());
    let first = create_topic(&mut engine, "owner", "a");
    let second = create_topic(&mut engine, "owner", "b");
    for (topic_id, body, time) in [(first, "one", 1), (second, "other", 2), (first, "two", 3)] {
        engine
            .apply(CommandV2::SpeakInTopic {
                room_id: "room".into(),
                author: "agent".into(),
                topic_id,
                at: LogicalTime(time),
                body: body.to_owned(),
                addressed_to: vec![],
            })
            .unwrap();
    }
    let observer = ParticipantId::new("member");
    let room = RoomId::new("room");
    let topics = engine.topic_page(&observer, &room, None, 1).unwrap();
    assert_eq!(topics.topics.len(), 1);
    assert!(topics.next.is_some());
    let messages = engine
        .topic_message_page(&observer, &room, first, None, 1)
        .unwrap();
    assert_eq!(messages.messages[0].body(), "one");
    let cursor = messages.next.unwrap();
    assert_eq!(
        engine.topic_message_page(&observer, &room, second, Some(&cursor), 1),
        Err(ObservationRefusal::ForeignCursor)
    );
    engine
        .apply(CommandV2::SpeakInTopic {
            room_id: "room".into(),
            author: "agent".into(),
            topic_id: first,
            at: LogicalTime(4),
            body: "three".to_owned(),
            addressed_to: vec![],
        })
        .unwrap();
    assert!(matches!(
        engine.topic_message_page(&observer, &room, first, Some(&cursor), 1),
        Err(ObservationRefusal::Stale { .. })
    ));
}

#[test]
fn topic_state_and_cursor_bindings_survive_canonical_recovery() {
    let mut engine = setup(Limits::default());
    let first = create_topic(&mut engine, "owner", "a");
    create_topic(&mut engine, "owner", "b");
    for body in ["one", "two"] {
        engine
            .apply(CommandV2::SpeakInTopic {
                room_id: "room".into(),
                author: "agent".into(),
                topic_id: first,
                at: LogicalTime(10),
                body: body.to_owned(),
                addressed_to: vec![],
            })
            .unwrap();
    }
    let observer = ParticipantId::new("member");
    let room = RoomId::new("room");
    let topic_cursor = engine
        .topic_page(&observer, &room, None, 1)
        .unwrap()
        .next
        .unwrap();
    let message_cursor = engine
        .topic_message_page(&observer, &room, first, None, 1)
        .unwrap()
        .next
        .unwrap();
    let topic_bytes = topic_cursor.prepare_checkpoint(1024).unwrap();
    let message_bytes = message_cursor.prepare_checkpoint(1024).unwrap();
    let bounds = CursorRecoveryLimitsV1 {
        max_snapshot_bytes: 1024,
        max_participant_id_bytes: 128,
        max_room_id_bytes: 128,
    };
    let topic_cursor =
        agora_engine::TopicPageCursorV1::recover_checkpoint(topic_bytes.bytes(), bounds).unwrap();
    let message_cursor =
        agora_engine::TopicMessagePageCursorV1::recover_checkpoint(message_bytes.bytes(), bounds)
            .unwrap();
    let checkpoint = engine.prepare_checkpoint(1_000_000).unwrap();
    let recovered = Engine::recover_checkpoint(
        checkpoint.bytes(),
        RecoveryLimits {
            max_snapshot_bytes: 1_000_000,
            maximum_engine_limits: Limits::default(),
        },
    )
    .unwrap();
    assert_eq!(recovered.topic(&room, first).unwrap().name(), "a");
    assert_eq!(
        recovered
            .topic_page(&observer, &room, Some(&topic_cursor), 1)
            .unwrap()
            .topics
            .len(),
        1
    );
    let page = recovered
        .topic_message_page(&observer, &room, first, Some(&message_cursor), 1)
        .unwrap();
    assert_eq!(page.messages[0].body(), "two");
}
