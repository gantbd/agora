use agora_engine::{
    AdmissionPolicy, CommandV2, Engine, Limits, LogicalTime, Participant, ParticipantId,
    ParticipantKind, Refusal, Role, RoomId,
};

fn register(engine: &mut Engine, id: &str, kind: ParticipantKind) {
    engine
        .apply(CommandV2::RegisterParticipant(Participant::new(
            id, id, kind,
        )))
        .unwrap();
}

fn create_room(engine: &mut Engine, admission: AdmissionPolicy) {
    engine
        .apply(CommandV2::CreateRoom {
            room_id: RoomId::from("commons"),
            name: "Commons".into(),
            owner: ParticipantId::from("alice"),
            admission,
        })
        .unwrap();
}

#[test]
fn human_and_agent_join_and_speak_under_the_same_rules() {
    let mut engine = Engine::new();
    register(&mut engine, "alice", ParticipantKind::Human);
    register(&mut engine, "bot", ParticipantKind::Agent);
    create_room(&mut engine, AdmissionPolicy::Open);

    engine
        .apply(CommandV2::Join {
            room_id: RoomId::from("commons"),
            participant: ParticipantId::from("bot"),
        })
        .unwrap();
    engine
        .apply(CommandV2::Speak {
            room_id: RoomId::from("commons"),
            author: ParticipantId::from("alice"),
            at: LogicalTime(10),
            body: "question".into(),
        })
        .unwrap();
    engine
        .apply(CommandV2::Speak {
            room_id: RoomId::from("commons"),
            author: ParticipantId::from("bot"),
            at: LogicalTime(11),
            body: "answer".into(),
        })
        .unwrap();

    let room = engine.room(&RoomId::from("commons")).unwrap();
    assert_eq!(room.messages.len(), 2);
    assert_eq!(room.messages[0].sequence(), 0);
    assert_eq!(room.messages[1].sequence(), 1);
    assert_eq!(room.messages[1].author(), &ParticipantId::from("bot"));
}

#[test]
fn invite_only_admission_requires_explicit_authority() {
    let mut engine = Engine::new();
    register(&mut engine, "alice", ParticipantKind::Human);
    register(&mut engine, "bob", ParticipantKind::Human);
    register(&mut engine, "bot", ParticipantKind::Agent);
    create_room(&mut engine, AdmissionPolicy::InviteOnly);

    assert_eq!(
        engine.apply(CommandV2::Join {
            room_id: RoomId::from("commons"),
            participant: ParticipantId::from("bot"),
        }),
        Err(Refusal::InvitationRequired(ParticipantId::from("bot")))
    );
    assert_eq!(
        engine.apply(CommandV2::Invite {
            room_id: RoomId::from("commons"),
            actor: ParticipantId::from("bob"),
            participant: ParticipantId::from("bot"),
        }),
        Err(Refusal::NotAuthorized(ParticipantId::from("bob")))
    );
    engine
        .apply(CommandV2::Invite {
            room_id: RoomId::from("commons"),
            actor: ParticipantId::from("alice"),
            participant: ParticipantId::from("bot"),
        })
        .unwrap();
    engine
        .apply(CommandV2::Join {
            room_id: RoomId::from("commons"),
            participant: ParticipantId::from("bot"),
        })
        .unwrap();

    let room = engine.room(&RoomId::from("commons")).unwrap();
    assert_eq!(
        room.members,
        vec![
            (ParticipantId::from("alice"), Role::Owner),
            (ParticipantId::from("bot"), Role::Member),
        ]
    );
    assert!(room.pending_invitations.is_empty());
}

#[test]
fn owner_can_delegate_invitation_authority_without_transferring_ownership() {
    let mut engine = Engine::new();
    for id in ["alice", "bob", "bot"] {
        register(&mut engine, id, ParticipantKind::Human);
    }
    create_room(&mut engine, AdmissionPolicy::Open);
    engine
        .apply(CommandV2::Join {
            room_id: RoomId::from("commons"),
            participant: ParticipantId::from("bob"),
        })
        .unwrap();
    engine
        .apply(CommandV2::SetRole {
            room_id: RoomId::from("commons"),
            actor: ParticipantId::from("alice"),
            participant: ParticipantId::from("bob"),
            role: Role::Moderator,
        })
        .unwrap();
    engine
        .apply(CommandV2::Invite {
            room_id: RoomId::from("commons"),
            actor: ParticipantId::from("bob"),
            participant: ParticipantId::from("bot"),
        })
        .unwrap();

    assert_eq!(
        engine.apply(CommandV2::Leave {
            room_id: RoomId::from("commons"),
            participant: ParticipantId::from("alice"),
        }),
        Err(Refusal::OwnerCannotLeave)
    );
}

#[test]
fn retention_is_fifo_and_sequences_remain_monotone() {
    let mut engine = Engine::with_limits(Limits {
        max_messages_per_room: 2,
        ..Limits::default()
    });
    register(&mut engine, "alice", ParticipantKind::Human);
    create_room(&mut engine, AdmissionPolicy::Open);

    for (at, body) in ["one", "two", "three"].into_iter().enumerate() {
        engine
            .apply(CommandV2::Speak {
                room_id: RoomId::from("commons"),
                author: ParticipantId::from("alice"),
                at: LogicalTime(at as u64),
                body: body.into(),
            })
            .unwrap();
    }

    let room = engine.room(&RoomId::from("commons")).unwrap();
    assert_eq!(room.next_message_sequence, 3);
    assert_eq!(
        room.messages
            .iter()
            .map(|message| (message.sequence(), message.body()))
            .collect::<Vec<_>>(),
        vec![(1, "two"), (2, "three")]
    );
}

#[test]
fn refused_message_does_not_consume_sequence_or_change_transcript() {
    let mut engine = Engine::with_limits(Limits {
        max_message_body_bytes: 4,
        ..Limits::default()
    });
    register(&mut engine, "alice", ParticipantKind::Human);
    create_room(&mut engine, AdmissionPolicy::Open);
    let before = engine.room(&RoomId::from("commons")).unwrap();

    assert_eq!(
        engine.apply(CommandV2::Speak {
            room_id: RoomId::from("commons"),
            author: ParticipantId::from("alice"),
            at: LogicalTime(9),
            body: "oversized".into(),
        }),
        Err(Refusal::InvalidMessageBody)
    );
    assert_eq!(engine.room(&RoomId::from("commons")).unwrap(), before);
}

#[test]
fn message_debug_redacts_content() {
    let mut engine = Engine::new();
    register(&mut engine, "alice", ParticipantKind::Human);
    create_room(&mut engine, AdmissionPolicy::Open);
    engine
        .apply(CommandV2::Speak {
            room_id: RoomId::from("commons"),
            author: ParticipantId::from("alice"),
            at: LogicalTime(1),
            body: "secret phrase".into(),
        })
        .unwrap();

    let debug = format!(
        "{:?}",
        engine.room(&RoomId::from("commons")).unwrap().messages[0]
    );
    assert!(!debug.contains("secret phrase"));
    assert!(debug.contains("body_bytes"));
}
