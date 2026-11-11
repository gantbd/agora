use agora_engine::{
    AdmissionPolicy, CommandV2 as EngineCommand, Engine, Limits, LogicalTime, MessageId, OutcomeV2,
    Participant, ParticipantId, ParticipantKind, Revision, RoomId, TopicId,
};
use agora_service::transient::{PresenceSourceV1, TransientParticipantKindV1, TransientTimeV1};
use agora_service::typing::{
    TypingCollaborationV1, TypingLimitsV1, TypingRefusalV1, TypingTargetV1,
};

struct Fixture {
    engine: Engine,
    human: ParticipantId,
    agent: ParticipantId,
    outsider: ParticipantId,
    room: RoomId,
    root: MessageId,
    reply: MessageId,
    topic: TopicId,
}

fn fixture() -> Fixture {
    let mut engine = Engine::new();
    let human = ParticipantId::new("human");
    let agent = ParticipantId::new("agent");
    let outsider = ParticipantId::new("outsider");
    let room = RoomId::new("room");
    for (id, kind) in [
        ("human", ParticipantKind::Human),
        ("agent", ParticipantKind::Agent),
        ("outsider", ParticipantKind::Human),
    ] {
        engine
            .apply(EngineCommand::RegisterParticipant(Participant::new(
                id, id, kind,
            )))
            .unwrap();
    }
    engine
        .apply(EngineCommand::CreateRoom {
            room_id: room.clone(),
            name: "room".to_owned(),
            owner: human.clone(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(EngineCommand::Join {
            room_id: room.clone(),
            participant: agent.clone(),
        })
        .unwrap();
    let OutcomeV2::MessageAccepted { sequence, .. } = engine
        .apply(EngineCommand::Speak {
            room_id: room.clone(),
            author: human.clone(),
            at: LogicalTime(1),
            body: "root".to_owned(),
        })
        .unwrap()
    else {
        panic!("root acceptance expected");
    };
    let root = engine.message(&room, sequence).unwrap().id();
    let OutcomeV2::ReplyAccepted { sequence, .. } = engine
        .apply(EngineCommand::SpeakReply {
            room_id: room.clone(),
            author: agent.clone(),
            root_message_id: root,
            at: LogicalTime(2),
            body: "reply".to_owned(),
            addressed_to: Vec::new(),
        })
        .unwrap()
    else {
        panic!("reply acceptance expected");
    };
    let reply = engine.message(&room, sequence).unwrap().id();
    let OutcomeV2::TopicCreated { topic_id, .. } = engine
        .apply(EngineCommand::CreateTopic {
            room_id: room.clone(),
            actor: human.clone(),
            name: "topic".to_owned(),
        })
        .unwrap()
    else {
        panic!("topic creation expected");
    };
    Fixture {
        engine,
        human,
        agent,
        outsider,
        room,
        root,
        reply,
        topic: topic_id,
    }
}

fn owner(max_signals: usize, max_targets_per_source: usize) -> TypingCollaborationV1 {
    TypingCollaborationV1::new(TypingLimitsV1 {
        max_signals,
        max_targets_per_source,
        max_lifetime: 10,
        max_page_items: 8,
    })
    .unwrap()
}

#[test]
fn room_thread_and_topic_typing_are_kind_neutral_private_and_revision_free() {
    let fixture = fixture();
    let revision = fixture.engine.revision();
    let mut typing = owner(8, 4);
    for (index, (actor, observer, target, kind)) in [
        (
            &fixture.human,
            &fixture.agent,
            TypingTargetV1::Room,
            TransientParticipantKindV1::Human,
        ),
        (
            &fixture.agent,
            &fixture.human,
            TypingTargetV1::Thread(fixture.root),
            TransientParticipantKindV1::Agent,
        ),
        (
            &fixture.agent,
            &fixture.human,
            TypingTargetV1::Topic(fixture.topic),
            TransientParticipantKindV1::Agent,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let now = TransientTimeV1(index as u64 + 1);
        let source = PresenceSourceV1::new([index as u8 + 1; 16]);
        assert_eq!(
            typing
                .publish(
                    &fixture.engine,
                    &fixture.room,
                    actor,
                    target,
                    source,
                    now,
                    5
                )
                .unwrap(),
            TransientTimeV1(now.0 + 5)
        );
        let page = typing
            .page(
                &fixture.engine,
                observer,
                &fixture.room,
                target,
                None,
                8,
                now,
                now,
            )
            .unwrap();
        let entry = page
            .entries
            .iter()
            .find(|entry| entry.participant_id == *actor)
            .unwrap();
        assert!(entry.active);
        assert_eq!(entry.participant_kind, kind);
        assert!(entry.source.is_some());
    }
    assert_eq!(fixture.engine.revision(), revision);
}

#[test]
fn authority_and_target_validation_fail_closed_while_cleanup_remains_possible() {
    let fixture = fixture();
    let source = PresenceSourceV1::new([4; 16]);
    let mut typing = owner(4, 2);
    assert_eq!(
        typing.publish(
            &fixture.engine,
            &fixture.room,
            &fixture.outsider,
            TypingTargetV1::Room,
            source,
            TransientTimeV1(1),
            2,
        ),
        Err(TypingRefusalV1::NotMember)
    );
    assert_eq!(
        typing.publish(
            &fixture.engine,
            &fixture.room,
            &fixture.human,
            TypingTargetV1::Thread(fixture.reply),
            source,
            TransientTimeV1(1),
            2,
        ),
        Err(TypingRefusalV1::InvalidTarget)
    );
    assert_eq!(
        typing.publish(
            &fixture.engine,
            &fixture.room,
            &fixture.human,
            TypingTargetV1::Topic(TopicId::from_bytes([0xff; 16])),
            source,
            TransientTimeV1(1),
            2,
        ),
        Err(TypingRefusalV1::InvalidTarget)
    );
    typing
        .publish(
            &fixture.engine,
            &fixture.room,
            &fixture.human,
            TypingTargetV1::Room,
            source,
            TransientTimeV1(1),
            2,
        )
        .unwrap();
    assert!(
        typing
            .clear(
                &fixture.engine,
                &fixture.room,
                &fixture.human,
                TypingTargetV1::Topic(TopicId::from_bytes([0xff; 16])),
                source,
                TransientTimeV1(2),
            )
            .is_ok()
    );
}

#[test]
fn capacity_refusals_are_atomic_and_expiry_is_bounded() {
    let fixture = fixture();
    let source = PresenceSourceV1::new([5; 16]);
    let mut typing = owner(2, 1);
    typing
        .publish(
            &fixture.engine,
            &fixture.room,
            &fixture.human,
            TypingTargetV1::Room,
            source,
            TransientTimeV1(1),
            2,
        )
        .unwrap();
    let generation = typing.generation();
    assert_eq!(
        typing.publish(
            &fixture.engine,
            &fixture.room,
            &fixture.human,
            TypingTargetV1::Thread(fixture.root),
            source,
            TransientTimeV1(1),
            2,
        ),
        Err(TypingRefusalV1::Capacity)
    );
    assert_eq!(typing.generation(), generation);
    assert_eq!(typing.signal_count(), 1);
    typing
        .publish(
            &fixture.engine,
            &fixture.room,
            &fixture.agent,
            TypingTargetV1::Room,
            PresenceSourceV1::new([9; 16]),
            TransientTimeV1(1),
            2,
        )
        .unwrap();
    let generation = typing.generation();
    assert_eq!(
        typing.publish(
            &fixture.engine,
            &fixture.room,
            &fixture.human,
            TypingTargetV1::Thread(fixture.root),
            PresenceSourceV1::new([10; 16]),
            TransientTimeV1(1),
            2,
        ),
        Err(TypingRefusalV1::Capacity)
    );
    assert_eq!(typing.generation(), generation);
    assert_eq!(typing.signal_count(), 2);
    assert_eq!(
        typing.expire(TransientTimeV1(3), 0),
        Err(TypingRefusalV1::InvalidWorkBudget)
    );
    assert_eq!(typing.expire(TransientTimeV1(3), 1).unwrap(), 1);
    assert_eq!(typing.signal_count(), 1);
    assert_eq!(typing.expire(TransientTimeV1(3), 1).unwrap(), 1);
    assert_eq!(typing.signal_count(), 0);
    assert_eq!(
        typing.expire(TransientTimeV1(2), 1),
        Err(TypingRefusalV1::ClockRegression)
    );
}

#[test]
fn cursors_bind_observer_target_snapshot_and_engine_revision() {
    let mut fixture = fixture();
    let mut typing = owner(8, 4);
    typing
        .publish(
            &fixture.engine,
            &fixture.room,
            &fixture.human,
            TypingTargetV1::Room,
            PresenceSourceV1::new([6; 16]),
            TransientTimeV1(1),
            8,
        )
        .unwrap();
    let first = typing
        .page(
            &fixture.engine,
            &fixture.human,
            &fixture.room,
            TypingTargetV1::Room,
            None,
            1,
            TransientTimeV1(2),
            TransientTimeV1(2),
        )
        .unwrap();
    let cursor = first.next.unwrap();
    assert_eq!(
        typing.page(
            &fixture.engine,
            &fixture.agent,
            &fixture.room,
            TypingTargetV1::Room,
            Some(&cursor),
            1,
            TransientTimeV1(2),
            TransientTimeV1(2),
        ),
        Err(TypingRefusalV1::ForeignCursor)
    );
    assert_eq!(
        typing.page(
            &fixture.engine,
            &fixture.human,
            &fixture.room,
            TypingTargetV1::Topic(fixture.topic),
            Some(&cursor),
            1,
            TransientTimeV1(2),
            TransientTimeV1(2),
        ),
        Err(TypingRefusalV1::ForeignCursor)
    );
    let continuation = typing
        .page(
            &fixture.engine,
            &fixture.human,
            &fixture.room,
            TypingTargetV1::Room,
            Some(&cursor),
            1,
            TransientTimeV1(3),
            TransientTimeV1(2),
        )
        .unwrap();
    assert_eq!(continuation.as_of, TransientTimeV1(2));
    fixture
        .engine
        .apply(EngineCommand::Leave {
            room_id: fixture.room.clone(),
            participant: fixture.agent.clone(),
        })
        .unwrap();
    assert_eq!(
        typing.page(
            &fixture.engine,
            &fixture.human,
            &fixture.room,
            TypingTargetV1::Room,
            Some(&cursor),
            1,
            TransientTimeV1(3),
            TransientTimeV1(2),
        ),
        Err(TypingRefusalV1::EngineChanged)
    );
    assert_eq!(format!("{cursor:?}"), "TypingCursorV1(<opaque>)");
}

#[test]
fn retirement_reconciliation_and_restart_prevent_signal_resurrection() {
    let mut fixture = fixture();
    let source = PresenceSourceV1::new([7; 16]);
    let mut typing = owner(8, 4);
    for target in [TypingTargetV1::Room, TypingTargetV1::Thread(fixture.root)] {
        typing
            .publish(
                &fixture.engine,
                &fixture.room,
                &fixture.agent,
                target,
                source,
                TransientTimeV1(1),
                8,
            )
            .unwrap();
    }
    assert_eq!(
        typing.retire_participant_room(&fixture.room, &fixture.agent),
        2
    );
    typing
        .publish(
            &fixture.engine,
            &fixture.room,
            &fixture.agent,
            TypingTargetV1::Room,
            source,
            TransientTimeV1(2),
            8,
        )
        .unwrap();
    fixture
        .engine
        .apply(EngineCommand::Leave {
            room_id: fixture.room.clone(),
            participant: fixture.agent.clone(),
        })
        .unwrap();
    assert_eq!(typing.reconcile(&fixture.engine), 1);
    fixture
        .engine
        .apply(EngineCommand::Join {
            room_id: fixture.room.clone(),
            participant: fixture.agent.clone(),
        })
        .unwrap();
    let page = typing
        .page(
            &fixture.engine,
            &fixture.human,
            &fixture.room,
            TypingTargetV1::Room,
            None,
            8,
            TransientTimeV1(3),
            TransientTimeV1(3),
        )
        .unwrap();
    assert!(page.entries.iter().all(|entry| !entry.active));
    assert!(!format!("{page:?}").contains("human"));

    let mut restarted = owner(8, 4);
    let restarted_page = restarted
        .page(
            &fixture.engine,
            &fixture.human,
            &fixture.room,
            TypingTargetV1::Room,
            None,
            8,
            TransientTimeV1(100),
            TransientTimeV1(100),
        )
        .unwrap();
    assert!(restarted_page.entries.iter().all(|entry| !entry.active));
    assert_eq!(fixture.engine.revision(), Revision(10));
}

#[test]
fn reconciliation_retires_an_evicted_thread_target() {
    let mut engine = Engine::with_limits(Limits {
        max_messages_per_room: 1,
        ..Limits::default()
    });
    let participant = ParticipantId::new("participant");
    let room = RoomId::new("room");
    engine
        .apply(EngineCommand::RegisterParticipant(Participant::new(
            "participant",
            "participant",
            ParticipantKind::Agent,
        )))
        .unwrap();
    engine
        .apply(EngineCommand::CreateRoom {
            room_id: room.clone(),
            name: "room".to_owned(),
            owner: participant.clone(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    let OutcomeV2::MessageAccepted { sequence, .. } = engine
        .apply(EngineCommand::Speak {
            room_id: room.clone(),
            author: participant.clone(),
            at: LogicalTime(1),
            body: "first".to_owned(),
        })
        .unwrap()
    else {
        panic!("root acceptance expected");
    };
    let root = engine.message(&room, sequence).unwrap().id();
    let mut typing = owner(2, 2);
    typing
        .publish(
            &engine,
            &room,
            &participant,
            TypingTargetV1::Thread(root),
            PresenceSourceV1::new([11; 16]),
            TransientTimeV1(1),
            8,
        )
        .unwrap();
    engine
        .apply(EngineCommand::Speak {
            room_id: room.clone(),
            author: participant,
            at: LogicalTime(2),
            body: "replacement".to_owned(),
        })
        .unwrap();
    assert!(engine.message_by_id(&room, root).is_none());
    assert_eq!(typing.reconcile(&engine), 1);
    assert_eq!(typing.signal_count(), 0);
}
