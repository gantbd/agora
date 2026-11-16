use agora_engine::{AdmissionPolicy, Limits, ParticipantId, ParticipantKind, Refusal, RoomId};
use agora_private_local::{
    BootstrapParticipantV1, BootstrapRefusalV1, BootstrapRoomV1, BootstrapStageV1, BootstrapV1,
};

fn participant(id: &str) -> BootstrapParticipantV1 {
    BootstrapParticipantV1 {
        id: ParticipantId::new(id),
        display_name: id.to_uppercase(),
        kind: ParticipantKind::Human,
    }
}

fn bootstrap() -> BootstrapV1 {
    BootstrapV1 {
        principal: ParticipantId::new("alice"),
        participants: vec![participant("alice"), participant("bob")],
        rooms: vec![BootstrapRoomV1 {
            id: RoomId::new("general"),
            name: "General".to_owned(),
            owner: ParticipantId::new("alice"),
            admission: AdmissionPolicy::InviteOnly,
        }],
    }
}

#[test]
fn builds_exact_bounded_initial_state() {
    let engine = bootstrap().build_engine(Limits::default()).unwrap();

    assert_eq!(engine.participants().len(), 2);
    assert_eq!(
        engine
            .participant(&ParticipantId::new("alice"))
            .unwrap()
            .display_name(),
        "ALICE"
    );
    let room = engine.room(&RoomId::new("general")).unwrap();
    assert_eq!(room.name, "General");
    assert_eq!(
        engine.member_role(&RoomId::new("general"), &ParticipantId::new("alice")),
        Some(agora_engine::Role::Owner)
    );
}

#[test]
fn refuses_missing_principal_before_applying_any_state() {
    let mut plan = bootstrap();
    plan.principal = ParticipantId::new("carol");

    assert_eq!(
        plan.build_engine(Limits::default()).unwrap_err(),
        BootstrapRefusalV1::PrincipalNotRegistered
    );
}

#[test]
fn preserves_exact_engine_refusal_location() {
    let mut plan = bootstrap();
    plan.participants.push(participant("alice"));

    assert_eq!(
        plan.build_engine(Limits::default()).unwrap_err(),
        BootstrapRefusalV1::Engine {
            stage: BootstrapStageV1::Participant(2),
            refusal: Refusal::DuplicateParticipant(ParticipantId::new("alice")),
        }
    );
}

#[test]
fn plan_bounds_refuse_before_engine_construction() {
    let limits = Limits {
        max_participants: 1,
        ..Limits::default()
    };
    assert_eq!(
        bootstrap().build_engine(limits).unwrap_err(),
        BootstrapRefusalV1::ParticipantLimit
    );

    let limits = Limits {
        max_rooms: 0,
        ..Limits::default()
    };
    assert_eq!(
        bootstrap().build_engine(limits).unwrap_err(),
        BootstrapRefusalV1::RoomLimit
    );
}
