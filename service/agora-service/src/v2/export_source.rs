//! Administrative projection of one exact Service V2 recovery image.
//!
//! This profile projects the canonical complete service checkpoint as one
//! `OperationalCopies` record. It deliberately does not reinterpret checkpoint
//! internals as room, change, receipt, private, or federation records. The
//! result is suitable only for the administrative export profile and remains
//! subject to separate Host authority and custody admission.

use super::ServiceV2;
use super::persistence::{ServiceCheckpointErrorV2, ServiceCheckpointScopeV1};
use agora_engine::retention::{
    ContentDigestProfileV1, ContentDigestV1, ExportMaterializationRefusalV1,
    ExportMaterializationV1, ExportProfileV1, ExportRecordV1, ExportRequestIdV1,
    ExportScopeBindingV1, ExportSourceFragmentV1, RecordClassV1, RetentionFrontierV1,
    RetentionPolicyRevisionV1, RetentionRecordIdV1, materialize_export_sources_v1,
};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdministrativeCheckpointSourceRefusalV1 {
    InvalidLimit,
    NoCurrentPolicy,
    OperationalCopiesDisabled,
    OrdinalExhausted,
    Checkpoint(ServiceCheckpointErrorV2),
    RecordLimit,
}

/// One immutable, bounded recovery-image source captured from a borrowed
/// authoritative service owner.
#[derive(Clone, Eq, PartialEq)]
pub struct AdministrativeCheckpointSourceV1 {
    fragment: ExportSourceFragmentV1,
    max_record_bytes: usize,
}

impl AdministrativeCheckpointSourceV1 {
    pub fn capture(
        service: &ServiceV2,
        checkpoint_scope: ServiceCheckpointScopeV1,
        max_snapshot_bytes: usize,
        max_record_bytes: usize,
    ) -> Result<Self, AdministrativeCheckpointSourceRefusalV1> {
        if max_snapshot_bytes == 0 || max_record_bytes == 0 {
            return Err(AdministrativeCheckpointSourceRefusalV1::InvalidLimit);
        }
        let policy = service
            .retention_policy()
            .ok_or(AdministrativeCheckpointSourceRefusalV1::NoCurrentPolicy)?;
        if !policy
            .bound(RecordClassV1::OperationalCopies)
            .retains_records()
        {
            return Err(AdministrativeCheckpointSourceRefusalV1::OperationalCopiesDisabled);
        }
        let source_engine_revision = service.engine().revision().0;
        let ordinal = source_engine_revision
            .checked_add(1)
            .ok_or(AdministrativeCheckpointSourceRefusalV1::OrdinalExhausted)?;
        let checkpoint = service
            .prepare_checkpoint(checkpoint_scope, max_snapshot_bytes)
            .map_err(AdministrativeCheckpointSourceRefusalV1::Checkpoint)?;
        let id = RetentionRecordIdV1::new(RecordClassV1::OperationalCopies, ordinal)
            .expect("revision successor is nonzero");
        let record = ExportRecordV1::new(id, checkpoint.bytes().to_vec(), max_record_bytes)
            .ok_or(AdministrativeCheckpointSourceRefusalV1::RecordLimit)?;
        let frontier =
            RetentionFrontierV1::new(RecordClassV1::OperationalCopies, ordinal - 1, Some(ordinal))
                .expect("the captured operational copy has an exact frontier");
        let fragment = ExportSourceFragmentV1::new(
            source_engine_revision,
            policy.revision,
            RecordClassV1::OperationalCopies,
            frontier,
            vec![record],
            1,
            max_record_bytes,
        )
        .map_err(|_| AdministrativeCheckpointSourceRefusalV1::RecordLimit)?;
        Ok(Self {
            fragment,
            max_record_bytes,
        })
    }

    pub const fn source_engine_revision(&self) -> u64 {
        self.fragment.source_engine_revision()
    }

    pub const fn policy_revision(&self) -> RetentionPolicyRevisionV1 {
        self.fragment.policy_revision()
    }

    pub fn record_id(&self) -> RetentionRecordIdV1 {
        self.fragment.records()[0].id
    }

    pub fn checkpoint_byte_length(&self) -> usize {
        self.fragment.records()[0].payload().len()
    }

    /// Materializes this exact recovery image under the administrative profile.
    /// Digest qualification and later authority/custody remain external.
    pub fn materialize<D>(
        &self,
        request: ExportRequestIdV1,
        scope: ExportScopeBindingV1,
        max_chunks: usize,
        max_chunk_bytes: usize,
        digest: D,
    ) -> Result<ExportMaterializationV1, ExportMaterializationRefusalV1>
    where
        D: FnMut(&[u8]) -> ContentDigestV1,
    {
        materialize_export_sources_v1(
            request,
            ExportProfileV1::Administrative,
            scope,
            ContentDigestProfileV1::Sha256,
            std::slice::from_ref(&self.fragment),
            1,
            self.max_record_bytes,
            max_chunks,
            max_chunk_bytes,
            digest,
        )
    }
}

impl fmt::Debug for AdministrativeCheckpointSourceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdministrativeCheckpointSourceV1")
            .field(
                "source_engine_revision",
                &self.fragment.source_engine_revision(),
            )
            .field("policy_revision", &self.fragment.policy_revision())
            .field("record_id", &self.fragment.records()[0].id)
            .field(
                "checkpoint_byte_length",
                &self.fragment.records()[0].payload().len(),
            )
            .field("checkpoint", &"<redacted>")
            .finish()
    }
}
