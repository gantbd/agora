use agora_engine::export_source::AuthoritativeChangeSourceV1;
use agora_engine::retention::{
    ExportProfileV1, ExportRequestIdV1, ExportScopeBindingV1, RECORD_CLASSES_V1, RecordClassV1,
    RetentionBoundV1, RetentionPolicyRevisionV1, RetentionPolicyV1, materialize_export_sources_v1,
};
use agora_engine::{Engine, Limits, Participant, ParticipantKind};
use agora_protocol::v2::{CommandV2, ReplyV2, RequestIdV2, RequestV2};
use agora_service::v2::export_source::{
    AdministrativeCheckpointSourceRefusalV1, AdministrativeCheckpointSourceV1,
    RequestReceiptSourceRefusalV1, RequestReceiptSourceV1, recover_request_receipt_record_v1,
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

#[test]
fn request_receipt_source_is_owner_native_exact_and_canonical() {
    let mut engine = Engine::new();
    for (id, kind) in [
        ("alice", ParticipantKind::Human),
        ("bot", ParticipantKind::Agent),
    ] {
        engine
            .apply(agora_engine::CommandV2::RegisterParticipant(
                Participant::new(id, id, kind),
            ))
            .unwrap();
    }
    let mut service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    let bot = service.connect("bot").unwrap();
    let policy_revision = RetentionPolicyRevisionV1::new(9).unwrap();
    service
        .install_retention_policy(RetentionPolicyV1::new(
            policy_revision,
            service.engine().revision().0,
            [RetentionBoundV1::entries(64).unwrap(); 6],
        ))
        .unwrap();
    assert_eq!(
        RequestReceiptSourceV1::capture(&service, MAX_CHECKPOINT),
        Err(RequestReceiptSourceRefusalV1::RevisionUnavailable)
    );

    let alice_request = RequestV2 {
        request_id: RequestIdV2(1),
        command: CommandV2::Capabilities,
    };
    let alice_reply = service.execute(alice, alice_request.clone());
    assert!(matches!(alice_reply, ReplyV2::Accepted(_)));
    assert_eq!(service.execute(alice, alice_request), alice_reply);
    assert!(matches!(
        service.execute(
            bot,
            RequestV2 {
                request_id: RequestIdV2(1),
                command: CommandV2::Capabilities,
            },
        ),
        ReplyV2::Accepted(_)
    ));
    assert_eq!(service.receipt_revision(), Some(2));
    service
        .retire_request_receipts(
            policy_revision,
            service.engine().revision(),
            &[(alice, RequestIdV2(1))],
        )
        .unwrap();

    assert_eq!(
        RequestReceiptSourceV1::capture(&service, 1),
        Err(RequestReceiptSourceRefusalV1::RecordLimit)
    );
    let source = RequestReceiptSourceV1::capture(&service, MAX_CHECKPOINT).unwrap();
    assert_eq!(source.receipt_revision(), 3);
    assert_eq!(source.fragment().class(), RecordClassV1::RequestReceipts);
    assert_eq!(source.fragment().frontier().discarded_through, 2);
    assert_eq!(source.fragment().frontier().oldest_retained, Some(3));
    let recovered = recover_request_receipt_record_v1(
        source.record_payload(),
        ServiceLimitsV2::default(),
        MAX_CHECKPOINT,
    )
    .unwrap();
    assert_eq!(recovered.revision(), 3);
    assert_eq!(recovered.live_receipt_count(), 1);
    assert_eq!(recovered.retired_receipt_count(), 1);
    assert_eq!(
        recovered.canonical_bytes(MAX_CHECKPOINT).unwrap(),
        source.record_payload()
    );

    let materialization = source
        .materialize(
            ExportRequestIdV1::new([8; 16]).unwrap(),
            ExportScopeBindingV1::new([9; 32]).unwrap(),
            2,
            MAX_CHECKPOINT,
            digest_v1,
        )
        .unwrap();
    assert_eq!(
        materialization.manifest.included_classes,
        [false, false, true, false, false, false]
    );
    assert_eq!(
        materialization.manifest.frontiers[2],
        source.fragment().frontier()
    );
    let changes = AuthoritativeChangeSourceV1::capture(
        service.engine(),
        service.retention_policy().unwrap(),
        64,
        MAX_CHECKPOINT,
    )
    .unwrap();
    let composed = materialize_export_sources_v1(
        ExportRequestIdV1::new([10; 16]).unwrap(),
        ExportProfileV1::Administrative,
        ExportScopeBindingV1::new([11; 32]).unwrap(),
        agora_engine::retention::ContentDigestProfileV1::Sha256,
        &[changes.fragment().clone(), source.fragment().clone()],
        65,
        MAX_CHECKPOINT,
        4,
        MAX_CHECKPOINT,
        digest_v1,
    )
    .unwrap();
    assert_eq!(
        composed.manifest.included_classes,
        [false, true, true, false, false, false]
    );
    let rendered = format!("{source:?} {recovered:?}");
    assert!(rendered.contains("<redacted>"));
    assert!(!rendered.contains("alice"));
    assert!(!rendered.contains("bot"));

    for bit in 0..source.record_payload().len() * 8 {
        let mut candidate = source.record_payload().to_vec();
        candidate[bit / 8] ^= 1 << (bit % 8);
        if let Ok(mutant) = recover_request_receipt_record_v1(
            &candidate,
            ServiceLimitsV2::default(),
            MAX_CHECKPOINT,
        ) {
            assert_eq!(mutant.canonical_bytes(MAX_CHECKPOINT).unwrap(), candidate);
        }
    }
}

#[test]
fn empty_receipt_generation_has_frozen_standalone_bytes() {
    let mut engine = Engine::new();
    engine
        .apply(agora_engine::CommandV2::RegisterParticipant(
            Participant::new("alice", "Alice", ParticipantKind::Human),
        ))
        .unwrap();
    let mut service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    service
        .install_retention_policy(RetentionPolicyV1::new(
            RetentionPolicyRevisionV1::new(1).unwrap(),
            service.engine().revision().0,
            [RetentionBoundV1::entries(8).unwrap(); 6],
        ))
        .unwrap();
    service.execute(
        alice,
        RequestV2 {
            request_id: RequestIdV2(1),
            command: CommandV2::Capabilities,
        },
    );
    service.disconnect(alice).unwrap();
    let source = RequestReceiptSourceV1::capture(&service, MAX_CHECKPOINT).unwrap();
    assert_eq!(source.receipt_revision(), 2);
    assert_eq!(
        source.record_payload(),
        &[
            0x41, 0x47, 0x41, 0x52, 0x52, 0x43, 0x31, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00,
        ]
    );
    let recovered = recover_request_receipt_record_v1(
        source.record_payload(),
        ServiceLimitsV2::default(),
        MAX_CHECKPOINT,
    )
    .unwrap();
    assert_eq!(recovered.live_receipt_count(), 0);
    assert_eq!(recovered.retired_receipt_count(), 0);
}

#[test]
fn request_receipt_source_preflights_policy_class_and_record_limit() {
    assert_eq!(
        RequestReceiptSourceV1::capture(
            &ServiceV2::new(Engine::new(), ServiceLimitsV2::default()),
            MAX_CHECKPOINT,
        ),
        Err(RequestReceiptSourceRefusalV1::NoCurrentPolicy)
    );
    let mut service = service_with_policy(RetentionBoundV1::entries(1).unwrap());
    assert_eq!(
        RequestReceiptSourceV1::capture(&service, MAX_CHECKPOINT),
        Err(RequestReceiptSourceRefusalV1::RevisionUnavailable)
    );
    let disabled_policy = RetentionPolicyV1::new(
        RetentionPolicyRevisionV1::new(8).unwrap(),
        service.engine().revision().0,
        [RetentionBoundV1::Disabled; 6],
    );
    service.install_retention_policy(disabled_policy).unwrap();
    assert_eq!(
        RequestReceiptSourceV1::capture(&service, MAX_CHECKPOINT),
        Err(RequestReceiptSourceRefusalV1::RequestReceiptsDisabled)
    );
}
