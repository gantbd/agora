use agora_engine::retention::{
    ExportProfileV1, ExportRequestIdV1, ExportScopeBindingV1, RECORD_CLASSES_V1, RecordClassV1,
    RetentionBoundV1, RetentionPolicyRevisionV1, RetentionPolicyV1,
};
use agora_engine::{Engine, Limits};
use agora_service::v2::export_source::{
    AdministrativeCheckpointSourceRefusalV1, AdministrativeCheckpointSourceV1,
};
use agora_service::v2::persistence::{ServiceCheckpointScopeV1, ServiceRecoveryLimitsV2};
use agora_service::v2::{ServiceLimitsV2, ServiceV2};
use agora_sha256::digest_v1;

const MAX_CHECKPOINT: usize = 2 * 1_024 * 1_024;

fn service_with_policy(operational_bound: RetentionBoundV1) -> ServiceV2 {
    let mut service = ServiceV2::new(Engine::new(), ServiceLimitsV2::default());
    let mut bounds = [RetentionBoundV1::entries(64).unwrap(); 6];
    bounds[RecordClassV1::OperationalCopies as usize - 1] = operational_bound;
    service
        .install_retention_policy(RetentionPolicyV1::new(
            RetentionPolicyRevisionV1::new(7).unwrap(),
            service.engine().revision().0,
            bounds,
        ))
        .unwrap();
    service
}

#[test]
fn administrative_projection_is_one_exact_recoverable_operational_copy() {
    let service = service_with_policy(RetentionBoundV1::entries(1).unwrap());
    let checkpoint_scope = ServiceCheckpointScopeV1::new([3; 16]);
    let source = AdministrativeCheckpointSourceV1::capture(
        &service,
        checkpoint_scope,
        MAX_CHECKPOINT,
        MAX_CHECKPOINT,
    )
    .unwrap();
    assert_eq!(
        source.source_engine_revision(),
        service.engine().revision().0
    );
    assert_eq!(
        source.policy_revision(),
        RetentionPolicyRevisionV1::new(7).unwrap()
    );
    assert_eq!(source.record_id().class, RecordClassV1::OperationalCopies);
    assert_eq!(
        source.record_id().ordinal,
        service.engine().revision().0 + 1
    );

    let materialization = source
        .materialize(
            ExportRequestIdV1::new([4; 16]).unwrap(),
            ExportScopeBindingV1::new([5; 32]).unwrap(),
            2,
            MAX_CHECKPOINT,
            digest_v1,
        )
        .unwrap();
    assert_eq!(
        materialization.manifest.profile,
        ExportProfileV1::Administrative
    );
    assert_eq!(materialization.manifest.source_engine_revision, 0);
    assert_eq!(
        materialization.manifest.included_classes,
        [false, false, false, false, false, true]
    );
    assert_eq!(
        materialization.manifest.frontiers[5].oldest_retained,
        Some(1)
    );
    assert!(
        materialization.manifest.frontiers[..5]
            .iter()
            .all(|frontier| frontier.oldest_retained.is_none())
    );

    let bytes = materialization.chunks()[0].bytes();
    assert_eq!(&bytes[..8], b"AGEXCH1\0");
    assert_eq!(bytes[8], RecordClassV1::OperationalCopies as u8);
    assert_eq!(u64::from_be_bytes(bytes[9..17].try_into().unwrap()), 1);
    let payload_length = u32::from_be_bytes(bytes[17..21].try_into().unwrap()) as usize;
    assert_eq!(payload_length, source.checkpoint_byte_length());
    let recovered = ServiceV2::recover_checkpoint(
        &bytes[21..21 + payload_length],
        checkpoint_scope,
        ServiceRecoveryLimitsV2 {
            max_snapshot_bytes: MAX_CHECKPOINT,
            max_engine_snapshot_bytes: MAX_CHECKPOINT,
            maximum_service_limits: ServiceLimitsV2::default(),
            maximum_engine_limits: Limits::default(),
        },
    )
    .unwrap();
    assert_eq!(recovered.engine().revision(), service.engine().revision());
    assert_eq!(recovered.retention_policy(), service.retention_policy());
    assert_eq!(
        materialization
            .manifest
            .frontiers
            .map(|frontier| frontier.class),
        RECORD_CLASSES_V1
    );
}

#[test]
fn policy_class_and_size_preflight_before_export_bytes_escape() {
    let without_policy = ServiceV2::new(Engine::new(), ServiceLimitsV2::default());
    assert_eq!(
        AdministrativeCheckpointSourceV1::capture(
            &without_policy,
            ServiceCheckpointScopeV1::new([1; 16]),
            MAX_CHECKPOINT,
            MAX_CHECKPOINT,
        ),
        Err(AdministrativeCheckpointSourceRefusalV1::NoCurrentPolicy)
    );
    let disabled = service_with_policy(RetentionBoundV1::Disabled);
    assert_eq!(
        AdministrativeCheckpointSourceV1::capture(
            &disabled,
            ServiceCheckpointScopeV1::new([1; 16]),
            MAX_CHECKPOINT,
            MAX_CHECKPOINT,
        ),
        Err(AdministrativeCheckpointSourceRefusalV1::OperationalCopiesDisabled)
    );
    let enabled = service_with_policy(RetentionBoundV1::entries(1).unwrap());
    assert_eq!(
        AdministrativeCheckpointSourceV1::capture(
            &enabled,
            ServiceCheckpointScopeV1::new([1; 16]),
            MAX_CHECKPOINT,
            1,
        ),
        Err(AdministrativeCheckpointSourceRefusalV1::RecordLimit)
    );
}

#[test]
fn equal_capture_is_deterministic_and_diagnostics_are_redacted() {
    let service = service_with_policy(RetentionBoundV1::entries(1).unwrap());
    let capture = || {
        AdministrativeCheckpointSourceV1::capture(
            &service,
            ServiceCheckpointScopeV1::new([6; 16]),
            MAX_CHECKPOINT,
            MAX_CHECKPOINT,
        )
        .unwrap()
    };
    let first = capture();
    assert_eq!(first, capture());
    let rendered = format!("{first:?}");
    assert!(rendered.contains("checkpoint_byte_length"));
    assert!(rendered.contains("<redacted>"));
    assert!(!rendered.contains("AGORASV"));
}
