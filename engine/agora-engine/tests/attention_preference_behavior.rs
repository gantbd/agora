use agora_engine::persistence::RecoveryLimits;
use agora_engine::{
    AdmissionPolicy, AttentionPreferenceRefusalV1, AttentionPreferenceV1, AttentionScopeV1,
    CommandV2, Engine, Limits, LogicalTime, NotificationIntentV1, OutcomeV2, Participant,
    ParticipantId, ParticipantKind, RoomId,
};

fn setup(max_messages: usize) -> Engine {
    let mut engine = Engine::with_limits(Limits {
        max_messages_per_room: max_messages,
        ..Limits::default()
    });
    for (id, kind) in [
        ("owner", ParticipantKind::Human),
        ("agent", ParticipantKind::Agent),
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
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(CommandV2::Join {
            room_id: "room".into(),
            participant: "agent".into(),
        })
        .unwrap();
    engine
}

fn root(engine: &mut Engine, author: &str) -> agora_engine::MessageId {
    let OutcomeV2::MessageAccepted { sequence, .. } = engine
        .apply(CommandV2::Speak {
            room_id: "room".into(),
            author: author.into(),
            at: LogicalTime(engine.revision().0),
            body: "root".to_owned(),
        })
        .unwrap()
    else {
        panic!("message expected");
    };
    engine.message(&RoomId::new("room"), sequence).unwrap().id()
}

#[test]
fn preferences_are_private_idempotent_and_domain_neutral() {
    let mut engine = setup(8);
    let revision = engine.revision();
    let event_count = engine.pending_events().count();
    let value = AttentionPreferenceV1::new(true, true, NotificationIntentV1::AllActivity);

    for actor in ["owner", "agent"] {
        assert_eq!(
            engine
                .set_attention_preference(
                    &ParticipantId::new(actor),
                    &RoomId::new("room"),
                    AttentionScopeV1::Room,
                    revision,
                    value,
                )
                .unwrap(),
            value
        );
        assert_eq!(engine.revision(), revision);
        assert_eq!(engine.pending_events().count(), event_count);
        assert_eq!(
            engine
                .attention_preference(
                    &ParticipantId::new(actor),
                    &RoomId::new("room"),
                    AttentionScopeV1::Room,
                )
                .unwrap(),
            value
        );
    }

    assert_eq!(
        engine
            .set_attention_preference(
                &ParticipantId::new("agent"),
                &RoomId::new("room"),
                AttentionScopeV1::Room,
                revision,
                AttentionPreferenceV1::default(),
            )
            .unwrap(),
        AttentionPreferenceV1::default()
    );
    assert_eq!(
        engine
            .attention_preference(
                &ParticipantId::new("agent"),
                &RoomId::new("room"),
                AttentionScopeV1::Room,
            )
            .unwrap(),
        AttentionPreferenceV1::default()
    );
}

#[test]
fn scope_staleness_and_membership_refuse_without_mutation() {
    let mut engine = setup(8);
    let root_id = root(&mut engine, "owner");
    let OutcomeV2::TopicCreated { topic_id, .. } = engine
        .apply(CommandV2::CreateTopic {
            room_id: "room".into(),
            actor: "owner".into(),
            name: "ops".to_owned(),
        })
        .unwrap()
    else {
        panic!("topic expected");
    };
    let revision = engine.revision();
    let value = AttentionPreferenceV1::new(false, true, NotificationIntentV1::AddressedOnly);

    for scope in [
        AttentionScopeV1::Thread(root_id),
        AttentionScopeV1::Topic(topic_id),
    ] {
        engine
            .set_attention_preference(
                &ParticipantId::new("agent"),
                &RoomId::new("room"),
                scope,
                revision,
                value,
            )
            .unwrap();
        assert_eq!(
            engine
                .attention_preference(&ParticipantId::new("agent"), &RoomId::new("room"), scope,)
                .unwrap(),
            value
        );
    }
    assert!(matches!(
        engine.set_attention_preference(
            &ParticipantId::new("agent"),
            &RoomId::new("room"),
            AttentionScopeV1::Room,
            agora_engine::Revision(revision.0 - 1),
            value,
        ),
        Err(AttentionPreferenceRefusalV1::Stale { .. })
    ));

    engine
        .apply(CommandV2::Leave {
            room_id: "room".into(),
            participant: "agent".into(),
        })
        .unwrap();
    assert_eq!(
        engine.attention_preference(
            &ParticipantId::new("agent"),
            &RoomId::new("room"),
            AttentionScopeV1::Room,
        ),
        Err(AttentionPreferenceRefusalV1::NotMember)
    );
}

#[test]
fn preference_checkpoint_is_canonical_and_survives_history_eviction() {
    let mut engine = setup(1);
    let root_id = root(&mut engine, "owner");
    let revision = engine.revision();
    let value = AttentionPreferenceV1::new(true, false, NotificationIntentV1::None);
    engine
        .set_attention_preference(
            &ParticipantId::new("agent"),
            &RoomId::new("room"),
            AttentionScopeV1::Thread(root_id),
            revision,
            value,
        )
        .unwrap();
    root(&mut engine, "owner");
    assert!(
        engine
            .message_by_id(&RoomId::new("room"), root_id)
            .is_none()
    );

    let checkpoint = engine.prepare_checkpoint(65_536).unwrap();
    assert_eq!(checkpoint.bytes()[8], 24);
    let recovered = Engine::recover_checkpoint(
        checkpoint.bytes(),
        RecoveryLimits {
            max_snapshot_bytes: 65_536,
            maximum_engine_limits: Limits::default(),
        },
    )
    .unwrap();
    assert_eq!(
        recovered
            .attention_preference(
                &ParticipantId::new("agent"),
                &RoomId::new("room"),
                AttentionScopeV1::Thread(root_id),
            )
            .unwrap(),
        value
    );
    assert_eq!(
        recovered.prepare_checkpoint(65_536).unwrap().bytes(),
        checkpoint.bytes()
    );
}
