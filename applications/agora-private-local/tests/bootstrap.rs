use aegis_local_client::LimitsV1 as AegisWireLimitsV1;
use aegis_local_client::UnixPort;
use aegis_protocol::{GrantHandleV1, SessionHandleV1};
use agora_engine::{AdmissionPolicy, Limits, ParticipantId, ParticipantKind, Refusal, RoomId};
use agora_private_local::{
    AuthorityConnectErrorV1, AuthoritySelectionV1, BootstrapParticipantV1, BootstrapRefusalV1,
    BootstrapRoomV1, BootstrapStageV1, BootstrapV1, SessionGrantSelectorV1, connect_authority_v1,
};
use std::time::Duration;

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

#[test]
fn authority_participant_mismatch_refuses_before_socket_io() {
    struct Selector;
    impl SessionGrantSelectorV1 for Selector {
        type Error = ();

        fn select(
            &mut self,
            _session: SessionHandleV1,
            _port: &mut UnixPort,
        ) -> Result<GrantHandleV1, Self::Error> {
            panic!("selector must not run before principal validation")
        }
    }

    let selection = AuthoritySelectionV1::new(ParticipantId::new("bob"), [7; 16], [11; 16]);
    let mut selector = Selector;
    let result = connect_authority_v1(
        "/path/that/must/not/be-opened",
        AegisWireLimitsV1::default(),
        Duration::from_secs(1),
        &ParticipantId::new("alice"),
        selection,
        &mut selector,
    );

    assert!(matches!(
        result,
        Err(AuthorityConnectErrorV1::PrincipalMismatch)
    ));
}

#[test]
fn authority_selection_and_connection_diagnostics_are_redacted() {
    let selection = AuthoritySelectionV1::new(
        ParticipantId::new("private-participant"),
        [19; 16],
        [29; 16],
    );
    let rendered = format!("{selection:?}");

    assert!(!rendered.contains("private-participant"));
    assert!(!rendered.contains("29"));
}
