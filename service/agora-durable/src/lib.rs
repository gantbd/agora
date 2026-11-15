//! Receipt-gated durability boundary for Agora Service V2.
//!
//! A semantic reply is publishable only after the complete service checkpoint
//! has a matching durable compare-and-exchange receipt. Known store refusal
//! restores the pre-offer checkpoint. An indeterminate or weak acknowledgement
//! fences the owner and suppresses the semantic reply.

use agora_delivery_projection_port::{
    DeliveryProjectionRefusalV1, DeliveryProjectionV1, DeliverySourceV1,
};
use agora_federated_publication_port::{
    FederatedPublicationAcceptedV1, FederatedPublicationOutcomeV1, FederatedPublicationPortV1,
    FederatedPublicationRefusalV1, FederatedPublicationRequestV1,
};
use agora_host::transient::{TransientHostBuildRefusalV1, TransientHostV1};
use agora_host::v2::persistence::{
    HostCheckpointErrorV2, HostCheckpointScopeV1, HostRecoveryLimitsV2,
};
use agora_host::v2::{
    AuthoritativeChangeRetentionHostRefusalV1, HostSessionIdV2, HostV2, OpenReplyV2,
    ReceiptRetirementHostRefusalV1, RetentionPolicyHostRefusalV1,
};
use agora_host::{HostRefusal, HostTimeV1, PrincipalVerifier};
use agora_interaction_port::{
    InteractionContextRefusalV1, InteractionContextV1, InteractionPromotionAcceptedV1,
    InteractionPromotionRefusalV1, InteractionPromotionRequestV1, InteractionSourceV1,
};
use agora_protocol::v2::{ClientIdV2, RefusalV2, ReplyV2, RequestIdV2, RequestV2};
use agora_retention_authority_port::{
    AuthoritativeChangeRetentionApplicationV1, RetentionAuthorityPortV1, RetentionPolicyRevisionV1,
    RetentionPolicyV1,
};
use agora_scheduled_publication_port::{
    ScheduledPublicationAcceptedV1, ScheduledPublicationRefusalV1, ScheduledPublicationRequestV1,
};
use agora_service::transient::TransientLimitsV1;
use agora_service::typing::TypingLimitsV1;
use agora_service::v2::RetentionPolicyInstallV1;
use agora_service::v2::{
    ServiceV2,
    persistence::{ServiceCheckpointErrorV2, ServiceCheckpointScopeV1, ServiceRecoveryLimitsV2},
};
pub use agora_storage_observation::{ObservableCheckpointStoreV1, ObservationFidelityV1};
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

/// Passive content-free publication facts for an authenticated durable Host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DurableHostObservationV2 {
    pub state: DurableOwnerStateV2,
    pub store_revision: u64,
    pub offers: u64,
    pub committed: u64,
    pub previously_committed: u64,
    pub checkpoint_suppressed: u64,
    pub store_refused: u64,
    pub outcome_unknown: u64,
    pub not_durable: u64,
    pub invalid_receipt: u64,
    pub fenced: u64,
    pub saturated: bool,
}

/// Passive publication and physical-store facts copied at one call boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DurableStoreObservationV2<O> {
    pub durability: DurableHostObservationV2,
    pub physical: O,
    /// Sticky loss of counter fidelity in either represented owner.
    pub fidelity_lost: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DurableHostCountersV2 {
    offers: u64,
    committed: u64,
    previously_committed: u64,
    checkpoint_suppressed: u64,
    store_refused: u64,
    outcome_unknown: u64,
    not_durable: u64,
    invalid_receipt: u64,
    fenced: u64,
    saturated: bool,
}

impl DurableHostCountersV2 {
    fn record<T>(&mut self, publication: &DurableHostPublicationV2<T>) {
        increment_counter(&mut self.offers, &mut self.saturated);
        let counter = match publication {
            DurableHostPublicationV2::Published {
                evidence: DurableEvidenceV2::Committed { .. },
                ..
            } => &mut self.committed,
            DurableHostPublicationV2::Published {
                evidence: DurableEvidenceV2::PreviouslyCommitted { .. },
                ..
            } => &mut self.previously_committed,
            DurableHostPublicationV2::Suppressed(DurableHostSuppressionV2::Checkpoint(_)) => {
                &mut self.checkpoint_suppressed
            }
            DurableHostPublicationV2::Suppressed(DurableHostSuppressionV2::StoreRefused(_)) => {
                &mut self.store_refused
            }
            DurableHostPublicationV2::Suppressed(DurableHostSuppressionV2::OutcomeUnknown) => {
                &mut self.outcome_unknown
            }
            DurableHostPublicationV2::Suppressed(DurableHostSuppressionV2::NotDurable) => {
                &mut self.not_durable
            }
            DurableHostPublicationV2::Suppressed(DurableHostSuppressionV2::InvalidReceipt) => {
                &mut self.invalid_receipt
            }
            DurableHostPublicationV2::Suppressed(DurableHostSuppressionV2::Fenced) => {
                &mut self.fenced
            }
        };
        increment_counter(counter, &mut self.saturated);
    }
}

fn increment_counter(counter: &mut u64, saturated: &mut bool) {
    if *counter == u64::MAX {
        *saturated = true;
    } else {
        *counter += 1;
    }
}

fn host_observation(
    state: DurableOwnerStateV2,
    store_revision: Revision,
    counters: DurableHostCountersV2,
) -> DurableHostObservationV2 {
    DurableHostObservationV2 {
        state,
        store_revision: store_revision.0,
        offers: counters.offers,
        committed: counters.committed,
        previously_committed: counters.previously_committed,
        checkpoint_suppressed: counters.checkpoint_suppressed,
        store_refused: counters.store_refused,
        outcome_unknown: counters.outcome_unknown,
        not_durable: counters.not_durable,
        invalid_receipt: counters.invalid_receipt,
        fenced: counters.fenced,
        saturated: counters.saturated,
    }
}

fn durable_store_observation<S: ObservableCheckpointStoreV1>(
    durability: DurableHostObservationV2,
    store: &S,
) -> DurableStoreObservationV2<S::Observation> {
    let physical = store.physical_observation();
    DurableStoreObservationV2 {
        durability,
        physical,
        fidelity_lost: durability.saturated || physical.fidelity_lost(),
    }
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
    Unauthorized,
}

/// The exact durable object an operator asks to sanitize after an external
/// restore.
///
/// The store key may identify private deployment topology, so diagnostics
/// expose only its byte length. Authorization is bound to the revision that
/// the subsequent compare-and-exchange must replace.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct OperatorRestoreRequestV1<'a> {
    key: &'a str,
    scope: HostCheckpointScopeV1,
    revision: Revision,
}

impl<'a> OperatorRestoreRequestV1<'a> {
    pub const fn new(key: &'a str, scope: HostCheckpointScopeV1, revision: u64) -> Self {
        Self {
            key,
            scope,
            revision: Revision(revision),
        }
    }

    pub const fn key(&self) -> &'a str {
        self.key
    }

    pub const fn scope(&self) -> HostCheckpointScopeV1 {
        self.scope
    }

    pub const fn revision(&self) -> Revision {
        self.revision
    }
}

impl fmt::Debug for OperatorRestoreRequestV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OperatorRestoreRequestV1")
            .field("key_bytes", &self.key.len())
            .field("scope", &self.scope)
            .field("revision", &self.revision)
            .finish()
    }
}

/// Fail-closed authority port for the exceptional operator-restore path.
///
/// Ordinary crash recovery does not use this port. Implementations should
/// authorize the exact key, checkpoint scope, and loaded store revision in
/// `request`; any error is deliberately erased to `Unauthorized` at Agora's
/// durable boundary.
pub trait OperatorRestoreAuthorityV1 {
    type Context;
    type Error;

    fn authorize(
        &mut self,
        request: &OperatorRestoreRequestV1<'_>,
        context: &Self::Context,
    ) -> Result<(), Self::Error>;
}

/// A borrowed authority implementation and its matching trusted context for
/// one operator-restore attempt.
pub struct OperatorRestoreAuthorizationV1<'a, A: OperatorRestoreAuthorityV1> {
    authority: &'a mut A,
    context: &'a A::Context,
}

impl<'a, A: OperatorRestoreAuthorityV1> OperatorRestoreAuthorizationV1<'a, A> {
    pub const fn new(authority: &'a mut A, context: &'a A::Context) -> Self {
        Self { authority, context }
    }

    fn authorize(&mut self, request: &OperatorRestoreRequestV1<'_>) -> Result<(), A::Error> {
        self.authority.authorize(request, self.context)
    }
}

impl<A: OperatorRestoreAuthorityV1> fmt::Debug for OperatorRestoreAuthorizationV1<'_, A> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OperatorRestoreAuthorizationV1")
            .field("authority", &"<redacted>")
            .field("context", &"<redacted>")
            .finish()
    }
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
    counters: DurableHostCountersV2,
}

/// A configured federation publication endpoint over one durable Host owner.
///
/// The endpoint captures the trusted local session, verifier context, and
/// clock. Federation callers can offer only the bounded publication request;
/// they cannot choose local authority or time.
pub struct DurableFederatedPublicationEndpointV1<'a, V: PrincipalVerifier, S, C> {
    owner: &'a mut DurableHostV2<V, S>,
    session: HostSessionIdV2,
    context: &'a V::Context,
    clock: C,
}

impl<'a, V, S, C> DurableFederatedPublicationEndpointV1<'a, V, S, C>
where
    V: PrincipalVerifier,
    S: CheckpointStore,
    C: FnMut() -> HostTimeV1,
{
    pub fn new(
        owner: &'a mut DurableHostV2<V, S>,
        session: HostSessionIdV2,
        context: &'a V::Context,
        clock: C,
    ) -> Self {
        Self {
            owner,
            session,
            context,
            clock,
        }
    }
}

impl<V, S, C> fmt::Debug for DurableFederatedPublicationEndpointV1<'_, V, S, C>
where
    V: PrincipalVerifier,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableFederatedPublicationEndpointV1")
            .field("owner", &"<redacted>")
            .field("session", &"<redacted>")
            .field("context", &"<redacted>")
            .field("clock", &"<redacted>")
            .finish()
    }
}

impl<V, S, C> FederatedPublicationPortV1 for DurableFederatedPublicationEndpointV1<'_, V, S, C>
where
    V: PrincipalVerifier,
    S: CheckpointStore,
    C: FnMut() -> HostTimeV1,
{
    fn publish_federated(
        &mut self,
        request: FederatedPublicationRequestV1,
    ) -> FederatedPublicationOutcomeV1 {
        self.owner
            .publish_federated_outcome(self.session, request, self.context, (self.clock)())
    }
}

fn commit_host_restore_boundary<
    V: PrincipalVerifier,
    S: CheckpointStore,
    A: OperatorRestoreAuthorityV1,
>(
    mut store: S,
    config: &DurableHostConfigV2,
    verifier: V,
    authorization: &mut OperatorRestoreAuthorizationV1<'_, A>,
) -> Result<(HostV2<V>, S, Revision), DurableHostStartErrorV2> {
    let stored = store
        .load(config.key())
        .map_err(|_| DurableHostStartErrorV2::StoreFault)?
        .ok_or(DurableHostStartErrorV2::Missing)?;
    if stored.strength != AcknowledgementStrength::Durable {
        return Err(DurableHostStartErrorV2::NotDurable);
    }
    let next = stored
        .revision
        .0
        .checked_add(1)
        .map(Revision)
        .ok_or(DurableHostStartErrorV2::InvalidReceipt)?;
    let host = HostV2::recover_checkpoint_after_restore(
        &stored.bytes,
        config.scope,
        config.recovery,
        verifier,
    )
    .map_err(DurableHostStartErrorV2::Checkpoint)?;
    authorization
        .authorize(&OperatorRestoreRequestV1 {
            key: config.key(),
            scope: config.scope,
            revision: stored.revision,
        })
        .map_err(|_| DurableHostStartErrorV2::Unauthorized)?;
    let checkpoint = host
        .prepare_checkpoint(config.scope, config.max_snapshot_bytes)
        .map_err(DurableHostStartErrorV2::Checkpoint)?;
    let outcome = store
        .compare_exchange(config.key(), Some(stored.revision), checkpoint.bytes())
        .map_err(|_| DurableHostStartErrorV2::StoreFault)?;
    let revision = match outcome {
        CommitOutcome::Committed(receipt)
            if receipt.revision == next && receipt.strength == AcknowledgementStrength::Durable =>
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
    Ok((host, store, revision))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DurableTransientHostStartErrorV1 {
    Missing,
    Checkpoint(HostCheckpointErrorV2),
    Transient(TransientHostBuildRefusalV1),
    StoreFault,
    StoreRefused(StoreRefusal),
    OutcomeUnknown,
    NotDurable,
    InvalidReceipt,
    Unauthorized,
}

/// Receipt-gated durable Host plus deliberately non-durable collaboration.
///
/// Only the underlying Host checkpoint enters the store. Presence, typing,
/// transient cursors, replay receipts, and clock state always restart empty.
pub struct DurableTransientHostV1<V, S> {
    host: TransientHostV1<V>,
    store: S,
    config: DurableHostConfigV2,
    store_revision: Revision,
    state: DurableOwnerStateV2,
    counters: DurableHostCountersV2,
}

impl<V, S> fmt::Debug for DurableTransientHostV1<V, S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableTransientHostV1")
            .field("config", &self.config)
            .field("store_revision", &self.store_revision)
            .field("state", &self.state)
            .field("transient", &self.host)
            .finish()
    }
}

impl<V: PrincipalVerifier, S: CheckpointStore> DurableTransientHostV1<V, S> {
    pub fn initialize(
        host: TransientHostV1<V>,
        mut store: S,
        config: DurableHostConfigV2,
    ) -> Result<Self, DurableTransientHostStartErrorV1> {
        let checkpoint = host
            .prepare_host_checkpoint(config.scope, config.max_snapshot_bytes)
            .map_err(DurableTransientHostStartErrorV1::Checkpoint)?;
        let outcome = store
            .compare_exchange(config.key(), None, checkpoint.bytes())
            .map_err(|_| DurableTransientHostStartErrorV1::StoreFault)?;
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
                return Err(DurableTransientHostStartErrorV1::NotDurable);
            }
            CommitOutcome::Committed(_) => {
                return Err(DurableTransientHostStartErrorV1::InvalidReceipt);
            }
            CommitOutcome::Refused(refusal) => {
                return Err(DurableTransientHostStartErrorV1::StoreRefused(refusal));
            }
            CommitOutcome::Indeterminate => {
                return Err(DurableTransientHostStartErrorV1::OutcomeUnknown);
            }
        };
        Ok(Self {
            host,
            store,
            config,
            store_revision: revision,
            state: DurableOwnerStateV2::Active,
            counters: DurableHostCountersV2::default(),
        })
    }

    pub fn recover(
        store: S,
        config: DurableHostConfigV2,
        verifier: V,
        presence_limits: TransientLimitsV1,
        typing_limits: TypingLimitsV1,
        max_cursor_age: u64,
    ) -> Result<Self, DurableTransientHostStartErrorV1> {
        let stored = store
            .load(config.key())
            .map_err(|_| DurableTransientHostStartErrorV1::StoreFault)?
            .ok_or(DurableTransientHostStartErrorV1::Missing)?;
        if stored.strength != AcknowledgementStrength::Durable {
            return Err(DurableTransientHostStartErrorV1::NotDurable);
        }
        let host =
            HostV2::recover_checkpoint(&stored.bytes, config.scope, config.recovery, verifier)
                .map_err(DurableTransientHostStartErrorV1::Checkpoint)?;
        let host =
            TransientHostV1::new_with_typing(host, presence_limits, typing_limits, max_cursor_age)
                .map_err(DurableTransientHostStartErrorV1::Transient)?;
        Ok(Self {
            host,
            store,
            config,
            store_revision: stored.revision,
            state: DurableOwnerStateV2::Active,
            counters: DurableHostCountersV2::default(),
        })
    }

    /// Recovers an operator-restored checkpoint, durably invalidates recovered
    /// sessions and their private Service state, then starts transient state
    /// empty.
    pub fn recover_after_restore<A: OperatorRestoreAuthorityV1>(
        mut store: S,
        config: DurableHostConfigV2,
        verifier: V,
        mut authorization: OperatorRestoreAuthorizationV1<'_, A>,
        presence_limits: TransientLimitsV1,
        typing_limits: TypingLimitsV1,
        max_cursor_age: u64,
    ) -> Result<Self, DurableTransientHostStartErrorV1> {
        let stored = store
            .load(config.key())
            .map_err(|_| DurableTransientHostStartErrorV1::StoreFault)?
            .ok_or(DurableTransientHostStartErrorV1::Missing)?;
        if stored.strength != AcknowledgementStrength::Durable {
            return Err(DurableTransientHostStartErrorV1::NotDurable);
        }
        let next = stored
            .revision
            .0
            .checked_add(1)
            .map(Revision)
            .ok_or(DurableTransientHostStartErrorV1::InvalidReceipt)?;
        let host = HostV2::recover_checkpoint_after_restore(
            &stored.bytes,
            config.scope,
            config.recovery,
            verifier,
        )
        .map_err(DurableTransientHostStartErrorV1::Checkpoint)?;
        let host =
            TransientHostV1::new_with_typing(host, presence_limits, typing_limits, max_cursor_age)
                .map_err(DurableTransientHostStartErrorV1::Transient)?;
        authorization
            .authorize(&OperatorRestoreRequestV1 {
                key: config.key(),
                scope: config.scope,
                revision: stored.revision,
            })
            .map_err(|_| DurableTransientHostStartErrorV1::Unauthorized)?;
        let checkpoint = host
            .prepare_host_checkpoint(config.scope, config.max_snapshot_bytes)
            .map_err(DurableTransientHostStartErrorV1::Checkpoint)?;
        let outcome = store
            .compare_exchange(config.key(), Some(stored.revision), checkpoint.bytes())
            .map_err(|_| DurableTransientHostStartErrorV1::StoreFault)?;
        let store_revision = match outcome {
            CommitOutcome::Committed(receipt)
                if receipt.revision == next
                    && receipt.strength == AcknowledgementStrength::Durable =>
            {
                receipt.revision
            }
            CommitOutcome::Committed(receipt)
                if receipt.strength != AcknowledgementStrength::Durable =>
            {
                return Err(DurableTransientHostStartErrorV1::NotDurable);
            }
            CommitOutcome::Committed(_) => {
                return Err(DurableTransientHostStartErrorV1::InvalidReceipt);
            }
            CommitOutcome::Refused(refusal) => {
                return Err(DurableTransientHostStartErrorV1::StoreRefused(refusal));
            }
            CommitOutcome::Indeterminate => {
                return Err(DurableTransientHostStartErrorV1::OutcomeUnknown);
            }
        };
        Ok(Self {
            host,
            store,
            config,
            store_revision,
            state: DurableOwnerStateV2::Active,
            counters: DurableHostCountersV2::default(),
        })
    }

    pub const fn state(&self) -> DurableOwnerStateV2 {
        self.state
    }

    pub fn observation(&self) -> DurableHostObservationV2 {
        host_observation(self.state, self.store_revision, self.counters)
    }

    pub fn layered_observation(&self) -> DurableStoreObservationV2<S::Observation>
    where
        S: ObservableCheckpointStoreV1,
    {
        durable_store_observation(self.observation(), &self.store)
    }

    pub fn host(&self) -> Option<&TransientHostV1<V>> {
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
        let publication = self.publish(|host| host.open(frame, context, now));
        self.counters.record(&publication);
        publication
    }

    pub fn handle(
        &mut self,
        session: HostSessionIdV2,
        frame: &[u8],
        context: &V::Context,
        now: HostTimeV1,
    ) -> DurableHostPublicationV2<Result<Vec<u8>, HostRefusal>> {
        let publication = self.publish(|host| host.handle(session, frame, context, now));
        self.counters.record(&publication);
        if matches!(publication, DurableHostPublicationV2::Suppressed(_)) {
            self.host.retire_transient_session(session);
        }
        publication
    }

    pub fn handle_transient(
        &mut self,
        session: HostSessionIdV2,
        frame: &[u8],
        context: &V::Context,
        now: HostTimeV1,
    ) -> DurableHostPublicationV2<Result<Vec<u8>, HostRefusal>> {
        let publication = self.publish(|host| host.handle_transient(session, frame, context, now));
        self.counters.record(&publication);
        if matches!(publication, DurableHostPublicationV2::Suppressed(_)) {
            self.host.retire_transient_session(session);
        }
        publication
    }

    pub fn close(&mut self, session: HostSessionIdV2) -> DurableHostPublicationV2<bool> {
        let publication = self.publish(|host| host.close(session));
        self.counters.record(&publication);
        if matches!(publication, DurableHostPublicationV2::Suppressed(_)) {
            self.host.retire_transient_session(session);
        }
        publication
    }

    fn publish<T>(
        &mut self,
        operation: impl FnOnce(&mut TransientHostV1<V>) -> T,
    ) -> DurableHostPublicationV2<T> {
        if self.state == DurableOwnerStateV2::Fenced {
            return DurableHostPublicationV2::Suppressed(DurableHostSuppressionV2::Fenced);
        }
        let before = match self
            .host
            .prepare_host_checkpoint(self.config.scope, self.config.max_snapshot_bytes)
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
            .prepare_host_checkpoint(self.config.scope, self.config.max_snapshot_bytes)
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
        self.host.restore_host_checkpoint_preserving_runtime(
            bytes,
            self.config.scope,
            self.config.recovery,
        )
    }
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
            counters: DurableHostCountersV2::default(),
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
            counters: DurableHostCountersV2::default(),
        })
    }

    /// Recovers an operator-restored checkpoint only after durably committing
    /// invalidation of every recovered Host session and its private Service
    /// client state.
    pub fn recover_after_restore<A: OperatorRestoreAuthorityV1>(
        store: S,
        config: DurableHostConfigV2,
        verifier: V,
        mut authorization: OperatorRestoreAuthorizationV1<'_, A>,
    ) -> Result<Self, DurableHostStartErrorV2> {
        let (host, store, store_revision) =
            commit_host_restore_boundary(store, &config, verifier, &mut authorization)?;
        Ok(Self {
            host,
            store,
            config,
            store_revision,
            state: DurableOwnerStateV2::Active,
            counters: DurableHostCountersV2::default(),
        })
    }

    pub fn state(&self) -> DurableOwnerStateV2 {
        self.state
    }

    pub fn observation(&self) -> DurableHostObservationV2 {
        host_observation(self.state, self.store_revision, self.counters)
    }

    pub fn layered_observation(&self) -> DurableStoreObservationV2<S::Observation>
    where
        S: ObservableCheckpointStoreV1,
    {
        durable_store_observation(self.observation(), &self.store)
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
        let publication = self.publish(|host| host.open(frame, context, now));
        self.counters.record(&publication);
        publication
    }

    pub fn handle(
        &mut self,
        session: HostSessionIdV2,
        frame: &[u8],
        context: &V::Context,
        now: HostTimeV1,
    ) -> DurableHostPublicationV2<Result<Vec<u8>, HostRefusal>> {
        let publication = self.publish(|host| host.handle(session, frame, context, now));
        self.counters.record(&publication);
        publication
    }

    /// Offers one actor-bound scheduled publication and publishes its semantic
    /// result only after the resulting Host checkpoint is durable.
    pub fn publish_scheduled(
        &mut self,
        session: HostSessionIdV2,
        request: ScheduledPublicationRequestV1,
        context: &V::Context,
        now: HostTimeV1,
    ) -> DurableHostPublicationV2<
        Result<ScheduledPublicationAcceptedV1, ScheduledPublicationRefusalV1>,
    > {
        let publication =
            self.publish(|host| host.publish_scheduled(session, request, context, now));
        self.counters.record(&publication);
        publication
    }

    /// Offers one actor-bound federated publication and publishes its semantic
    /// result only after the resulting Host checkpoint is durable.
    pub fn publish_federated(
        &mut self,
        session: HostSessionIdV2,
        request: FederatedPublicationRequestV1,
        context: &V::Context,
        now: HostTimeV1,
    ) -> DurableHostPublicationV2<
        Result<FederatedPublicationAcceptedV1, FederatedPublicationRefusalV1>,
    > {
        let publication =
            self.publish(|host| host.publish_federated(session, request, context, now));
        self.counters.record(&publication);
        publication
    }

    /// Offers one federated publication through the durable Host and exposes
    /// only the retry certainty that the commit boundary can prove.
    ///
    /// A known store refusal is definite non-execution only because the Host
    /// checkpoint was restored successfully. Every other suppressed result is
    /// conservative: a caller must reconcile the same scoped request identity
    /// rather than assume a retry is a fresh execution.
    pub fn publish_federated_outcome(
        &mut self,
        session: HostSessionIdV2,
        request: FederatedPublicationRequestV1,
        context: &V::Context,
        now: HostTimeV1,
    ) -> FederatedPublicationOutcomeV1 {
        match self.publish_federated(session, request, context, now) {
            DurableHostPublicationV2::Published { value, .. } => {
                FederatedPublicationOutcomeV1::Reply(value)
            }
            DurableHostPublicationV2::Suppressed(DurableHostSuppressionV2::StoreRefused(_)) => {
                FederatedPublicationOutcomeV1::DefinitelyNotExecuted
            }
            DurableHostPublicationV2::Suppressed(
                DurableHostSuppressionV2::Fenced
                | DurableHostSuppressionV2::Checkpoint(_)
                | DurableHostSuppressionV2::OutcomeUnknown
                | DurableHostSuppressionV2::NotDurable
                | DurableHostSuppressionV2::InvalidReceipt,
            ) => FederatedPublicationOutcomeV1::OutcomeUnknown,
        }
    }

    /// Revalidates and projects through the durable Host owner. A projection
    /// escapes only after any session-verification state change is committed.
    pub fn project_delivery(
        &mut self,
        session: HostSessionIdV2,
        source: DeliverySourceV1,
        context: &V::Context,
        now: HostTimeV1,
    ) -> DurableHostPublicationV2<Result<DeliveryProjectionV1, DeliveryProjectionRefusalV1>> {
        let publication = self.publish(|host| host.project_delivery(session, source, context, now));
        self.counters.record(&publication);
        publication
    }

    /// Receipt-gates Host revalidation before interaction context can escape.
    pub fn interaction_context(
        &mut self,
        session: HostSessionIdV2,
        source: InteractionSourceV1,
        context: &V::Context,
        now: HostTimeV1,
    ) -> DurableHostPublicationV2<Result<InteractionContextV1, InteractionContextRefusalV1>> {
        let publication =
            self.publish(|host| host.interaction_context(session, source, context, now));
        self.counters.record(&publication);
        publication
    }

    /// Publishes a proposal only after session verification and ordinary
    /// message admission are committed in one complete Host checkpoint.
    pub fn promote_interaction(
        &mut self,
        session: HostSessionIdV2,
        request: InteractionPromotionRequestV1,
        context: &V::Context,
        now: HostTimeV1,
    ) -> DurableHostPublicationV2<
        Result<InteractionPromotionAcceptedV1, InteractionPromotionRefusalV1>,
    > {
        let publication =
            self.publish(|host| host.promote_interaction(session, request, context, now));
        self.counters.record(&publication);
        publication
    }

    /// Publishes receipt retirement only after its tombstone-bearing Host
    /// checkpoint receives a durable store acknowledgement.
    pub fn retire_request_receipts<A: RetentionAuthorityPortV1>(
        &mut self,
        session: HostSessionIdV2,
        policy_revision: RetentionPolicyRevisionV1,
        selection: &[(ClientIdV2, RequestIdV2)],
        authority: &mut A,
        context: &V::Context,
        now: HostTimeV1,
    ) -> DurableHostPublicationV2<Result<usize, ReceiptRetirementHostRefusalV1>> {
        let publication = self.publish(|host| {
            host.retire_request_receipts(
                session,
                policy_revision,
                selection,
                authority,
                context,
                now,
            )
        });
        self.counters.record(&publication);
        publication
    }

    /// Publishes retention-policy installation only after the complete Host
    /// checkpoint receives a durable store acknowledgement.
    pub fn install_retention_policy<A: RetentionAuthorityPortV1>(
        &mut self,
        session: HostSessionIdV2,
        proposed: RetentionPolicyV1,
        authority: &mut A,
        context: &V::Context,
        now: HostTimeV1,
    ) -> DurableHostPublicationV2<Result<RetentionPolicyInstallV1, RetentionPolicyHostRefusalV1>>
    {
        let publication = self.publish(|host| {
            host.install_retention_policy(session, proposed, authority, context, now)
        });
        self.counters.record(&publication);
        publication
    }

    /// Publishes authoritative-change pruning only after the complete Host
    /// checkpoint receives a durable store acknowledgement.
    pub fn apply_authoritative_change_retention<A: RetentionAuthorityPortV1>(
        &mut self,
        session: HostSessionIdV2,
        policy_revision: RetentionPolicyRevisionV1,
        authority: &mut A,
        context: &V::Context,
        now: HostTimeV1,
    ) -> DurableHostPublicationV2<
        Result<
            AuthoritativeChangeRetentionApplicationV1,
            AuthoritativeChangeRetentionHostRefusalV1,
        >,
    > {
        let publication = self.publish(|host| {
            host.apply_authoritative_change_retention(
                session,
                policy_revision,
                authority,
                context,
                now,
            )
        });
        self.counters.record(&publication);
        publication
    }

    pub fn close(&mut self, session: HostSessionIdV2) -> DurableHostPublicationV2<bool> {
        let publication = self.publish(|host| host.close(session));
        self.counters.record(&publication);
        publication
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durable_observation_saturation_is_sticky_and_value_preserving() {
        let mut counters = DurableHostCountersV2 {
            offers: u64::MAX,
            committed: u64::MAX,
            ..DurableHostCountersV2::default()
        };
        counters.record(&DurableHostPublicationV2::Published {
            value: (),
            evidence: DurableEvidenceV2::Committed {
                revision: Revision(1),
            },
        });
        assert_eq!(counters.offers, u64::MAX);
        assert_eq!(counters.committed, u64::MAX);
        assert!(counters.saturated);

        counters.record(&DurableHostPublicationV2::<()>::Suppressed(
            DurableHostSuppressionV2::Fenced,
        ));
        assert_eq!(counters.offers, u64::MAX);
        assert_eq!(counters.fenced, 1);
        assert!(counters.saturated);
    }
}
