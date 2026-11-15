use agora_engine::export_source::{
    AuthoritativeChangeSourceRefusalV1, AuthoritativeChangeSourceV1,
};
use agora_engine::persistence::{
    CheckpointError, RecoveryLimits, prepare_authoritative_change_checkpoint_v1,
    recover_authoritative_change_checkpoint_v1,
};
use agora_engine::retention::{
    ContentDigestV1, ExportProfileV1, ExportRequestIdV1, ExportScopeBindingV1, RecordClassV1,
    RetentionBoundV1, RetentionPolicyRevisionV1, RetentionPolicyV1,
};
use agora_engine::{
    AdmissionPolicy, CommandV2, Engine, Limits, Participant, ParticipantId, ParticipantKind, RoomId,
};

fn policy(engine: &Engine, changes: RetentionBoundV1) -> RetentionPolicyV1 {
    let mut bounds = [RetentionBoundV1::entries(8).unwrap(); 6];
    bounds[RecordClassV1::AuthoritativeChanges as usize - 1] = changes;
    RetentionPolicyV1::new(
        RetentionPolicyRevisionV1::new(7).unwrap(),
        engine.revision().0,
        bounds,
    )
}

fn engine_with_retention(max_retained_changes: usize) -> Engine {
    let mut engine = Engine::with_limits(Limits {
        max_retained_changes,
        ..Limits::default()
    });
    engine
        .apply(CommandV2::RegisterParticipant(Participant::new(
            "alice",
            "Alice",
            ParticipantKind::Human,
        )))
        .unwrap();
    engine
        .apply(CommandV2::RegisterParticipant(Participant::new(
            "agent",
            "Agent",
            ParticipantKind::Agent,
        )))
        .unwrap();
    engine
        .apply(CommandV2::CreateRoom {
            room_id: RoomId::from("room"),
            name: "Room".into(),
            owner: ParticipantId::from("alice"),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
}

#[test]
fn retained_authoritative_changes_export_with_exact_gap_and_identity() {
    let engine = engine_with_retention(2);
    let expected: Vec<_> = engine.retained_changes().cloned().collect();
    let source = AuthoritativeChangeSourceV1::capture(
        &engine,
        policy(&engine, RetentionBoundV1::entries(2).unwrap()),
        2,
        16 * 1_024,
    )
    .unwrap();
    let fragment = source.fragment();
    assert_eq!(fragment.class(), RecordClassV1::AuthoritativeChanges);
    assert_eq!(fragment.source_engine_revision(), engine.revision().0);
    assert_eq!(fragment.frontier().discarded_through, 1);
    assert_eq!(fragment.frontier().oldest_retained, Some(2));
    assert_eq!(
        fragment
            .records()
            .iter()
            .map(|record| record.id.ordinal)
            .collect::<Vec<_>>(),
        vec![2, 3]
    );

    let recovery = RecoveryLimits {
        max_snapshot_bytes: 16 * 1_024,
        maximum_engine_limits: Limits::default(),
    };
    let frozen_agent_registration = [
        b"AGACHG1\0".as_slice(),
        &[0],
        &1_u64.to_be_bytes(),
        &2_u64.to_be_bytes(),
        &[0],
        &5_u32.to_be_bytes(),
        b"agent",
        &[1],
    ]
    .concat();
    assert_eq!(fragment.records()[0].payload(), frozen_agent_registration);
    for (record, envelope) in fragment.records().iter().zip(&expected) {
        let recovered =
            recover_authoritative_change_checkpoint_v1(record.payload(), recovery).unwrap();
        assert_eq!(&recovered, envelope);
        let reencoded = prepare_authoritative_change_checkpoint_v1(&recovered, 16 * 1_024).unwrap();
        assert_eq!(reencoded.bytes(), record.payload());
    }
    assert!(!format!("{source:?}").contains("alice"));
    assert!(!format!("{source:?}").contains("agent"));

    let digest = |bytes: &[u8]| {
        let value = bytes
            .iter()
            .fold(0_u8, |state, byte| state.wrapping_add(*byte));
        ContentDigestV1::new([value; 32])
    };
    let materialization = source
        .materialize(
            ExportRequestIdV1::new([8; 16]).unwrap(),
            ExportScopeBindingV1::new([9; 32]).unwrap(),
            2,
            64 * 1_024,
            digest,
        )
        .unwrap();
    assert_eq!(
        materialization.manifest.profile,
        ExportProfileV1::Administrative
    );
    assert_eq!(
        materialization.manifest.included_classes,
        [false, true, false, false, false, false]
    );
    assert_eq!(materialization.manifest.frontiers[1], fragment.frontier());
}

#[test]
fn authoritative_change_source_and_record_fail_closed() {
    let empty = Engine::new();
    assert_eq!(
        AuthoritativeChangeSourceV1::capture(
            &empty,
            policy(&empty, RetentionBoundV1::entries(1).unwrap()),
            1,
            1_024,
        ),
        Err(AuthoritativeChangeSourceRefusalV1::NoRetainedChanges)
    );

    let engine = engine_with_retention(2);
    let mut stale = policy(&engine, RetentionBoundV1::entries(2).unwrap());
    stale.effective_engine_revision -= 1;
    assert_eq!(
        AuthoritativeChangeSourceV1::capture(&engine, stale, 2, 16 * 1_024),
        Err(AuthoritativeChangeSourceRefusalV1::StalePolicy)
    );
    assert_eq!(
        AuthoritativeChangeSourceV1::capture(
            &engine,
            policy(&engine, RetentionBoundV1::Disabled),
            2,
            16 * 1_024,
        ),
        Err(AuthoritativeChangeSourceRefusalV1::ClassDisabled)
    );
    assert_eq!(
        AuthoritativeChangeSourceV1::capture(
            &engine,
            policy(&engine, RetentionBoundV1::entries(2).unwrap()),
            1,
            16 * 1_024,
        ),
        Err(AuthoritativeChangeSourceRefusalV1::RecordLimit)
    );

    let source = AuthoritativeChangeSourceV1::capture(
        &engine,
        policy(&engine, RetentionBoundV1::entries(2).unwrap()),
        2,
        16 * 1_024,
    )
    .unwrap();
    let bytes = source.fragment().records()[0].payload();
    let recovery = RecoveryLimits {
        max_snapshot_bytes: 16 * 1_024,
        maximum_engine_limits: Limits::default(),
    };
    let mut foreign_profile = bytes.to_vec();
    foreign_profile[8] |= 0x80;
    assert_eq!(
        recover_authoritative_change_checkpoint_v1(&foreign_profile, recovery),
        Err(CheckpointError::Profile)
    );
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert_eq!(
        recover_authoritative_change_checkpoint_v1(&trailing, recovery),
        Err(CheckpointError::Trailing)
    );
    assert_eq!(
        recover_authoritative_change_checkpoint_v1(&bytes[..bytes.len() - 1], recovery),
        Err(CheckpointError::Truncated)
    );

    let mut refused = 0_usize;
    for bit in 0..bytes.len() * 8 {
        let mut mutated = bytes.to_vec();
        mutated[bit / 8] ^= 1 << (bit % 8);
        match recover_authoritative_change_checkpoint_v1(&mutated, recovery) {
            Ok(envelope) => {
                let canonical =
                    prepare_authoritative_change_checkpoint_v1(&envelope, 16 * 1_024).unwrap();
                assert_eq!(canonical.bytes(), mutated);
            }
            Err(_) => refused += 1,
        }
    }
    assert!(refused > 0);
}
