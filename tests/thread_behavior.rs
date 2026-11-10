use agora_engine::{
    AdmissionPolicy, CommandV2, Engine, Limits, LogicalTime, MessageId, ObservationRefusal,
    OutcomeV2, Participant, ParticipantId, ParticipantKind, Refusal, RevisionedCommandV2, RoomId,
};

fn setup(max_messages_per_room: usize) -> Engine {
    let mut engine = Engine::with_limits(Limits {
        max_messages_per_room,
        ..Limits::default()
    });
    for (id, kind) in [
        ("alice", ParticipantKind::Human),
        ("bot", ParticipantKind::Agent),
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
            owner: "alice".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(CommandV2::Join {
            room_id: "room".into(),
            participant: "bot".into(),
        })
        .unwrap();
    engine
}

fn root(engine: &mut Engine, author: &str, body: &str) -> MessageId {
    let at = LogicalTime(sequence_hint(engine));
    let OutcomeV2::MessageAccepted { sequence, .. } = engine
        .apply(CommandV2::Speak {
            room_id: "room".into(),
            author: author.into(),
            at,
            body: body.to_owned(),
        })
        .unwrap()
    else {
        panic!("root acceptance expected");
    };
    engine.message(&RoomId::new("room"), sequence).unwrap().id()
}

fn sequence_hint(engine: &Engine) -> u64 {
    engine.revision().0
}

fn reply(engine: &mut Engine, author: &str, root: MessageId, body: &str) -> MessageId {
    let at = LogicalTime(sequence_hint(engine));
    let OutcomeV2::ReplyAccepted {
        root_message_id,
        sequence,
        ..
    } = engine
        .apply(CommandV2::SpeakReply {
            room_id: "room".into(),
            author: author.into(),
            root_message_id: root,
            at,
            body: body.to_owned(),
            addressed_to: vec![ParticipantId::new("alice")],
        })
        .unwrap()
    else {
        panic!("reply acceptance expected");
    };
    assert_eq!(root_message_id, root);
    engine.message(&RoomId::new("room"), sequence).unwrap().id()
}

#[test]
fn replies_are_one_level_room_local_kind_neutral_and_atomic() {
    let mut engine = setup(16);
    let root_id = root(&mut engine, "alice", "root secret");
    let reply_id = reply(&mut engine, "bot", root_id, "reply secret");
    let retained = engine
        .message_by_id(&RoomId::new("room"), reply_id)
        .unwrap();
    assert_eq!(retained.reply_to_root(), Some(root_id));
    assert_eq!(retained.addressed_to(), &[ParticipantId::new("alice")]);

    let revision = engine.revision();
    let events = engine.pending_events().count();
    assert_eq!(
        engine.apply_revisioned(RevisionedCommandV2 {
            expected_revision: revision,
            command: CommandV2::SpeakReply {
                room_id: "room".into(),
                author: "alice".into(),
                root_message_id: reply_id,
                at: LogicalTime(99),
                body: "nested".to_owned(),
                addressed_to: vec![],
            },
        }),
        Err(Refusal::InvalidReplyTarget)
    );
    assert_eq!(engine.revision(), revision);
    assert_eq!(engine.pending_events().count(), events);
    assert_eq!(
        engine.apply(CommandV2::SpeakReply {
            room_id: "room".into(),
            author: "alice".into(),
            root_message_id: MessageId::from_bytes([0xff; 16]),
            at: LogicalTime(100),
            body: "unknown".to_owned(),
            addressed_to: vec![],
        }),
        Err(Refusal::UnknownMessage)
    );
}

#[test]
fn roots_and_replies_page_independently_with_selector_isolation() {
    let mut engine = setup(16);
    let first = root(&mut engine, "alice", "first");
    reply(&mut engine, "bot", first, "one");
    reply(&mut engine, "alice", first, "two");
    let second = root(&mut engine, "bot", "second");
    reply(&mut engine, "alice", second, "other");

    let room = RoomId::new("room");
    let observer = ParticipantId::new("alice");
    let roots = engine.thread_root_page(&observer, &room, None, 1).unwrap();
    assert_eq!(roots.messages.len(), 1);
    assert_eq!(roots.messages[0].id(), first);
    assert_eq!(roots.root_message_id, None);
    let cursor = roots.next.as_ref().unwrap();
    assert_eq!(
        engine.thread_reply_page(&observer, &room, first, Some(cursor), 1),
        Err(ObservationRefusal::ForeignCursor)
    );
    let second_roots = engine
        .thread_root_page(&observer, &room, Some(cursor), 1)
        .unwrap();
    assert_eq!(second_roots.messages[0].id(), second);
    assert!(second_roots.next.is_none());

    let replies = engine
        .thread_reply_page(&observer, &room, first, None, 1)
        .unwrap();
    assert_eq!(replies.root_message_id, Some(first));
    assert_eq!(replies.messages[0].reply_to_root(), Some(first));
    let continued = engine
        .thread_reply_page(&observer, &room, first, replies.next.as_ref(), 1)
        .unwrap();
    assert_eq!(continued.messages.len(), 1);
    assert!(continued.next.is_none());

    let rendered = format!("{roots:?} {replies:?} {cursor:?}");
    for secret in ["first", "one", "alice"] {
        assert!(!rendered.contains(secret));
    }
}

#[test]
fn tombstones_preserve_relationship_and_retention_makes_evicted_roots_unreplyable() {
    let mut engine = setup(3);
    let root_id = root(&mut engine, "alice", "root");
    let first_reply = reply(&mut engine, "bot", root_id, "reply");
    engine
        .apply(CommandV2::WithdrawMessage {
            room_id: "room".into(),
            actor: "alice".into(),
            message_id: root_id,
        })
        .unwrap();
    let after_tombstone = reply(&mut engine, "bot", root_id, "still valid");
    assert_eq!(
        engine
            .message_by_id(&RoomId::new("room"), after_tombstone)
            .unwrap()
            .reply_to_root(),
        Some(root_id)
    );

    root(&mut engine, "alice", "evicts root");
    assert!(
        engine
            .message_by_id(&RoomId::new("room"), root_id)
            .is_none()
    );
    let revision = engine.revision();
    assert_eq!(
        engine.apply(CommandV2::SpeakReply {
            room_id: "room".into(),
            author: "bot".into(),
            root_message_id: root_id,
            at: LogicalTime(200),
            body: "too late".to_owned(),
            addressed_to: vec![],
        }),
        Err(Refusal::UnknownMessage)
    );
    assert_eq!(engine.revision(), revision);
    let page = engine
        .thread_reply_page(
            &ParticipantId::new("alice"),
            &RoomId::new("room"),
            root_id,
            None,
            3,
        )
        .unwrap();
    assert!(page.oldest_retained_sequence > 0);
    assert!(
        page.messages
            .iter()
            .any(|message| message.id() == first_reply)
    );
}
