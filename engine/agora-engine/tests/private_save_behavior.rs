use agora_engine::{
    AdmissionPolicy, CommandV2, Engine, Limits, LogicalTime, OutcomeV2, Participant, ParticipantId,
    ParticipantKind, PrivateSaveRefusalV1, RoomId, StableReferenceAvailabilityV1,
};

fn setup(kind: ParticipantKind, max_messages: usize) -> Engine {
    let mut engine = Engine::with_limits(Limits {
        max_messages_per_room: max_messages,
        ..Limits::default()
    });
    engine
        .apply(CommandV2::RegisterParticipant(Participant::new(
            "owner", "Owner", kind,
        )))
        .unwrap();
    engine
        .apply(CommandV2::RegisterParticipant(Participant::new(
            "other",
            "Other",
            ParticipantKind::Agent,
        )))
        .unwrap();
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
            participant: "other".into(),
        })
        .unwrap();
    engine
}

fn speak(engine: &mut Engine, at: u64) -> agora_engine::MessageId {
    let OutcomeV2::MessageAccepted { sequence, .. } = engine
        .apply(CommandV2::Speak {
            room_id: "room".into(),
            author: "owner".into(),
            at: LogicalTime(at),
            body: "body".to_owned(),
        })
        .unwrap()
    else {
        panic!("message accepted expected");
    };
    engine.message(&RoomId::new("room"), sequence).unwrap().id()
}

#[test]
fn saves_are_private_kind_neutral_and_domain_neutral() {
    for kind in [ParticipantKind::Human, ParticipantKind::Agent] {
        let mut engine = setup(kind, 4);
        let message_id = speak(&mut engine, 1);
        let revision = engine.revision();
        let pending = engine.pending_events().len();
        let saved = engine
            .set_saved_reference(
                &ParticipantId::new("owner"),
                &RoomId::new("room"),
                message_id,
                revision,
                true,
            )
            .unwrap();
        assert_eq!(saved.message_id(), message_id);
        assert_eq!(
            saved.availability(),
            StableReferenceAvailabilityV1::Available
        );
        assert_eq!(engine.revision(), revision);
        assert_eq!(engine.pending_events().len(), pending);
        assert_eq!(
            engine
                .saved_reference(
                    &ParticipantId::new("other"),
                    &RoomId::new("room"),
                    message_id,
                )
                .unwrap(),
            None
        );
        assert_eq!(
            engine.set_saved_reference(
                &ParticipantId::new("owner"),
                &RoomId::new("room"),
                message_id,
                revision,
                true,
            ),
            Err(PrivateSaveRefusalV1::InvalidChange)
        );
    }
}

#[test]
fn tombstone_eviction_and_membership_loss_preserve_only_the_stable_relation() {
    let mut engine = setup(ParticipantKind::Human, 1);
    let message_id = speak(&mut engine, 1);
    let revision = engine.revision();
    engine
        .set_saved_reference(
            &ParticipantId::new("other"),
            &RoomId::new("room"),
            message_id,
            revision,
            true,
        )
        .unwrap();
    engine
        .apply(CommandV2::WithdrawMessage {
            room_id: "room".into(),
            actor: "owner".into(),
            message_id,
        })
        .unwrap();
    assert_eq!(
        engine
            .saved_reference(
                &ParticipantId::new("other"),
                &RoomId::new("room"),
                message_id,
            )
            .unwrap()
            .unwrap()
            .availability(),
        StableReferenceAvailabilityV1::Tombstoned
    );
    speak(&mut engine, 2);
    assert_eq!(
        engine
            .saved_reference(
                &ParticipantId::new("other"),
                &RoomId::new("room"),
                message_id,
            )
            .unwrap()
            .unwrap()
            .availability(),
        StableReferenceAvailabilityV1::Evicted
    );
    engine
        .apply(CommandV2::Leave {
            room_id: "room".into(),
            participant: "other".into(),
        })
        .unwrap();
    assert_eq!(
        engine.saved_reference(
            &ParticipantId::new("other"),
            &RoomId::new("room"),
            message_id,
        ),
        Err(PrivateSaveRefusalV1::NotMember)
    );
}

#[test]
fn stale_and_unknown_creation_refuse_before_mutation_while_evicted_removal_works() {
    let mut engine = setup(ParticipantKind::Human, 1);
    let message_id = speak(&mut engine, 1);
    let revision = engine.revision();
    assert!(matches!(
        engine.set_saved_reference(
            &ParticipantId::new("owner"),
            &RoomId::new("room"),
            message_id,
            agora_engine::Revision(revision.0 - 1),
            true,
        ),
        Err(PrivateSaveRefusalV1::Stale { .. })
    ));
    engine
        .set_saved_reference(
            &ParticipantId::new("owner"),
            &RoomId::new("room"),
            message_id,
            revision,
            true,
        )
        .unwrap();
    speak(&mut engine, 2);
    let revision = engine.revision();
    let removed = engine
        .set_saved_reference(
            &ParticipantId::new("owner"),
            &RoomId::new("room"),
            message_id,
            revision,
            false,
        )
        .unwrap();
    assert_eq!(
        removed.availability(),
        StableReferenceAvailabilityV1::Evicted
    );
    assert_eq!(
        engine
            .saved_reference(
                &ParticipantId::new("owner"),
                &RoomId::new("room"),
                message_id,
            )
            .unwrap(),
        None
    );
}
