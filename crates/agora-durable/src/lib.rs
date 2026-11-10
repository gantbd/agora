//! Receipt-gated durability boundary for Agora Service V2.
//!
//! A semantic reply is publishable only after the complete service checkpoint
//! has a matching durable compare-and-exchange receipt. Known store refusal
//! restores the pre-offer checkpoint. An indeterminate or weak acknowledgement
//! fences the owner and suppresses the semantic reply.

use agora_host::v2::persistence::{
    HostCheckpointErrorV2, HostCheckpointScopeV1, HostRecoveryLimitsV2,
};
use agora_host::v2::{HostSessionIdV2, HostV2, OpenReplyV2};
use agora_host::{HostRefusal, HostTimeV1, PrincipalVerifier};
use agora_protocol::v2::{ClientIdV2, RefusalV2, ReplyV2, RequestV2};
use agora_service::v2::{
    ServiceV2,
    persistence::{ServiceCheckpointErrorV2, ServiceCheckpointScopeV1, ServiceRecoveryLimitsV2},
};
use keystone_storage::{
    AcknowledgementStrength, CheckpointStore, CommitOutcome, Revision, StoreRefusal,
};
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DurableStartErrorV2 {
    Missing,
    Checkpoint(ServiceCheckpointErrorV2),
    StoreFault,
    StoreRefused(StoreRefusal),
    OutcomeUnknown,
    NotDurable,
    InvalidReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DurableSuppressionV2 {
    Fenced,
    Checkpoint(ServiceCheckpointErrorV2),
    StoreRefused(StoreRefusal),
    OutcomeUnknown,
    NotDurable,
    InvalidReceipt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableEvidenceV2 {
    /// The exact semantic state was already represented by the current durable
    /// checkpoint, as for matching replay or a non-state-changing refusal.
    PreviouslyCommitted {
        revision: Revision,
    },
    Committed {
        revision: Revision,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DurablePublicationV2<T> {
    Published {
        value: T,
        evidence: DurableEvidenceV2,
    },
    Suppressed(DurableSuppressionV2),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableOwnerStateV2 {
    Active,
    Fenced,
}

#[derive(Clone, Eq, PartialEq)]
pub struct DurableConfigV2 {
    key: String,
    pub scope: ServiceCheckpointScopeV1,
    pub max_snapshot_bytes: usize,
    pub recovery: ServiceRecoveryLimitsV2,
}

impl DurableConfigV2 {
    pub fn new(
        key: impl Into<String>,
        scope: ServiceCheckpointScopeV1,
        max_snapshot_bytes: usize,
        recovery: ServiceRecoveryLimitsV2,
    ) -> Self {
        Self {
            key: key.into(),
            scope,
            max_snapshot_bytes,
            recovery,
        }
    }

    pub fn key(&self) -> &str {
        &self.key
    }
}

impl fmt::Debug for DurableConfigV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableConfigV2")
            .field("key_bytes", &self.key.len())
            .field("scope", &self.scope)
            .field("max_snapshot_bytes", &self.max_snapshot_bytes)
            .field("recovery", &self.recovery)
            .finish()
    }
}

pub struct DurableServiceV2<S> {
    service: ServiceV2,
    store: S,
    config: DurableConfigV2,
    store_revision: Revision,
    state: DurableOwnerStateV2,
}

impl<S> fmt::Debug for DurableServiceV2<S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableServiceV2")
            .field("config", &self.config)
            .field("store_revision", &self.store_revision)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl<S: CheckpointStore> DurableServiceV2<S> {
    pub fn initialize(
        service: ServiceV2,
        mut store: S,
        config: DurableConfigV2,
    ) -> Result<Self, DurableStartErrorV2> {
        let checkpoint = service
            .prepare_checkpoint(config.scope, config.max_snapshot_bytes)
            .map_err(DurableStartErrorV2::Checkpoint)?;
        let outcome = store
            .compare_exchange(config.key(), None, checkpoint.bytes())
            .map_err(|_| DurableStartErrorV2::StoreFault)?;
        let revision = match outcome {
            CommitOutcome::Committed(receipt)
                if receipt.revision == Revision(0)
                    && receipt.strength == AcknowledgementStrength::Durable =>
            {
                receipt.revision
            }
            CommitOutcome::Committed(receipt)
                if receipt.strength != AcknowledgementStrength::Durable =>
            {
                return Err(DurableStartErrorV2::NotDurable);
            }
            CommitOutcome::Committed(_) => return Err(DurableStartErrorV2::InvalidReceipt),
            CommitOutcome::Refused(refusal) => {
                return Err(DurableStartErrorV2::StoreRefused(refusal));
            }
            CommitOutcome::Indeterminate => return Err(DurableStartErrorV2::OutcomeUnknown),
        };
        Ok(Self {
            service,
            store,
            config,
            store_revision: revision,
            state: DurableOwnerStateV2::Active,
        })
    }

    pub fn recover(store: S, config: DurableConfigV2) -> Result<Self, DurableStartErrorV2> {
        let stored = store
            .load(config.key())
            .map_err(|_| DurableStartErrorV2::StoreFault)?
            .ok_or(DurableStartErrorV2::Missing)?;
        if stored.strength != AcknowledgementStrength::Durable {
            return Err(DurableStartErrorV2::NotDurable);
        }
        let service = ServiceV2::recover_checkpoint(&stored.bytes, config.scope, config.recovery)
            .map_err(DurableStartErrorV2::Checkpoint)?;
        Ok(Self {
            service,
            store,
            config,
            store_revision: stored.revision,
            state: DurableOwnerStateV2::Active,
        })
    }

    pub fn state(&self) -> DurableOwnerStateV2 {
        self.state
    }

    pub fn service(&self) -> Option<&ServiceV2> {
        (self.state == DurableOwnerStateV2::Active).then_some(&self.service)
    }

    pub fn store_revision(&self) -> Revision {
        self.store_revision
    }

    pub fn into_store(self) -> S {
        self.store
    }

    pub fn connect(
        &mut self,
        participant: &str,
    ) -> DurablePublicationV2<Result<ClientIdV2, RefusalV2>> {
        self.publish(|service| service.connect(participant))
    }

    pub fn disconnect(
        &mut self,
        client: ClientIdV2,
    ) -> DurablePublicationV2<Result<bool, RefusalV2>> {
        self.publish(|service| service.disconnect(client))
    }

    pub fn execute(
        &mut self,
        client: ClientIdV2,
        request: RequestV2,
    ) -> DurablePublicationV2<ReplyV2> {
        self.publish(|service| service.execute(client, request))
    }

    fn publish<T>(
        &mut self,
        operation: impl FnOnce(&mut ServiceV2) -> T,
    ) -> DurablePublicationV2<T> {
        if self.state == DurableOwnerStateV2::Fenced {
            return DurablePublicationV2::Suppressed(DurableSuppressionV2::Fenced);
        }
        let before = match self
            .service
            .prepare_checkpoint(self.config.scope, self.config.max_snapshot_bytes)
        {
            Ok(checkpoint) => checkpoint,
            Err(error) => {
                return DurablePublicationV2::Suppressed(DurableSuppressionV2::Checkpoint(error));
            }
        };
        let value = operation(&mut self.service);
        let candidate = match self
            .service
            .prepare_checkpoint(self.config.scope, self.config.max_snapshot_bytes)
        {
            Ok(checkpoint) => checkpoint,
            Err(error) => {
                if self.restore(before.bytes()).is_err() {
                    self.state = DurableOwnerStateV2::Fenced;
                }
                return DurablePublicationV2::Suppressed(DurableSuppressionV2::Checkpoint(error));
            }
        };
        if candidate.bytes() == before.bytes() {
            return DurablePublicationV2::Published {
                value,
                evidence: DurableEvidenceV2::PreviouslyCommitted {
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
                    self.state = DurableOwnerStateV2::Fenced;
                    return DurablePublicationV2::Suppressed(DurableSuppressionV2::OutcomeUnknown);
                }
            };
        match outcome {
            CommitOutcome::Committed(receipt) => {
                let Some(next) = expected.0.checked_add(1).map(Revision) else {
                    self.state = DurableOwnerStateV2::Fenced;
                    return DurablePublicationV2::Suppressed(DurableSuppressionV2::InvalidReceipt);
                };
                if receipt.revision != next {
                    self.state = DurableOwnerStateV2::Fenced;
                    return DurablePublicationV2::Suppressed(DurableSuppressionV2::InvalidReceipt);
                }
                if receipt.strength != AcknowledgementStrength::Durable {
                    self.state = DurableOwnerStateV2::Fenced;
                    return DurablePublicationV2::Suppressed(DurableSuppressionV2::NotDurable);
                }
                self.store_revision = receipt.revision;
                DurablePublicationV2::Published {
                    value,
                    evidence: DurableEvidenceV2::Committed {
                        revision: receipt.revision,
                    },
                }
            }
            CommitOutcome::Refused(refusal) => {
                if self.restore(before.bytes()).is_err() {
                    self.state = DurableOwnerStateV2::Fenced;
                    return DurablePublicationV2::Suppressed(DurableSuppressionV2::InvalidReceipt);
                }
                DurablePublicationV2::Suppressed(DurableSuppressionV2::StoreRefused(refusal))
            }
            CommitOutcome::Indeterminate => {
                self.state = DurableOwnerStateV2::Fenced;
                DurablePublicationV2::Suppressed(DurableSuppressionV2::OutcomeUnknown)
            }
        }
    }

    fn restore(&mut self, bytes: &[u8]) -> Result<(), ServiceCheckpointErrorV2> {
        self.service.restore_checkpoint_preserving_usage(
            bytes,
            self.config.scope,
            self.config.recovery,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DurableHostStartErrorV2 {
    Missing,
    Checkpoint(HostCheckpointErrorV2),
    StoreFault,
    StoreRefused(StoreRefusal),
    OutcomeUnknown,
    NotDurable,
    InvalidReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DurableHostSuppressionV2 {
    Fenced,
    Checkpoint(HostCheckpointErrorV2),
    StoreRefused(StoreRefusal),
    OutcomeUnknown,
    NotDurable,
    InvalidReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DurableHostPublicationV2<T> {
    Published {
        value: T,
        evidence: DurableEvidenceV2,
    },
    Suppressed(DurableHostSuppressionV2),
}

#[derive(Clone, Eq, PartialEq)]
pub struct DurableHostConfigV2 {
    key: String,
    pub scope: HostCheckpointScopeV1,
    pub max_snapshot_bytes: usize,
    pub recovery: HostRecoveryLimitsV2,
}

impl DurableHostConfigV2 {
    pub fn new(
        key: impl Into<String>,
        scope: HostCheckpointScopeV1,
        max_snapshot_bytes: usize,
        recovery: HostRecoveryLimitsV2,
    ) -> Self {
        Self {
            key: key.into(),
            scope,
            max_snapshot_bytes,
            recovery,
        }
    }

    pub fn key(&self) -> &str {
        &self.key
    }
}

impl fmt::Debug for DurableHostConfigV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableHostConfigV2")
            .field("key_bytes", &self.key.len())
            .field("scope", &self.scope)
            .field("max_snapshot_bytes", &self.max_snapshot_bytes)
            .field("recovery", &self.recovery)
            .finish()
    }
}

pub struct DurableHostV2<V, S> {
    host: HostV2<V>,
    store: S,
    config: DurableHostConfigV2,
    store_revision: Revision,
    state: DurableOwnerStateV2,
}

impl<V, S> fmt::Debug for DurableHostV2<V, S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableHostV2")
            .field("config", &self.config)
            .field("store_revision", &self.store_revision)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl<V: PrincipalVerifier, S: CheckpointStore> DurableHostV2<V, S> {
    pub fn initialize(
        host: HostV2<V>,
        mut store: S,
        config: DurableHostConfigV2,
    ) -> Result<Self, DurableHostStartErrorV2> {
        let checkpoint = host
            .prepare_checkpoint(config.scope, config.max_snapshot_bytes)
            .map_err(DurableHostStartErrorV2::Checkpoint)?;
        let outcome = store
            .compare_exchange(config.key(), None, checkpoint.bytes())
            .map_err(|_| DurableHostStartErrorV2::StoreFault)?;
        let revision = match outcome {
            CommitOutcome::Committed(receipt)
                if receipt.revision == Revision(0)
                    && receipt.strength == AcknowledgementStrength::Durable =>
            {
                receipt.revision
            }
            CommitOutcome::Committed(receipt)
                if receipt.strength != AcknowledgementStrength::Durable =>
            {
                return Err(DurableHostStartErrorV2::NotDurable);
            }
            CommitOutcome::Committed(_) => return Err(DurableHostStartErrorV2::InvalidReceipt),
            CommitOutcome::Refused(refusal) => {
                return Err(DurableHostStartErrorV2::StoreRefused(refusal));
            }
            CommitOutcome::Indeterminate => return Err(DurableHostStartErrorV2::OutcomeUnknown),
        };
        Ok(Self {
            host,
            store,
            config,
            store_revision: revision,
            state: DurableOwnerStateV2::Active,
        })
    }

    pub fn recover(
        store: S,
        config: DurableHostConfigV2,
        verifier: V,
    ) -> Result<Self, DurableHostStartErrorV2> {
        let stored = store
            .load(config.key())
            .map_err(|_| DurableHostStartErrorV2::StoreFault)?
            .ok_or(DurableHostStartErrorV2::Missing)?;
        if stored.strength != AcknowledgementStrength::Durable {
            return Err(DurableHostStartErrorV2::NotDurable);
        }
        let host =
            HostV2::recover_checkpoint(&stored.bytes, config.scope, config.recovery, verifier)
                .map_err(DurableHostStartErrorV2::Checkpoint)?;
        Ok(Self {
            host,
            store,
            config,
            store_revision: stored.revision,
            state: DurableOwnerStateV2::Active,
        })
    }

    pub fn state(&self) -> DurableOwnerStateV2 {
        self.state
    }

    pub fn host(&self) -> Option<&HostV2<V>> {
        (self.state == DurableOwnerStateV2::Active).then_some(&self.host)
    }

    pub fn into_store(self) -> S {
        self.store
    }

    pub fn open(
        &mut self,
        frame: &[u8],
        context: &V::Context,
        now: HostTimeV1,
    ) -> DurableHostPublicationV2<Result<OpenReplyV2, HostRefusal>> {
        self.publish(|host| host.open(frame, context, now))
    }

    pub fn handle(
        &mut self,
        session: HostSessionIdV2,
        frame: &[u8],
        context: &V::Context,
        now: HostTimeV1,
    ) -> DurableHostPublicationV2<Result<Vec<u8>, HostRefusal>> {
        self.publish(|host| host.handle(session, frame, context, now))
    }

    pub fn close(&mut self, session: HostSessionIdV2) -> DurableHostPublicationV2<bool> {
        self.publish(|host| host.close(session))
    }

    fn publish<T>(
        &mut self,
        operation: impl FnOnce(&mut HostV2<V>) -> T,
    ) -> DurableHostPublicationV2<T> {
        if self.state == DurableOwnerStateV2::Fenced {
            return DurableHostPublicationV2::Suppressed(DurableHostSuppressionV2::Fenced);
        }
        let before = match self
            .host
            .prepare_checkpoint(self.config.scope, self.config.max_snapshot_bytes)
        {
            Ok(checkpoint) => checkpoint,
            Err(error) => {
                return DurableHostPublicationV2::Suppressed(DurableHostSuppressionV2::Checkpoint(
                    error,
                ));
            }
        };
        let value = operation(&mut self.host);
        let candidate = match self
            .host
            .prepare_checkpoint(self.config.scope, self.config.max_snapshot_bytes)
        {
            Ok(checkpoint) => checkpoint,
            Err(error) => {
                if self.restore(before.bytes()).is_err() {
                    self.state = DurableOwnerStateV2::Fenced;
                }
                return DurableHostPublicationV2::Suppressed(DurableHostSuppressionV2::Checkpoint(
                    error,
                ));
            }
        };
        if candidate.bytes() == before.bytes() {
            return DurableHostPublicationV2::Published {
                value,
                evidence: DurableEvidenceV2::PreviouslyCommitted {
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
                    self.state = DurableOwnerStateV2::Fenced;
                    return DurableHostPublicationV2::Suppressed(
                        DurableHostSuppressionV2::OutcomeUnknown,
                    );
                }
            };
        match outcome {
            CommitOutcome::Committed(receipt) => {
                let Some(next) = expected.0.checked_add(1).map(Revision) else {
                    self.state = DurableOwnerStateV2::Fenced;
                    return DurableHostPublicationV2::Suppressed(
                        DurableHostSuppressionV2::InvalidReceipt,
                    );
                };
                if receipt.revision != next {
                    self.state = DurableOwnerStateV2::Fenced;
                    return DurableHostPublicationV2::Suppressed(
                        DurableHostSuppressionV2::InvalidReceipt,
                    );
                }
                if receipt.strength != AcknowledgementStrength::Durable {
                    self.state = DurableOwnerStateV2::Fenced;
                    return DurableHostPublicationV2::Suppressed(
                        DurableHostSuppressionV2::NotDurable,
                    );
                }
                self.store_revision = receipt.revision;
                DurableHostPublicationV2::Published {
                    value,
                    evidence: DurableEvidenceV2::Committed {
                        revision: receipt.revision,
                    },
                }
            }
            CommitOutcome::Refused(refusal) => {
                if self.restore(before.bytes()).is_err() {
                    self.state = DurableOwnerStateV2::Fenced;
                    return DurableHostPublicationV2::Suppressed(
                        DurableHostSuppressionV2::InvalidReceipt,
                    );
                }
                DurableHostPublicationV2::Suppressed(DurableHostSuppressionV2::StoreRefused(
                    refusal,
                ))
            }
            CommitOutcome::Indeterminate => {
                self.state = DurableOwnerStateV2::Fenced;
                DurableHostPublicationV2::Suppressed(DurableHostSuppressionV2::OutcomeUnknown)
            }
        }
    }

    fn restore(&mut self, bytes: &[u8]) -> Result<(), HostCheckpointErrorV2> {
        self.host.restore_checkpoint_preserving_runtime(
            bytes,
            self.config.scope,
            self.config.recovery,
        )
    }
}
