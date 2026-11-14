use agora_engine::persistence::RecoveryLimits;
use agora_engine::{
    AdmissionPolicy, CommandV2, DomainEventV2, Engine, Limits, LogicalTime, MAX_ROOM_PINS_V1,
    OutcomeV2, Participant, ParticipantId, ParticipantKind, Refusal, RoomId,
    StableReferenceAvailabilityV1,
};

fn setup(kind: ParticipantKind, max_messages: usize) -> Engine {
    let mut engine = Engine::with_limits(Limits {
        max_messages_per_room: max_messages,
        ..Limits::default()
    });
    for (id, participant_kind) in [("owner", kind), ("member", ParticipantKind::Agent)] {
        engine
            .apply(CommandV2::RegisterParticipant(Participant::new(
                id,
                id,
                participant_kind,
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
            participant: "member".into(),
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
        panic!("message expected");
    };
    engine.message(&"room".into(), sequence).unwrap().id()
}

#[test]
fn pins_are_shared_kind_neutral_revisioned_room_facts() {
    for kind in [ParticipantKind::Human, ParticipantKind::Agent] {
        let mut engine = setup(kind, 4);
        let message_id = speak(&mut engine, 1);
        let before = engine.revision();
        let outcome = engine
            .apply(CommandV2::SetPin {
                room_id: "room".into(),
                actor: "member".into(),
                message_id,
                pinned: true,
            })
            .unwrap();
        assert!(matches!(
            outcome,
            OutcomeV2::PinChanged { pinned: true, .. }
        ));
        assert_eq!(engine.revision().0, before.0 + 1);
        let pins = engine
            .pinned_references(&ParticipantId::new("owner"), &RoomId::new("room"))
            .unwrap();
        assert_eq!(pins.len(), 1);
        assert_eq!(pins[0].pinned_by(), &ParticipantId::new("member"));
        assert!(matches!(
            &engine.pending_events().last().unwrap().event,
            DomainEventV2::PinChanged { pinned: true, .. }
        ));
        assert_eq!(
            engine.apply(CommandV2::SetPin {
                room_id: "room".into(),
                actor: "owner".into(),
                message_id,
                pinned: true,
            }),
            Err(Refusal::InvalidPinChange)
        );
    }
}

#[test]
fn tombstone_and_eviction_preserve_pin_identity_and_unpin_remains_possible() {
    let mut engine = setup(ParticipantKind::Human, 1);
    let message_id = speak(&mut engine, 1);
    engine
        .apply(CommandV2::SetPin {
            room_id: "room".into(),
            actor: "owner".into(),
            message_id,
            pinned: true,
        })
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
            .pinned_references(&"member".into(), &"room".into())
            .unwrap()[0]
            .availability(),
        StableReferenceAvailabilityV1::Tombstoned
    );
    speak(&mut engine, 2);
    assert_eq!(
        engine
            .pinned_references(&"member".into(), &"room".into())
            .unwrap()[0]
            .availability(),
        StableReferenceAvailabilityV1::Evicted
    );
    engine
        .apply(CommandV2::SetPin {
            room_id: "room".into(),
            actor: "member".into(),
            message_id,
            pinned: false,
        })
        .unwrap();
    assert!(
        engine
            .pinned_references(&"owner".into(), &"room".into())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn shared_pin_checkpoint_is_canonical_after_history_eviction() {
    let mut engine = setup(ParticipantKind::Human, 1);
    let message_id = speak(&mut engine, 1);
    engine
        .apply(CommandV2::SetPin {
            room_id: "room".into(),
            actor: "member".into(),
            message_id,
            pinned: true,
        })
        .unwrap();
    speak(&mut engine, 2);
    let checkpoint = engine.prepare_checkpoint(1 << 20).unwrap();
    assert_eq!(checkpoint.bytes()[8], 36);
    let recovered = Engine::recover_checkpoint(
        checkpoint.bytes(),
        RecoveryLimits {
            max_snapshot_bytes: 1 << 20,
            maximum_engine_limits: engine.limits(),
        },
    )
    .unwrap();
    let pins = recovered
        .pinned_references(&"owner".into(), &"room".into())
        .unwrap();
    assert_eq!(pins[0].message_id(), message_id);
    assert_eq!(pins[0].original_sequence(), 0);
    assert_eq!(
        pins[0].availability(),
        StableReferenceAvailabilityV1::Evicted
    );
    assert_eq!(
        recovered.prepare_checkpoint(1 << 20).unwrap().bytes(),
        checkpoint.bytes()
    );
}

#[test]
fn room_pin_capacity_refuses_before_mutation() {
    let mut engine = setup(ParticipantKind::Human, MAX_ROOM_PINS_V1 + 1);
    let mut message_ids = Vec::with_capacity(MAX_ROOM_PINS_V1 + 1);
    for at in 0..=MAX_ROOM_PINS_V1 {
        message_ids.push(speak(&mut engine, at as u64 + 1));
    }
    for message_id in &message_ids[..MAX_ROOM_PINS_V1] {
        engine
            .apply(CommandV2::SetPin {
                room_id: "room".into(),
                actor: "owner".into(),
                message_id: *message_id,
                pinned: true,
            })
            .unwrap();
    }
    let revision = engine.revision();
    let pending = engine.pending_events().len();
    assert_eq!(
        engine.apply(CommandV2::SetPin {
            room_id: "room".into(),
            actor: "owner".into(),
            message_id: message_ids[MAX_ROOM_PINS_V1],
            pinned: true,
        }),
        Err(Refusal::PinLimit)
    );
    assert_eq!(engine.revision(), revision);
    assert_eq!(engine.pending_events().len(), pending);
    assert_eq!(
        engine
            .pinned_references(&ParticipantId::new("owner"), &RoomId::new("room"))
            .unwrap()
            .len(),
        MAX_ROOM_PINS_V1
    );
}
