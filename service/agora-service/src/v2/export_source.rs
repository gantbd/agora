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
    ExportScopeBindingV1, RECORD_CLASSES_V1, RecordClassV1, RetentionFrontierV1,
    RetentionPolicyRevisionV1, RetentionRecordIdV1, materialize_export_v1,
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
    source_engine_revision: u64,
    policy_revision: RetentionPolicyRevisionV1,
    frontier: RetentionFrontierV1,
    record: ExportRecordV1,
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
        Ok(Self {
            source_engine_revision,
            policy_revision: policy.revision,
            frontier,
            record,
            max_record_bytes,
        })
    }

    pub const fn source_engine_revision(&self) -> u64 {
        self.source_engine_revision
    }

    pub const fn policy_revision(&self) -> RetentionPolicyRevisionV1 {
        self.policy_revision
    }

    pub const fn record_id(&self) -> RetentionRecordIdV1 {
        self.record.id
    }

    pub fn checkpoint_byte_length(&self) -> usize {
        self.record.payload().len()
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
        let included_classes =
            RECORD_CLASSES_V1.map(|class| matches!(class, RecordClassV1::OperationalCopies));
        let frontiers = RECORD_CLASSES_V1.map(|class| {
            if class == RecordClassV1::OperationalCopies {
                self.frontier
            } else {
                RetentionFrontierV1::new(class, 0, None).expect("empty class frontier is canonical")
            }
        });
        materialize_export_v1(
            request,
            ExportProfileV1::Administrative,
            scope,
            ContentDigestProfileV1::Sha256,
            self.source_engine_revision,
            self.policy_revision,
            included_classes,
            frontiers,
            std::slice::from_ref(&self.record),
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
            .field("source_engine_revision", &self.source_engine_revision)
            .field("policy_revision", &self.policy_revision)
            .field("record_id", &self.record.id)
            .field("checkpoint_byte_length", &self.record.payload().len())
            .field("checkpoint", &"<redacted>")
            .finish()
    }
}
