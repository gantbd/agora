//! Durable commit gate for the independent Agora export-custody owner.
//!
//! This package gives export custody its own store key and revision line. It
//! does not share the conversation checkpoint, project Agora source records,
//! authorize export, implement SHA-256, or expose participant protocol.

use agora_engine::retention::{
    ContentDigestV1, ExportMaterializationApplyV1, ExportMaterializationV1,
};
use agora_export_service::{
    ExportAdmissionV1, ExportCheckpointErrorV1, ExportCheckpointScopeV1, ExportRecoveryLimitsV1,
    ExportServiceRefusalV1, ExportServiceV1,
};
use keystone_storage::{
    AcknowledgementStrength, CheckpointStore, CommitOutcome, Revision, StoreRefusal,
};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableExportStateV1 {
    Active,
    Fenced,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DurableExportStartErrorV1 {
    Missing,
    Checkpoint(ExportCheckpointErrorV1),
    StoreFault,
    StoreRefused(StoreRefusal),
    OutcomeUnknown,
    NotDurable,
    InvalidReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DurableExportSuppressionV1 {
    Fenced,
    Checkpoint(ExportCheckpointErrorV1),
    StoreRefused(StoreRefusal),
    OutcomeUnknown,
    NotDurable,
    InvalidReceipt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableExportEvidenceV1 {
    PreviouslyCommitted { revision: Revision },
    Committed { revision: Revision },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DurableExportPublicationV1<T> {
    Published {
        value: T,
        evidence: DurableExportEvidenceV1,
    },
    Suppressed(DurableExportSuppressionV1),
}

#[derive(Clone, Eq, PartialEq)]
pub struct DurableExportConfigV1 {
    key: String,
    pub scope: ExportCheckpointScopeV1,
    pub max_snapshot_bytes: usize,
    pub recovery: ExportRecoveryLimitsV1,
}

impl DurableExportConfigV1 {
    pub fn new(
        key: impl Into<String>,
        scope: ExportCheckpointScopeV1,
        max_snapshot_bytes: usize,
        recovery: ExportRecoveryLimitsV1,
    ) -> Option<Self> {
        let key = key.into();
        (!key.is_empty() && max_snapshot_bytes != 0).then_some(Self {
            key,
            scope,
            max_snapshot_bytes,
            recovery,
        })
    }

    pub fn key(&self) -> &str {
        &self.key
    }
}

impl fmt::Debug for DurableExportConfigV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableExportConfigV1")
            .field("key_bytes", &self.key.len())
            .field("scope", &self.scope)
            .field("max_snapshot_bytes", &self.max_snapshot_bytes)
            .field("recovery", &self.recovery)
            .finish()
    }
}

pub struct DurableExportServiceV1<S, D> {
    service: ExportServiceV1,
    store: S,
    digest: D,
    config: DurableExportConfigV1,
    store_revision: Revision,
    state: DurableExportStateV1,
}

impl<S, D> fmt::Debug for DurableExportServiceV1<S, D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableExportServiceV1")
            .field("config", &self.config)
            .field("store_revision", &self.store_revision)
            .field("state", &self.state)
            .field("store", &"<redacted>")
            .field("digest", &"<redacted>")
            .finish()
    }
}

impl<S, D> DurableExportServiceV1<S, D>
where
    S: CheckpointStore,
    D: FnMut(&[u8]) -> ContentDigestV1,
{
    pub fn initialize(
        service: ExportServiceV1,
        mut store: S,
        mut digest: D,
        config: DurableExportConfigV1,
    ) -> Result<Self, DurableExportStartErrorV1> {
        let checkpoint = service
            .prepare_checkpoint(config.scope, config.max_snapshot_bytes, &mut digest)
            .map_err(DurableExportStartErrorV1::Checkpoint)?;
        let outcome = store
            .compare_exchange(config.key(), None, checkpoint.bytes())
            .map_err(|_| DurableExportStartErrorV1::StoreFault)?;
        let revision = start_revision(outcome)?;
        Ok(Self {
            service,
            store,
            digest,
            config,
            store_revision: revision,
            state: DurableExportStateV1::Active,
        })
    }

    pub fn recover(
        store: S,
        mut digest: D,
        config: DurableExportConfigV1,
    ) -> Result<Self, DurableExportStartErrorV1> {
        let stored = store
            .load(config.key())
            .map_err(|_| DurableExportStartErrorV1::StoreFault)?
            .ok_or(DurableExportStartErrorV1::Missing)?;
        if stored.strength != AcknowledgementStrength::Durable {
            return Err(DurableExportStartErrorV1::NotDurable);
        }
        let service = ExportServiceV1::recover_checkpoint(
            &stored.bytes,
            config.scope,
            config.recovery,
            &mut digest,
        )
        .map_err(DurableExportStartErrorV1::Checkpoint)?;
        Ok(Self {
            service,
            store,
            digest,
            config,
            store_revision: stored.revision,
            state: DurableExportStateV1::Active,
        })
    }

    pub const fn state(&self) -> DurableExportStateV1 {
        self.state
    }

    pub fn service(&self) -> Option<&ExportServiceV1> {
        (self.state == DurableExportStateV1::Active).then_some(&self.service)
    }

    pub const fn store_revision(&self) -> Revision {
        self.store_revision
    }

    pub fn into_parts(self) -> (ExportServiceV1, S, D) {
        (self.service, self.store, self.digest)
    }

    pub fn admit(
        &mut self,
        admission: ExportAdmissionV1,
        materialization: ExportMaterializationV1,
    ) -> DurableExportPublicationV1<Result<ExportMaterializationApplyV1, ExportServiceRefusalV1>>
    {
        self.publish(|service| service.admit(admission, materialization))
    }

    fn publish<T>(
        &mut self,
        operation: impl FnOnce(&mut ExportServiceV1) -> T,
    ) -> DurableExportPublicationV1<T> {
        if self.state == DurableExportStateV1::Fenced {
            return DurableExportPublicationV1::Suppressed(DurableExportSuppressionV1::Fenced);
        }
        let before = match self.service.prepare_checkpoint(
            self.config.scope,
            self.config.max_snapshot_bytes,
            &mut self.digest,
        ) {
            Ok(checkpoint) => checkpoint,
            Err(error) => {
                return DurableExportPublicationV1::Suppressed(
                    DurableExportSuppressionV1::Checkpoint(error),
                );
            }
        };
        let value = operation(&mut self.service);
        let candidate = match self.service.prepare_checkpoint(
            self.config.scope,
            self.config.max_snapshot_bytes,
            &mut self.digest,
        ) {
            Ok(checkpoint) => checkpoint,
            Err(error) => {
                if self.restore(before.bytes()).is_err() {
                    self.state = DurableExportStateV1::Fenced;
                }
                return DurableExportPublicationV1::Suppressed(
                    DurableExportSuppressionV1::Checkpoint(error),
                );
            }
        };
        if candidate.bytes() == before.bytes() {
            return DurableExportPublicationV1::Published {
                value,
                evidence: DurableExportEvidenceV1::PreviouslyCommitted {
                    revision: self.store_revision,
                },
            };
        }
        let expected = self.store_revision;
        let outcome =
            match self
                .store
                .compare_exchange(self.config.key(), Some(expected), candidate.bytes())
            {
                Ok(outcome) => outcome,
                Err(_) => {
                    self.state = DurableExportStateV1::Fenced;
                    return DurableExportPublicationV1::Suppressed(
                        DurableExportSuppressionV1::OutcomeUnknown,
                    );
                }
            };
        match outcome {
            CommitOutcome::Committed(receipt) => {
                let Some(next) = expected.0.checked_add(1).map(Revision) else {
                    self.state = DurableExportStateV1::Fenced;
                    return DurableExportPublicationV1::Suppressed(
                        DurableExportSuppressionV1::InvalidReceipt,
                    );
                };
                if receipt.revision != next {
                    self.state = DurableExportStateV1::Fenced;
                    return DurableExportPublicationV1::Suppressed(
                        DurableExportSuppressionV1::InvalidReceipt,
                    );
                }
                if receipt.strength != AcknowledgementStrength::Durable {
                    self.state = DurableExportStateV1::Fenced;
                    return DurableExportPublicationV1::Suppressed(
                        DurableExportSuppressionV1::NotDurable,
                    );
                }
                self.store_revision = receipt.revision;
                DurableExportPublicationV1::Published {
                    value,
                    evidence: DurableExportEvidenceV1::Committed {
                        revision: receipt.revision,
                    },
                }
            }
            CommitOutcome::Refused(refusal) => {
                if self.restore(before.bytes()).is_err() {
                    self.state = DurableExportStateV1::Fenced;
                    return DurableExportPublicationV1::Suppressed(
                        DurableExportSuppressionV1::InvalidReceipt,
                    );
                }
                DurableExportPublicationV1::Suppressed(DurableExportSuppressionV1::StoreRefused(
                    refusal,
                ))
            }
            CommitOutcome::Indeterminate => {
                self.state = DurableExportStateV1::Fenced;
                DurableExportPublicationV1::Suppressed(DurableExportSuppressionV1::OutcomeUnknown)
            }
        }
    }

    fn restore(&mut self, bytes: &[u8]) -> Result<(), ExportCheckpointErrorV1> {
        self.service = ExportServiceV1::recover_checkpoint(
            bytes,
            self.config.scope,
            self.config.recovery,
            &mut self.digest,
        )?;
        Ok(())
    }
}

fn start_revision(outcome: CommitOutcome) -> Result<Revision, DurableExportStartErrorV1> {
    match outcome {
        CommitOutcome::Committed(receipt)
            if receipt.revision == Revision(0)
                && receipt.strength == AcknowledgementStrength::Durable =>
        {
            Ok(receipt.revision)
        }
        CommitOutcome::Committed(receipt)
            if receipt.strength != AcknowledgementStrength::Durable =>
        {
            Err(DurableExportStartErrorV1::NotDurable)
        }
        CommitOutcome::Committed(_) => Err(DurableExportStartErrorV1::InvalidReceipt),
        CommitOutcome::Refused(refusal) => Err(DurableExportStartErrorV1::StoreRefused(refusal)),
        CommitOutcome::Indeterminate => Err(DurableExportStartErrorV1::OutcomeUnknown),
    }
}
