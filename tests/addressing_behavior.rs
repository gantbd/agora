use agora_engine::{
    AdmissionPolicy, CommandV2, DomainEventV2, Engine, Limits, LogicalTime, Participant,
    ParticipantId, ParticipantKind, Refusal, RoomId,
};

fn setup_room(limits: Limits) -> Engine {
    let mut engine = Engine::with_limits(limits);
    for (id, kind) in [
        ("alice", ParticipantKind::Human),
        ("bot-a", ParticipantKind::Agent),
        ("bot-b", ParticipantKind::Agent),
        ("outsider", ParticipantKind::Human),
    ] {
        engine
            .apply(CommandV2::RegisterParticipant(Participant::new(
                id, id, kind,
            )))
            .unwrap();
    }
    engine
        .apply(CommandV2::CreateRoom {
            room_id: RoomId::from("room"),
            name: "Room".into(),
            owner: ParticipantId::from("alice"),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    for participant in ["bot-a", "bot-b"] {
        engine
            .apply(CommandV2::Join {
                room_id: RoomId::from("room"),
                participant: ParticipantId::from(participant),
            })
            .unwrap();
    }
    engine
}

fn addressed(author: &str, recipients: &[&str]) -> CommandV2 {
    CommandV2::SpeakAddressed {
        room_id: RoomId::from("room"),
        author: ParticipantId::from(author),
        at: LogicalTime(1),
        body: "please review".into(),
        addressed_to: recipients
            .iter()
            .map(|id| ParticipantId::from(*id))
            .collect(),
    }
}

#[test]
fn humans_and_agents_address_members_under_identical_rules() {
    for author in ["alice", "bot-a"] {
        let mut engine = setup_room(Limits::default());
        engine
            .apply(addressed(author, &["bot-a", "bot-b"]))
            .unwrap();

        let message = engine.message(&RoomId::from("room"), 0).unwrap();
        assert_eq!(
            message.addressed_to(),
            &[ParticipantId::from("bot-a"), ParticipantId::from("bot-b")]
        );
        let DomainEventV2::MessageAccepted { message, .. } =
            &engine.pending_events().last().unwrap().event
        else {
            panic!("expected message event")
        };
        assert_eq!(message.addressed_to().len(), 2);
    }
}

#[test]
fn addressing_must_be_canonical_bounded_and_room_local() {
    let mut engine = setup_room(Limits {
        max_addressed_participants_per_message: 2,
        ..Limits::default()
    });
    let revision = engine.revision();
    let pending = engine.pending_events().len();

    assert_eq!(
        engine.apply(addressed("alice", &["bot-b", "bot-a"])),
        Err(Refusal::NonCanonicalAddressing)
    );
    assert_eq!(engine.revision(), revision);
    assert_eq!(engine.pending_events().len(), pending);

    assert_eq!(
        engine.apply(addressed("alice", &["outsider"])),
        Err(Refusal::NotMember(ParticipantId::from("outsider")))
    );
    assert_eq!(engine.revision(), revision);

    assert_eq!(
        engine.apply(addressed("alice", &["alice", "bot-a", "bot-b"])),
        Err(Refusal::AddressedParticipantLimit)
    );
    assert_eq!(engine.revision(), revision);
}

#[test]
fn plain_messages_address_nobody_and_never_imply_execution() {
    let mut engine = setup_room(Limits::default());
    engine
        .apply(CommandV2::Speak {
            room_id: RoomId::from("room"),
            author: ParticipantId::from("alice"),
            at: LogicalTime(1),
            body: "hello everyone".into(),
        })
        .unwrap();

    let message = engine.message(&RoomId::from("room"), 0).unwrap();
    assert!(message.addressed_to().is_empty());
    let debug = format!("{:?}", engine.pending_events().last().unwrap());
    assert!(debug.contains("addressed_count"));
    assert!(!debug.contains("hello everyone"));
}
