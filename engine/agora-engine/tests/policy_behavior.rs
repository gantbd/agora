use agora_engine::{
    AdmissionPolicy, CommandV2, DomainEventV2, Engine, OutcomeV2, Participant, ParticipantKind,
    Refusal,
};

fn engine() -> Engine {
    let mut engine = Engine::new();
    for (id, kind) in [
        ("owner", ParticipantKind::Agent),
        ("member", ParticipantKind::Human),
        ("other", ParticipantKind::Agent),
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
            admission: AdmissionPolicy::InviteOnly,
        })
        .unwrap();
    engine
}

#[test]
fn policy_changes_are_owner_scoped_observable_and_do_not_rewrite_prior_facts() {
    let mut engine = engine();
    engine
        .apply(CommandV2::Invite {
            room_id: "room".into(),
            actor: "owner".into(),
            participant: "member".into(),
        })
        .unwrap();
    let revision = engine.revision();
    assert_eq!(
        engine.apply(CommandV2::SetAdmissionPolicy {
            room_id: "room".into(),
            actor: "member".into(),
            admission: AdmissionPolicy::Open,
        }),
        Err(Refusal::NotAuthorized("member".into()))
    );
    assert_eq!(engine.revision(), revision);

    assert!(matches!(
        engine
            .apply(CommandV2::SetAdmissionPolicy {
                room_id: "room".into(),
                actor: "owner".into(),
                admission: AdmissionPolicy::Open,
            })
            .unwrap(),
        OutcomeV2::RoomPolicyChanged {
            previous: AdmissionPolicy::InviteOnly,
            admission: AdmissionPolicy::Open,
            ..
        }
    ));
    assert!(matches!(
        engine.pending_events().last().unwrap().event,
        DomainEventV2::RoomPolicyChanged {
            previous: AdmissionPolicy::InviteOnly,
            admission: AdmissionPolicy::Open,
            ..
        }
    ));
    assert!(
        engine
            .room(&"room".into())
            .unwrap()
            .pending_invitations
            .contains(&"member".into())
    );
    engine
        .apply(CommandV2::Join {
            room_id: "room".into(),
            participant: "other".into(),
        })
        .unwrap();

    let revision = engine.revision();
    assert_eq!(
        engine.apply(CommandV2::SetAdmissionPolicy {
            room_id: "room".into(),
            actor: "owner".into(),
            admission: AdmissionPolicy::Open,
        }),
        Err(Refusal::InvalidPolicyChange)
    );
    assert_eq!(engine.revision(), revision);
}
