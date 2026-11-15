//! Owner-native exact-export sources for authoritative engine state.
//!
//! These projections perform no authorization, custody, or I/O. They expose
//! only records already owned by the engine and bind one current retention
//! policy and engine revision for later composition.

use crate::Engine;
use crate::persistence::{
    CheckpointError, prepare_authoritative_change_checkpoint_v1,
    prepare_private_participant_state_checkpoint_v1, prepare_room_projection_checkpoint_v1,
};
use crate::retention::{
    ContentDigestProfileV1, ContentDigestV1, ExportMaterializationRefusalV1,
    ExportMaterializationV1, ExportProfileV1, ExportRecordV1, ExportRequestIdV1,
    ExportScopeBindingV1, ExportSourceFragmentV1, RecordClassV1, RetentionFrontierV1,
    RetentionPolicyV1, RetentionRecordIdV1, materialize_export_sources_v1,
};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthoritativeChangeSourceRefusalV1 {
    InvalidLimit,
    StalePolicy,
    ClassDisabled,
    NoRetainedChanges,
    RecordLimit,
    IdentityExhausted,
    Checkpoint(CheckpointError),
    NonCanonical,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoomProjectionSourceRefusalV1 {
    InvalidLimit,
    StalePolicy,
    ClassDisabled,
    IdentityExhausted,
    Checkpoint(CheckpointError),
    NonCanonical,
}

/// One complete current public-room generation from one borrowed engine
/// revision. Individual ballots and every other participant-private projection
/// remain outside this source.
#[derive(Clone, Eq, PartialEq)]
pub struct RoomProjectionSourceV1 {
    fragment: ExportSourceFragmentV1,
    max_record_bytes: usize,
}

impl RoomProjectionSourceV1 {
    pub fn capture(
        engine: &Engine,
        policy: RetentionPolicyV1,
        max_record_bytes: usize,
    ) -> Result<Self, RoomProjectionSourceRefusalV1> {
        if max_record_bytes == 0 {
            return Err(RoomProjectionSourceRefusalV1::InvalidLimit);
        }
        let source_engine_revision = engine.revision().0;
        if policy.effective_engine_revision != source_engine_revision {
            return Err(RoomProjectionSourceRefusalV1::StalePolicy);
        }
        if !policy
            .bound(RecordClassV1::RoomProjection)
            .retains_records()
        {
            return Err(RoomProjectionSourceRefusalV1::ClassDisabled);
        }
        let ordinal = source_engine_revision
            .checked_add(1)
            .ok_or(RoomProjectionSourceRefusalV1::IdentityExhausted)?;
        let checkpoint = prepare_room_projection_checkpoint_v1(engine, max_record_bytes)
            .map_err(RoomProjectionSourceRefusalV1::Checkpoint)?;
        let id = RetentionRecordIdV1::new(RecordClassV1::RoomProjection, ordinal)
            .ok_or(RoomProjectionSourceRefusalV1::IdentityExhausted)?;
        let record = ExportRecordV1::new(id, checkpoint.bytes().to_vec(), max_record_bytes)
            .ok_or(RoomProjectionSourceRefusalV1::InvalidLimit)?;
        let frontier =
            RetentionFrontierV1::new(RecordClassV1::RoomProjection, ordinal - 1, Some(ordinal))
                .ok_or(RoomProjectionSourceRefusalV1::NonCanonical)?;
        let fragment = ExportSourceFragmentV1::new(
            source_engine_revision,
            policy.revision,
            RecordClassV1::RoomProjection,
            frontier,
            vec![record],
            1,
            max_record_bytes,
        )
        .map_err(|_| RoomProjectionSourceRefusalV1::NonCanonical)?;
        Ok(Self {
            fragment,
            max_record_bytes,
        })
    }

    pub fn fragment(&self) -> &ExportSourceFragmentV1 {
        &self.fragment
    }

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

impl fmt::Debug for RoomProjectionSourceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RoomProjectionSourceV1")
            .field(
                "source_engine_revision",
                &self.fragment.source_engine_revision(),
            )
            .field("policy_revision", &self.fragment.policy_revision())
            .field("frontier", &self.fragment.frontier())
            .field("record_count", &self.fragment.records().len())
            .field("records", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrivateParticipantStateSourceRefusalV1 {
    InvalidLimit,
    StalePolicy,
    ClassDisabled,
    LegacyIdentityUnavailable,
    IdentityExhausted,
    Checkpoint(CheckpointError),
    NonCanonical,
}

/// One complete Engine-owned participant-private generation. This source is
/// administrative-only; participant-scoped visibility remains an enclosing
/// authorization and materialization concern.
#[derive(Clone, Eq, PartialEq)]
pub struct PrivateParticipantStateSourceV1 {
    fragment: ExportSourceFragmentV1,
    max_record_bytes: usize,
}

impl PrivateParticipantStateSourceV1 {
    pub fn capture(
        engine: &Engine,
        policy: RetentionPolicyV1,
        max_record_bytes: usize,
    ) -> Result<Self, PrivateParticipantStateSourceRefusalV1> {
        if max_record_bytes == 0 {
            return Err(PrivateParticipantStateSourceRefusalV1::InvalidLimit);
        }
        let source_engine_revision = engine.revision().0;
        if policy.effective_engine_revision != source_engine_revision {
            return Err(PrivateParticipantStateSourceRefusalV1::StalePolicy);
        }
        if !policy
            .bound(RecordClassV1::PrivateParticipantState)
            .retains_records()
        {
            return Err(PrivateParticipantStateSourceRefusalV1::ClassDisabled);
        }
        let ordinal = engine
            .private_state_source_revision()
            .ok_or(PrivateParticipantStateSourceRefusalV1::LegacyIdentityUnavailable)?;
        if engine.private_state_source_exhausted() {
            return Err(PrivateParticipantStateSourceRefusalV1::IdentityExhausted);
        }
        let checkpoint = prepare_private_participant_state_checkpoint_v1(engine, max_record_bytes)
            .map_err(PrivateParticipantStateSourceRefusalV1::Checkpoint)?;
        let id = RetentionRecordIdV1::new(RecordClassV1::PrivateParticipantState, ordinal)
            .ok_or(PrivateParticipantStateSourceRefusalV1::IdentityExhausted)?;
        let record = ExportRecordV1::new(id, checkpoint.bytes().to_vec(), max_record_bytes)
            .ok_or(PrivateParticipantStateSourceRefusalV1::InvalidLimit)?;
        let frontier = RetentionFrontierV1::new(
            RecordClassV1::PrivateParticipantState,
            ordinal - 1,
            Some(ordinal),
        )
        .ok_or(PrivateParticipantStateSourceRefusalV1::NonCanonical)?;
        let fragment = ExportSourceFragmentV1::new(
            source_engine_revision,
            policy.revision,
            RecordClassV1::PrivateParticipantState,
            frontier,
            vec![record],
            1,
            max_record_bytes,
        )
        .map_err(|_| PrivateParticipantStateSourceRefusalV1::NonCanonical)?;
        Ok(Self {
            fragment,
            max_record_bytes,
        })
    }

    pub fn fragment(&self) -> &ExportSourceFragmentV1 {
        &self.fragment
    }

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

impl fmt::Debug for PrivateParticipantStateSourceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateParticipantStateSourceV1")
            .field(
                "source_engine_revision",
                &self.fragment.source_engine_revision(),
            )
            .field("policy_revision", &self.fragment.policy_revision())
            .field("frontier", &self.fragment.frontier())
            .field("record_count", &self.fragment.records().len())
            .field("records", &"<redacted>")
            .finish()
    }
}

/// Exact retained authoritative changes from one borrowed engine revision.
#[derive(Clone, Eq, PartialEq)]
pub struct AuthoritativeChangeSourceV1 {
    fragment: ExportSourceFragmentV1,
    max_records: usize,
    max_record_bytes: usize,
}

impl AuthoritativeChangeSourceV1 {
    pub fn capture(
        engine: &Engine,
        policy: RetentionPolicyV1,
        max_records: usize,
        max_record_bytes: usize,
    ) -> Result<Self, AuthoritativeChangeSourceRefusalV1> {
        if max_records == 0 || max_record_bytes == 0 {
            return Err(AuthoritativeChangeSourceRefusalV1::InvalidLimit);
        }
        let source_engine_revision = engine.revision().0;
        if policy.effective_engine_revision != source_engine_revision {
            return Err(AuthoritativeChangeSourceRefusalV1::StalePolicy);
        }
        if !policy
            .bound(RecordClassV1::AuthoritativeChanges)
            .retains_records()
        {
            return Err(AuthoritativeChangeSourceRefusalV1::ClassDisabled);
        }
        let retained_count = engine.retained_changes().len();
        if retained_count == 0 {
            return Err(AuthoritativeChangeSourceRefusalV1::NoRetainedChanges);
        }
        if retained_count > max_records {
            return Err(AuthoritativeChangeSourceRefusalV1::RecordLimit);
        }

        let mut records = Vec::new();
        records
            .try_reserve_exact(retained_count)
            .map_err(|_| AuthoritativeChangeSourceRefusalV1::RecordLimit)?;
        for envelope in engine.retained_changes() {
            let ordinal = envelope
                .sequence
                .0
                .checked_add(1)
                .ok_or(AuthoritativeChangeSourceRefusalV1::IdentityExhausted)?;
            let checkpoint = prepare_authoritative_change_checkpoint_v1(envelope, max_record_bytes)
                .map_err(AuthoritativeChangeSourceRefusalV1::Checkpoint)?;
            let id = RetentionRecordIdV1::new(RecordClassV1::AuthoritativeChanges, ordinal)
                .ok_or(AuthoritativeChangeSourceRefusalV1::IdentityExhausted)?;
            records.push(
                ExportRecordV1::new(id, checkpoint.bytes().to_vec(), max_record_bytes)
                    .ok_or(AuthoritativeChangeSourceRefusalV1::RecordLimit)?,
            );
        }

        let discarded_through = match engine.discarded_changes_through() {
            Some(sequence) => sequence
                .0
                .checked_add(1)
                .ok_or(AuthoritativeChangeSourceRefusalV1::IdentityExhausted)?,
            None => 0,
        };
        let oldest_retained = records.first().map(|record| record.id.ordinal);
        let frontier = RetentionFrontierV1::new(
            RecordClassV1::AuthoritativeChanges,
            discarded_through,
            oldest_retained,
        )
        .ok_or(AuthoritativeChangeSourceRefusalV1::NonCanonical)?;
        let fragment = ExportSourceFragmentV1::new(
            source_engine_revision,
            policy.revision,
            RecordClassV1::AuthoritativeChanges,
            frontier,
            records,
            max_records,
            max_record_bytes,
        )
        .map_err(|_| AuthoritativeChangeSourceRefusalV1::NonCanonical)?;
        Ok(Self {
            fragment,
            max_records,
            max_record_bytes,
        })
    }

    pub fn fragment(&self) -> &ExportSourceFragmentV1 {
        &self.fragment
    }

    /// Materializes this exact retained-change source under the administrative
    /// profile. Authority and custody remain enclosing obligations.
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
            self.max_records,
            self.max_record_bytes,
            max_chunks,
            max_chunk_bytes,
            digest,
        )
    }
}

impl fmt::Debug for AuthoritativeChangeSourceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthoritativeChangeSourceV1")
            .field(
                "source_engine_revision",
                &self.fragment.source_engine_revision(),
            )
            .field("policy_revision", &self.fragment.policy_revision())
            .field("frontier", &self.fragment.frontier())
            .field("record_count", &self.fragment.records().len())
            .field("records", &"<redacted>")
            .finish()
    }
}
