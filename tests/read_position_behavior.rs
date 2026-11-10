use agora_engine::persistence::CheckpointError;
use agora_engine::{
    AdmissionPolicy, CommandV2, Engine, Limits, LogicalTime, OutcomeV2, Participant, ParticipantId,
    ParticipantKind, ReadPositionRefusalV1, ReadScopeV1, RoomId,
};

fn setup(max_messages: usize) -> Engine {
    let mut engine = Engine::with_limits(Limits {
        max_messages_per_room: max_messages,
        ..Limits::default()
    });
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
    engine
        .apply(CommandV2::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
            owner: "owner".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(CommandV2::Join {
            room_id: "room".into(),
            participant: "agent".into(),
        })
        .unwrap();
    engine
}

fn root(engine: &mut Engine, author: &str, body: &str) -> agora_engine::MessageId {
    let OutcomeV2::MessageAccepted { sequence, .. } = engine
        .apply(CommandV2::Speak {
            room_id: "room".into(),
            author: author.into(),
            at: LogicalTime(engine.revision().0),
            body: body.to_owned(),
        })
        .unwrap()
    else {
        panic!("message expected");
    };
    engine.message(&RoomId::new("room"), sequence).unwrap().id()
}

#[test]
fn private_room_positions_are_monotone_idempotent_and_not_domain_events() {
    let mut engine = setup(8);
    let first = root(&mut engine, "owner", "first");
    let second = root(&mut engine, "agent", "second");
    let revision = engine.revision();
    let events = engine.pending_events().count();

    let marker = engine
        .advance_read_position(
            &ParticipantId::new("agent"),
            &RoomId::new("room"),
            ReadScopeV1::Room,
            revision,
            first,
        )
        .unwrap();
    assert_eq!(marker.target_message_id(), first);
    assert_eq!(engine.revision(), revision);
    assert_eq!(engine.pending_events().count(), events);
    assert_eq!(
        engine
            .advance_read_position(
                &ParticipantId::new("agent"),
                &RoomId::new("room"),
                ReadScopeV1::Room,
                revision,
                first,
            )
            .unwrap(),
        marker
    );

    let advanced = engine
        .advance_read_position(
            &ParticipantId::new("agent"),
            &RoomId::new("room"),
            ReadScopeV1::Room,
            revision,
            second,
        )
        .unwrap();
    assert!(advanced.target_sequence() > marker.target_sequence());
    assert_eq!(
        engine.advance_read_position(
            &ParticipantId::new("agent"),
            &RoomId::new("room"),
            ReadScopeV1::Room,
            revision,
            first,
        ),
        Err(ReadPositionRefusalV1::Regression)
    );
    assert_eq!(
        engine
            .read_position(
                &ParticipantId::new("agent"),
                &RoomId::new("room"),
                &ReadScopeV1::Room,
            )
            .unwrap(),
        Some(&advanced)
    );
}

#[test]
fn thread_and_topic_scopes_validate_exact_targets() {
    let mut engine = setup(12);
    let OutcomeV2::TopicCreated { topic_id, .. } = engine
        .apply(CommandV2::CreateTopic {
            room_id: "room".into(),
            actor: "owner".into(),
            name: "ops".to_owned(),
        })
        .unwrap()
    else {
        panic!("topic expected");
    };
    let OutcomeV2::TopicMessageAccepted { sequence, .. } = engine
        .apply(CommandV2::SpeakInTopic {
            room_id: "room".into(),
            author: "owner".into(),
            topic_id,
            at: LogicalTime(1),
            body: "root".to_owned(),
            addressed_to: vec![],
        })
        .unwrap()
    else {
        panic!("topic message expected");
    };
    let root_id = engine.message(&RoomId::new("room"), sequence).unwrap().id();
    let OutcomeV2::ReplyAccepted { sequence, .. } = engine
        .apply(CommandV2::SpeakReply {
            room_id: "room".into(),
            author: "agent".into(),
            root_message_id: root_id,
            at: LogicalTime(2),
            body: "reply".to_owned(),
            addressed_to: vec![],
        })
        .unwrap()
    else {
        panic!("reply expected");
    };
    let reply_id = engine.message(&RoomId::new("room"), sequence).unwrap().id();
    let other = root(&mut engine, "owner", "other");
    let revision = engine.revision();

    for scope in [ReadScopeV1::Thread(root_id), ReadScopeV1::Topic(topic_id)] {
        let marker = engine
            .advance_read_position(
                &ParticipantId::new("agent"),
                &RoomId::new("room"),
                scope,
                revision,
                reply_id,
            )
            .unwrap();
        assert_eq!(marker.scope(), &scope);
        assert_eq!(
            engine.advance_read_position(
                &ParticipantId::new("agent"),
                &RoomId::new("room"),
                scope,
                revision,
                other,
            ),
            Err(ReadPositionRefusalV1::InvalidTarget)
        );
    }
}

#[test]
fn membership_and_retention_do_not_silently_destroy_markers() {
    let mut engine = setup(2);
    let first = root(&mut engine, "owner", "first");
    let revision = engine.revision();
    let marker = engine
        .advance_read_position(
            &ParticipantId::new("agent"),
            &RoomId::new("room"),
            ReadScopeV1::Room,
            revision,
            first,
        )
        .unwrap();
    engine
        .apply(CommandV2::Leave {
            room_id: "room".into(),
            participant: "agent".into(),
        })
        .unwrap();
    assert_eq!(
        engine.read_position(
            &ParticipantId::new("agent"),
            &RoomId::new("room"),
            &ReadScopeV1::Room,
        ),
        Err(ReadPositionRefusalV1::NotMember)
    );
    engine
        .apply(CommandV2::Join {
            room_id: "room".into(),
            participant: "agent".into(),
        })
        .unwrap();
    root(&mut engine, "owner", "second");
    root(&mut engine, "owner", "third");
    assert!(engine.message_by_id(&RoomId::new("room"), first).is_none());
    assert_eq!(
        engine
            .read_position(
                &ParticipantId::new("agent"),
                &RoomId::new("room"),
                &ReadScopeV1::Room,
            )
            .unwrap(),
        Some(&marker)
    );
}

#[test]
fn stale_and_unknown_targets_refuse_atomically_and_persistence_is_fenced() {
    let mut engine = setup(4);
    let target = root(&mut engine, "owner", "target");
    let revision = engine.revision();
    assert!(matches!(
        engine.advance_read_position(
            &ParticipantId::new("agent"),
            &RoomId::new("room"),
            ReadScopeV1::Room,
            agora_engine::Revision(revision.0 - 1),
            target,
        ),
        Err(ReadPositionRefusalV1::Stale { .. })
    ));
    assert!(
        engine
            .read_position(
                &ParticipantId::new("agent"),
                &RoomId::new("room"),
                &ReadScopeV1::Room,
            )
            .unwrap()
            .is_none()
    );
    engine
        .advance_read_position(
            &ParticipantId::new("agent"),
            &RoomId::new("room"),
            ReadScopeV1::Room,
            revision,
            target,
        )
        .unwrap();
    assert!(matches!(
        engine.prepare_checkpoint(65_536),
        Err(CheckpointError::UnsupportedState)
    ));
}
