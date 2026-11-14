use agora_engine::{
    AdmissionPolicy, CommandV2, Engine, Limits, LogicalTime, MAX_REACTION_KEY_BYTES_V1,
    MAX_REACTION_KEYS_PER_MESSAGE_V1, MessageId, ModerationCausationV1, ModerationReasonV1,
    ObservationRefusal, OutcomeV2, Participant, ParticipantId, ParticipantKind, Refusal,
    RevisionedCommandV2, RoomId,
};

fn setup(retention: usize) -> (Engine, MessageId) {
    let mut engine = Engine::with_limits(Limits {
        max_messages_per_room: retention,
        ..Limits::default()
    });
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
            room_id: "room".into(),
            name: "Room".to_owned(),
            owner: "human".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(CommandV2::Join {
            room_id: "room".into(),
            participant: "agent".into(),
        })
        .unwrap();
    let OutcomeV2::MessageAccepted { sequence, .. } = engine
        .apply(CommandV2::Speak {
            room_id: "room".into(),
            author: "human".into(),
            at: LogicalTime(1),
            body: "secret body".to_owned(),
        })
        .unwrap()
    else {
        panic!("message acceptance expected");
    };
    let message = engine.message(&RoomId::new("room"), sequence).unwrap().id();
    (engine, message)
}

fn react(engine: &mut Engine, actor: &str, message_id: MessageId, key: &str, reacted: bool) {
    engine
        .apply(CommandV2::SetReaction {
            room_id: "room".into(),
            actor: actor.into(),
            message_id,
            key: key.to_owned(),
            reacted,
        })
        .unwrap();
}

#[test]
fn reactions_are_kind_neutral_canonical_sets_and_atomic() {
    let (mut engine, message_id) = setup(8);
    react(&mut engine, "agent", message_id, "spark", true);
    react(&mut engine, "human", message_id, "spark", true);
    react(&mut engine, "human", message_id, "approve", true);

    let status = engine
        .reaction_status(
            &ParticipantId::new("agent"),
            &RoomId::new("room"),
            message_id,
        )
        .unwrap();
    assert_eq!(status.reactions.len(), 2);
    assert_eq!(status.reactions[0].key, "approve");
    assert_eq!(status.reactions[0].count, 1);
    assert!(!status.reactions[0].caller_reacted);
    assert_eq!(status.reactions[1].key, "spark");
    assert_eq!(status.reactions[1].count, 2);
    assert!(status.reactions[1].caller_reacted);

    let revision = engine.revision();
    let events = engine.pending_events().count();
    assert_eq!(
        engine.apply_revisioned(RevisionedCommandV2 {
            expected_revision: revision,
            command: CommandV2::SetReaction {
                room_id: "room".into(),
                actor: "agent".into(),
                message_id,
                key: "spark".to_owned(),
                reacted: true,
            },
        }),
        Err(Refusal::InvalidReactionChange)
    );
    assert_eq!(engine.revision(), revision);
    assert_eq!(engine.pending_events().count(), events);

    react(&mut engine, "agent", message_id, "spark", false);
    assert!(
        !engine
            .reaction_status(
                &ParticipantId::new("agent"),
                &RoomId::new("room"),
                message_id,
            )
            .unwrap()
            .reactions[1]
            .caller_reacted
    );
}

#[test]
fn reaction_bounds_membership_tombstones_and_retention_are_closed() {
    let (mut engine, message_id) = setup(2);
    let revision = engine.revision();
    assert_eq!(
        engine.apply(CommandV2::SetReaction {
            room_id: "room".into(),
            actor: "agent".into(),
            message_id,
            key: "x".repeat(MAX_REACTION_KEY_BYTES_V1 + 1),
            reacted: true,
        }),
        Err(Refusal::InvalidReaction)
    );
    assert_eq!(engine.revision(), revision);

    for index in 0..MAX_REACTION_KEYS_PER_MESSAGE_V1 {
        react(
            &mut engine,
            "agent",
            message_id,
            &format!("r{index:02}"),
            true,
        );
    }
    assert_eq!(
        engine.apply(CommandV2::SetReaction {
            room_id: "room".into(),
            actor: "human".into(),
            message_id,
            key: "overflow".to_owned(),
            reacted: true,
        }),
        Err(Refusal::ReactionLimit)
    );

    engine
        .apply(CommandV2::SetThreadArchived {
            room_id: "room".into(),
            actor: "human".into(),
            root_message_id: message_id,
            archived: true,
            reason: ModerationReasonV1::new("pause"),
            causation: ModerationCausationV1::new([7; 16]),
        })
        .unwrap();
    assert_eq!(
        engine
            .reaction_status(
                &ParticipantId::new("human"),
                &RoomId::new("room"),
                message_id,
            )
            .unwrap()
            .reactions
            .len(),
        MAX_REACTION_KEYS_PER_MESSAGE_V1
    );

    engine
        .apply(CommandV2::WithdrawMessage {
            room_id: "room".into(),
            actor: "human".into(),
            message_id,
        })
        .unwrap();
    assert_eq!(
        engine
            .reaction_status(
                &ParticipantId::new("human"),
                &RoomId::new("room"),
                message_id,
            )
            .unwrap()
            .reactions
            .len(),
        MAX_REACTION_KEYS_PER_MESSAGE_V1
    );

    engine
        .apply(CommandV2::Speak {
            room_id: "room".into(),
            author: "agent".into(),
            at: LogicalTime(2),
            body: "next".to_owned(),
        })
        .unwrap();
    engine
        .apply(CommandV2::Speak {
            room_id: "room".into(),
            author: "agent".into(),
            at: LogicalTime(3),
            body: "evict".to_owned(),
        })
        .unwrap();
    assert_eq!(
        engine.reaction_status(
            &ParticipantId::new("human"),
            &RoomId::new("room"),
            message_id,
        ),
        Err(ObservationRefusal::InvalidTarget)
    );
}
