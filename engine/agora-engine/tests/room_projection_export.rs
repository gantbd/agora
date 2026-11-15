use agora_engine::export_source::{RoomProjectionSourceRefusalV1, RoomProjectionSourceV1};
use agora_engine::persistence::{
    CheckpointError, RecoveryLimits, prepare_room_projection_checkpoint_v1,
    recover_room_projection_checkpoint_v1,
};
use agora_engine::retention::{
    ContentDigestV1, ExportRequestIdV1, ExportScopeBindingV1, RecordClassV1, RetentionBoundV1,
    RetentionPolicyRevisionV1, RetentionPolicyV1,
};
use agora_engine::{
    AdmissionPolicy, CommandV2, Engine, Limits, LogicalTime, OutcomeV2, Participant, ParticipantId,
    ParticipantKind, ReadScopeV1, RoomId,
};

const MAX_BYTES: usize = 128 * 1_024;

fn policy(engine: &Engine, room: RetentionBoundV1) -> RetentionPolicyV1 {
    let mut bounds = [RetentionBoundV1::entries(8).unwrap(); 6];
    bounds[RecordClassV1::RoomProjection as usize - 1] = room;
    RetentionPolicyV1::new(
        RetentionPolicyRevisionV1::new(11).unwrap(),
        engine.revision().0,
        bounds,
    )
}

fn poll_engine(voter: &str) -> (Engine, RoomId, ParticipantId) {
    let mut engine = Engine::new();
    for (id, kind) in [
        ("agent", ParticipantKind::Agent),
        ("human", ParticipantKind::Human),
    ] {
        engine
            .apply(CommandV2::RegisterParticipant(Participant::new(
                id, id, kind,
            )))
            .unwrap();
    }
    let room = RoomId::from("room");
    let human = ParticipantId::from("human");
    engine
        .apply(CommandV2::CreateRoom {
            room_id: room.clone(),
            name: "Room".to_owned(),
            owner: human.clone(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(CommandV2::Join {
            room_id: room.clone(),
            participant: "agent".into(),
        })
        .unwrap();
    engine
        .apply(CommandV2::Speak {
            room_id: room.clone(),
            author: human.clone(),
            at: LogicalTime(3),
            body: "public".to_owned(),
        })
        .unwrap();
    let OutcomeV2::PollCreated { poll_id, .. } = engine
        .apply(CommandV2::CreatePoll {
            room_id: room.clone(),
            actor: human.clone(),
            prompt: "Choose".to_owned(),
            options: vec!["one".to_owned(), "two".to_owned()],
            eligible_participants: vec!["agent".into(), "human".into()],
            max_choices: 1,
        })
        .unwrap()
    else {
        panic!("poll expected");
    };
    engine
        .apply(CommandV2::SetPollBallot {
            room_id: room.clone(),
            actor: voter.into(),
            poll_id,
            selections: vec![0],
        })
        .unwrap();
    (engine, room, human)
}

#[test]
fn empty_generation_has_frozen_identity_and_exact_frontier() {
    let engine = Engine::new();
    let source = RoomProjectionSourceV1::capture(
        &engine,
        policy(&engine, RetentionBoundV1::entries(1).unwrap()),
        MAX_BYTES,
    )
    .unwrap();
    let record = &source.fragment().records()[0];
    let expected = [
        b"AGAROM1\0".as_slice(),
        &0_u64.to_be_bytes(),
        &0_u64.to_be_bytes(),
        &0_u64.to_be_bytes(),
    ]
    .concat();
    assert_eq!(record.payload(), expected);
    assert_eq!(record.id.class, RecordClassV1::RoomProjection);
    assert_eq!(record.id.ordinal, 1);
    assert_eq!(source.fragment().frontier().discarded_through, 0);
    assert_eq!(source.fragment().frontier().oldest_retained, Some(1));

    let recovered = recover_room_projection_checkpoint_v1(
        record.payload(),
        RecoveryLimits {
            max_snapshot_bytes: MAX_BYTES,
            maximum_engine_limits: Limits::default(),
        },
    )
    .unwrap();
    assert_eq!(recovered.source_engine_revision(), 0);
    assert_eq!(recovered.referenced_participant_count(), 0);
    assert_eq!(recovered.room_count(), 0);
    assert_eq!(recovered.bytes(), expected);
}

#[test]
fn public_projection_preserves_kind_and_aggregate_but_excludes_private_ballot_owner() {
    let (human_ballot, room, human) = poll_engine("human");
    let (agent_ballot, _, _) = poll_engine("agent");
    assert_eq!(human_ballot.revision(), agent_ballot.revision());
    let human_bytes = prepare_room_projection_checkpoint_v1(&human_ballot, MAX_BYTES).unwrap();
    let agent_bytes = prepare_room_projection_checkpoint_v1(&agent_ballot, MAX_BYTES).unwrap();
    assert_eq!(human_bytes.bytes(), agent_bytes.bytes());
    assert_eq!(human_bytes.referenced_participant_count(), 2);
    assert_eq!(human_bytes.room_count(), 1);
    assert!(!format!("{human_bytes:?}").contains("public"));

    let message_id = human_ballot.message(&room, 0).unwrap().id();
    let mut with_private_read = human_ballot;
    let before = with_private_read.revision();
    with_private_read
        .advance_read_position(&human, &room, ReadScopeV1::Room, before, message_id)
        .unwrap();
    assert_eq!(with_private_read.revision(), before);
    assert_eq!(
        prepare_room_projection_checkpoint_v1(&with_private_read, MAX_BYTES)
            .unwrap()
            .bytes(),
        human_bytes.bytes()
    );
}

#[test]
fn room_projection_source_is_bounded_canonical_and_fail_closed() {
    let (engine, _, _) = poll_engine("agent");
    let enabled = policy(&engine, RetentionBoundV1::entries(1).unwrap());
    let source = RoomProjectionSourceV1::capture(&engine, enabled, MAX_BYTES).unwrap();
    assert_eq!(
        source.fragment().source_engine_revision(),
        engine.revision().0
    );
    assert_eq!(
        source.fragment().records()[0].id.ordinal,
        engine.revision().0 + 1
    );
    assert_eq!(
        source.fragment().frontier().discarded_through,
        engine.revision().0
    );
    assert!(!format!("{source:?}").contains("public"));

    let digest = |bytes: &[u8]| {
        let byte = bytes.iter().fold(0_u8, |state, value| state ^ value);
        ContentDigestV1::new([byte; 32])
    };
    let materialized = source
        .materialize(
            ExportRequestIdV1::new([4; 16]).unwrap(),
            ExportScopeBindingV1::new([5; 32]).unwrap(),
            2,
            MAX_BYTES * 2,
            digest,
        )
        .unwrap();
    assert_eq!(
        materialized.manifest.included_classes,
        [true, false, false, false, false, false]
    );

    let mut stale = enabled;
    stale.effective_engine_revision += 1;
    assert_eq!(
        RoomProjectionSourceV1::capture(&engine, stale, MAX_BYTES),
        Err(RoomProjectionSourceRefusalV1::StalePolicy)
    );
    let mut advanced = Engine::new();
    let established = policy(&advanced, RetentionBoundV1::entries(1).unwrap());
    advanced
        .apply(CommandV2::RegisterParticipant(Participant::new(
            "later",
            "Later",
            ParticipantKind::Human,
        )))
        .unwrap();
    assert!(RoomProjectionSourceV1::capture(&advanced, established, MAX_BYTES).is_ok());
    assert_eq!(
        RoomProjectionSourceV1::capture(
            &engine,
            policy(&engine, RetentionBoundV1::Disabled),
            MAX_BYTES,
        ),
        Err(RoomProjectionSourceRefusalV1::ClassDisabled)
    );
    assert_eq!(
        RoomProjectionSourceV1::capture(&engine, enabled, 0),
        Err(RoomProjectionSourceRefusalV1::InvalidLimit)
    );

    let bytes = source.fragment().records()[0].payload();
    let recovery = RecoveryLimits {
        max_snapshot_bytes: MAX_BYTES,
        maximum_engine_limits: Limits::default(),
    };
    let mut refused = 0;
    for bit in 0..bytes.len() * 8 {
        let mut mutated = bytes.to_vec();
        mutated[bit / 8] ^= 1 << (bit % 8);
        match recover_room_projection_checkpoint_v1(&mutated, recovery) {
            Ok(checkpoint) => assert_eq!(checkpoint.bytes(), mutated),
            Err(_) => refused += 1,
        }
    }
    assert!(refused > 0);
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert_eq!(
        recover_room_projection_checkpoint_v1(&trailing, recovery).unwrap_err(),
        CheckpointError::Trailing
    );
}
