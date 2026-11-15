//! Private local composition for durable Agora export custody.
//!
//! This application binds the qualified pure SHA-256 provider and the separate
//! durable export owner to one exclusive private-directory checkpoint store.
//! It does not prepare source records, decide authority, expose participant
//! protocol, or decide physical reclamation. It can durably replace one live
//! export with the custody service's bounded non-reuse tombstone. The v1
//! directory store remains a single-head register and is not checkpoint-before-
//! reclaim evidence. A separately typed generational composition retains every
//! committed custody checkpoint and exposes reclamation only after a fresh
//! owner reopens and validates the exact successor. Even then, the strongest
//! result is generation-unlink evidence, never secure erasure.

use agora_engine::retention::ContentDigestV1;
use agora_export_durable::{
    DurableExportConfigV1, DurableExportServiceV1, DurableExportStartErrorV1,
};
use agora_export_service::{ExportServiceLimitsV1, ExportServiceRefusalV1, ExportServiceV1};
use agora_sha256::digest_v1;
use agora_storage_directory::generational::{
    GenerationalDirectoryCheckpointStoreV1, GenerationalDirectoryErrorV1,
    GenerationalDirectoryLimitsV1,
};
use agora_storage_directory::{DirectoryCheckpointStoreV1, DirectoryErrorV1, DirectoryLimitsV1};
use std::path::Path;

pub type LocalExportCustodyV1 =
    DurableExportServiceV1<DirectoryCheckpointStoreV1, fn(&[u8]) -> ContentDigestV1>;
pub type LocalGenerationalExportCustodyV1 =
    DurableExportServiceV1<GenerationalDirectoryCheckpointStoreV1, fn(&[u8]) -> ContentDigestV1>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocalExportStartErrorV1 {
    InvalidComposition,
    Service(ExportServiceRefusalV1),
    Directory(DirectoryErrorV1),
    Durable(DurableExportStartErrorV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocalGenerationalExportStartErrorV1 {
    InvalidComposition,
    Service(ExportServiceRefusalV1),
    Directory(GenerationalDirectoryErrorV1),
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

/// Creates a new private generational custody register. Every committed
/// checkpoint remains immutable until a later reopened owner validates the
/// successor and explicitly reclaims named obsolete generations.
pub fn initialize_new_generational_v1(
    path: impl AsRef<Path>,
    directory_limits: GenerationalDirectoryLimitsV1,
    service_limits: ExportServiceLimitsV1,
    durable_config: DurableExportConfigV1,
) -> Result<LocalGenerationalExportCustodyV1, LocalGenerationalExportStartErrorV1> {
    validate_generational_composition(directory_limits, &durable_config)?;
    let service = ExportServiceV1::new(service_limits)
        .map_err(LocalGenerationalExportStartErrorV1::Service)?;
    let store = GenerationalDirectoryCheckpointStoreV1::create_new(path, directory_limits)
        .map_err(LocalGenerationalExportStartErrorV1::Directory)?;
    DurableExportServiceV1::initialize(
        service,
        store,
        digest_v1 as fn(&[u8]) -> ContentDigestV1,
        durable_config,
    )
    .map_err(LocalGenerationalExportStartErrorV1::Durable)
}

/// Reopens and validates the current generational custody head before
/// returning the durable owner. Reclamation remains an explicit operation on
/// the returned store after custody is relinquished with `into_parts`.
pub fn recover_existing_generational_v1(
    path: impl AsRef<Path>,
    directory_limits: GenerationalDirectoryLimitsV1,
    durable_config: DurableExportConfigV1,
) -> Result<LocalGenerationalExportCustodyV1, LocalGenerationalExportStartErrorV1> {
    validate_generational_composition(directory_limits, &durable_config)?;
    let store = GenerationalDirectoryCheckpointStoreV1::open_existing(path, directory_limits)
        .map_err(LocalGenerationalExportStartErrorV1::Directory)?;
    DurableExportServiceV1::recover(
        store,
        digest_v1 as fn(&[u8]) -> ContentDigestV1,
        durable_config,
    )
    .map_err(LocalGenerationalExportStartErrorV1::Durable)
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

fn validate_generational_composition(
    directory_limits: GenerationalDirectoryLimitsV1,
    durable_config: &DurableExportConfigV1,
) -> Result<(), LocalGenerationalExportStartErrorV1> {
    if durable_config.max_snapshot_bytes > directory_limits.max_value_bytes
        || durable_config.recovery.max_snapshot_bytes > directory_limits.max_value_bytes
    {
        return Err(LocalGenerationalExportStartErrorV1::InvalidComposition);
    }
    Ok(())
}
