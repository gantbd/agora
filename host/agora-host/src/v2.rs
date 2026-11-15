//! Authenticated transport-neutral composition for Agora Wire/Service V2.

use super::{
    HostLimits, HostRefusal, HostTimeV1, HostUsage, PrincipalVerifier, VerifiedSubjectV1, count,
    identity_bytes, map_encode,
};
use agora_delivery_projection_port::{
    DeliveryProjectionRefusalV1, DeliveryProjectionV1, DeliverySourceV1,
};
use agora_export_custody_port::{
    ExportCustodyAcceptedV1, ExportCustodyPortV1, ExportCustodyRefusalV1, ExportCustodyRequestV1,
    ExportCustodyRetirementAcceptedV1, ExportCustodyRetirementRefusalV1,
    ExportCustodyRetirementRequestV1, ExportMaterializationV1, ExportRetirementV1,
};
use agora_federated_publication_port::{
    FederatedPublicationAcceptedV1, FederatedPublicationRefusalV1, FederatedPublicationRequestV1,
};
use agora_interaction_port::{
    InteractionContextRefusalV1, InteractionContextV1, InteractionPromotionAcceptedV1,
    InteractionPromotionRefusalV1, InteractionPromotionRequestV1, InteractionSourceV1,
};
use agora_protocol::v2::{ClientIdV2, RefusalV2};
use agora_retention_authority_port::{
    AuthoritativeChangeRetentionApplicationV1, AuthoritativeChangeRetentionAuthorityRequestV1,
    AuthorityDecisionV1, ExportAuthorityRequestV1, ExportRetirementAuthorityRequestV1,
    PolicyAuthorityRequestV1, ReceiptRetirementAuthorityRequestV1, ReceiptRetirementKeyV1,
    RecordClassV1, RetentionAuthorityPortV1, RetentionPolicyRevisionV1, RetentionPolicyV1,
};
use agora_scheduled_publication_port::{
    ScheduledPublicationAcceptedV1, ScheduledPublicationRefusalV1, ScheduledPublicationRequestV1,
};
use agora_service::v2::{
    AuthoritativeChangeRetentionServiceRefusalV1, ReceiptRetentionRefusalV1,
    RetentionPolicyInstallRefusalV1, RetentionPolicyInstallV1, ServiceV2,
};
use agora_wire::v2::{WireReplyV2, WireRequestV2, decode_request_v2, encode_reply_v2};
use std::collections::{BTreeMap, VecDeque};
use std::fmt;

pub mod persistence;

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct HostSessionIdV2([u8; 16]);

impl fmt::Debug for HostSessionIdV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HostSessionIdV2(<opaque>)")
    }
}

impl HostSessionIdV2 {
    pub(crate) const fn source_bytes(self) -> [u8; 16] {
        self.0
    }
}

pub struct OpenReplyV2 {
    session: Option<HostSessionIdV2>,
    frame: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiptRetirementHostRefusalV1 {
    NotAuthorized,
    InvalidSelection,
    AuthorityDenied,
    AuthorityStale,
    AuthorityUnavailable,
    Service(ReceiptRetentionRefusalV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetentionPolicyHostRefusalV1 {
    NotAuthorized,
    AuthorityDenied,
    AuthorityStale,
    AuthorityUnavailable,
    Service(RetentionPolicyInstallRefusalV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthoritativeChangeRetentionHostRefusalV1 {
    NotAuthorized,
    NoCurrentPolicy,
    StalePolicy,
    InvalidSource,
    AuthorityDenied,
    AuthorityStale,
    AuthorityUnavailable,
    Service(AuthoritativeChangeRetentionServiceRefusalV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportHostRefusalV1 {
    NotAuthorized,
    NoCurrentPolicy,
    StaleSource,
    StalePolicy,
    AuthorityDenied,
    AuthorityStale,
    AuthorityUnavailable,
    Custody(ExportCustodyRefusalV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportRetirementHostRefusalV1 {
    NotAuthorized,
    NoCurrentPolicy,
    StaleSource,
    StalePolicy,
    AuthorityDenied,
    AuthorityStale,
    AuthorityUnavailable,
    Custody(ExportCustodyRetirementRefusalV1),
}

impl OpenReplyV2 {
    pub fn session(&self) -> Option<HostSessionIdV2> {
        self.session
    }

    pub fn frame(&self) -> &[u8] {
        &self.frame
    }
}

impl fmt::Debug for OpenReplyV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenReplyV2")
            .field("session", &self.session)
            .field("frame_bytes", &self.frame.len())
            .finish()
    }
}

#[derive(Clone)]
struct SessionV2 {
    subject: VerifiedSubjectV1,
    participant: String,
    client: ClientIdV2,
    last_verified_at: HostTimeV1,
}

pub struct HostV2<V> {
    service: ServiceV2,
    verifier: V,
    limits: HostLimits,
    sessions: BTreeMap<HostSessionIdV2, SessionV2>,
    retired: VecDeque<HostSessionIdV2>,
    next_session: u64,
    usage: HostUsage,
}

impl<V> fmt::Debug for HostV2<V> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HostV2")
            .field("limits", &self.limits)
            .field("active_sessions", &self.sessions.len())
            .field("retired_sessions", &self.retired.len())
            .field("usage", &self.usage)
            .finish_non_exhaustive()
    }
}

impl<V: PrincipalVerifier> HostV2<V> {
    pub fn new(service: ServiceV2, verifier: V, limits: HostLimits) -> Self {
        Self {
            service,
            verifier,
            limits,
            sessions: BTreeMap::new(),
            retired: VecDeque::new(),
            next_session: 0,
            usage: HostUsage::default(),
        }
    }

    pub fn service(&self) -> &ServiceV2 {
        &self.service
    }

    pub fn usage(&self) -> HostUsage {
        self.usage
    }

    pub fn open(
        &mut self,
        frame: &[u8],
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<OpenReplyV2, HostRefusal> {
        count(&mut self.usage.open_offers, &mut self.usage.overflowed);
        let request =
            decode_request_v2(frame, self.limits.wire).map_err(HostRefusal::InvalidFrame)?;
        let WireRequestV2::Connect { participant_id } = request else {
            return Err(HostRefusal::ProtocolViolation);
        };
        if self.sessions.len() >= self.limits.max_sessions {
            return self.open_refused(RefusalV2::ClientLimit);
        }
        let Some(next_session) = self.next_session.checked_add(1) else {
            return self.open_refused(RefusalV2::Exhausted);
        };
        let subject = match self.verifier.authenticate(&participant_id, context, now) {
            Ok(subject) => subject,
            Err(_) => {
                count(
                    &mut self.usage.authentication_refused,
                    &mut self.usage.overflowed,
                );
                return self.open_refused(RefusalV2::Unavailable);
            }
        };
        count(&mut self.usage.authenticated, &mut self.usage.overflowed);
        let client = match self.service.connect(&participant_id) {
            Ok(client) => client,
            Err(refusal) => return self.open_refused(refusal),
        };
        let reply = match encode_reply_v2(&WireReplyV2::Connected(client), self.limits.wire) {
            Ok(reply) => reply,
            Err(error) => {
                let _ = self.service.disconnect(client);
                return Err(map_encode(error));
            }
        };
        let session = HostSessionIdV2(identity_bytes(b'2', self.next_session));
        self.next_session = next_session;
        self.sessions.insert(
            session,
            SessionV2 {
                subject,
                participant: participant_id,
                client,
                last_verified_at: now,
            },
        );
        Ok(OpenReplyV2 {
            session: Some(session),
            frame: reply,
        })
    }

    pub fn handle(
        &mut self,
        session_id: HostSessionIdV2,
        frame: &[u8],
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<Vec<u8>, HostRefusal> {
        count(&mut self.usage.frame_offers, &mut self.usage.overflowed);
        let session = self.authorize_session(session_id, context, now)?;
        let request =
            decode_request_v2(frame, self.limits.wire).map_err(HostRefusal::InvalidFrame)?;
        match request {
            WireRequestV2::Execute { client, request } => {
                if client != session.client {
                    self.retire(session_id);
                    return self.unavailable();
                }
                let reply = self.service.execute_at(client, request, now.0);
                let frame = encode_reply_v2(&WireReplyV2::Executed(reply), self.limits.wire)
                    .map_err(map_encode)?;
                count(&mut self.usage.executed, &mut self.usage.overflowed);
                Ok(frame)
            }
            WireRequestV2::Disconnect(client) => {
                if client != session.client {
                    self.retire(session_id);
                    return self.unavailable();
                }
                self.retire(session_id);
                let frame = encode_reply_v2(&WireReplyV2::Disconnected, self.limits.wire)
                    .map_err(map_encode)?;
                count(&mut self.usage.disconnected, &mut self.usage.overflowed);
                Ok(frame)
            }
            WireRequestV2::Connect { .. } => Err(HostRefusal::ProtocolViolation),
        }
    }

    /// Trusted transport-neutral publication path for a configured scheduler
    /// adapter. The session actor and admission clock are Host-owned facts.
    pub fn publish_scheduled(
        &mut self,
        session_id: HostSessionIdV2,
        request: ScheduledPublicationRequestV1,
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<ScheduledPublicationAcceptedV1, ScheduledPublicationRefusalV1> {
        let session = self
            .authorize_session(session_id, context, now)
            .map_err(|_| ScheduledPublicationRefusalV1::NotAuthorized)?;
        self.service
            .publish_scheduled_at(session.client, request, now.0)
    }

    /// Trusted transport-neutral publication path for a configured federation
    /// adapter. Host revalidation supplies the actor and current clock.
    pub fn publish_federated(
        &mut self,
        session_id: HostSessionIdV2,
        request: FederatedPublicationRequestV1,
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<FederatedPublicationAcceptedV1, FederatedPublicationRefusalV1> {
        let session = self
            .authorize_session(session_id, context, now)
            .map_err(|_| FederatedPublicationRefusalV1::NotAuthorized)?;
        self.service
            .publish_federated_at(session.client, request, now.0)
    }

    /// Revalidates the authenticated observer before projecting a minimized
    /// wake fact. This path is outside ordinary participant Wire V2.
    pub fn project_delivery(
        &mut self,
        session_id: HostSessionIdV2,
        source: DeliverySourceV1,
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<DeliveryProjectionV1, DeliveryProjectionRefusalV1> {
        let session = self
            .authorize_session(session_id, context, now)
            .map_err(|_| DeliveryProjectionRefusalV1::NotAuthorized)?;
        self.service.project_delivery(session.client, source)
    }

    /// Revalidates the authenticated participant before exposing one minimized
    /// typed-interaction source context outside ordinary Wire V2.
    pub fn interaction_context(
        &mut self,
        session_id: HostSessionIdV2,
        source: InteractionSourceV1,
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<InteractionContextV1, InteractionContextRefusalV1> {
        let session = self
            .authorize_session(session_id, context, now)
            .map_err(|_| InteractionContextRefusalV1::NotAuthorized)?;
        self.service.interaction_context(session.client, source)
    }

    /// Promotes one proposal as the current authenticated participant at
    /// current Host time. The adapter cannot assert author authority.
    pub fn promote_interaction(
        &mut self,
        session_id: HostSessionIdV2,
        request: InteractionPromotionRequestV1,
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<InteractionPromotionAcceptedV1, InteractionPromotionRefusalV1> {
        let session = self
            .authorize_session(session_id, context, now)
            .map_err(|_| InteractionPromotionRefusalV1::NotAuthorized)?;
        self.service
            .promote_interaction_at(session.client, request, now.0)
    }

    /// Revalidates the administrative session and installs policy metadata only
    /// after a fresh decision from the configured retention authority.
    pub fn install_retention_policy<A: RetentionAuthorityPortV1>(
        &mut self,
        session_id: HostSessionIdV2,
        proposed: RetentionPolicyV1,
        authority: &mut A,
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<RetentionPolicyInstallV1, RetentionPolicyHostRefusalV1> {
        let session = self
            .authorize_session(session_id, context, now)
            .map_err(|_| RetentionPolicyHostRefusalV1::NotAuthorized)?;
        let request = PolicyAuthorityRequestV1 {
            principal: session.subject.administrative_principal_binding(),
            observed_engine_revision: self.service.engine().revision().0,
            current_policy_revision: self
                .service
                .retention_policy()
                .map(|policy| policy.revision),
            proposed_policy: proposed,
        };
        match authority.authorize_policy(request) {
            AuthorityDecisionV1::Authorized => self
                .service
                .install_retention_policy(proposed)
                .map_err(RetentionPolicyHostRefusalV1::Service),
            AuthorityDecisionV1::Denied => Err(RetentionPolicyHostRefusalV1::AuthorityDenied),
            AuthorityDecisionV1::Stale => Err(RetentionPolicyHostRefusalV1::AuthorityStale),
            AuthorityDecisionV1::Unavailable => {
                Err(RetentionPolicyHostRefusalV1::AuthorityUnavailable)
            }
        }
    }

    /// Revalidates the administrative session and exact native journal shape,
    /// obtains fresh authority, and applies only the installed
    /// `AuthoritativeChanges` bound. Delivery acknowledgement is independent.
    pub fn apply_authoritative_change_retention<A: RetentionAuthorityPortV1>(
        &mut self,
        session_id: HostSessionIdV2,
        policy_revision: RetentionPolicyRevisionV1,
        authority: &mut A,
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<AuthoritativeChangeRetentionApplicationV1, AuthoritativeChangeRetentionHostRefusalV1>
    {
        let session = self
            .authorize_session(session_id, context, now)
            .map_err(|_| AuthoritativeChangeRetentionHostRefusalV1::NotAuthorized)?;
        let Some(policy) = self.service.retention_policy() else {
            return Err(AuthoritativeChangeRetentionHostRefusalV1::NoCurrentPolicy);
        };
        if policy.revision != policy_revision {
            return Err(AuthoritativeChangeRetentionHostRefusalV1::StalePolicy);
        }
        let engine = self.service.engine();
        let observed_engine_revision = engine.revision();
        let prior_discarded_through = match engine.discarded_changes_through() {
            Some(sequence) => sequence
                .0
                .checked_add(1)
                .ok_or(AuthoritativeChangeRetentionHostRefusalV1::InvalidSource)?,
            None => 0,
        };
        let retained_records = u64::try_from(engine.retained_changes().len())
            .map_err(|_| AuthoritativeChangeRetentionHostRefusalV1::InvalidSource)?;
        let request = AuthoritativeChangeRetentionAuthorityRequestV1::new(
            session.subject.administrative_principal_binding(),
            policy_revision,
            observed_engine_revision.0,
            policy.bound(RecordClassV1::AuthoritativeChanges),
            prior_discarded_through,
            retained_records,
        )
        .ok_or(AuthoritativeChangeRetentionHostRefusalV1::InvalidSource)?;
        match authority.authorize_authoritative_change_retention(request) {
            AuthorityDecisionV1::Authorized => self
                .service
                .apply_authoritative_change_retention(policy_revision, observed_engine_revision)
                .map_err(AuthoritativeChangeRetentionHostRefusalV1::Service),
            AuthorityDecisionV1::Denied => {
                Err(AuthoritativeChangeRetentionHostRefusalV1::AuthorityDenied)
            }
            AuthorityDecisionV1::Stale => {
                Err(AuthoritativeChangeRetentionHostRefusalV1::AuthorityStale)
            }
            AuthorityDecisionV1::Unavailable => {
                Err(AuthoritativeChangeRetentionHostRefusalV1::AuthorityUnavailable)
            }
        }
    }

    /// Revalidates the administrative session and exact current source/policy
    /// bindings, obtains a fresh authority decision over the complete manifest
    /// identity, and only then admits the prepared export to separate custody.
    /// Source projection and digest qualification occur before this boundary.
    pub fn admit_export<A: RetentionAuthorityPortV1, C: ExportCustodyPortV1>(
        &mut self,
        session_id: HostSessionIdV2,
        materialization: ExportMaterializationV1,
        authority: &mut A,
        custody: &mut C,
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<ExportCustodyAcceptedV1, ExportHostRefusalV1> {
        let session = self
            .authorize_session(session_id, context, now)
            .map_err(|_| ExportHostRefusalV1::NotAuthorized)?;
        let current_engine_revision = self.service.engine().revision().0;
        if materialization.manifest.source_engine_revision != current_engine_revision {
            return Err(ExportHostRefusalV1::StaleSource);
        }
        let Some(policy) = self.service.retention_policy() else {
            return Err(ExportHostRefusalV1::NoCurrentPolicy);
        };
        if materialization.manifest.policy_revision != policy.revision {
            return Err(ExportHostRefusalV1::StalePolicy);
        }
        let authority_request = ExportAuthorityRequestV1 {
            principal: session.subject.administrative_principal_binding(),
            request: materialization.manifest.request,
            profile: materialization.manifest.profile,
            scope: materialization.manifest.scope,
            source_engine_revision: current_engine_revision,
            policy_revision: policy.revision,
        };
        match authority.authorize_export(authority_request) {
            AuthorityDecisionV1::Authorized => custody
                .admit(ExportCustodyRequestV1::new(
                    current_engine_revision,
                    policy.revision,
                    materialization,
                ))
                .map_err(ExportHostRefusalV1::Custody),
            AuthorityDecisionV1::Denied => Err(ExportHostRefusalV1::AuthorityDenied),
            AuthorityDecisionV1::Stale => Err(ExportHostRefusalV1::AuthorityStale),
            AuthorityDecisionV1::Unavailable => Err(ExportHostRefusalV1::AuthorityUnavailable),
        }
    }

    /// Revalidates the administrative session and exact current source/policy
    /// bindings, obtains fresh authority over the full retirement identity,
    /// and only then asks separate custody to install its non-reuse tombstone.
    /// This method makes no physical-reclamation or erasure claim.
    pub fn retire_export<A: RetentionAuthorityPortV1, C: ExportCustodyPortV1>(
        &mut self,
        session_id: HostSessionIdV2,
        retirement: ExportRetirementV1,
        authority: &mut A,
        custody: &mut C,
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<ExportCustodyRetirementAcceptedV1, ExportRetirementHostRefusalV1> {
        let session = self
            .authorize_session(session_id, context, now)
            .map_err(|_| ExportRetirementHostRefusalV1::NotAuthorized)?;
        let current_engine_revision = self.service.engine().revision().0;
        if retirement.source_engine_revision != current_engine_revision {
            return Err(ExportRetirementHostRefusalV1::StaleSource);
        }
        let Some(policy) = self.service.retention_policy() else {
            return Err(ExportRetirementHostRefusalV1::NoCurrentPolicy);
        };
        if retirement.policy_revision != policy.revision {
            return Err(ExportRetirementHostRefusalV1::StalePolicy);
        }
        let request = ExportRetirementAuthorityRequestV1 {
            principal: session.subject.administrative_principal_binding(),
            retirement,
        };
        match authority.authorize_export_retirement(request) {
            AuthorityDecisionV1::Authorized => custody
                .retire(ExportCustodyRetirementRequestV1 {
                    current_engine_revision,
                    current_policy_revision: policy.revision,
                    retirement,
                })
                .map_err(ExportRetirementHostRefusalV1::Custody),
            AuthorityDecisionV1::Denied => Err(ExportRetirementHostRefusalV1::AuthorityDenied),
            AuthorityDecisionV1::Stale => Err(ExportRetirementHostRefusalV1::AuthorityStale),
            AuthorityDecisionV1::Unavailable => {
                Err(ExportRetirementHostRefusalV1::AuthorityUnavailable)
            }
        }
    }

    /// Revalidates the administrative session, obtains a fresh decision from
    /// the configured retention authority, and only then atomically replaces
    /// exact replay receipts with durable non-reexecution tombstones.
    pub fn retire_request_receipts<A: RetentionAuthorityPortV1>(
        &mut self,
        session_id: HostSessionIdV2,
        policy_revision: RetentionPolicyRevisionV1,
        selection: &[(ClientIdV2, agora_protocol::v2::RequestIdV2)],
        authority: &mut A,
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<usize, ReceiptRetirementHostRefusalV1> {
        let session = self
            .authorize_session(session_id, context, now)
            .map_err(|_| ReceiptRetirementHostRefusalV1::NotAuthorized)?;
        let authority_selection = selection
            .iter()
            .map(|(client, request)| {
                let mut key = [0_u8; 16];
                key[..8].copy_from_slice(&client.0.to_be_bytes());
                key[8..].copy_from_slice(&request.0.to_be_bytes());
                ReceiptRetirementKeyV1::new(key)
            })
            .collect();
        let engine_revision = self.service.engine().revision();
        let request = ReceiptRetirementAuthorityRequestV1::new(
            session.subject.administrative_principal_binding(),
            policy_revision,
            engine_revision.0,
            authority_selection,
            self.service.limits().max_retained_requests,
        )
        .ok_or(ReceiptRetirementHostRefusalV1::InvalidSelection)?;
        match authority.authorize_receipt_retirement(&request) {
            AuthorityDecisionV1::Authorized => self
                .service
                .retire_request_receipts(policy_revision, engine_revision, selection)
                .map_err(ReceiptRetirementHostRefusalV1::Service),
            AuthorityDecisionV1::Denied => Err(ReceiptRetirementHostRefusalV1::AuthorityDenied),
            AuthorityDecisionV1::Stale => Err(ReceiptRetirementHostRefusalV1::AuthorityStale),
            AuthorityDecisionV1::Unavailable => {
                Err(ReceiptRetirementHostRefusalV1::AuthorityUnavailable)
            }
        }
    }

    pub fn close(&mut self, session: HostSessionIdV2) -> bool {
        if !self.sessions.contains_key(&session) {
            return false;
        }
        self.retire(session);
        count(&mut self.usage.disconnected, &mut self.usage.overflowed);
        true
    }

    pub(crate) fn has_session(&self, session: HostSessionIdV2) -> bool {
        self.sessions.contains_key(&session)
    }

    pub(crate) fn client_for_session(&self, session: HostSessionIdV2) -> Option<ClientIdV2> {
        self.sessions.get(&session).map(|state| state.client)
    }

    pub(crate) fn wire_limits(&self) -> agora_wire::WireLimits {
        self.limits.wire
    }

    pub(crate) fn authorize_transient(
        &mut self,
        session_id: HostSessionIdV2,
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<String, HostRefusal> {
        Ok(self
            .authorize_session(session_id, context, now)?
            .participant)
    }

    pub(crate) fn authorize_search(
        &mut self,
        session_id: HostSessionIdV2,
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<ClientIdV2, HostRefusal> {
        Ok(self.authorize_session(session_id, context, now)?.client)
    }

    pub(crate) fn retire_search_protocol_violation(
        &mut self,
        session_id: HostSessionIdV2,
    ) -> HostRefusal {
        self.retire(session_id);
        count(&mut self.usage.unavailable, &mut self.usage.overflowed);
        HostRefusal::Unavailable
    }

    fn open_refused(&self, refusal: RefusalV2) -> Result<OpenReplyV2, HostRefusal> {
        let frame = encode_reply_v2(&WireReplyV2::ConnectRefused(refusal), self.limits.wire)
            .map_err(map_encode)?;
        Ok(OpenReplyV2 {
            session: None,
            frame,
        })
    }

    fn unavailable<T>(&mut self) -> Result<T, HostRefusal> {
        count(&mut self.usage.unavailable, &mut self.usage.overflowed);
        Err(HostRefusal::Unavailable)
    }

    fn authorize_session(
        &mut self,
        session_id: HostSessionIdV2,
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<SessionV2, HostRefusal> {
        let Some(session) = self.sessions.get(&session_id).cloned() else {
            return self.unavailable();
        };
        if now < session.last_verified_at {
            self.retire(session_id);
            return self.unavailable();
        }
        if self
            .verifier
            .revalidate(session.subject, &session.participant, context, now)
            .is_err()
        {
            self.retire(session_id);
            return self.unavailable();
        }
        if let Some(active) = self.sessions.get_mut(&session_id) {
            active.last_verified_at = now;
        }
        Ok(session)
    }

    fn retire(&mut self, session_id: HostSessionIdV2) {
        let Some(session) = self.sessions.remove(&session_id) else {
            return;
        };
        let _ = self.service.disconnect(session.client);
        if self.limits.max_retired_sessions > 0 {
            if self.retired.len() >= self.limits.max_retired_sessions {
                self.retired.pop_front();
            }
            self.retired.push_back(session_id);
        }
    }
}
