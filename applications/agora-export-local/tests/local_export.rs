use agora_engine::Engine;
use agora_engine::retention::{
    ExportRequestIdV1, ExportRetirementV1, ExportScopeBindingV1, ReclamationStageV1, RecordClassV1,
    RetentionBoundV1, RetentionPolicyRevisionV1, RetentionPolicyV1,
};
use agora_export_custody_port::{
    ExportCustodyApplyV1, ExportCustodyDurabilityV1, ExportCustodyPortV1, ExportCustodyRequestV1,
    ExportCustodyRetirementApplyV1, ExportCustodyRetirementRequestV1,
};
use agora_export_durable::DurableExportConfigV1;
use agora_export_local::{
    initialize_new_generational_v1, initialize_new_v1, recover_existing_generational_v1,
    recover_existing_v1,
};
use agora_export_service::{
    ExportCheckpointScopeV1, ExportRecoveryLimitsV1, ExportServiceLimitsV1,
};
use agora_service::v2::export_source::AdministrativeCheckpointSourceV1;
use agora_service::v2::persistence::ServiceCheckpointScopeV1;
use agora_service::v2::{ServiceLimitsV2, ServiceV2};
use agora_sha256::digest_v1;
use agora_storage_directory::DirectoryLimitsV1;
use agora_storage_directory::generational::{
    GenerationalDirectoryCheckpointStoreV1, GenerationalDirectoryLimitsV1,
    GenerationalReclamationApplyV1,
};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_BYTES: usize = 2 * 1_024 * 1_024;

fn temporary_path() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("agora-export-local-{}-{nonce}", std::process::id()))
}

fn durable_config() -> DurableExportConfigV1 {
    DurableExportConfigV1::new(
        "export-custody-v1",
        ExportCheckpointScopeV1::new([9; 16]).unwrap(),
        MAX_BYTES,
        ExportRecoveryLimitsV1 {
            max_snapshot_bytes: MAX_BYTES,
            maximum_service_limits: ExportServiceLimitsV1 {
                max_exports: 4,
                max_retired_exports: 4,
                max_chunks_per_export: 4,
                max_chunk_bytes: MAX_BYTES,
            },
        },
    )
    .unwrap()
}

fn generational_limits() -> GenerationalDirectoryLimitsV1 {
    GenerationalDirectoryLimitsV1 {
        max_key_bytes: 128,
        max_value_bytes: MAX_BYTES,
        max_live_generations: 4,
        max_reclamation_receipts: 4,
        max_generations_per_reclamation: 2,
    }
}

fn materialization() -> (
    u64,
    RetentionPolicyRevisionV1,
    agora_engine::retention::ExportMaterializationV1,
) {
    let mut service = ServiceV2::new(Engine::new(), ServiceLimitsV2::default());
    let mut bounds = [RetentionBoundV1::entries(8).unwrap(); 6];
    bounds[RecordClassV1::OperationalCopies as usize - 1] = RetentionBoundV1::entries(1).unwrap();
    let policy_revision = RetentionPolicyRevisionV1::new(3).unwrap();
    service
        .install_retention_policy(RetentionPolicyV1::new(
            policy_revision,
            service.engine().revision().0,
            bounds,
        ))
        .unwrap();
    let source = AdministrativeCheckpointSourceV1::capture(
        &service,
        ServiceCheckpointScopeV1::new([7; 16]),
        MAX_BYTES,
        MAX_BYTES,
    )
    .unwrap();
    let materialization = source
        .materialize(
            ExportRequestIdV1::new([4; 16]).unwrap(),
            ExportScopeBindingV1::new([5; 32]).unwrap(),
            4,
            MAX_BYTES,
            digest_v1,
        )
        .unwrap();
    (
        source.source_engine_revision(),
        policy_revision,
        materialization,
    )
}

#[test]
fn physical_custody_commits_recovers_and_durably_retires_one_copy() {
    let path = temporary_path();
    let directory_limits = DirectoryLimitsV1 {
        max_key_bytes: 128,
        max_value_bytes: MAX_BYTES,
    };
    let service_limits = ExportServiceLimitsV1 {
        max_exports: 4,
        max_retired_exports: 4,
        max_chunks_per_export: 4,
        max_chunk_bytes: MAX_BYTES,
    };
    let (engine_revision, policy_revision, materialization) = materialization();
    let request =
        ExportCustodyRequestV1::new(engine_revision, policy_revision, materialization.clone());
    let mut custody =
        initialize_new_v1(&path, directory_limits, service_limits, durable_config()).unwrap();
    let accepted = ExportCustodyPortV1::admit(&mut custody, request.clone()).unwrap();
    assert_eq!(accepted.apply, ExportCustodyApplyV1::Materialized);
    assert_eq!(
        accepted.durability,
        ExportCustodyDurabilityV1::Committed { store_revision: 1 }
    );
    let (_, store, _) = custody.into_parts();
    drop(store);

    let mut recovered = recover_existing_v1(&path, directory_limits, durable_config()).unwrap();
    assert_eq!(recovered.service().unwrap().len(), 1);
    let replay = ExportCustodyPortV1::admit(&mut recovered, request.clone()).unwrap();
    assert_eq!(replay.apply, ExportCustodyApplyV1::Replay);
    assert_eq!(
        replay.durability,
        ExportCustodyDurabilityV1::PreviouslyCommitted { store_revision: 1 }
    );
    let retirement = ExportRetirementV1::from_manifest(&materialization.manifest);
    let retired = ExportCustodyPortV1::retire(
        &mut recovered,
        ExportCustodyRetirementRequestV1 {
            current_engine_revision: engine_revision,
            current_policy_revision: policy_revision,
            retirement,
        },
    )
    .unwrap();
    assert_eq!(retired.apply, ExportCustodyRetirementApplyV1::Retired);
    assert_eq!(
        retired.durability,
        ExportCustodyDurabilityV1::Committed { store_revision: 2 }
    );
    let (_, store, _) = recovered.into_parts();
    drop(store);

    let mut recovered = recover_existing_v1(&path, directory_limits, durable_config()).unwrap();
    assert_eq!(recovered.service().unwrap().len(), 0);
    assert_eq!(recovered.service().unwrap().retired_count(), 1);
    let replay = ExportCustodyPortV1::retire(
        &mut recovered,
        ExportCustodyRetirementRequestV1 {
            current_engine_revision: engine_revision,
            current_policy_revision: policy_revision,
            retirement,
        },
    )
    .unwrap();
    assert_eq!(replay.apply, ExportCustodyRetirementApplyV1::Replay);
    assert_eq!(
        replay.durability,
        ExportCustodyDurabilityV1::PreviouslyCommitted { store_revision: 2 }
    );
    assert!(ExportCustodyPortV1::admit(&mut recovered, request).is_err());
    let (_, store, _) = recovered.into_parts();
    drop(store);
    fs::remove_dir_all(&path).unwrap();
}

#[test]
fn generational_custody_reopens_before_reclaiming_retired_export_copies() {
    let path = temporary_path();
    let service_limits = ExportServiceLimitsV1 {
        max_exports: 4,
        max_retired_exports: 4,
        max_chunks_per_export: 4,
        max_chunk_bytes: MAX_BYTES,
    };
    let (engine_revision, policy_revision, materialization) = materialization();
    let request =
        ExportCustodyRequestV1::new(engine_revision, policy_revision, materialization.clone());
    let mut custody = initialize_new_generational_v1(
        &path,
        generational_limits(),
        service_limits,
        durable_config(),
    )
    .unwrap();
    assert_eq!(custody.store_revision().0, 0);
    assert_eq!(
        ExportCustodyPortV1::admit(&mut custody, request)
            .unwrap()
            .durability,
        ExportCustodyDurabilityV1::Committed { store_revision: 1 }
    );
    assert_eq!(
        ExportCustodyPortV1::retire(
            &mut custody,
            ExportCustodyRetirementRequestV1 {
                current_engine_revision: engine_revision,
                current_policy_revision: policy_revision,
                retirement: ExportRetirementV1::from_manifest(&materialization.manifest),
            },
        )
        .unwrap()
        .durability,
        ExportCustodyDurabilityV1::Committed { store_revision: 2 }
    );
    let (_, store, _) = custody.into_parts();
    assert_eq!(store.live_generation_count(), 3);
    drop(store);

    let recovered =
        recover_existing_generational_v1(&path, generational_limits(), durable_config()).unwrap();
    assert_eq!(recovered.store_revision().0, 2);
    assert_eq!(recovered.service().unwrap().len(), 0);
    assert_eq!(recovered.service().unwrap().retired_count(), 1);
    let (_, mut store, _) = recovered.into_parts();
    let plan = store
        .prepare_reclamation(engine_revision, 2, digest_v1)
        .unwrap();
    assert_eq!(plan.source_store_revision, 1);
    assert_eq!(plan.replacement_store_revision, 2);
    assert_eq!(plan.obsolete_generations().len(), 2);
    let outcome = store.reclaim(plan.clone(), digest_v1).unwrap();
    assert_eq!(outcome.apply, GenerationalReclamationApplyV1::Reclaimed);
    assert_eq!(outcome.evidence().len(), 2);
    assert!(outcome.evidence().iter().all(|evidence| {
        evidence.stage == ReclamationStageV1::GenerationUnlinked
            && evidence.replacement_store_revision == 2
            && !evidence.proves_secure_erasure()
    }));
    assert_eq!(store.live_generation_count(), 1);
    assert_eq!(
        store.reclaim(plan, digest_v1).unwrap().apply,
        GenerationalReclamationApplyV1::Replay
    );
    drop(store);

    let store = GenerationalDirectoryCheckpointStoreV1::open_existing(&path, generational_limits())
        .unwrap();
    assert_eq!(store.live_generation_count(), 1);
    drop(store);
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn invalid_cross_layer_limit_refuses_before_creating_a_directory() {
    let path = temporary_path();
    let refusal = initialize_new_v1(
        &path,
        DirectoryLimitsV1 {
            max_key_bytes: 128,
            max_value_bytes: 1,
        },
        ExportServiceLimitsV1::default(),
        durable_config(),
    );
    assert!(matches!(
        refusal,
        Err(agora_export_local::LocalExportStartErrorV1::InvalidComposition)
    ));
    assert!(!path.exists());
}

#[test]
fn diagnostics_hide_directory_store_and_digest_state() {
    let path = temporary_path();
    let custody = initialize_new_v1(
        &path,
        DirectoryLimitsV1 {
            max_key_bytes: 128,
            max_value_bytes: MAX_BYTES,
        },
        ExportServiceLimitsV1 {
            max_exports: 4,
            max_retired_exports: 4,
            max_chunks_per_export: 4,
            max_chunk_bytes: MAX_BYTES,
        },
        durable_config(),
    )
    .unwrap();
    let rendered = format!("{custody:?}");
    assert!(rendered.contains("<redacted>"));
    assert!(!rendered.contains(path.to_string_lossy().as_ref()));
    let (_, store, _) = custody.into_parts();
    drop(store);
    fs::remove_dir_all(path).unwrap();
}
