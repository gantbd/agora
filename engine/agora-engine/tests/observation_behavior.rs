use agora_engine::{
    AdmissionPolicy, CommandV2, Engine, Limits, LogicalTime, ObservationRefusal, Participant,
    ParticipantId, ParticipantKind, Revision, Role, RoomId,
};

fn observed_engine() -> Engine {
    let mut engine = Engine::with_limits(Limits {
        max_rooms: 4,
        max_members_per_room: 4,
        max_messages_per_room: 4,
        ..Limits::default()
    });
    for (id, name, kind) in [
        ("alice", "Alice", ParticipantKind::Human),
        ("bot", "Agent", ParticipantKind::Agent),
        ("carol", "Carol", ParticipantKind::Human),
        ("outsider", "Outsider", ParticipantKind::Agent),
    ] {
        engine
            .apply(CommandV2::RegisterParticipant(Participant::new(
                id, name, kind,
            )))
            .unwrap();
    }
    for (id, name) in [("alpha", "Alpha"), ("beta", "Beta"), ("gamma", "Gamma")] {
        engine
            .apply(CommandV2::CreateRoom {
                room_id: id.into(),
                name: name.to_owned(),
                owner: "alice".into(),
                admission: AdmissionPolicy::Open,
            })
            .unwrap();
    }
    engine
        .apply(CommandV2::Join {
            room_id: "alpha".into(),
            participant: "bot".into(),
        })
        .unwrap();
    engine
        .apply(CommandV2::Join {
            room_id: "alpha".into(),
            participant: "carol".into(),
        })
        .unwrap();
    for body in ["first", "second", "third"] {
        engine
            .apply(CommandV2::Speak {
                room_id: "alpha".into(),
                author: "alice".into(),
                at: LogicalTime(7),
                body: body.to_owned(),
            })
            .unwrap();
    }
    engine
}

#[test]
fn room_directory_is_membership_filtered_bounded_and_revision_bound() {
    let mut engine = observed_engine();
    let alice = ParticipantId::from("alice");
    let first = engine.room_page(&alice, None, 2).unwrap();
    assert_eq!(
        first
            .rooms
            .iter()
            .map(|room| room.id.as_str())
            .collect::<Vec<_>>(),
        ["alpha", "beta"]
    );
    assert_eq!(first.rooms[0].observer_role, Role::Owner);
    assert_eq!(first.rooms[0].member_count, 3);
    assert!(first.next.is_some());

    let second = engine.room_page(&alice, first.next.as_ref(), 2).unwrap();
    assert_eq!(
        second
            .rooms
            .iter()
            .map(|room| room.id.as_str())
            .collect::<Vec<_>>(),
        ["gamma"]
    );
    assert!(second.next.is_none());

    let bot = ParticipantId::from("bot");
    let bot_page = engine.room_page(&bot, None, 4).unwrap();
    assert_eq!(bot_page.rooms.len(), 1);
    assert_eq!(bot_page.rooms[0].id.as_str(), "alpha");
    assert_eq!(
        engine.room_page(&bot, first.next.as_ref(), 2),
        Err(ObservationRefusal::ForeignCursor)
    );

    engine
        .apply(CommandV2::Speak {
            room_id: "alpha".into(),
            author: "alice".into(),
            at: LogicalTime(8),
            body: "changed".to_owned(),
        })
        .unwrap();
    assert_eq!(
        engine.room_page(&alice, first.next.as_ref(), 2),
        Err(ObservationRefusal::Stale {
            actual_revision: engine.revision()
        })
    );
}

#[test]
fn member_pages_preserve_kind_without_kind_authority() {
    let engine = observed_engine();
    let room = RoomId::from("alpha");
    let alice = ParticipantId::from("alice");
    let first = engine.member_page(&alice, &room, None, 2).unwrap();
    assert_eq!(
        first
            .members
            .iter()
            .map(|member| (member.participant_id.as_str(), member.kind, member.role))
            .collect::<Vec<_>>(),
        [
            ("alice", ParticipantKind::Human, Role::Owner),
            ("bot", ParticipantKind::Agent, Role::Member),
        ]
    );
    let second = engine
        .member_page(&alice, &room, first.next.as_ref(), 2)
        .unwrap();
    assert_eq!(second.members[0].participant_id.as_str(), "carol");
    assert!(second.next.is_none());

    assert_eq!(
        engine.member_page(&ParticipantId::from("outsider"), &room, None, 2),
        Err(ObservationRefusal::NotMember)
    );
    assert_eq!(
        engine.member_page(&alice, &room, None, 0),
        Err(ObservationRefusal::InvalidLimit)
    );
}

#[test]
fn message_pages_cover_the_retained_suffix_without_duplication() {
    let engine = observed_engine();
    let room = RoomId::from("alpha");
    let observer = ParticipantId::from("bot");
    let first = engine.message_page(&observer, &room, None, 2).unwrap();
    assert_eq!(
        first
            .messages
            .iter()
            .map(|message| message.sequence())
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(first.next_message_sequence, 3);
    assert_eq!(first.oldest_retained_sequence, 0);
    let second = engine
        .message_page(&observer, &room, first.next.as_ref(), 2)
        .unwrap();
    assert_eq!(
        second
            .messages
            .iter()
            .map(|message| message.sequence())
            .collect::<Vec<_>>(),
        [2]
    );
    assert!(second.next.is_none());

    let all = first
        .messages
        .iter()
        .chain(&second.messages)
        .map(|message| message.sequence())
        .collect::<Vec<_>>();
    assert_eq!(all, [0, 1, 2]);
}

#[test]
fn message_page_exposes_the_retained_history_gap() {
    let mut engine = Engine::with_limits(Limits {
        max_messages_per_room: 2,
        ..Limits::default()
    });
    engine
        .apply(CommandV2::RegisterParticipant(Participant::new(
            "alice",
            "Alice",
            ParticipantKind::Human,
        )))
        .unwrap();
    engine
        .apply(CommandV2::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
            owner: "alice".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    for body in ["zero", "one", "two"] {
        engine
            .apply(CommandV2::Speak {
                room_id: "room".into(),
                author: "alice".into(),
                at: LogicalTime(0),
                body: body.to_owned(),
            })
            .unwrap();
    }
    let page = engine
        .message_page(
            &ParticipantId::from("alice"),
            &RoomId::from("room"),
            None,
            2,
        )
        .unwrap();
    assert_eq!(page.oldest_retained_sequence, 1);
    assert_eq!(page.next_message_sequence, 3);
    assert_eq!(
        page.messages
            .iter()
            .map(|message| message.sequence())
            .collect::<Vec<_>>(),
        [1, 2]
    );
}

#[test]
fn observation_debug_output_is_content_safe() {
    let engine = observed_engine();
    let alice = ParticipantId::from("alice");
    let room = RoomId::from("alpha");
    let room_page = engine.room_page(&alice, None, 1).unwrap();
    let member_page = engine.member_page(&alice, &room, None, 1).unwrap();
    let message_page = engine.message_page(&alice, &room, None, 1).unwrap();
    let rendered = format!("{room_page:?} {member_page:?} {message_page:?}");
    for secret in ["alpha", "Alpha", "alice", "Alice", "first"] {
        assert!(!rendered.contains(secret));
    }
}

#[test]
fn unknown_and_invalid_observations_refuse_without_state_change() {
    let engine = observed_engine();
    let revision = engine.revision();
    assert_eq!(
        engine.room_page(&ParticipantId::from("missing"), None, 1),
        Err(ObservationRefusal::UnknownParticipant)
    );
    assert_eq!(
        engine.member_page(
            &ParticipantId::from("alice"),
            &RoomId::from("missing"),
            None,
            1,
        ),
        Err(ObservationRefusal::UnknownRoom)
    );
    assert_eq!(engine.revision(), revision);
    assert!(revision > Revision(0));
}
