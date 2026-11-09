use agora_engine::{
    AdmissionPolicy, CommandV1, Engine, Limits, Participant, ParticipantId, ParticipantKind,
    Refusal, RoomId,
};

#[test]
fn invalid_registration_and_capacity_refusals_are_atomic() {
    let mut engine = Engine::with_limits(Limits {
        max_participants: 1,
        max_participant_id_bytes: 4,
        max_display_name_bytes: 4,
        ..Limits::default()
    });

    assert_eq!(
        engine.apply(CommandV1::RegisterParticipant(Participant::new(
            "large",
            "safe",
            ParticipantKind::Agent,
        ))),
        Err(Refusal::InvalidParticipantId)
    );
    assert_eq!(engine.participants().len(), 0);

    engine
        .apply(CommandV1::RegisterParticipant(Participant::new(
            "safe",
            "safe",
            ParticipantKind::Human,
        )))
        .unwrap();
    assert_eq!(
        engine.apply(CommandV1::RegisterParticipant(Participant::new(
            "next",
            "next",
            ParticipantKind::Agent,
        ))),
        Err(Refusal::ParticipantLimit)
    );
    assert_eq!(engine.participants().len(), 1);
}

#[test]
fn room_creation_requires_registered_owner_and_member_capacity() {
    let mut engine = Engine::with_limits(Limits {
        max_members_per_room: 0,
        ..Limits::default()
    });
    engine
        .apply(CommandV1::RegisterParticipant(Participant::new(
            "owner",
            "Owner",
            ParticipantKind::Human,
        )))
        .unwrap();

    assert_eq!(
        engine.apply(CommandV1::CreateRoom {
            room_id: RoomId::from("room"),
            name: "Room".into(),
            owner: ParticipantId::from("missing"),
            admission: AdmissionPolicy::Open,
        }),
        Err(Refusal::UnknownParticipant(ParticipantId::from("missing")))
    );
    assert_eq!(engine.rooms().count(), 0);
    assert_eq!(
        engine.apply(CommandV1::CreateRoom {
            room_id: RoomId::from("room"),
            name: "Room".into(),
            owner: ParticipantId::from("owner"),
            admission: AdmissionPolicy::Open,
        }),
        Err(Refusal::MemberLimit)
    );
    assert_eq!(engine.rooms().count(), 0);
}
