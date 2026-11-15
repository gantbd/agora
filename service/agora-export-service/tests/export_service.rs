use agora_engine::retention::{
    ContentDigestProfileV1, ContentDigestV1, ExportMaterializationApplyV1, ExportProfileV1,
    ExportRecordV1, ExportRegistryRefusalV1, ExportRequestIdV1, ExportScopeBindingV1,
    RECORD_CLASSES_V1, RecordClassV1, RetentionFrontierV1, RetentionPolicyRevisionV1,
    RetentionRecordIdV1, materialize_export_v1,
};
use agora_export_service::{
    ExportAdmissionV1, ExportCheckpointErrorV1, ExportCheckpointScopeV1, ExportRecoveryLimitsV1,
    ExportServiceLimitsV1, ExportServiceRefusalV1, ExportServiceV1,
};

fn digest(bytes: &[u8]) -> ContentDigestV1 {
    let mut output = [0_u8; 32];
    for (index, byte) in bytes.iter().enumerate() {
        let slot = index % output.len();
        output[slot] = output[slot].wrapping_add(*byte).wrapping_add(index as u8);
    }
    ContentDigestV1::new(output)
}

fn policy_revision() -> RetentionPolicyRevisionV1 {
    RetentionPolicyRevisionV1::new(3).unwrap()
}

fn materialization(
    request: u8,
    payload: &[u8],
) -> agora_engine::retention::ExportMaterializationV1 {
    materialization_with_chunk_limit(request, payload, 64)
}

fn materialization_with_chunk_limit(
    request: u8,
    payload: &[u8],
    max_chunk_bytes: usize,
) -> agora_engine::retention::ExportMaterializationV1 {
    let id = RetentionRecordIdV1::new(RecordClassV1::RoomProjection, 1).unwrap();
    let record = ExportRecordV1::new(id, payload.to_vec(), 128).unwrap();
    let frontiers = RECORD_CLASSES_V1.map(|class| {
        if class == RecordClassV1::RoomProjection {
            RetentionFrontierV1::new(class, 0, Some(1)).unwrap()
        } else {
            RetentionFrontierV1::new(class, 0, None).unwrap()
        }
    });
    materialize_export_v1(
        ExportRequestIdV1::new([request; 16]).unwrap(),
        ExportProfileV1::RoomVisible,
        ExportScopeBindingV1::new([9; 32]).unwrap(),
        ContentDigestProfileV1::Sha256,
        8,
        policy_revision(),
        [true, false, false, false, false, false],
        frontiers,
        &[record],
        1,
        128,
        2,
        max_chunk_bytes,
        digest,
    )
    .unwrap()
}

fn limits(max_exports: usize) -> ExportServiceLimitsV1 {
    ExportServiceLimitsV1 {
        max_exports,
        max_chunks_per_export: 2,
        max_chunk_bytes: 64,
    }
}

fn admission() -> ExportAdmissionV1 {
    ExportAdmissionV1 {
        current_engine_revision: 8,
        current_policy_revision: policy_revision(),
    }
}

#[test]
fn custody_is_revision_bound_exact_and_capacity_atomic() {
    let mut service = ExportServiceV1::new(limits(1)).unwrap();
    let first = materialization(1, b"one");
    assert_eq!(
        service.admit(
            admission(),
            materialization_with_chunk_limit(3, &[7; 60], 128),
        ),
        Err(ExportServiceRefusalV1::MaterializationLimit)
    );
    assert_eq!(
        service.admit(
            ExportAdmissionV1 {
                current_engine_revision: 9,
                ..admission()
            },
            first.clone(),
        ),
        Err(ExportServiceRefusalV1::StaleSource {
            expected: 8,
            actual: 9,
        })
    );
    assert_eq!(
        service.admit(
            ExportAdmissionV1 {
                current_policy_revision: RetentionPolicyRevisionV1::new(4).unwrap(),
                ..admission()
            },
            first.clone(),
        ),
        Err(ExportServiceRefusalV1::PolicyMismatch {
            expected: policy_revision(),
            actual: RetentionPolicyRevisionV1::new(4).unwrap(),
        })
    );
    assert!(service.is_empty());
    assert_eq!(
        service.admit(admission(), first.clone()),
        Ok(ExportMaterializationApplyV1::Materialized)
    );
    assert_eq!(
        service.admit(admission(), first.clone()),
        Ok(ExportMaterializationApplyV1::Replay)
    );
    assert_eq!(
        service.admit(admission(), materialization(1, b"ONE")),
        Err(ExportServiceRefusalV1::Registry(
            ExportRegistryRefusalV1::Conflict
        ))
    );
    assert_eq!(
        service.admit(admission(), materialization(2, b"two")),
        Err(ExportServiceRefusalV1::Registry(
            ExportRegistryRefusalV1::Capacity
        ))
    );
    assert_eq!(service.len(), 1);
    assert_eq!(
        service.materialization(first.manifest.request),
        Some(&first)
    );
}

#[test]
fn separate_checkpoint_is_canonical_scope_bound_and_digest_verified() {
    let scope = ExportCheckpointScopeV1::new([4; 16]).unwrap();
    let mut service = ExportServiceV1::new(limits(2)).unwrap();
    let export = materialization(1, b"one");
    service.admit(admission(), export.clone()).unwrap();
    let checkpoint = service.prepare_checkpoint(scope, 4_096, digest).unwrap();
    assert_eq!(
        checkpoint,
        service.prepare_checkpoint(scope, 4_096, digest).unwrap()
    );
    assert!(!format!("{checkpoint:?}").contains("one"));
    let recovery = ExportRecoveryLimitsV1 {
        max_snapshot_bytes: 4_096,
        maximum_service_limits: limits(2),
    };
    let recovered =
        ExportServiceV1::recover_checkpoint(checkpoint.bytes(), scope, recovery, digest).unwrap();
    assert_eq!(recovered, service);
    assert_eq!(
        recovered.materialization(export.manifest.request),
        Some(&export)
    );
    assert_eq!(
        ExportServiceV1::recover_checkpoint(
            checkpoint.bytes(),
            ExportCheckpointScopeV1::new([5; 16]).unwrap(),
            recovery,
            digest,
        ),
        Err(ExportCheckpointErrorV1::Scope)
    );
    let mut corrupt = checkpoint.bytes().to_vec();
    corrupt[40] ^= 1;
    assert_eq!(
        ExportServiceV1::recover_checkpoint(&corrupt, scope, recovery, digest),
        Err(ExportCheckpointErrorV1::Digest)
    );
    assert_eq!(
        service.prepare_checkpoint(scope, 32, digest),
        Err(ExportCheckpointErrorV1::Limit)
    );
}
