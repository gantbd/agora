//! Private local composition for durable Agora export custody.
//!
//! This application binds the qualified pure SHA-256 provider and the separate
//! durable export owner to one exclusive private-directory checkpoint store.
//! It does not prepare source records, decide authority, expose participant
//! protocol, or provide physical reclamation. It can durably replace one live
//! export with the custody service's bounded non-reuse tombstone. That logical
//! retirement is not proof that prior filesystem bytes were reclaimed. In
//! particular, the v1 directory store remains a single-head register and is
//! not checkpoint-before-reclaim evidence.

use agora_engine::retention::ContentDigestV1;
use agora_export_durable::{
    DurableExportConfigV1, DurableExportServiceV1, DurableExportStartErrorV1,
};
use agora_export_service::{ExportServiceLimitsV1, ExportServiceRefusalV1, ExportServiceV1};
use agora_sha256::digest_v1;
use agora_storage_directory::{DirectoryCheckpointStoreV1, DirectoryErrorV1, DirectoryLimitsV1};
use std::path::Path;

pub type LocalExportCustodyV1 =
    DurableExportServiceV1<DirectoryCheckpointStoreV1, fn(&[u8]) -> ContentDigestV1>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocalExportStartErrorV1 {
    InvalidComposition,
    Service(ExportServiceRefusalV1),
    Directory(DirectoryErrorV1),
    Durable(DurableExportStartErrorV1),
}

/// Creates a new private directory and initializes an empty durable custody
/// checkpoint. Failure after directory creation leaves inspectable residue.
pub fn initialize_new_v1(
    path: impl AsRef<Path>,
    directory_limits: DirectoryLimitsV1,
    service_limits: ExportServiceLimitsV1,
    durable_config: DurableExportConfigV1,
) -> Result<LocalExportCustodyV1, LocalExportStartErrorV1> {
    validate_composition(directory_limits, &durable_config)?;
    let service = ExportServiceV1::new(service_limits).map_err(LocalExportStartErrorV1::Service)?;
    let store = DirectoryCheckpointStoreV1::create_new(path, directory_limits)
        .map_err(LocalExportStartErrorV1::Directory)?;
    DurableExportServiceV1::initialize(
        service,
        store,
        digest_v1 as fn(&[u8]) -> ContentDigestV1,
        durable_config,
    )
    .map_err(LocalExportStartErrorV1::Durable)
}

/// Reopens a previously initialized private directory and verifies the complete
/// export checkpoint through the qualified digest provider before activation.
pub fn recover_existing_v1(
    path: impl AsRef<Path>,
    directory_limits: DirectoryLimitsV1,
    durable_config: DurableExportConfigV1,
) -> Result<LocalExportCustodyV1, LocalExportStartErrorV1> {
    validate_composition(directory_limits, &durable_config)?;
    let store = DirectoryCheckpointStoreV1::open_existing(path, directory_limits)
        .map_err(LocalExportStartErrorV1::Directory)?;
    DurableExportServiceV1::recover(
        store,
        digest_v1 as fn(&[u8]) -> ContentDigestV1,
        durable_config,
    )
    .map_err(LocalExportStartErrorV1::Durable)
}

fn validate_composition(
    directory_limits: DirectoryLimitsV1,
    durable_config: &DurableExportConfigV1,
) -> Result<(), LocalExportStartErrorV1> {
    if durable_config.max_snapshot_bytes > directory_limits.max_value_bytes
        || durable_config.recovery.max_snapshot_bytes > directory_limits.max_value_bytes
    {
        return Err(LocalExportStartErrorV1::InvalidComposition);
    }
    Ok(())
}
