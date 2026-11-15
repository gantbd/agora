use agora_engine::persistence::{CheckpointError, RecoveryLimits};
use agora_engine::{
    AdmissionPolicy, CommandV2, ConversationProfileV1, DomainEventV2, Engine, Limits,
    ModerationCausationV1, ModerationReasonV1, ObservationRefusal, OutcomeV2, Participant,
    ParticipantId, ParticipantKind, Refusal, Role, RoomId,
};

fn register(engine: &mut Engine, id: &str, kind: ParticipantKind) {
    engine
        .apply(CommandV2::RegisterParticipant(Participant::new(
            id, id, kind,
        )))
        .unwrap();
}

fn create(
    engine: &mut Engine,
    room_id: &str,
    initiator: &str,
    profile: ConversationProfileV1,
    members: &[&str],
) -> Result<OutcomeV2, Refusal> {
    engine.apply(CommandV2::CreatePeerConversation {
        room_id: RoomId::from(room_id),
        name: room_id.to_owned(),
        initiator: ParticipantId::from(initiator),
        profile,
        founding_members: members.iter().copied().map(ParticipantId::from).collect(),
    })
}

#[test]
fn direct_and_group_direct_are_atomic_ownerless_kind_neutral_cohorts() {
    let mut engine = Engine::new();
    register(&mut engine, "agent", ParticipantKind::Agent);
    register(&mut engine, "human", ParticipantKind::Human);
    register(&mut engine, "third", ParticipantKind::Human);

    assert_eq!(
        create(
            &mut engine,
            "direct",
            "human",
            ConversationProfileV1::Direct,
            &["agent", "human"],
        )
        .unwrap(),
        OutcomeV2::PeerConversationCreated {
            room_id: "direct".into(),
            profile: ConversationProfileV1::Direct,
        }
    );
    let direct = engine.room(&"direct".into()).unwrap();
    assert_eq!(direct.profile, ConversationProfileV1::Direct);
    assert_eq!(direct.admission, AdmissionPolicy::InviteOnly);
    assert_eq!(
        direct.founding_members,
        vec![ParticipantId::from("agent"), ParticipantId::from("human")]
    );
    assert!(direct.members.iter().all(|(_, role)| *role == Role::Member));

    let before = engine.revision();
    assert_eq!(
        create(
            &mut engine,
            "bad",
            "agent",
            ConversationProfileV1::Direct,
            &["human", "third"],
        ),
        Err(Refusal::InvalidConversationCohort)
    );
    assert_eq!(engine.revision(), before);
    assert!(engine.room(&"bad".into()).is_none());

    assert!(matches!(
        &engine.pending_events().last().unwrap().event,
        DomainEventV2::PeerConversationCreated {
            initiator,
            profile: ConversationProfileV1::Direct,
            founding_members,
            ..
        } if initiator.as_str() == "human"
            && founding_members == &vec!["agent".into(), "human".into()]
    ));

    assert!(matches!(
        create(
            &mut engine,
            "group",
            "agent",
            ConversationProfileV1::GroupDirect,
            &["agent", "human", "third"],
        ),
        Ok(OutcomeV2::PeerConversationCreated {
            profile: ConversationProfileV1::GroupDirect,
            ..
        })
    ));
}

#[test]
fn peer_profile_cardinality_identity_and_capacity_refuse_atomically() {
    let limits = Limits {
        max_members_per_room: 3,
        ..Limits::default()
    };
    let mut engine = Engine::with_limits(limits);
    for id in ["a", "b", "c", "d"] {
        register(&mut engine, id, ParticipantKind::Human);
    }

    let cases = [
        (
            ConversationProfileV1::Ordinary,
            vec!["a"],
            Refusal::InvalidConversationProfile,
        ),
        (
            ConversationProfileV1::Direct,
            vec!["a"],
            Refusal::InvalidConversationCohort,
        ),
        (
            ConversationProfileV1::Direct,
            vec!["a", "b", "c"],
            Refusal::InvalidConversationCohort,
        ),
        (
            ConversationProfileV1::GroupDirect,
            vec!["a", "b"],
            Refusal::InvalidConversationCohort,
        ),
        (
            ConversationProfileV1::GroupDirect,
            vec!["a", "b", "c", "d"],
            Refusal::InvalidConversationCohort,
        ),
        (
            ConversationProfileV1::Direct,
            vec!["a", "a"],
            Refusal::InvalidConversationCohort,
        ),
        (
            ConversationProfileV1::Direct,
            vec!["b", "a"],
            Refusal::InvalidConversationCohort,
        ),
        (
            ConversationProfileV1::Direct,
            vec!["a", "missing"],
            Refusal::UnknownParticipant("missing".into()),
        ),
    ];
    for (index, (profile, members, expected)) in cases.into_iter().enumerate() {
        let revision = engine.revision();
        let room_id = format!("invalid-{index}");
        assert_eq!(
            create(&mut engine, &room_id, "a", profile, &members),
            Err(expected)
        );
        assert_eq!(engine.revision(), revision);
        assert!(engine.room(&RoomId::from(room_id.as_str())).is_none());
    }
}

#[test]
fn peer_cohort_is_immutable_while_departure_is_irreversible_and_private() {
    let mut engine = Engine::new();
    register(&mut engine, "alice", ParticipantKind::Human);
    register(&mut engine, "bot", ParticipantKind::Agent);
    create(
        &mut engine,
        "direct",
        "alice",
        ConversationProfileV1::Direct,
        &["alice", "bot"],
    )
    .unwrap();

    for command in [
        CommandV2::Invite {
            room_id: "direct".into(),
            actor: "alice".into(),
            participant: "bot".into(),
        },
        CommandV2::Join {
            room_id: "direct".into(),
            participant: "bot".into(),
        },
        CommandV2::SetRole {
            room_id: "direct".into(),
            actor: "alice".into(),
            participant: "bot".into(),
            role: Role::Moderator,
        },
        CommandV2::SetAdmissionPolicy {
            room_id: "direct".into(),
            actor: "alice".into(),
            admission: AdmissionPolicy::Open,
        },
        CommandV2::Kick {
            room_id: "direct".into(),
            actor: "alice".into(),
            participant: "bot".into(),
            reason: ModerationReasonV1::new("reason"),
            causation: ModerationCausationV1::new([1; 16]),
        },
        CommandV2::Ban {
            room_id: "direct".into(),
            actor: "alice".into(),
            participant: "bot".into(),
            reason: ModerationReasonV1::new("reason"),
            causation: ModerationCausationV1::new([2; 16]),
        },
        CommandV2::Unban {
            room_id: "direct".into(),
            actor: "alice".into(),
            participant: "bot".into(),
            reason: ModerationReasonV1::new("reason"),
            causation: ModerationCausationV1::new([3; 16]),
        },
    ] {
        let before = engine.revision();
        assert_eq!(
            engine.apply(command),
            Err(Refusal::ImmutableConversationCohort)
        );
        assert_eq!(engine.revision(), before);
    }

    engine
        .apply(CommandV2::Leave {
            room_id: "direct".into(),
            participant: "alice".into(),
        })
        .unwrap();
    assert_eq!(
        engine.message_page(&"alice".into(), &"direct".into(), None, 1),
        Err(ObservationRefusal::NotMember)
    );
    assert_eq!(
        engine.apply(CommandV2::Join {
            room_id: "direct".into(),
            participant: "alice".into(),
        }),
        Err(Refusal::ImmutableConversationCohort)
    );
    engine
        .apply(CommandV2::Leave {
            room_id: "direct".into(),
            participant: "bot".into(),
        })
        .unwrap();
    assert!(engine.room(&"direct".into()).unwrap().members.is_empty());
}

#[test]
fn peer_profile_and_original_cohort_recover_canonically() {
    let mut engine = Engine::new();
    register(&mut engine, "alice", ParticipantKind::Human);
    register(&mut engine, "bot", ParticipantKind::Agent);
    create(
        &mut engine,
        "direct",
        "bot",
        ConversationProfileV1::Direct,
        &["alice", "bot"],
    )
    .unwrap();
    engine
        .apply(CommandV2::Leave {
            room_id: "direct".into(),
            participant: "alice".into(),
        })
        .unwrap();

    let checkpoint = engine.prepare_checkpoint(65_536).unwrap();
    assert_eq!(checkpoint.bytes()[8], 22);
    let mut recovered = Engine::recover_checkpoint(
        checkpoint.bytes(),
        RecoveryLimits {
            max_snapshot_bytes: 65_536,
            maximum_engine_limits: Limits::default(),
        },
    )
    .unwrap();
    assert_eq!(
        recovered.room(&"direct".into()),
        engine.room(&"direct".into())
    );
    assert_eq!(
        recovered.apply(CommandV2::Join {
            room_id: "direct".into(),
            participant: "alice".into(),
        }),
        Err(Refusal::ImmutableConversationCohort)
    );
    assert_eq!(
        recovered.prepare_checkpoint(65_536).unwrap().bytes(),
        checkpoint.bytes()
    );

    let mut malformed = checkpoint.bytes().to_vec();
    malformed[8] = 151;
    assert!(matches!(
        Engine::recover_checkpoint(
            &malformed,
            RecoveryLimits {
                max_snapshot_bytes: 65_536,
                maximum_engine_limits: Limits::default(),
            }
        ),
        Err(CheckpointError::Profile)
    ));
}

#[test]
fn bounded_peer_schedule_model_is_kind_initiator_and_departure_order_symmetric() {
    const IDS: [&str; 3] = ["a", "b", "c"];
    const LEAVE_ORDERS: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];

    for kind_mask in 0_u8..8 {
        for initiator in IDS {
            for leave_order in LEAVE_ORDERS {
                let mut engine = Engine::new();
                for (index, id) in IDS.iter().enumerate() {
                    register(
                        &mut engine,
                        id,
                        if kind_mask & (1 << index) == 0 {
                            ParticipantKind::Human
                        } else {
                            ParticipantKind::Agent
                        },
                    );
                }
                create(
                    &mut engine,
                    "group",
                    initiator,
                    ConversationProfileV1::GroupDirect,
                    &IDS,
                )
                .unwrap();

                for (step, departing_index) in leave_order.iter().copied().enumerate() {
                    let departing = ParticipantId::from(IDS[departing_index]);
                    engine
                        .apply(CommandV2::Leave {
                            room_id: "group".into(),
                            participant: departing.clone(),
                        })
                        .unwrap();
                    assert_eq!(
                        engine.conversation_profile(&departing, &"group".into()),
                        Err(ObservationRefusal::NotMember)
                    );
                    for remaining_index in leave_order.iter().skip(step + 1) {
                        let status = engine
                            .conversation_profile(
                                &ParticipantId::from(IDS[*remaining_index]),
                                &"group".into(),
                            )
                            .unwrap();
                        assert_eq!(status.profile, ConversationProfileV1::GroupDirect);
                        assert_eq!(
                            status.founding_members,
                            IDS.iter()
                                .copied()
                                .map(ParticipantId::from)
                                .collect::<Vec<_>>()
                        );
                    }
                }

                let checkpoint = engine.prepare_checkpoint(65_536).unwrap();
                let recovered = Engine::recover_checkpoint(
                    checkpoint.bytes(),
                    RecoveryLimits {
                        max_snapshot_bytes: 65_536,
                        maximum_engine_limits: Limits::default(),
                    },
                )
                .unwrap();
                assert!(recovered.room(&"group".into()).unwrap().members.is_empty());
                assert_eq!(
                    recovered.prepare_checkpoint(65_536).unwrap().bytes(),
                    checkpoint.bytes()
                );
            }
        }
    }
}
