use agora_engine::{
    AdmissionPolicy, CommandV2, DomainEventV2, Engine, Limits, ModerationCausationV1,
    ModerationReasonV1, OutcomeV2, Participant, ParticipantKind, Refusal, Role,
};

fn moderation(cause: u8) -> (ModerationReasonV1, ModerationCausationV1) {
    (
        ModerationReasonV1::new("private moderation reason"),
        ModerationCausationV1::new([cause; 16]),
    )
}

fn configured(
    limits: Limits,
    owner_kind: ParticipantKind,
    moderator_kind: ParticipantKind,
) -> Engine {
    let mut engine = Engine::with_limits(limits);
    for (id, kind) in [
        ("owner", owner_kind),
        ("moderator", moderator_kind),
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
            name: "Room".into(),
            owner: "owner".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    for participant in ["moderator", "member"] {
        engine
            .apply(CommandV2::Join {
                room_id: "room".into(),
                participant: participant.into(),
            })
            .unwrap();
    }
    engine
        .apply(CommandV2::SetRole {
            room_id: "room".into(),
            actor: "owner".into(),
            participant: "moderator".into(),
            role: Role::Moderator,
        })
        .unwrap();
    engine
}

#[test]
fn kick_and_ban_have_distinct_reentry_semantics() {
    let mut engine = configured(
        Limits::default(),
        ParticipantKind::Human,
        ParticipantKind::Agent,
    );
    let (reason, causation) = moderation(1);
    assert!(matches!(
        engine
            .apply(CommandV2::Kick {
                room_id: "room".into(),
                actor: "moderator".into(),
                participant: "member".into(),
                reason,
                causation,
            })
            .unwrap(),
        OutcomeV2::Kicked { .. }
    ));
    assert!(
        engine
            .apply(CommandV2::Join {
                room_id: "room".into(),
                participant: "member".into(),
            })
            .is_ok()
    );

    let (reason, causation) = moderation(2);
    assert!(matches!(
        engine
            .apply(CommandV2::Ban {
                room_id: "room".into(),
                actor: "owner".into(),
                participant: "member".into(),
                reason,
                causation,
            })
            .unwrap(),
        OutcomeV2::Banned { .. }
    ));
    assert!(matches!(
        engine.apply(CommandV2::Join {
            room_id: "room".into(),
            participant: "member".into(),
        }),
        Err(Refusal::ParticipantBanned(_))
    ));
    assert!(matches!(
        engine.apply(CommandV2::Invite {
            room_id: "room".into(),
            actor: "owner".into(),
            participant: "member".into(),
        }),
        Err(Refusal::ParticipantBanned(_))
    ));
    let (reason, causation) = moderation(3);
    assert!(matches!(
        engine
            .apply(CommandV2::Unban {
                room_id: "room".into(),
                actor: "moderator".into(),
                participant: "member".into(),
                reason,
                causation,
            })
            .unwrap(),
        OutcomeV2::Unbanned { .. }
    ));
    assert!(
        engine
            .apply(CommandV2::Join {
                room_id: "room".into(),
                participant: "member".into(),
            })
            .is_ok()
    );
}

#[test]
fn authority_is_role_scoped_kind_neutral_and_has_no_admin_bypass() {
    for (owner_kind, moderator_kind) in [
        (ParticipantKind::Human, ParticipantKind::Agent),
        (ParticipantKind::Agent, ParticipantKind::Human),
    ] {
        let mut engine = configured(Limits::default(), owner_kind, moderator_kind);
        let revision = engine.revision();
        let (reason, causation) = moderation(4);
        assert!(matches!(
            engine.apply(CommandV2::Kick {
                room_id: "room".into(),
                actor: "moderator".into(),
                participant: "owner".into(),
                reason,
                causation,
            }),
            Err(Refusal::NotAuthorized(_))
        ));
        assert_eq!(engine.revision(), revision);

        let (reason, causation) = moderation(5);
        assert!(matches!(
            engine.apply(CommandV2::Ban {
                room_id: "room".into(),
                actor: "other".into(),
                participant: "member".into(),
                reason,
                causation,
            }),
            Err(Refusal::NotAuthorized(_))
        ));
        assert_eq!(engine.revision(), revision);

        engine
            .apply(CommandV2::Join {
                room_id: "room".into(),
                participant: "other".into(),
            })
            .unwrap();
        engine
            .apply(CommandV2::SetRole {
                room_id: "room".into(),
                actor: "owner".into(),
                participant: "other".into(),
                role: Role::Moderator,
            })
            .unwrap();
        let (reason, causation) = moderation(9);
        engine
            .apply(CommandV2::Ban {
                room_id: "room".into(),
                actor: "owner".into(),
                participant: "moderator".into(),
                reason,
                causation,
            })
            .unwrap();
        let (reason, causation) = moderation(10);
        assert!(matches!(
            engine.apply(CommandV2::Unban {
                room_id: "room".into(),
                actor: "other".into(),
                participant: "moderator".into(),
                reason,
                causation,
            }),
            Err(Refusal::NotAuthorized(_))
        ));
    }
}

#[test]
fn moderation_evidence_is_bounded_atomic_and_content_safe() {
    let mut engine = configured(
        Limits {
            max_bans_per_room: 1,
            max_moderation_reason_bytes: 64,
            ..Limits::default()
        },
        ParticipantKind::Human,
        ParticipantKind::Agent,
    );
    let (reason, causation) = moderation(6);
    engine
        .apply(CommandV2::Ban {
            room_id: "room".into(),
            actor: "owner".into(),
            participant: "member".into(),
            reason,
            causation,
        })
        .unwrap();
    let event = engine.pending_events().last().unwrap();
    assert!(matches!(
        event.event,
        DomainEventV2::ParticipantBanned { .. }
    ));
    let rendered = format!("{event:?}");
    assert!(!rendered.contains("private moderation reason"));
    assert!(!rendered.contains("060606"));

    let revision = engine.revision();
    let events = engine.pending_events().len();
    let (reason, causation) = moderation(7);
    assert_eq!(
        engine.apply(CommandV2::Ban {
            room_id: "room".into(),
            actor: "owner".into(),
            participant: "other".into(),
            reason,
            causation,
        }),
        Err(Refusal::BanLimit)
    );
    assert_eq!(engine.revision(), revision);
    assert_eq!(engine.pending_events().len(), events);

    let (reason, causation) = moderation(8);
    assert_eq!(
        engine.apply(CommandV2::Unban {
            room_id: "room".into(),
            actor: "owner".into(),
            participant: "other".into(),
            reason,
            causation,
        }),
        Err(Refusal::NotBanned("other".into()))
    );
}
