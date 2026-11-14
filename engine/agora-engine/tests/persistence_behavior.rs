use agora_engine::persistence::{CheckpointError, RecoveryLimits};
use agora_engine::*;
fn configured() -> Engine {
    let mut e = Engine::with_limits(Limits {
        max_messages_per_room: 1,
        ..Limits::default()
    });
    for (id, kind) in [
        ("human", ParticipantKind::Human),
        ("agent", ParticipantKind::Agent),
    ] {
        e.apply(CommandV2::RegisterParticipant(Participant::new(
            id, id, kind,
        )))
        .unwrap();
    }
    e.apply(CommandV2::CreateRoom {
        room_id: "room".into(),
        name: "Room".into(),
        owner: "human".into(),
        admission: AdmissionPolicy::Open,
    })
    .unwrap();
    e.apply(CommandV2::Join {
        room_id: "room".into(),
        participant: "agent".into(),
    })
    .unwrap();
    e
}
fn recover(bytes: &[u8]) -> Engine {
    Engine::recover_checkpoint(
        bytes,
        RecoveryLimits {
            max_snapshot_bytes: 65536,
            maximum_engine_limits: Limits::default(),
        },
    )
    .unwrap()
}
#[test]
fn native_checkpoint_retains_evicted_publication_receipts_and_exact_outbox() {
    let mut e = configured();
    let publication = PublicationCommandV2::new(
        e.revision(),
        PublicationKeyV2::new([1; 16]),
        "room".into(),
        "agent".into(),
        LogicalTime(10),
        "reply\0\r\n🇩🇪",
    );
    let first = e.publish(publication.clone()).unwrap();
    e.apply(CommandV2::Speak {
        room_id: "room".into(),
        author: "human".into(),
        at: LogicalTime(11),
        body: "newest".into(),
    })
    .unwrap();
    e.acknowledge_events(EventSequence(1)).unwrap();
    let cp = e.prepare_checkpoint(65536).unwrap();
    let mut r = recover(cp.bytes());
    assert_eq!(r.revision(), e.revision());
    assert_eq!(r.acknowledged_through(), e.acknowledged_through());
    assert_eq!(r.room(&"room".into()), e.room(&"room".into()));
    assert_eq!(
        r.pending_events().collect::<Vec<_>>(),
        e.pending_events().collect::<Vec<_>>()
    );
    assert_eq!(r.publish(publication).unwrap(), first);
    assert_eq!(r.revision(), e.revision());
    assert_eq!(r.prepare_checkpoint(65536).unwrap().bytes(), cp.bytes());
    assert!(!format!("{cp:?}").contains("reply"));
}
#[test]
fn snapshot_framing_and_host_limits_are_checked_before_reconstruction() {
    let e = configured();
    let cp = e.prepare_checkpoint(65536).unwrap();
    let mut v1 = cp.bytes().to_vec();
    v1[8] = 1;
    assert!(matches!(
        Engine::recover_checkpoint(
            &v1,
            RecoveryLimits {
                max_snapshot_bytes: 65536,
                maximum_engine_limits: Limits::default(),
            }
        ),
        Err(CheckpointError::Profile)
    ));
    for end in 0..cp.bytes().len() {
        assert!(
            Engine::recover_checkpoint(
                &cp.bytes()[..end],
                RecoveryLimits {
                    max_snapshot_bytes: 65536,
                    maximum_engine_limits: Limits::default()
                }
            )
            .is_err()
        )
    }
    let mut bytes = cp.bytes().to_vec();
    bytes.push(0);
    assert!(
        Engine::recover_checkpoint(
            &bytes,
            RecoveryLimits {
                max_snapshot_bytes: 65536,
                maximum_engine_limits: Limits::default()
            }
        )
        .is_err()
    );
    assert!(matches!(
        e.prepare_checkpoint(1),
        Err(CheckpointError::Limit)
    ));
    assert!(
        Engine::recover_checkpoint(
            cp.bytes(),
            RecoveryLimits {
                max_snapshot_bytes: 65536,
                maximum_engine_limits: Limits {
                    max_participants: 1,
                    ..Limits::default()
                }
            }
        )
        .is_err()
    );
}

#[test]
fn every_event_kind_and_addressing_survives_checkpoint_and_prefix_acknowledgement() {
    let mut e = configured();
    let commands = vec![
        CommandV2::RegisterParticipant(Participant::new("other", "Other", ParticipantKind::Human)),
        CommandV2::Invite {
            room_id: "room".into(),
            actor: "human".into(),
            participant: "other".into(),
        },
        CommandV2::Join {
            room_id: "room".into(),
            participant: "other".into(),
        },
        CommandV2::SetRole {
            room_id: "room".into(),
            actor: "human".into(),
            participant: "other".into(),
            role: Role::Moderator,
        },
        CommandV2::SetRole {
            room_id: "room".into(),
            actor: "human".into(),
            participant: "other".into(),
            role: Role::Member,
        },
        CommandV2::SpeakAddressed {
            room_id: "room".into(),
            author: "human".into(),
            at: LogicalTime(1),
            body: "question".into(),
            addressed_to: vec!["other".into()],
        },
        CommandV2::Speak {
            room_id: "room".into(),
            author: "human".into(),
            at: LogicalTime(2),
            body: "second".into(),
        },
        CommandV2::Speak {
            room_id: "room".into(),
            author: "agent".into(),
            at: LogicalTime(3),
            body: "answer".into(),
        },
        CommandV2::Leave {
            room_id: "room".into(),
            participant: "other".into(),
        },
        CommandV2::Invite {
            room_id: "room".into(),
            actor: "human".into(),
            participant: "other".into(),
        },
        CommandV2::CreateRoom {
            room_id: "closed".into(),
            name: "Closed".into(),
            owner: "agent".into(),
            admission: AdmissionPolicy::InviteOnly,
        },
    ];
    for command in commands {
        e.apply(command).unwrap();
        let cp = e.prepare_checkpoint(65536).unwrap();
        let r = recover(cp.bytes());
        assert_eq!(r.prepare_checkpoint(65536).unwrap().bytes(), cp.bytes());
        assert_eq!(
            r.pending_events().collect::<Vec<_>>(),
            e.pending_events().collect::<Vec<_>>()
        );
    }
    assert!(e.message(&"room".into(), 0).is_none());
    assert_eq!(e.message(&"room".into(), 2).unwrap().body(), "answer");
    for sequence in 0..e.revision().0 {
        e.acknowledge_events(EventSequence(sequence)).unwrap();
        let cp = e.prepare_checkpoint(65536).unwrap();
        assert_eq!(
            recover(cp.bytes())
                .prepare_checkpoint(65536)
                .unwrap()
                .bytes(),
            cp.bytes()
        );
    }
}

#[test]
fn every_accepted_single_bit_mutation_has_one_canonical_encoding() {
    let e = configured();
    let cp = e.prepare_checkpoint(65536).unwrap();
    for i in 0..cp.bytes().len() {
        for bit in 0..8 {
            let mut changed = cp.bytes().to_vec();
            changed[i] ^= 1 << bit;
            if let Ok(r) = Engine::recover_checkpoint(
                &changed,
                RecoveryLimits {
                    max_snapshot_bytes: 65536,
                    maximum_engine_limits: Limits::default(),
                },
            ) {
                assert_eq!(r.prepare_checkpoint(65536).unwrap().bytes(), changed);
            }
        }
    }
}
