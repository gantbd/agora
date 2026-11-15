use agora_engine::retention::{
    ContentDigestV1, ExportChunkV1, ExportManifestV1, ExportProfileV1, ExportRequestIdV1,
    ExportScopeBindingV1, RECORD_CLASSES_V1, ReceiptRetirementApplyV1, ReceiptRetirementRefusalV1,
    ReceiptRetirementRegistryV1, ReclamationEvidenceV1, ReclamationStageV1, RecordClassV1,
    RetentionBoundV1, RetentionFrontierV1, RetentionIdentityBindingV1, RetentionPolicyRevisionV1,
    RetentionPolicyV1, RetentionScopeBindingV1, RetentionTombstoneReasonV1, RetentionTombstoneV1,
    RetentionTransitionV1, StoreGenerationIdV1, StoreReclamationPlanV1,
};

fn revision() -> RetentionPolicyRevisionV1 {
    RetentionPolicyRevisionV1::new(4).unwrap()
}

fn frontiers() -> [RetentionFrontierV1; 6] {
    RECORD_CLASSES_V1.map(|class| RetentionFrontierV1::new(class, 2, Some(3)).unwrap())
}

#[test]
fn policy_is_complete_and_classes_do_not_alias() {
    let policy = RetentionPolicyV1::new(
        revision(),
        8,
        [
            RetentionBoundV1::entries(10).unwrap(),
            RetentionBoundV1::entries(20).unwrap(),
            RetentionBoundV1::entries(30).unwrap(),
            RetentionBoundV1::Disabled,
            RetentionBoundV1::entries(50).unwrap(),
            RetentionBoundV1::Disabled,
        ],
    );
    assert_eq!(
        policy.bound(RecordClassV1::RoomProjection).maximum(),
        Some(10)
    );
    assert_eq!(
        policy.bound(RecordClassV1::RequestReceipts).maximum(),
        Some(30)
    );
    assert_eq!(
        policy.bound(RecordClassV1::OperationalCopies).maximum(),
        None
    );
    assert!(
        !policy
            .bound(RecordClassV1::OperationalCopies)
            .retains_records()
    );
    assert!(RetentionBoundV1::entries(0).is_none());
    assert!(RetentionPolicyRevisionV1::new(0).is_none());
}

#[test]
fn transition_and_export_require_canonical_class_and_chunk_order() {
    assert!(RetentionTransitionV1::new(revision(), 8, 9, [0; 6], frontiers()).is_some());
    assert!(RetentionTransitionV1::new(revision(), 9, 8, [0; 6], frontiers()).is_none());
    let chunks = vec![
        ExportChunkV1::new(0, 20, ContentDigestV1::new([1; 32])).unwrap(),
        ExportChunkV1::new(1, 10, ContentDigestV1::new([2; 32])).unwrap(),
    ];
    let manifest = ExportManifestV1::new(
        ExportRequestIdV1::new([7; 16]).unwrap(),
        ExportProfileV1::RoomVisible,
        ExportScopeBindingV1::new([6; 32]).unwrap(),
        9,
        revision(),
        [true, true, false, false, false, false],
        frontiers(),
        chunks.clone(),
        ContentDigestV1::new([3; 32]),
        2,
    )
    .unwrap();
    assert_eq!(manifest.chunks(), chunks);
    let reversed = vec![chunks[1], chunks[0]];
    assert!(
        ExportManifestV1::new(
            ExportRequestIdV1::new([7; 16]).unwrap(),
            ExportProfileV1::RoomVisible,
            ExportScopeBindingV1::new([6; 32]).unwrap(),
            9,
            revision(),
            [true; 6],
            frontiers(),
            reversed,
            ContentDigestV1::new([3; 32]),
            2,
        )
        .is_none()
    );
}

#[test]
fn retirement_tombstone_is_distinct_redacted_and_revision_bound() {
    let tombstone = RetentionTombstoneV1::new(
        RecordClassV1::RequestReceipts,
        RetentionTombstoneReasonV1::ReceiptRetired,
        revision(),
        9,
        RetentionIdentityBindingV1::new([0x41; 32]).unwrap(),
        RetentionScopeBindingV1::new([0x42; 32]).unwrap(),
    );
    let diagnostic = format!("{tombstone:?}");
    assert!(!diagnostic.contains("41"));
    assert!(!diagnostic.contains("42"));
    assert_eq!(
        tombstone.identity_binding(),
        RetentionIdentityBindingV1::new([0x41; 32]).unwrap()
    );
    assert_eq!(
        RetentionTombstoneV1::new(
            RecordClassV1::RequestReceipts,
            RetentionTombstoneReasonV1::ReceiptRetired,
            revision(),
            0,
            RetentionIdentityBindingV1::new([1; 32]).unwrap(),
            RetentionScopeBindingV1::new([2; 32]).unwrap(),
        )
        .transition_engine_revision,
        0
    );
}

#[test]
fn retired_receipt_identity_cannot_become_executable_again() {
    let first = RetentionTombstoneV1::new(
        RecordClassV1::RequestReceipts,
        RetentionTombstoneReasonV1::ReceiptRetired,
        revision(),
        9,
        RetentionIdentityBindingV1::new([1; 32]).unwrap(),
        RetentionScopeBindingV1::new([2; 32]).unwrap(),
    );
    let mut registry = ReceiptRetirementRegistryV1::new(1).unwrap();
    assert_eq!(
        registry.retire(first),
        Ok(ReceiptRetirementApplyV1::Retired)
    );
    assert_eq!(registry.retire(first), Ok(ReceiptRetirementApplyV1::Replay));
    assert!(registry.is_retired(
        RetentionIdentityBindingV1::new([1; 32]).unwrap(),
        RetentionScopeBindingV1::new([2; 32]).unwrap()
    ));
    let changed = RetentionTombstoneV1::new(
        RecordClassV1::RequestReceipts,
        RetentionTombstoneReasonV1::ReceiptRetired,
        revision(),
        10,
        RetentionIdentityBindingV1::new([1; 32]).unwrap(),
        RetentionScopeBindingV1::new([2; 32]).unwrap(),
    );
    assert_eq!(
        registry.retire(changed),
        Err(ReceiptRetirementRefusalV1::Conflict)
    );
    let recovered = ReceiptRetirementRegistryV1::recover(1, registry.checkpoint()).unwrap();
    assert!(recovered.is_retired(
        RetentionIdentityBindingV1::new([1; 32]).unwrap(),
        RetentionScopeBindingV1::new([2; 32]).unwrap()
    ));
    assert_eq!(
        registry.retire(RetentionTombstoneV1::new(
            RecordClassV1::RequestReceipts,
            RetentionTombstoneReasonV1::ReceiptRetired,
            revision(),
            9,
            RetentionIdentityBindingV1::new([3; 32]).unwrap(),
            RetentionScopeBindingV1::new([4; 32]).unwrap(),
        )),
        Err(ReceiptRetirementRefusalV1::Capacity)
    );
}

#[test]
fn reclamation_is_ordered_bounded_and_never_erasure_evidence() {
    let first = StoreGenerationIdV1::new([1; 16]).unwrap();
    let second = StoreGenerationIdV1::new([2; 16]).unwrap();
    let plan = StoreReclamationPlanV1::new(
        9,
        11,
        12,
        ContentDigestV1::new([8; 32]),
        vec![first, second],
        2,
    )
    .unwrap();
    let evidence =
        ReclamationEvidenceV1::new(&plan, first, ReclamationStageV1::ProviderAttested).unwrap();
    assert!(!evidence.proves_secure_erasure());
    assert!(
        ReclamationEvidenceV1::new(
            &plan,
            StoreGenerationIdV1::new([3; 16]).unwrap(),
            ReclamationStageV1::GenerationUnlinked,
        )
        .is_none()
    );
    assert!(
        StoreReclamationPlanV1::new(9, 12, 12, ContentDigestV1::new([8; 32]), vec![first], 1,)
            .is_none()
    );
}
