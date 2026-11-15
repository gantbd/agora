//! Serialized volatile implementation of Agora Protocol V2.

use agora_delivery_projection_port::{
    DeliveryProjectionRefusalV1, DeliveryProjectionV1, DeliverySourceV1, WakeClassV1,
    WakeProjectionV1,
};
use agora_engine::retention::{
    ReceiptRetirementApplyV1, ReceiptRetirementRefusalV1, ReceiptRetirementRegistryV1,
    RecordClassV1, RetentionBoundV1, RetentionIdentityBindingV1, RetentionPolicyRevisionV1,
    RetentionPolicyV1, RetentionScopeBindingV1, RetentionTombstoneReasonV1, RetentionTombstoneV1,
};
use agora_engine::{
    AdmissionPolicy, AdmissionTimeRefusalV1, AdmissionTimeV1,
    AttachmentDescriptorV1 as EngineAttachmentDescriptorV1, AttachmentIdV1 as EngineAttachmentIdV1,
    AttachmentIntegrityV1 as EngineAttachmentIntegrityV1,
    AttachmentMessageTargetV1 as EngineAttachmentMessageTargetV1,
    AttachmentObjectKeyV1 as EngineAttachmentObjectKeyV1,
    AttachmentStoreIdV1 as EngineAttachmentStoreIdV1, AttentionPreferenceRefusalV1,
    AttentionPreferenceV1 as EngineAttentionPreferenceV1,
    AttentionScopeV1 as EngineAttentionScopeV1, BanPageCursor, CommandV2 as EngineCommand,
    ConversationProfileV1 as EngineConversationProfileV1, DomainEventV2, Engine,
    MAX_REACTION_KEY_BYTES_V1, MemberPageCursor, MessageId, MessagePageCursor,
    MessageReportDispositionV1 as EngineMessageReportDispositionV1,
    MessageReportIdV1 as EngineMessageReportIdV1, MessageReportRefusalV1,
    MessageReportV1 as EngineMessageReportV1, MessageRevisionPageCursorV1, ModerationCausationV1,
    ModerationReasonV1, NotificationIntentV1 as EngineNotificationIntentV1, ObservationRefusal,
    OutcomeV2 as EngineOutcome, ParticipantId, ParticipantKind, PollId as EnginePollId,
    PollObservationRefusalV1, PrivateSaveRefusalV1, PublicationCommandV2, PublicationDomainV2,
    PublicationKeyV2, PublicationRefusalV2, ReadPositionRefusalV1,
    ReadScopeV1 as EngineReadScopeV1, Refusal as EngineRefusal, Revision, RevisionedCommandV2,
    Role, RoomChangeGap, RoomChangePosition, RoomChangeResult, RoomId, RoomPageCursor,
    Sha256DigestV1 as EngineSha256DigestV1, ThreadMessagePageCursor, TopicCollectionCursorV1,
    TopicCollectionFilterV1 as EngineTopicCollectionFilterV1,
    TopicCollectionLifecycleV1 as EngineTopicCollectionLifecycleV1, TopicId,
    TopicMessagePageCursorV1, TopicPageCursorV1,
    VellumArtifactMessageTargetV1 as EngineVellumArtifactMessageTargetV1,
    VellumArtifactReferenceIdV1 as EngineVellumArtifactReferenceIdV1,
    VellumArtifactReferenceV1 as EngineVellumArtifactReferenceV1,
    VellumDocumentTargetV1 as EngineVellumDocumentTargetV1,
};
use agora_federated_publication_port::{
    FederatedPublicationAcceptedV1, FederatedPublicationRefusalV1, FederatedPublicationRequestV1,
};
use agora_interaction_port::{
    InteractionContextRefusalV1, InteractionContextV1, InteractionPromotionAcceptedV1,
    InteractionPromotionRefusalV1, InteractionPromotionRequestV1, InteractionSourceV1,
};
use agora_protocol::v2::{
    AdmissionControlStatusV1, AdmissionPolicyV2, AttachmentDescriptorV1, AttachmentIdV1,
    AttachmentIntegrityV1, AttachmentMessagePageV1, AttachmentMessageTargetV1, AttachmentMessageV1,
    AttachmentObjectKeyV1, AttachmentRoomChangePageV1, AttachmentStoreIdV1, AttentionPreferenceV1,
    AttentionScopeV1, BanPageV2, BannedParticipantV2, CapabilityV2, ChangeGapV2, ClientIdV2,
    CommandV2, ConversationProfileStatusV1, ConversationProfileV1, IdentifiedMessagePageV1,
    IdentifiedMessageV1, IdentifiedRoomChangePageV1, IdentifiedRoomChangeV1, IdentifiedRoomEventV1,
    MemberPageV2, MemberV2, MessageAttachmentV1, MessageIdV1, MessagePageV2,
    MessageReportDispositionV1, MessageReportIdV1, MessageReportV1, MessageRevisionAvailabilityV1,
    MessageRevisionPageV1, MessageRevisionV1, MessageV2, MessageVellumArtifactReferenceV1,
    NotificationIntentV1, OutcomeV2, PageCursorV2, ParticipantKindV2, PeerConversationCreatedV1,
    PinnedReferenceV1, PollBallotV1, PollIdV1, PollStatusV1, ReactionStatusV1, ReactionSummaryV1,
    ReadPositionV1, ReadScopeV1, RefusalV2, ReplyV2, RequestIdV2, RequestV2, RoleV2,
    RoomChangePageV2, RoomChangeV2, RoomEventV2, RoomPageV2, RoomSummaryV2, SavedReferenceV1,
    Sha256DigestV1, StableReferenceAvailabilityV1, ThreadLifecycleStateV1, ThreadMessagePageV1,
    ThreadStatusV1, ThreadedMessageV1, ThreadedRoomChangePageV1, ThreadedRoomChangeV1,
    ThreadedRoomEventV1, TopicAliasStatusV1, TopicCollectionEntryV1, TopicCollectionFilterV1,
    TopicCollectionLifecycleV1, TopicCollectionPageV1, TopicIdV1, TopicMessagePageV1,
    TopicMessageV1, TopicPageV1, TopicRoomChangePageV1, TopicRoomChangeV1, TopicRoomEventV1,
    TopicStatusV1, TopicSupersessionStatusV1, TopicTagStatusV1, TopicV1,
    VellumArtifactMessagePageV1, VellumArtifactMessageTargetV1, VellumArtifactMessageV1,
    VellumArtifactReferenceIdV1, VellumArtifactReferenceV1, VellumDocumentTargetV1,
};
use agora_scheduled_publication_port::{
    ScheduledPublicationAcceptedV1, ScheduledPublicationRefusalV1, ScheduledPublicationRequestV1,
};
use std::collections::{BTreeMap, VecDeque};

pub mod export_source;
pub mod persistence;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceLimitsV2 {
    pub max_clients: usize,
    pub max_retired_clients: usize,
    pub max_retained_requests: usize,
    pub max_retained_cursors: usize,
    pub max_page_items: u32,
    pub max_participant_id_bytes: usize,
    pub max_room_id_bytes: usize,
    pub max_room_name_bytes: usize,
    pub max_message_body_bytes: usize,
    pub max_moderation_reason_bytes: usize,
    pub max_addressed_participants: usize,
}

impl Default for ServiceLimitsV2 {
    fn default() -> Self {
        Self {
            max_clients: 64,
            max_retired_clients: 256,
            max_retained_requests: 4_096,
            max_retained_cursors: 4_096,
            max_page_items: 256,
            max_participant_id_bytes: 128,
            max_room_id_bytes: 128,
            max_room_name_bytes: 256,
            max_message_body_bytes: 16 * 1_024,
            max_moderation_reason_bytes: 1_024,
            max_addressed_participants: 64,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UsageV2 {
    pub connect_offers: u64,
    pub connected: u64,
    pub request_offers: u64,
    pub accepted: u64,
    pub refused: u64,
    pub replayed: u64,
    pub conflicts: u64,
    pub disconnected: u64,
    pub overflowed: bool,
}

fn count(value: &mut u64, overflowed: &mut bool) {
    let old = *value;
    *value = value.saturating_add(1);
    *overflowed |= *value == old && old == u64::MAX;
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RetainedRequestV2 {
    request: RequestV2,
    reply: ReplyV2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiptRetentionRefusalV1 {
    EmptySelection,
    NonCanonicalSelection,
    StaleEngineRevision {
        expected: Revision,
        actual: Revision,
    },
    UnknownReceipt,
    NoCurrentPolicy,
    PolicyRevisionMismatch {
        expected: RetentionPolicyRevisionV1,
        actual: RetentionPolicyRevisionV1,
    },
    Registry(ReceiptRetirementRefusalV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetentionPolicyInstallV1 {
    Installed,
    Replay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetentionPolicyInstallRefusalV1 {
    StaleEngineRevision {
        expected: Revision,
        actual: Revision,
    },
    RevisionRegression,
    RevisionConflict,
}

fn receipt_retirement_bindings(
    client: ClientIdV2,
    request: RequestIdV2,
) -> (RetentionIdentityBindingV1, RetentionScopeBindingV1) {
    let mut identity = [0_u8; 32];
    identity[..8].copy_from_slice(b"AGSVREQ1");
    identity[16..24].copy_from_slice(&client.0.to_be_bytes());
    identity[24..].copy_from_slice(&request.0.to_be_bytes());
    let mut scope = [0_u8; 32];
    scope[..8].copy_from_slice(b"AGORASV\0");
    (
        RetentionIdentityBindingV1::new(identity).expect("receipt domain is nonzero"),
        RetentionScopeBindingV1::new(scope).expect("service scope domain is nonzero"),
    )
}

fn receipt_retirement_key(
    identity: RetentionIdentityBindingV1,
    scope: RetentionScopeBindingV1,
) -> Option<(ClientIdV2, RequestIdV2)> {
    let identity = identity.as_bytes();
    let scope = scope.as_bytes();
    if &identity[..8] != b"AGSVREQ1"
        || identity[8..16] != [0; 8]
        || &scope[..8] != b"AGORASV\0"
        || scope[8..] != [0; 24]
    {
        return None;
    }
    Some((
        ClientIdV2(u64::from_be_bytes(identity[16..24].try_into().ok()?)),
        RequestIdV2(u64::from_be_bytes(identity[24..].try_into().ok()?)),
    ))
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CursorPage {
    Rooms(RoomPageCursor),
    Members {
        room_id: RoomId,
        cursor: MemberPageCursor,
    },
    Bans {
        room_id: RoomId,
        cursor: BanPageCursor,
    },
    Messages {
        room_id: RoomId,
        cursor: MessagePageCursor,
    },
    IdentifiedMessages {
        room_id: RoomId,
        cursor: MessagePageCursor,
    },
    AttachmentMessages {
        room_id: RoomId,
        cursor: MessagePageCursor,
    },
    VellumArtifactMessages {
        room_id: RoomId,
        cursor: MessagePageCursor,
    },
    Changes {
        room_id: RoomId,
        position: RoomChangePosition,
    },
    IdentifiedChanges {
        room_id: RoomId,
        position: RoomChangePosition,
    },
    AttachmentChanges {
        room_id: RoomId,
        position: RoomChangePosition,
    },
    ThreadRoots {
        room_id: RoomId,
        cursor: ThreadMessagePageCursor,
    },
    ThreadReplies {
        room_id: RoomId,
        root_message_id: MessageId,
        cursor: ThreadMessagePageCursor,
    },
    ThreadedChanges {
        room_id: RoomId,
        position: RoomChangePosition,
    },
    TopicChanges {
        room_id: RoomId,
        position: RoomChangePosition,
    },
    MessageRevisions {
        room_id: RoomId,
        message_id: MessageId,
        cursor: MessageRevisionPageCursorV1,
    },
    Topics {
        room_id: RoomId,
        cursor: TopicPageCursorV1,
    },
    TopicCollection {
        room_id: RoomId,
        cursor: TopicCollectionCursorV1,
    },
    TopicMessages {
        room_id: RoomId,
        topic_id: TopicId,
        cursor: TopicMessagePageCursorV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CursorRecord {
    owner: ClientIdV2,
    page: CursorPage,
}

/// One serialized volatile Protocol V2 owner.
#[derive(Debug)]
pub struct ServiceV2 {
    engine: Engine,
    limits: ServiceLimitsV2,
    clients: BTreeMap<ClientIdV2, ParticipantId>,
    retired_clients: VecDeque<ClientIdV2>,
    retained: BTreeMap<(ClientIdV2, RequestIdV2), RetainedRequestV2>,
    receipt_revision: u64,
    receipt_revision_exhausted: bool,
    retention_policy: Option<RetentionPolicyV1>,
    retired_receipts: ReceiptRetirementRegistryV1,
    cursors: BTreeMap<PageCursorV2, CursorRecord>,
    next_client: u64,
    next_cursor: u64,
    usage: UsageV2,
}

impl ServiceV2 {
    pub fn new(engine: Engine, limits: ServiceLimitsV2) -> Self {
        Self {
            engine,
            limits,
            clients: BTreeMap::new(),
            retired_clients: VecDeque::new(),
            retained: BTreeMap::new(),
            receipt_revision: 0,
            receipt_revision_exhausted: false,
            retention_policy: None,
            retired_receipts: ReceiptRetirementRegistryV1::new(limits.max_retained_requests.max(1))
                .expect("positive receipt-retirement capacity"),
            cursors: BTreeMap::new(),
            next_client: 0,
            next_cursor: 0,
            usage: UsageV2::default(),
        }
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn limits(&self) -> ServiceLimitsV2 {
        self.limits
    }

    /// Trusted-host inspection of the participant bound to an active client.
    /// A client handle remains a correlation value, not a credential.
    pub fn participant_for_client(&self, client: ClientIdV2) -> Option<&ParticipantId> {
        self.clients.get(&client)
    }

    pub fn active_client_count(&self) -> usize {
        self.clients.len()
    }

    pub fn usage(&self) -> UsageV2 {
        self.usage
    }

    pub fn retired_receipt_count(&self) -> usize {
        self.retired_receipts.len()
    }

    /// Current exact receipt-state generation, if at least one mutation has
    /// occurred and its identity domain has not saturated.
    pub fn receipt_revision(&self) -> Option<u64> {
        (!self.receipt_revision_exhausted && self.receipt_revision != 0)
            .then_some(self.receipt_revision)
    }

    /// Saturation never blocks cleanup, but permanently fences exact receipt
    /// export until a separately qualified migration establishes a new domain.
    pub fn receipt_revision_exhausted(&self) -> bool {
        self.receipt_revision_exhausted
    }

    pub fn retention_policy(&self) -> Option<RetentionPolicyV1> {
        self.retention_policy
    }

    fn advance_receipt_revision(&mut self) {
        let previous = self.receipt_revision;
        self.receipt_revision = self.receipt_revision.saturating_add(1);
        self.receipt_revision_exhausted |= previous == u64::MAX;
    }

    /// Installs current administrative policy metadata without itself pruning
    /// any record class. Later class-specific transitions must bind this exact
    /// policy revision and current engine frontier.
    pub fn install_retention_policy(
        &mut self,
        proposed: RetentionPolicyV1,
    ) -> Result<RetentionPolicyInstallV1, RetentionPolicyInstallRefusalV1> {
        let actual = self.engine.revision();
        let expected = Revision(proposed.effective_engine_revision);
        if expected != actual {
            return Err(RetentionPolicyInstallRefusalV1::StaleEngineRevision { expected, actual });
        }
        if let Some(current) = self.retention_policy {
            if proposed.revision < current.revision {
                return Err(RetentionPolicyInstallRefusalV1::RevisionRegression);
            }
            if proposed.revision == current.revision {
                return if proposed == current {
                    Ok(RetentionPolicyInstallV1::Replay)
                } else {
                    Err(RetentionPolicyInstallRefusalV1::RevisionConflict)
                };
            }
        }
        self.retention_policy = Some(proposed);
        Ok(RetentionPolicyInstallV1::Installed)
    }

    /// Trusted administrative receipt pruning. This path is absent from the
    /// participant protocol. The complete selection is preflighted and its
    /// non-reexecution tombstones are installed atomically before receipts are
    /// removed.
    pub fn retire_request_receipts(
        &mut self,
        policy_revision: RetentionPolicyRevisionV1,
        observed_engine_revision: Revision,
        selection: &[(ClientIdV2, RequestIdV2)],
    ) -> Result<usize, ReceiptRetentionRefusalV1> {
        if selection.is_empty() {
            return Err(ReceiptRetentionRefusalV1::EmptySelection);
        }
        if observed_engine_revision != self.engine.revision() {
            return Err(ReceiptRetentionRefusalV1::StaleEngineRevision {
                expected: observed_engine_revision,
                actual: self.engine.revision(),
            });
        }
        let Some(policy) = self.retention_policy else {
            return Err(ReceiptRetentionRefusalV1::NoCurrentPolicy);
        };
        if policy_revision != policy.revision {
            return Err(ReceiptRetentionRefusalV1::PolicyRevisionMismatch {
                expected: policy_revision,
                actual: policy.revision,
            });
        }
        if selection.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(ReceiptRetentionRefusalV1::NonCanonicalSelection);
        }
        if selection.iter().any(|key| !self.retained.contains_key(key)) {
            return Err(ReceiptRetentionRefusalV1::UnknownReceipt);
        }
        let mut candidate = self.retired_receipts.clone();
        for &(client, request) in selection {
            let (identity, scope) = receipt_retirement_bindings(client, request);
            let tombstone = RetentionTombstoneV1::new(
                RecordClassV1::RequestReceipts,
                RetentionTombstoneReasonV1::ReceiptRetired,
                policy_revision,
                observed_engine_revision.0,
                identity,
                scope,
            );
            match candidate.retire(tombstone) {
                Ok(ReceiptRetirementApplyV1::Retired) => {}
                Ok(ReceiptRetirementApplyV1::Replay) => {
                    return Err(ReceiptRetentionRefusalV1::Registry(
                        ReceiptRetirementRefusalV1::Conflict,
                    ));
                }
                Err(refusal) => return Err(ReceiptRetentionRefusalV1::Registry(refusal)),
            }
        }
        for key in selection {
            self.retained.remove(key);
        }
        self.retired_receipts = candidate;
        self.advance_receipt_revision();
        Ok(selection.len())
    }

    pub fn connect(&mut self, participant: &str) -> Result<ClientIdV2, RefusalV2> {
        count(&mut self.usage.connect_offers, &mut self.usage.overflowed);
        if !valid_text(participant, self.limits.max_participant_id_bytes) {
            return Err(RefusalV2::InvalidInput);
        }
        if self.clients.len() >= self.limits.max_clients {
            return Err(RefusalV2::ClientLimit);
        }
        let participant_id = ParticipantId::new(participant);
        if self.engine.participant(&participant_id).is_none() {
            return Err(RefusalV2::UnknownParticipant);
        }
        let client = ClientIdV2(self.next_client);
        self.next_client = self
            .next_client
            .checked_add(1)
            .ok_or(RefusalV2::Exhausted)?;
        self.clients.insert(client, participant_id);
        count(&mut self.usage.connected, &mut self.usage.overflowed);
        Ok(client)
    }

    pub fn disconnect(&mut self, client: ClientIdV2) -> Result<bool, RefusalV2> {
        if self.retired_clients.contains(&client) {
            return Ok(false);
        }
        if !self.clients.contains_key(&client) {
            return Err(RefusalV2::UnknownClient);
        }
        self.clients.remove(&client);
        if self.limits.max_retired_clients > 0 {
            if self.retired_clients.len() >= self.limits.max_retired_clients {
                self.retired_clients.pop_front();
            }
            self.retired_clients.push_back(client);
        }
        let retained_before = self.retained.len();
        self.retained.retain(|(owner, _), _| *owner != client);
        if self.retained.len() != retained_before {
            self.advance_receipt_revision();
        }
        self.cursors.retain(|_, record| record.owner != client);
        count(&mut self.usage.disconnected, &mut self.usage.overflowed);
        Ok(true)
    }

    pub fn execute(&mut self, client: ClientIdV2, request: RequestV2) -> ReplyV2 {
        self.execute_with_host_time(client, request, None)
    }

    /// Executes with a trusted host-derived, non-regressing admission instant.
    /// Exact retained-request replay is resolved before the clock advances.
    pub fn execute_at(
        &mut self,
        client: ClientIdV2,
        request: RequestV2,
        instant_ms: u64,
    ) -> ReplyV2 {
        self.execute_with_host_time(client, request, Some(instant_ms))
    }

    /// Trusted actor-bound publication path used by scheduling adapters. This
    /// is deliberately absent from Protocol and Wire V2.
    pub fn publish_scheduled_at(
        &mut self,
        client: ClientIdV2,
        request: ScheduledPublicationRequestV1,
        instant_ms: u64,
    ) -> Result<ScheduledPublicationAcceptedV1, ScheduledPublicationRefusalV1> {
        let participant = self
            .clients
            .get(&client)
            .cloned()
            .ok_or(ScheduledPublicationRefusalV1::NotAuthorized)?;
        if !valid_text(request.intent.room_id(), self.limits.max_room_id_bytes)
            || !valid_text(request.intent.body(), self.limits.max_message_body_bytes)
        {
            return Err(ScheduledPublicationRefusalV1::InvalidInput);
        }
        let command = PublicationCommandV2::new(
            self.engine.revision(),
            PublicationKeyV2::namespaced(PublicationDomainV2::Scheduled, request.id.into_bytes()),
            RoomId::new(request.intent.room_id()),
            participant,
            agora_engine::LogicalTime(request.intent.logical_time()),
            request.intent.body(),
        );
        let transition = self
            .engine
            .publish_at_instant(command, instant_ms)
            .map_err(map_scheduled_publication_refusal)?;
        let EngineOutcome::MessageAccepted { sequence, .. } = transition.outcome else {
            return Err(ScheduledPublicationRefusalV1::Unavailable);
        };
        Ok(ScheduledPublicationAcceptedV1 {
            revision: transition.revision.0,
            message_sequence: sequence,
        })
    }

    /// Trusted actor-bound home publication path used by federation adapters.
    /// The current Host clock supplies both message and admission time.
    pub fn publish_federated_at(
        &mut self,
        client: ClientIdV2,
        request: FederatedPublicationRequestV1,
        instant_ms: u64,
    ) -> Result<FederatedPublicationAcceptedV1, FederatedPublicationRefusalV1> {
        let participant = self
            .clients
            .get(&client)
            .cloned()
            .ok_or(FederatedPublicationRefusalV1::NotAuthorized)?;
        if !valid_text(request.intent.room_id(), self.limits.max_room_id_bytes)
            || !valid_text(request.intent.body(), self.limits.max_message_body_bytes)
        {
            return Err(FederatedPublicationRefusalV1::InvalidInput);
        }
        let command = PublicationCommandV2::new(
            self.engine.revision(),
            PublicationKeyV2::federation(request.scope.into_bytes(), request.id.into_bytes()),
            RoomId::new(request.intent.room_id()),
            participant,
            agora_engine::LogicalTime(instant_ms),
            request.intent.body(),
        );
        let transition = self
            .engine
            .publish_at_instant(command, instant_ms)
            .map_err(map_federated_publication_refusal)?;
        let EngineOutcome::MessageAccepted { sequence, .. } = transition.outcome else {
            return Err(FederatedPublicationRefusalV1::Unavailable);
        };
        Ok(FederatedPublicationAcceptedV1 {
            revision: transition.revision.0,
            event_sequence: transition.event_sequence.0,
            message_sequence: sequence,
        })
    }

    /// Projects one retained event into the initial content-minimized delivery
    /// profile for the participant bound to `client`. This is observation only:
    /// no read position, event acknowledgment, or domain state changes.
    pub fn project_delivery(
        &self,
        client: ClientIdV2,
        source: DeliverySourceV1,
    ) -> Result<DeliveryProjectionV1, DeliveryProjectionRefusalV1> {
        let participant = self
            .clients
            .get(&client)
            .ok_or(DeliveryProjectionRefusalV1::NotAuthorized)?;
        if let Some(discarded) = self.engine.discarded_changes_through()
            && source.event_sequence <= discarded.0
        {
            return Ok(discarded
                .0
                .checked_add(1)
                .map_or(DeliveryProjectionV1::Unavailable, |retained_from| {
                    DeliveryProjectionV1::Gap { retained_from }
                }));
        }
        let Some(event) = self
            .engine
            .retained_changes()
            .find(|event| event.sequence.0 == source.event_sequence)
        else {
            return Ok(DeliveryProjectionV1::Unavailable);
        };
        let Some(room_id) = event.event.room_id() else {
            return Ok(DeliveryProjectionV1::NotVisible);
        };
        if self.engine.member_role(room_id, participant).is_none() {
            return Ok(DeliveryProjectionV1::NotVisible);
        }
        Ok(DeliveryProjectionV1::Visible(WakeProjectionV1 {
            source,
            class: WakeClassV1::ConversationActivity,
        }))
    }

    /// Validates that one retained source is a currently visible message for
    /// the participant bound to `client`. No message content is projected.
    pub fn interaction_context(
        &self,
        client: ClientIdV2,
        source: InteractionSourceV1,
    ) -> Result<InteractionContextV1, InteractionContextRefusalV1> {
        let participant = self
            .clients
            .get(&client)
            .ok_or(InteractionContextRefusalV1::NotAuthorized)?;
        if let Some(discarded) = self.engine.discarded_changes_through()
            && source.event_sequence <= discarded.0
        {
            return Ok(discarded
                .0
                .checked_add(1)
                .map_or(InteractionContextV1::Unavailable, |retained_from| {
                    InteractionContextV1::Gap { retained_from }
                }));
        }
        let Some(event) = self
            .engine
            .retained_changes()
            .find(|event| event.sequence.0 == source.event_sequence)
        else {
            return Ok(InteractionContextV1::Unavailable);
        };
        let DomainEventV2::MessageAccepted { room_id, .. } = &event.event else {
            return Ok(InteractionContextV1::NotVisible);
        };
        if self.engine.member_role(room_id, participant).is_none() {
            return Ok(InteractionContextV1::NotVisible);
        }
        Ok(InteractionContextV1::Available { source })
    }

    /// Promotes a proposal through the same publication primitive as every
    /// trusted plain-message adapter, after exact replay and current source/
    /// membership validation. This path is absent from ordinary Wire V2.
    pub fn promote_interaction_at(
        &mut self,
        client: ClientIdV2,
        request: InteractionPromotionRequestV1,
        instant_ms: u64,
    ) -> Result<InteractionPromotionAcceptedV1, InteractionPromotionRefusalV1> {
        let participant = self
            .clients
            .get(&client)
            .cloned()
            .ok_or(InteractionPromotionRefusalV1::NotAuthorized)?;
        if !valid_text(request.intent.room_id(), self.limits.max_room_id_bytes)
            || !valid_text(request.intent.body(), self.limits.max_message_body_bytes)
        {
            return Err(InteractionPromotionRefusalV1::InvalidInput);
        }
        let command = PublicationCommandV2::new(
            self.engine.revision(),
            PublicationKeyV2::namespaced(PublicationDomainV2::Interaction, request.id.into_bytes()),
            RoomId::new(request.intent.room_id()),
            participant.clone(),
            agora_engine::LogicalTime(request.intent.logical_time()),
            request.intent.body(),
        );
        if let Some(transition) = self
            .engine
            .replay_publication(&command)
            .map_err(map_interaction_promotion_refusal)?
        {
            let EngineOutcome::MessageAccepted { sequence, .. } = transition.outcome else {
                return Err(InteractionPromotionRefusalV1::Unavailable);
            };
            return Ok(InteractionPromotionAcceptedV1 {
                revision: transition.revision.0,
                message_sequence: sequence,
            });
        }
        let source = request.intent.source();
        let Some(event) = self
            .engine
            .retained_changes()
            .find(|event| event.sequence.0 == source.event_sequence)
        else {
            return Err(InteractionPromotionRefusalV1::SourceUnavailable);
        };
        let DomainEventV2::MessageAccepted { room_id, .. } = &event.event else {
            return Err(InteractionPromotionRefusalV1::SourceUnavailable);
        };
        if room_id.as_str() != request.intent.room_id()
            || self.engine.member_role(room_id, &participant).is_none()
        {
            return Err(InteractionPromotionRefusalV1::NotAuthorized);
        }
        let transition = self
            .engine
            .publish_at_instant(command, instant_ms)
            .map_err(map_interaction_promotion_refusal)?;
        let EngineOutcome::MessageAccepted { sequence, .. } = transition.outcome else {
            return Err(InteractionPromotionRefusalV1::Unavailable);
        };
        Ok(InteractionPromotionAcceptedV1 {
            revision: transition.revision.0,
            message_sequence: sequence,
        })
    }

    fn execute_with_host_time(
        &mut self,
        client: ClientIdV2,
        request: RequestV2,
        instant_ms: Option<u64>,
    ) -> ReplyV2 {
        count(&mut self.usage.request_offers, &mut self.usage.overflowed);
        let Some(participant) = self.clients.get(&client).cloned() else {
            count(&mut self.usage.refused, &mut self.usage.overflowed);
            return ReplyV2::Refused(RefusalV2::UnknownClient);
        };
        let key = (client, request.request_id);
        let (retirement_identity, retirement_scope) =
            receipt_retirement_bindings(client, request.request_id);
        if self
            .retired_receipts
            .is_retired(retirement_identity, retirement_scope)
        {
            count(&mut self.usage.conflicts, &mut self.usage.overflowed);
            return ReplyV2::Refused(RefusalV2::RequestConflict);
        }
        if let Some(existing) = self.retained.get(&key) {
            if existing.request == request {
                count(&mut self.usage.replayed, &mut self.usage.overflowed);
                return existing.reply.clone();
            }
            count(&mut self.usage.conflicts, &mut self.usage.overflowed);
            return ReplyV2::Refused(RefusalV2::RequestConflict);
        }
        if !self.valid_command_shape(&request.command) {
            count(&mut self.usage.refused, &mut self.usage.overflowed);
            return ReplyV2::Refused(RefusalV2::InvalidInput);
        }
        if self.retained.len() >= self.limits.max_retained_requests {
            count(&mut self.usage.refused, &mut self.usage.overflowed);
            return ReplyV2::Refused(RefusalV2::RequestLimit);
        }
        let admission_time =
            if command_needs_admission_time(&self.engine, &participant, &request.command) {
                match instant_ms {
                    Some(instant_ms) => match self.engine.advance_admission_time(instant_ms) {
                        Ok(time) => Some(time),
                        Err(refusal) => {
                            let reply = ReplyV2::Refused(map_admission_time_refusal(refusal));
                            count(&mut self.usage.refused, &mut self.usage.overflowed);
                            self.retained.insert(
                                key,
                                RetainedRequestV2 {
                                    request,
                                    reply: reply.clone(),
                                },
                            );
                            self.advance_receipt_revision();
                            return reply;
                        }
                    },
                    None => None,
                }
            } else {
                None
            };
        let reply = match self.execute_new(
            client,
            &participant,
            &request.command,
            admission_time.as_ref(),
        ) {
            Ok(outcome) => ReplyV2::Accepted(outcome),
            Err(refusal) => ReplyV2::Refused(refusal),
        };
        match &reply {
            ReplyV2::Accepted(_) => count(&mut self.usage.accepted, &mut self.usage.overflowed),
            ReplyV2::Refused(_) => count(&mut self.usage.refused, &mut self.usage.overflowed),
        }
        self.retained.insert(
            key,
            RetainedRequestV2 {
                request,
                reply: reply.clone(),
            },
        );
        self.advance_receipt_revision();
        reply
    }

    fn execute_new(
        &mut self,
        client: ClientIdV2,
        participant: &ParticipantId,
        command: &CommandV2,
        admission_time: Option<&AdmissionTimeV1>,
    ) -> Result<OutcomeV2, RefusalV2> {
        match command {
            CommandV2::Capabilities => Ok(OutcomeV2::Capabilities(vec![
                CapabilityV2::RoomLifecycleV2,
                CapabilityV2::RetainedHistoryV2,
                CapabilityV2::RoomChangesV1,
                CapabilityV2::ModerationV1,
                CapabilityV2::RoomPolicyV1,
                CapabilityV2::BanObservationV1,
                CapabilityV2::IdentifiedMessagesV1,
                CapabilityV2::AuthorWithdrawalV1,
                CapabilityV2::ModeratorRedactionV1,
                CapabilityV2::IdentifiedRoomChangesV1,
                CapabilityV2::ThreadsV1,
                CapabilityV2::ThreadLifecycleV1,
                CapabilityV2::ReactionsV1,
                CapabilityV2::MessageRevisionsV1,
                CapabilityV2::TopicsV1,
                CapabilityV2::PrivateReadPositionsV1,
                CapabilityV2::IndependentChangeRetentionV1,
                CapabilityV2::TopicRenameV1,
                CapabilityV2::TopicResolutionV1,
                CapabilityV2::TopicSupersessionV1,
                CapabilityV2::TopicAliasesV1,
                CapabilityV2::TopicTagsV1,
                CapabilityV2::ImmutableAttachmentReferencesV1,
                CapabilityV2::VellumArtifactReferencesV1,
                CapabilityV2::ConversationProfilesV1,
                CapabilityV2::PrivateAttentionPreferencesV1,
                CapabilityV2::SharedPinsV1,
                CapabilityV2::PrivateSavedReferencesV1,
                CapabilityV2::BoundedPollsV1,
                CapabilityV2::TopicCollectionsV1,
                CapabilityV2::AdmissionModerationControlsV1,
            ])),
            CommandV2::Rooms { cursor, limit } => self.rooms(client, participant, *cursor, *limit),
            CommandV2::ConversationProfile { room_id } => {
                let status = self
                    .engine
                    .conversation_profile(participant, &RoomId::new(room_id))
                    .map_err(map_observation_refusal)?;
                Ok(OutcomeV2::ConversationProfile(
                    ConversationProfileStatusV1 {
                        revision: status.revision.0,
                        room_id: status.room_id.as_str().to_owned(),
                        profile: map_conversation_profile_from_engine(status.profile),
                        founding_participant_ids: status
                            .founding_members
                            .into_iter()
                            .map(|participant| participant.as_str().to_owned())
                            .collect(),
                    },
                ))
            }
            CommandV2::AdmissionControl { room_id } => {
                let status = self
                    .engine
                    .admission_control(participant, &RoomId::new(room_id))
                    .map_err(map_observation_refusal)?;
                Ok(OutcomeV2::AdmissionControl(AdmissionControlStatusV1 {
                    revision: status.revision.0,
                    room_id: status.room_id.as_str().to_owned(),
                    minimum_message_interval_ms: status.minimum_message_interval_ms,
                    observer_posting_restricted: status.observer_posting_restricted,
                }))
            }
            CommandV2::MessageReports { room_id } => self
                .engine
                .message_reports(participant, &RoomId::new(room_id))
                .map(|reports| {
                    OutcomeV2::MessageReports(
                        reports
                            .into_iter()
                            .map(map_message_report_from_engine)
                            .collect(),
                    )
                })
                .map_err(map_message_report_refusal),
            CommandV2::ModerationReports { room_id } => self
                .engine
                .moderation_reports(participant, &RoomId::new(room_id))
                .map(|reports| {
                    OutcomeV2::MessageReports(
                        reports
                            .into_iter()
                            .map(map_message_report_from_engine)
                            .collect(),
                    )
                })
                .map_err(map_message_report_refusal),
            CommandV2::SubmitMessageReport {
                room_id,
                observed_revision,
                message_id,
                reason,
                causation,
            } => self
                .engine
                .submit_message_report(
                    participant,
                    &RoomId::new(room_id),
                    MessageId::from_bytes(message_id.into_bytes()),
                    Revision(*observed_revision),
                    ModerationReasonV1::new(reason),
                    ModerationCausationV1::new(*causation),
                )
                .map(|report| {
                    OutcomeV2::MessageReportSubmitted(map_message_report_from_engine(report))
                })
                .map_err(map_message_report_refusal),
            CommandV2::ReviewMessageReport {
                room_id,
                report_id,
                disposition,
                reason,
                causation,
            } => {
                let report_id = EngineMessageReportIdV1::new(report_id.0)
                    .ok_or(RefusalV2::InvalidMessageReport)?;
                self.engine
                    .review_message_report(
                        participant,
                        &RoomId::new(room_id),
                        report_id,
                        map_report_disposition_to_engine(*disposition),
                        ModerationReasonV1::new(reason),
                        ModerationCausationV1::new(*causation),
                    )
                    .map(|report| {
                        OutcomeV2::MessageReportReviewed(map_message_report_from_engine(report))
                    })
                    .map_err(map_message_report_refusal)
            }
            CommandV2::Members {
                room_id,
                cursor,
                limit,
            } => self.members(client, participant, room_id, *cursor, *limit),
            CommandV2::Bans {
                room_id,
                cursor,
                limit,
            } => self.bans(client, participant, room_id, *cursor, *limit),
            CommandV2::Messages {
                room_id,
                cursor,
                limit,
            } => self.messages(client, participant, room_id, *cursor, *limit),
            CommandV2::IdentifiedMessages {
                room_id,
                cursor,
                limit,
            } => self.identified_messages(client, participant, room_id, *cursor, *limit),
            CommandV2::AttachmentMessages {
                room_id,
                cursor,
                limit,
            } => self.attachment_messages(client, participant, room_id, *cursor, *limit),
            CommandV2::VellumArtifactMessages {
                room_id,
                cursor,
                limit,
            } => self.vellum_artifact_messages(client, participant, room_id, *cursor, *limit),
            CommandV2::OpenChanges {
                room_id,
                observed_revision,
            } => self.open_changes(client, participant, room_id, *observed_revision),
            CommandV2::Changes {
                room_id,
                cursor,
                limit,
            } => self.changes(client, participant, room_id, *cursor, *limit),
            CommandV2::OpenIdentifiedChanges {
                room_id,
                observed_revision,
            } => self.open_identified_changes(client, participant, room_id, *observed_revision),
            CommandV2::IdentifiedChanges {
                room_id,
                cursor,
                limit,
            } => self.identified_changes(client, participant, room_id, *cursor, *limit),
            CommandV2::OpenAttachmentChanges {
                room_id,
                observed_revision,
            } => self.open_attachment_changes(client, participant, room_id, *observed_revision),
            CommandV2::AttachmentChanges {
                room_id,
                cursor,
                limit,
            } => self.attachment_changes(client, participant, room_id, *cursor, *limit),
            CommandV2::ThreadRoots {
                room_id,
                cursor,
                limit,
            } => self.thread_roots(client, participant, room_id, *cursor, *limit),
            CommandV2::ThreadReplies {
                room_id,
                root_message_id,
                cursor,
                limit,
            } => self.thread_replies(
                client,
                participant,
                room_id,
                *root_message_id,
                *cursor,
                *limit,
            ),
            CommandV2::OpenThreadedChanges {
                room_id,
                observed_revision,
            } => self.open_threaded_changes(client, participant, room_id, *observed_revision),
            CommandV2::ThreadedChanges {
                room_id,
                cursor,
                limit,
            } => self.threaded_changes(client, participant, room_id, *cursor, *limit),
            CommandV2::OpenTopicChanges {
                room_id,
                observed_revision,
            } => self.open_topic_changes(client, participant, room_id, *observed_revision),
            CommandV2::TopicChanges {
                room_id,
                cursor,
                limit,
            } => self.topic_changes(client, participant, room_id, *cursor, *limit),
            CommandV2::ThreadStatus {
                room_id,
                root_message_id,
            } => self.thread_status(participant, room_id, *root_message_id),
            CommandV2::ReactionStatus {
                room_id,
                message_id,
            } => self.reaction_status(participant, room_id, *message_id),
            CommandV2::MessageRevisions {
                room_id,
                message_id,
                cursor,
                limit,
            } => self.message_revisions(client, participant, room_id, *message_id, *cursor, *limit),
            CommandV2::Topics {
                room_id,
                cursor,
                limit,
            } => self.topics(client, participant, room_id, *cursor, *limit),
            CommandV2::TopicCollection {
                room_id,
                filter,
                cursor,
                limit,
            } => self.topic_collection(client, participant, room_id, filter, *cursor, *limit),
            CommandV2::TopicMessages {
                room_id,
                topic_id,
                cursor,
                limit,
            } => self.topic_messages(client, participant, room_id, *topic_id, *cursor, *limit),
            CommandV2::TopicStatus { room_id, topic_id } => {
                self.topic_status(participant, room_id, *topic_id)
            }
            CommandV2::TopicSupersessionStatus { room_id, topic_id } => {
                self.topic_supersession_status(participant, room_id, *topic_id)
            }
            CommandV2::TopicAliasStatus { room_id, topic_id } => {
                self.topic_alias_status(participant, room_id, *topic_id)
            }
            CommandV2::TopicTagStatus { room_id, topic_id } => {
                self.topic_tag_status(participant, room_id, *topic_id)
            }
            CommandV2::ReadPosition { room_id, scope } => {
                self.read_position(participant, room_id, *scope)
            }
            CommandV2::AdvanceReadPosition {
                room_id,
                observed_revision,
                scope,
                target_message_id,
            } => self.advance_read_position(
                participant,
                room_id,
                *observed_revision,
                *scope,
                *target_message_id,
            ),
            CommandV2::AttentionPreference { room_id, scope } => {
                self.attention_preference(participant, room_id, *scope)
            }
            CommandV2::SetAttentionPreference {
                room_id,
                observed_revision,
                scope,
                preference,
            } => self.set_attention_preference(
                participant,
                room_id,
                *observed_revision,
                *scope,
                *preference,
            ),
            CommandV2::Pins { room_id } => self.pins(participant, room_id),
            CommandV2::SetPin {
                room_id,
                expected_revision,
                message_id,
                pinned,
            } => self.mutate(
                *expected_revision,
                EngineCommand::SetPin {
                    room_id: RoomId::new(room_id),
                    actor: participant.clone(),
                    message_id: MessageId::from_bytes(message_id.into_bytes()),
                    pinned: *pinned,
                },
            ),
            CommandV2::SavedReference {
                room_id,
                message_id,
            } => self.saved_reference(participant, room_id, *message_id),
            CommandV2::SavedReferences { room_id } => self.saved_references(participant, room_id),
            CommandV2::SetSavedReference {
                room_id,
                observed_revision,
                message_id,
                saved,
            } => self.set_saved_reference(
                participant,
                room_id,
                *observed_revision,
                *message_id,
                *saved,
            ),
            CommandV2::Polls { room_id } => self
                .engine
                .polls(participant, &RoomId::new(room_id))
                .map(|polls| OutcomeV2::Polls(polls.into_iter().map(map_poll_status).collect()))
                .map_err(map_poll_observation_refusal),
            CommandV2::Poll { room_id, poll_id } => self
                .engine
                .poll(
                    participant,
                    &RoomId::new(room_id),
                    EnginePollId::from_bytes(poll_id.into_bytes()),
                )
                .map(|poll| OutcomeV2::Poll(map_poll_status(poll)))
                .map_err(map_poll_observation_refusal),
            CommandV2::PollBallot { room_id, poll_id } => self
                .engine
                .poll_ballot(
                    participant,
                    &RoomId::new(room_id),
                    EnginePollId::from_bytes(poll_id.into_bytes()),
                )
                .map(|ballot| {
                    OutcomeV2::PollBallot(PollBallotV1 {
                        poll_id: PollIdV1::new(ballot.poll_id().into_bytes()),
                        selections: ballot.selections().to_vec(),
                    })
                })
                .map_err(map_poll_observation_refusal),
            CommandV2::CreatePoll {
                room_id,
                expected_revision,
                prompt,
                options,
                eligible_participant_ids,
                max_choices,
            } => self.mutate(
                *expected_revision,
                EngineCommand::CreatePoll {
                    room_id: RoomId::new(room_id),
                    actor: participant.clone(),
                    prompt: prompt.clone(),
                    options: options.clone(),
                    eligible_participants: eligible_participant_ids
                        .iter()
                        .map(ParticipantId::new)
                        .collect(),
                    max_choices: *max_choices,
                },
            ),
            CommandV2::SetPollBallot {
                room_id,
                expected_revision,
                poll_id,
                selections,
            } => self.mutate(
                *expected_revision,
                EngineCommand::SetPollBallot {
                    room_id: RoomId::new(room_id),
                    actor: participant.clone(),
                    poll_id: EnginePollId::from_bytes(poll_id.into_bytes()),
                    selections: selections.clone(),
                },
            ),
            CommandV2::SetPollOpen {
                room_id,
                expected_revision,
                poll_id,
                open,
            } => self.mutate(
                *expected_revision,
                EngineCommand::SetPollOpen {
                    room_id: RoomId::new(room_id),
                    actor: participant.clone(),
                    poll_id: EnginePollId::from_bytes(poll_id.into_bytes()),
                    open: *open,
                },
            ),
            CommandV2::CreateTopic {
                room_id,
                expected_revision,
                name,
            } => self.mutate(
                *expected_revision,
                EngineCommand::CreateTopic {
                    room_id: RoomId::new(room_id),
                    actor: participant.clone(),
                    name: name.clone(),
                },
            ),
            CommandV2::RenameTopic {
                room_id,
                expected_revision,
                topic_id,
                name,
            } => self.mutate(
                *expected_revision,
                EngineCommand::RenameTopic {
                    room_id: RoomId::new(room_id),
                    actor: participant.clone(),
                    topic_id: agora_engine::TopicId::from_bytes(topic_id.into_bytes()),
                    name: name.clone(),
                },
            ),
            CommandV2::ResolveTopic {
                room_id,
                expected_revision,
                topic_id,
            }
            | CommandV2::ReopenTopic {
                room_id,
                expected_revision,
                topic_id,
            } => self.mutate(
                *expected_revision,
                EngineCommand::SetTopicResolved {
                    room_id: RoomId::new(room_id),
                    actor: participant.clone(),
                    topic_id: TopicId::from_bytes(topic_id.into_bytes()),
                    resolved: matches!(command, CommandV2::ResolveTopic { .. }),
                },
            ),
            CommandV2::SupersedeTopic {
                room_id,
                expected_revision,
                topic_id,
                successor_topic_id,
            } => self.mutate(
                *expected_revision,
                EngineCommand::SetTopicSuccessor {
                    room_id: RoomId::new(room_id),
                    actor: participant.clone(),
                    topic_id: TopicId::from_bytes(topic_id.into_bytes()),
                    successor_topic_id: Some(TopicId::from_bytes(successor_topic_id.into_bytes())),
                },
            ),
            CommandV2::ClearTopicSupersession {
                room_id,
                expected_revision,
                topic_id,
            } => self.mutate(
                *expected_revision,
                EngineCommand::SetTopicSuccessor {
                    room_id: RoomId::new(room_id),
                    actor: participant.clone(),
                    topic_id: TopicId::from_bytes(topic_id.into_bytes()),
                    successor_topic_id: None,
                },
            ),
            CommandV2::AddTopicAlias {
                room_id,
                expected_revision,
                topic_id,
                alias,
            }
            | CommandV2::RemoveTopicAlias {
                room_id,
                expected_revision,
                topic_id,
                alias,
            } => self.mutate(
                *expected_revision,
                EngineCommand::SetTopicAlias {
                    room_id: RoomId::new(room_id),
                    actor: participant.clone(),
                    topic_id: TopicId::from_bytes(topic_id.into_bytes()),
                    alias: alias.clone(),
                    added: matches!(command, CommandV2::AddTopicAlias { .. }),
                },
            ),
            CommandV2::AddTopicTag {
                room_id,
                expected_revision,
                topic_id,
                tag,
            }
            | CommandV2::RemoveTopicTag {
                room_id,
                expected_revision,
                topic_id,
                tag,
            } => self.mutate(
                *expected_revision,
                EngineCommand::SetTopicTag {
                    room_id: RoomId::new(room_id),
                    actor: participant.clone(),
                    topic_id: TopicId::from_bytes(topic_id.into_bytes()),
                    tag: tag.clone(),
                    added: matches!(command, CommandV2::AddTopicTag { .. }),
                },
            ),
            CommandV2::SpeakInTopic {
                room_id,
                expected_revision,
                topic_id,
                logical_time,
                body,
                addressed_to,
            } => self.mutate_at(
                *expected_revision,
                EngineCommand::SpeakInTopic {
                    room_id: RoomId::new(room_id),
                    author: participant.clone(),
                    topic_id: TopicId::from_bytes(topic_id.into_bytes()),
                    at: agora_engine::LogicalTime(*logical_time),
                    body: body.clone(),
                    addressed_to: addressed_to.iter().map(ParticipantId::new).collect(),
                },
                admission_time,
            ),
            CommandV2::CreateRoom {
                room_id,
                name,
                admission,
                expected_revision,
            } => self.mutate(
                *expected_revision,
                EngineCommand::CreateRoom {
                    room_id: RoomId::new(room_id),
                    name: name.clone(),
                    owner: participant.clone(),
                    admission: map_admission_to_engine(*admission),
                },
            ),
            CommandV2::CreatePeerConversation {
                room_id,
                name,
                profile,
                founding_participant_ids,
                expected_revision,
            } => self.mutate(
                *expected_revision,
                EngineCommand::CreatePeerConversation {
                    room_id: RoomId::new(room_id),
                    name: name.clone(),
                    initiator: participant.clone(),
                    profile: map_conversation_profile_to_engine(*profile),
                    founding_members: founding_participant_ids
                        .iter()
                        .map(ParticipantId::new)
                        .collect(),
                },
            ),
            CommandV2::EditMessage {
                room_id,
                expected_revision,
                message_id,
                logical_time,
                body,
            } => self.mutate(
                *expected_revision,
                EngineCommand::EditMessage {
                    room_id: RoomId::new(room_id),
                    actor: participant.clone(),
                    message_id: MessageId::from_bytes(message_id.into_bytes()),
                    at: agora_engine::LogicalTime(*logical_time),
                    body: body.clone(),
                },
            ),
            CommandV2::AddReaction {
                room_id,
                expected_revision,
                message_id,
                key,
            }
            | CommandV2::RemoveReaction {
                room_id,
                expected_revision,
                message_id,
                key,
            } => self.mutate(
                *expected_revision,
                EngineCommand::SetReaction {
                    room_id: RoomId::new(room_id),
                    actor: participant.clone(),
                    message_id: MessageId::from_bytes(message_id.into_bytes()),
                    key: key.clone(),
                    reacted: matches!(command, CommandV2::AddReaction { .. }),
                },
            ),
            CommandV2::Invite {
                room_id,
                participant_id,
                expected_revision,
            } => self.mutate(
                *expected_revision,
                EngineCommand::Invite {
                    room_id: RoomId::new(room_id),
                    actor: participant.clone(),
                    participant: ParticipantId::new(participant_id),
                },
            ),
            CommandV2::Join {
                room_id,
                expected_revision,
            } => self.mutate(
                *expected_revision,
                EngineCommand::Join {
                    room_id: RoomId::new(room_id),
                    participant: participant.clone(),
                },
            ),
            CommandV2::Leave {
                room_id,
                expected_revision,
            } => self.mutate(
                *expected_revision,
                EngineCommand::Leave {
                    room_id: RoomId::new(room_id),
                    participant: participant.clone(),
                },
            ),
            CommandV2::SetRole {
                room_id,
                participant_id,
                role,
                expected_revision,
            } => self.mutate(
                *expected_revision,
                EngineCommand::SetRole {
                    room_id: RoomId::new(room_id),
                    actor: participant.clone(),
                    participant: ParticipantId::new(participant_id),
                    role: map_role_to_engine(*role),
                },
            ),
            CommandV2::SetAdmissionPolicy {
                room_id,
                admission,
                expected_revision,
            } => self.mutate(
                *expected_revision,
                EngineCommand::SetAdmissionPolicy {
                    room_id: RoomId::new(room_id),
                    actor: participant.clone(),
                    admission: map_admission_to_engine(*admission),
                },
            ),
            CommandV2::SetMinimumMessageInterval {
                room_id,
                minimum_interval_ms,
                expected_revision,
            } => self.mutate(
                *expected_revision,
                EngineCommand::SetMinimumMessageInterval {
                    room_id: RoomId::new(room_id),
                    actor: participant.clone(),
                    minimum_interval_ms: *minimum_interval_ms,
                },
            ),
            CommandV2::Kick {
                room_id,
                participant_id,
                reason,
                causation,
                expected_revision,
            } => self.mutate(
                *expected_revision,
                EngineCommand::Kick {
                    room_id: RoomId::new(room_id),
                    actor: participant.clone(),
                    participant: ParticipantId::new(participant_id),
                    reason: ModerationReasonV1::new(reason),
                    causation: ModerationCausationV1::new(*causation),
                },
            ),
            CommandV2::Ban {
                room_id,
                participant_id,
                reason,
                causation,
                expected_revision,
            } => self.mutate(
                *expected_revision,
                EngineCommand::Ban {
                    room_id: RoomId::new(room_id),
                    actor: participant.clone(),
                    participant: ParticipantId::new(participant_id),
                    reason: ModerationReasonV1::new(reason),
                    causation: ModerationCausationV1::new(*causation),
                },
            ),
            CommandV2::Unban {
                room_id,
                participant_id,
                reason,
                causation,
                expected_revision,
            } => self.mutate(
                *expected_revision,
                EngineCommand::Unban {
                    room_id: RoomId::new(room_id),
                    actor: participant.clone(),
                    participant: ParticipantId::new(participant_id),
                    reason: ModerationReasonV1::new(reason),
                    causation: ModerationCausationV1::new(*causation),
                },
            ),
            CommandV2::SetPostingRestricted {
                room_id,
                participant_id,
                restricted,
                reason,
                causation,
                expected_revision,
            } => self.mutate(
                *expected_revision,
                EngineCommand::SetPostingRestricted {
                    room_id: RoomId::new(room_id),
                    actor: participant.clone(),
                    participant: ParticipantId::new(participant_id),
                    restricted: *restricted,
                    reason: ModerationReasonV1::new(reason),
                    causation: ModerationCausationV1::new(*causation),
                },
            ),
            CommandV2::Speak {
                room_id,
                expected_revision,
                logical_time,
                body,
                addressed_to,
            } => {
                let command = if addressed_to.is_empty() {
                    EngineCommand::Speak {
                        room_id: RoomId::new(room_id),
                        author: participant.clone(),
                        at: agora_engine::LogicalTime(*logical_time),
                        body: body.clone(),
                    }
                } else {
                    EngineCommand::SpeakAddressed {
                        room_id: RoomId::new(room_id),
                        author: participant.clone(),
                        at: agora_engine::LogicalTime(*logical_time),
                        body: body.clone(),
                        addressed_to: addressed_to.iter().map(ParticipantId::new).collect(),
                    }
                };
                self.mutate_at(*expected_revision, command, admission_time)
            }
            CommandV2::SpeakIdentified {
                room_id,
                expected_revision,
                logical_time,
                body,
                addressed_to,
            } => {
                let command = if addressed_to.is_empty() {
                    EngineCommand::Speak {
                        room_id: RoomId::new(room_id),
                        author: participant.clone(),
                        at: agora_engine::LogicalTime(*logical_time),
                        body: body.clone(),
                    }
                } else {
                    EngineCommand::SpeakAddressed {
                        room_id: RoomId::new(room_id),
                        author: participant.clone(),
                        at: agora_engine::LogicalTime(*logical_time),
                        body: body.clone(),
                        addressed_to: addressed_to.iter().map(ParticipantId::new).collect(),
                    }
                };
                self.mutate_identified(*expected_revision, command, admission_time)
            }
            CommandV2::SpeakReply {
                room_id,
                expected_revision,
                root_message_id,
                logical_time,
                body,
                addressed_to,
            } => self.mutate_reply(
                *expected_revision,
                EngineCommand::SpeakReply {
                    room_id: RoomId::new(room_id),
                    author: participant.clone(),
                    root_message_id: MessageId::from_bytes(root_message_id.into_bytes()),
                    at: agora_engine::LogicalTime(*logical_time),
                    body: body.clone(),
                    addressed_to: addressed_to.iter().map(ParticipantId::new).collect(),
                },
                admission_time,
            ),
            CommandV2::SpeakWithAttachments {
                room_id,
                expected_revision,
                target,
                logical_time,
                body,
                addressed_to,
                attachments,
            } => self.mutate_attachments(
                *expected_revision,
                EngineCommand::SpeakWithAttachments {
                    room_id: RoomId::new(room_id),
                    author: participant.clone(),
                    target: map_attachment_target_to_engine(*target),
                    at: agora_engine::LogicalTime(*logical_time),
                    body: body.clone(),
                    addressed_to: addressed_to.iter().map(ParticipantId::new).collect(),
                    attachments: attachments
                        .iter()
                        .cloned()
                        .map(map_attachment_descriptor_to_engine)
                        .collect(),
                },
                admission_time,
            ),
            CommandV2::SpeakWithVellumArtifacts {
                room_id,
                expected_revision,
                target,
                logical_time,
                body,
                addressed_to,
                references,
            } => self.mutate_vellum_artifacts(
                *expected_revision,
                EngineCommand::SpeakWithVellumArtifacts {
                    room_id: RoomId::new(room_id),
                    author: participant.clone(),
                    target: map_vellum_artifact_target_to_engine(*target),
                    at: agora_engine::LogicalTime(*logical_time),
                    body: body.clone(),
                    addressed_to: addressed_to.iter().map(ParticipantId::new).collect(),
                    references: references
                        .iter()
                        .copied()
                        .map(map_vellum_artifact_reference_to_engine)
                        .collect(),
                },
                admission_time,
            ),
            CommandV2::WithdrawMessage {
                room_id,
                expected_revision,
                message_id,
            } => self.mutate(
                *expected_revision,
                EngineCommand::WithdrawMessage {
                    room_id: RoomId::new(room_id),
                    actor: participant.clone(),
                    message_id: MessageId::from_bytes(message_id.into_bytes()),
                },
            ),
            CommandV2::RedactMessage {
                room_id,
                expected_revision,
                message_id,
                reason,
                causation,
            } => self.mutate(
                *expected_revision,
                EngineCommand::RedactMessage {
                    room_id: RoomId::new(room_id),
                    actor: participant.clone(),
                    message_id: MessageId::from_bytes(message_id.into_bytes()),
                    reason: ModerationReasonV1::new(reason),
                    causation: ModerationCausationV1::new(*causation),
                },
            ),
            CommandV2::ArchiveThread {
                room_id,
                expected_revision,
                root_message_id,
                reason,
                causation,
            }
            | CommandV2::ReopenThread {
                room_id,
                expected_revision,
                root_message_id,
                reason,
                causation,
            } => self.mutate(
                *expected_revision,
                EngineCommand::SetThreadArchived {
                    room_id: RoomId::new(room_id),
                    actor: participant.clone(),
                    root_message_id: MessageId::from_bytes(root_message_id.into_bytes()),
                    archived: matches!(command, CommandV2::ArchiveThread { .. }),
                    reason: ModerationReasonV1::new(reason),
                    causation: ModerationCausationV1::new(*causation),
                },
            ),
        }
    }

    fn mutate_identified(
        &mut self,
        expected_revision: u64,
        command: EngineCommand,
        admission_time: Option<&AdmissionTimeV1>,
    ) -> Result<OutcomeV2, RefusalV2> {
        let transition = apply_revisioned_with_optional_time(
            &mut self.engine,
            expected_revision,
            command,
            admission_time,
        )?;
        let EngineOutcome::MessageAccepted { room_id, sequence } = transition.outcome else {
            return Err(RefusalV2::Unavailable);
        };
        let message_id = self
            .engine
            .message(&room_id, sequence)
            .ok_or(RefusalV2::Unavailable)?
            .id();
        Ok(OutcomeV2::IdentifiedMessageAccepted {
            revision: transition.revision.0,
            message_id: MessageIdV1::new(message_id.into_bytes()),
            sequence,
        })
    }

    fn mutate_attachments(
        &mut self,
        expected_revision: u64,
        command: EngineCommand,
        admission_time: Option<&AdmissionTimeV1>,
    ) -> Result<OutcomeV2, RefusalV2> {
        let transition = apply_revisioned_with_optional_time(
            &mut self.engine,
            expected_revision,
            command,
            admission_time,
        )?;
        let EngineOutcome::AttachmentMessageAccepted {
            message_id,
            sequence,
            attachment_count,
            ..
        } = transition.outcome
        else {
            return Err(RefusalV2::Unavailable);
        };
        Ok(OutcomeV2::AttachmentMessageAccepted {
            revision: transition.revision.0,
            message_id: MessageIdV1::new(message_id.into_bytes()),
            sequence,
            attachment_count: u32::from(attachment_count),
        })
    }

    fn mutate_vellum_artifacts(
        &mut self,
        expected_revision: u64,
        command: EngineCommand,
        admission_time: Option<&AdmissionTimeV1>,
    ) -> Result<OutcomeV2, RefusalV2> {
        let transition = apply_revisioned_with_optional_time(
            &mut self.engine,
            expected_revision,
            command,
            admission_time,
        )?;
        let EngineOutcome::VellumArtifactMessageAccepted {
            message_id,
            sequence,
            reference_count,
            ..
        } = transition.outcome
        else {
            return Err(RefusalV2::Unavailable);
        };
        Ok(OutcomeV2::VellumArtifactMessageAccepted {
            revision: transition.revision.0,
            message_id: MessageIdV1::new(message_id.into_bytes()),
            sequence,
            reference_count: u32::from(reference_count),
        })
    }

    fn mutate_reply(
        &mut self,
        expected_revision: u64,
        command: EngineCommand,
        admission_time: Option<&AdmissionTimeV1>,
    ) -> Result<OutcomeV2, RefusalV2> {
        let transition = apply_revisioned_with_optional_time(
            &mut self.engine,
            expected_revision,
            command,
            admission_time,
        )?;
        let EngineOutcome::ReplyAccepted {
            room_id,
            root_message_id,
            sequence,
        } = transition.outcome
        else {
            return Err(RefusalV2::Unavailable);
        };
        let message_id = self
            .engine
            .message(&room_id, sequence)
            .ok_or(RefusalV2::Unavailable)?
            .id();
        Ok(OutcomeV2::ReplyAccepted {
            revision: transition.revision.0,
            message_id: MessageIdV1::new(message_id.into_bytes()),
            root_message_id: MessageIdV1::new(root_message_id.into_bytes()),
            sequence,
        })
    }

    fn mutate(
        &mut self,
        expected_revision: u64,
        command: EngineCommand,
    ) -> Result<OutcomeV2, RefusalV2> {
        let transition = self
            .engine
            .apply_revisioned(RevisionedCommandV2 {
                expected_revision: Revision(expected_revision),
                command,
            })
            .map_err(map_engine_refusal)?;
        self.map_transition(transition)
    }

    fn map_transition(
        &mut self,
        transition: agora_engine::TransitionV2,
    ) -> Result<OutcomeV2, RefusalV2> {
        let revision = transition.revision.0;
        match transition.outcome {
            EngineOutcome::RoomCreated(_) => Ok(OutcomeV2::RoomCreated { revision }),
            EngineOutcome::PeerConversationCreated { profile, .. } => {
                Ok(OutcomeV2::PeerConversationCreated {
                    revision,
                    profile: map_conversation_profile_from_engine(profile),
                })
            }
            EngineOutcome::InvitationRecorded { .. } => {
                Ok(OutcomeV2::InvitationRecorded { revision })
            }
            EngineOutcome::Joined { .. } => Ok(OutcomeV2::Joined { revision }),
            EngineOutcome::Left { .. } => Ok(OutcomeV2::Left { revision }),
            EngineOutcome::RoleChanged { .. } => Ok(OutcomeV2::RoleChanged { revision }),
            EngineOutcome::RoomPolicyChanged { .. } => {
                Ok(OutcomeV2::RoomPolicyChanged { revision })
            }
            EngineOutcome::MessageIntervalChanged { .. } => {
                Ok(OutcomeV2::MessageIntervalChanged { revision })
            }
            EngineOutcome::Kicked { .. } => Ok(OutcomeV2::Kicked { revision }),
            EngineOutcome::Banned { .. } => Ok(OutcomeV2::Banned { revision }),
            EngineOutcome::Unbanned { .. } => Ok(OutcomeV2::Unbanned { revision }),
            EngineOutcome::PostingRestrictionChanged { .. } => {
                Ok(OutcomeV2::PostingRestrictionChanged { revision })
            }
            EngineOutcome::MessageAccepted { sequence, .. } => {
                Ok(OutcomeV2::MessageAccepted { revision, sequence })
            }
            EngineOutcome::TopicMessageAccepted {
                room_id,
                topic_id,
                sequence,
            } => {
                let message_id = self
                    .engine
                    .message(&room_id, sequence)
                    .ok_or(RefusalV2::Unavailable)?
                    .id();
                Ok(OutcomeV2::TopicMessageAccepted {
                    revision,
                    message_id: MessageIdV1::new(message_id.into_bytes()),
                    topic_id: TopicIdV1::new(topic_id.into_bytes()),
                    sequence,
                })
            }
            EngineOutcome::TopicCreated { topic_id, .. } => Ok(OutcomeV2::TopicCreated {
                revision,
                topic_id: TopicIdV1::new(topic_id.into_bytes()),
            }),
            EngineOutcome::TopicRenamed { topic_id, .. } => Ok(OutcomeV2::TopicRenamed {
                revision,
                topic_id: TopicIdV1::new(topic_id.into_bytes()),
            }),
            EngineOutcome::TopicLifecycleChanged {
                topic_id, resolved, ..
            } => Ok(if resolved {
                OutcomeV2::TopicResolved {
                    revision,
                    topic_id: TopicIdV1::new(topic_id.into_bytes()),
                }
            } else {
                OutcomeV2::TopicReopened {
                    revision,
                    topic_id: TopicIdV1::new(topic_id.into_bytes()),
                }
            }),
            EngineOutcome::TopicSupersessionChanged {
                topic_id,
                successor_topic_id,
                ..
            } => Ok(match successor_topic_id {
                Some(successor_topic_id) => OutcomeV2::TopicSuperseded {
                    revision,
                    topic_id: TopicIdV1::new(topic_id.into_bytes()),
                    successor_topic_id: TopicIdV1::new(successor_topic_id.into_bytes()),
                },
                None => OutcomeV2::TopicSupersessionCleared {
                    revision,
                    topic_id: TopicIdV1::new(topic_id.into_bytes()),
                },
            }),
            EngineOutcome::TopicAliasChanged {
                topic_id,
                alias,
                added,
                ..
            } => Ok(if added {
                OutcomeV2::TopicAliasAdded {
                    revision,
                    topic_id: TopicIdV1::new(topic_id.into_bytes()),
                    alias,
                }
            } else {
                OutcomeV2::TopicAliasRemoved {
                    revision,
                    topic_id: TopicIdV1::new(topic_id.into_bytes()),
                    alias,
                }
            }),
            EngineOutcome::TopicTagChanged {
                topic_id,
                tag,
                added,
                ..
            } => Ok(if added {
                OutcomeV2::TopicTagAdded {
                    revision,
                    topic_id: TopicIdV1::new(topic_id.into_bytes()),
                    tag,
                }
            } else {
                OutcomeV2::TopicTagRemoved {
                    revision,
                    topic_id: TopicIdV1::new(topic_id.into_bytes()),
                    tag,
                }
            }),
            EngineOutcome::ReplyAccepted { .. } => Err(RefusalV2::Unavailable),
            EngineOutcome::AttachmentMessageAccepted { .. } => Err(RefusalV2::Unavailable),
            EngineOutcome::MessageWithdrawn {
                message_id,
                sequence,
                ..
            } => Ok(OutcomeV2::MessageWithdrawn {
                revision,
                message_id: MessageIdV1::new(message_id.into_bytes()),
                sequence,
            }),
            EngineOutcome::MessageRedacted {
                message_id,
                sequence,
                ..
            } => Ok(OutcomeV2::MessageRedacted {
                revision,
                message_id: MessageIdV1::new(message_id.into_bytes()),
                sequence,
            }),
            EngineOutcome::ThreadLifecycleChanged {
                root_message_id,
                archived,
                ..
            } => Ok(if archived {
                OutcomeV2::ThreadArchived {
                    revision,
                    root_message_id: MessageIdV1::new(root_message_id.into_bytes()),
                }
            } else {
                OutcomeV2::ThreadReopened {
                    revision,
                    root_message_id: MessageIdV1::new(root_message_id.into_bytes()),
                }
            }),
            EngineOutcome::ReactionChanged {
                message_id,
                reacted,
                ..
            } => Ok(if reacted {
                OutcomeV2::ReactionAdded {
                    revision,
                    message_id: MessageIdV1::new(message_id.into_bytes()),
                }
            } else {
                OutcomeV2::ReactionRemoved {
                    revision,
                    message_id: MessageIdV1::new(message_id.into_bytes()),
                }
            }),
            EngineOutcome::PinChanged {
                message_id,
                original_sequence,
                pinned,
                ..
            } => Ok(OutcomeV2::PinChanged {
                revision,
                message_id: MessageIdV1::new(message_id.into_bytes()),
                original_sequence,
                pinned,
            }),
            EngineOutcome::MessageEdited {
                message_id,
                message_revision,
                ..
            } => Ok(OutcomeV2::MessageEdited {
                revision,
                message_id: MessageIdV1::new(message_id.into_bytes()),
                message_revision,
            }),
            EngineOutcome::PollCreated { poll_id, .. } => Ok(OutcomeV2::PollCreated {
                revision,
                poll_id: PollIdV1::new(poll_id.into_bytes()),
            }),
            EngineOutcome::PollBallotChanged {
                poll_id,
                totals,
                total_ballots,
                ..
            } => Ok(OutcomeV2::PollBallotChanged {
                revision,
                poll_id: PollIdV1::new(poll_id.into_bytes()),
                totals,
                total_ballots,
            }),
            EngineOutcome::PollLifecycleChanged { poll_id, open, .. } => {
                Ok(OutcomeV2::PollLifecycleChanged {
                    revision,
                    poll_id: PollIdV1::new(poll_id.into_bytes()),
                    open,
                })
            }
            EngineOutcome::ParticipantRegistered(_)
            | EngineOutcome::VellumArtifactMessageAccepted { .. } => Err(RefusalV2::Unavailable),
        }
    }

    fn mutate_at(
        &mut self,
        expected_revision: u64,
        command: EngineCommand,
        admission_time: Option<&AdmissionTimeV1>,
    ) -> Result<OutcomeV2, RefusalV2> {
        let transition = apply_revisioned_with_optional_time(
            &mut self.engine,
            expected_revision,
            command,
            admission_time,
        )?;
        self.map_transition(transition)
    }

    fn thread_status(
        &self,
        observer: &ParticipantId,
        room_id: &str,
        root_message_id: MessageIdV1,
    ) -> Result<OutcomeV2, RefusalV2> {
        let status = self
            .engine
            .thread_status(
                observer,
                &RoomId::new(room_id),
                MessageId::from_bytes(root_message_id.into_bytes()),
            )
            .map_err(map_observation_refusal)?;
        Ok(OutcomeV2::ThreadStatus(ThreadStatusV1 {
            revision: status.revision.0,
            room_id: status.room_id.as_str().to_owned(),
            root_message_id: MessageIdV1::new(status.root_message_id.into_bytes()),
            state: match status.state {
                agora_engine::ThreadLifecycleStateV1::Active => ThreadLifecycleStateV1::Active,
                agora_engine::ThreadLifecycleStateV1::Archived => ThreadLifecycleStateV1::Archived,
            },
        }))
    }

    fn reaction_status(
        &self,
        observer: &ParticipantId,
        room_id: &str,
        message_id: MessageIdV1,
    ) -> Result<OutcomeV2, RefusalV2> {
        let status = self
            .engine
            .reaction_status(
                observer,
                &RoomId::new(room_id),
                MessageId::from_bytes(message_id.into_bytes()),
            )
            .map_err(map_observation_refusal)?;
        Ok(OutcomeV2::ReactionStatus(ReactionStatusV1 {
            revision: status.revision.0,
            room_id: status.room_id.as_str().to_owned(),
            message_id: MessageIdV1::new(status.message_id.into_bytes()),
            reactions: status
                .reactions
                .into_iter()
                .map(|reaction| ReactionSummaryV1 {
                    key: reaction.key,
                    count: reaction.count,
                    caller_reacted: reaction.caller_reacted,
                })
                .collect(),
        }))
    }

    fn rooms(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        cursor_id: Option<PageCursorV2>,
        limit: u32,
    ) -> Result<OutcomeV2, RefusalV2> {
        let cursor = match cursor_id {
            Some(id) => match self.cursor_for(client, id)? {
                CursorPage::Rooms(cursor) => Some(cursor),
                _ => return Err(RefusalV2::InvalidCursor),
            },
            None => None,
        };
        let page = self
            .engine
            .room_page(observer, cursor.as_ref(), limit as usize)
            .map_err(map_observation_refusal)?;
        let revision = page.revision.0;
        let next_page = page.next.map(CursorPage::Rooms);
        let rooms = page
            .rooms
            .into_iter()
            .map(|room| {
                Ok(RoomSummaryV2 {
                    room_id: room.id.as_str().to_owned(),
                    name: room.name,
                    admission: map_admission_from_engine(room.admission),
                    observer_role: map_role_from_engine(room.observer_role),
                    member_count: room
                        .member_count
                        .try_into()
                        .map_err(|_| RefusalV2::Unavailable)?,
                    retained_message_count: room
                        .retained_message_count
                        .try_into()
                        .map_err(|_| RefusalV2::Unavailable)?,
                    next_message_sequence: room.next_message_sequence,
                })
            })
            .collect::<Result<Vec<_>, RefusalV2>>()?;
        let next = self.replace_cursor(client, cursor_id, next_page)?;
        Ok(OutcomeV2::Rooms(RoomPageV2 {
            revision,
            rooms,
            next,
        }))
    }

    fn members(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        room_id: &str,
        cursor_id: Option<PageCursorV2>,
        limit: u32,
    ) -> Result<OutcomeV2, RefusalV2> {
        let room = RoomId::new(room_id);
        let cursor = match cursor_id {
            Some(id) => match self.cursor_for(client, id)? {
                CursorPage::Members {
                    room_id: bound_room,
                    cursor,
                } if bound_room == room => Some(cursor),
                _ => return Err(RefusalV2::InvalidCursor),
            },
            None => None,
        };
        let page = self
            .engine
            .member_page(observer, &room, cursor.as_ref(), limit as usize)
            .map_err(map_observation_refusal)?;
        let next_page = page.next.map(|cursor| CursorPage::Members {
            room_id: room.clone(),
            cursor,
        });
        let next = self.replace_cursor(client, cursor_id, next_page)?;
        let members = page
            .members
            .into_iter()
            .map(|member| MemberV2 {
                participant_id: member.participant_id.as_str().to_owned(),
                display_name: member.display_name,
                kind: match member.kind {
                    ParticipantKind::Human => ParticipantKindV2::Human,
                    ParticipantKind::Agent => ParticipantKindV2::Agent,
                },
                role: map_role_from_engine(member.role),
            })
            .collect();
        Ok(OutcomeV2::Members(MemberPageV2 {
            revision: page.revision.0,
            room_id: page.room_id.as_str().to_owned(),
            members,
            next,
        }))
    }

    fn messages(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        room_id: &str,
        cursor_id: Option<PageCursorV2>,
        limit: u32,
    ) -> Result<OutcomeV2, RefusalV2> {
        let room = RoomId::new(room_id);
        let cursor = match cursor_id {
            Some(id) => match self.cursor_for(client, id)? {
                CursorPage::Messages {
                    room_id: bound_room,
                    cursor,
                } if bound_room == room => Some(cursor),
                _ => return Err(RefusalV2::InvalidCursor),
            },
            None => None,
        };
        let page = self
            .engine
            .message_page(observer, &room, cursor.as_ref(), limit as usize)
            .map_err(map_observation_refusal)?;
        let next_page = page.next.map(|cursor| CursorPage::Messages {
            room_id: room.clone(),
            cursor,
        });
        let next = self.replace_cursor(client, cursor_id, next_page)?;
        let messages = page
            .messages
            .into_iter()
            .map(|message| MessageV2 {
                sequence: message.sequence(),
                author: message.author().as_str().to_owned(),
                logical_time: message.at().0,
                body: message.body().to_owned(),
                addressed_to: message
                    .addressed_to()
                    .iter()
                    .map(|participant| participant.as_str().to_owned())
                    .collect(),
            })
            .collect();
        Ok(OutcomeV2::Messages(MessagePageV2 {
            revision: page.revision.0,
            room_id: page.room_id.as_str().to_owned(),
            messages,
            oldest_retained_sequence: page.oldest_retained_sequence,
            next_message_sequence: page.next_message_sequence,
            next,
        }))
    }

    fn identified_messages(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        room_id: &str,
        cursor_id: Option<PageCursorV2>,
        limit: u32,
    ) -> Result<OutcomeV2, RefusalV2> {
        let room = RoomId::new(room_id);
        let cursor = match cursor_id {
            Some(id) => match self.cursor_for(client, id)? {
                CursorPage::IdentifiedMessages {
                    room_id: bound_room,
                    cursor,
                } if bound_room == room => Some(cursor),
                _ => return Err(RefusalV2::InvalidCursor),
            },
            None => None,
        };
        let page = self
            .engine
            .message_page(observer, &room, cursor.as_ref(), limit as usize)
            .map_err(map_observation_refusal)?;
        let next_page = page.next.map(|cursor| CursorPage::IdentifiedMessages {
            room_id: room.clone(),
            cursor,
        });
        let next = self.replace_cursor(client, cursor_id, next_page)?;
        let messages = page
            .messages
            .into_iter()
            .map(map_identified_message_from_engine)
            .collect();
        Ok(OutcomeV2::IdentifiedMessages(IdentifiedMessagePageV1 {
            revision: page.revision.0,
            room_id: page.room_id.as_str().to_owned(),
            messages,
            oldest_retained_sequence: page.oldest_retained_sequence,
            next_message_sequence: page.next_message_sequence,
            next,
        }))
    }

    fn attachment_messages(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        room_id: &str,
        cursor_id: Option<PageCursorV2>,
        limit: u32,
    ) -> Result<OutcomeV2, RefusalV2> {
        let room = RoomId::new(room_id);
        let cursor = match cursor_id {
            Some(id) => match self.cursor_for(client, id)? {
                CursorPage::AttachmentMessages {
                    room_id: bound_room,
                    cursor,
                } if bound_room == room => Some(cursor),
                _ => return Err(RefusalV2::InvalidCursor),
            },
            None => None,
        };
        let page = self
            .engine
            .message_page(observer, &room, cursor.as_ref(), limit as usize)
            .map_err(map_observation_refusal)?;
        let next_page = page.next.map(|cursor| CursorPage::AttachmentMessages {
            room_id: room.clone(),
            cursor,
        });
        let next = self.replace_cursor(client, cursor_id, next_page)?;
        let messages = page
            .messages
            .into_iter()
            .filter(|message| !message.attachments().is_empty())
            .map(map_attachment_message_from_engine)
            .collect();
        Ok(OutcomeV2::AttachmentMessages(AttachmentMessagePageV1 {
            revision: page.revision.0,
            room_id: page.room_id.as_str().to_owned(),
            messages,
            oldest_retained_sequence: page.oldest_retained_sequence,
            next_message_sequence: page.next_message_sequence,
            next,
        }))
    }

    fn vellum_artifact_messages(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        room_id: &str,
        cursor_id: Option<PageCursorV2>,
        limit: u32,
    ) -> Result<OutcomeV2, RefusalV2> {
        let room = RoomId::new(room_id);
        let cursor = match cursor_id {
            Some(id) => match self.cursor_for(client, id)? {
                CursorPage::VellumArtifactMessages {
                    room_id: bound_room,
                    cursor,
                } if bound_room == room => Some(cursor),
                _ => return Err(RefusalV2::InvalidCursor),
            },
            None => None,
        };
        let page = self
            .engine
            .message_page(observer, &room, cursor.as_ref(), limit as usize)
            .map_err(map_observation_refusal)?;
        let next_page = page.next.map(|cursor| CursorPage::VellumArtifactMessages {
            room_id: room.clone(),
            cursor,
        });
        let next = self.replace_cursor(client, cursor_id, next_page)?;
        let messages = page
            .messages
            .into_iter()
            .filter(|message| !message.vellum_artifacts().is_empty())
            .map(map_vellum_artifact_message_from_engine)
            .collect();
        Ok(OutcomeV2::VellumArtifactMessages(
            VellumArtifactMessagePageV1 {
                revision: page.revision.0,
                room_id: page.room_id.as_str().to_owned(),
                messages,
                oldest_retained_sequence: page.oldest_retained_sequence,
                next_message_sequence: page.next_message_sequence,
                next,
            },
        ))
    }

    fn message_revisions(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        room_id: &str,
        message_id: MessageIdV1,
        cursor_id: Option<PageCursorV2>,
        limit: u32,
    ) -> Result<OutcomeV2, RefusalV2> {
        let room = RoomId::new(room_id);
        let message = MessageId::from_bytes(message_id.into_bytes());
        let cursor = match cursor_id {
            Some(id) => match self.cursor_for(client, id)? {
                CursorPage::MessageRevisions {
                    room_id: bound_room,
                    message_id: bound_message,
                    cursor,
                } if bound_room == room && bound_message == message => Some(cursor),
                _ => return Err(RefusalV2::InvalidCursor),
            },
            None => None,
        };
        let page = self
            .engine
            .message_revision_page(observer, &room, message, cursor.as_ref(), limit as usize)
            .map_err(map_observation_refusal)?;
        let next_page = page.next.map(|cursor| CursorPage::MessageRevisions {
            room_id: room.clone(),
            message_id: message,
            cursor,
        });
        let next = self.replace_cursor(client, cursor_id, next_page)?;
        Ok(OutcomeV2::MessageRevisions(MessageRevisionPageV1 {
            revision: page.revision.0,
            room_id: page.room_id.as_str().to_owned(),
            message_id: MessageIdV1::new(page.message_id.into_bytes()),
            revisions: page
                .revisions
                .into_iter()
                .map(|revision| MessageRevisionV1 {
                    number: revision.number,
                    editor_id: revision.editor.as_str().to_owned(),
                    logical_time: revision.at.0,
                    availability: match revision.availability {
                        agora_engine::MessageRevisionAvailabilityV1::Available => {
                            MessageRevisionAvailabilityV1::Available
                        }
                        agora_engine::MessageRevisionAvailabilityV1::Tombstoned => {
                            MessageRevisionAvailabilityV1::Tombstoned
                        }
                    },
                    body: revision.body,
                })
                .collect(),
            next,
        }))
    }

    fn topics(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        room_id: &str,
        cursor_id: Option<PageCursorV2>,
        limit: u32,
    ) -> Result<OutcomeV2, RefusalV2> {
        let room = RoomId::new(room_id);
        let cursor = match cursor_id {
            Some(id) => match self.cursor_for(client, id)? {
                CursorPage::Topics {
                    room_id: bound_room,
                    cursor,
                } if bound_room == room => Some(cursor),
                _ => return Err(RefusalV2::InvalidCursor),
            },
            None => None,
        };
        let page = self
            .engine
            .topic_page(observer, &room, cursor.as_ref(), limit as usize)
            .map_err(map_observation_refusal)?;
        let next_page = page.next.map(|cursor| CursorPage::Topics {
            room_id: room.clone(),
            cursor,
        });
        let next = self.replace_cursor(client, cursor_id, next_page)?;
        Ok(OutcomeV2::Topics(TopicPageV1 {
            revision: page.revision.0,
            room_id: page.room_id.as_str().to_owned(),
            topics: page
                .topics
                .into_iter()
                .map(|topic| TopicV1 {
                    topic_id: TopicIdV1::new(topic.id().into_bytes()),
                    name: topic.name().to_owned(),
                    creator_id: topic.creator().as_str().to_owned(),
                })
                .collect(),
            next,
        }))
    }

    fn topic_collection(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        room_id: &str,
        filter: &TopicCollectionFilterV1,
        cursor_id: Option<PageCursorV2>,
        limit: u32,
    ) -> Result<OutcomeV2, RefusalV2> {
        let room = RoomId::new(room_id);
        let engine_filter = EngineTopicCollectionFilterV1 {
            lifecycle: match filter.lifecycle {
                TopicCollectionLifecycleV1::Any => EngineTopicCollectionLifecycleV1::Any,
                TopicCollectionLifecycleV1::Active => EngineTopicCollectionLifecycleV1::Active,
                TopicCollectionLifecycleV1::Resolved => EngineTopicCollectionLifecycleV1::Resolved,
            },
            tags: filter.tags.clone(),
        };
        let cursor = match cursor_id {
            Some(id) => match self.cursor_for(client, id)? {
                CursorPage::TopicCollection {
                    room_id: bound_room,
                    cursor,
                } if bound_room == room => Some(cursor),
                _ => return Err(RefusalV2::InvalidCursor),
            },
            None => None,
        };
        let page = self
            .engine
            .topic_collection_page(
                observer,
                &room,
                &engine_filter,
                cursor.as_ref(),
                limit as usize,
            )
            .map_err(map_observation_refusal)?;
        let next_page = page.next.map(|cursor| CursorPage::TopicCollection {
            room_id: room.clone(),
            cursor,
        });
        let next = self.replace_cursor(client, cursor_id, next_page)?;
        Ok(OutcomeV2::TopicCollection(TopicCollectionPageV1 {
            revision: page.revision.0,
            room_id: page.room_id.as_str().to_owned(),
            filter: filter.clone(),
            topics: page
                .topics
                .into_iter()
                .map(|topic| TopicCollectionEntryV1 {
                    topic_id: TopicIdV1::new(topic.topic_id.into_bytes()),
                    name: topic.name,
                    creator_id: topic.creator.as_str().to_owned(),
                    resolved: topic.resolved,
                    aliases: topic.aliases,
                    tags: topic.tags,
                    successor_topic_id: topic
                        .successor_topic_id
                        .map(|topic_id| TopicIdV1::new(topic_id.into_bytes())),
                })
                .collect(),
            next,
        }))
    }

    fn topic_status(
        &self,
        observer: &ParticipantId,
        room_id: &str,
        topic_id: TopicIdV1,
    ) -> Result<OutcomeV2, RefusalV2> {
        let status = self
            .engine
            .topic_status(
                observer,
                &RoomId::new(room_id),
                TopicId::from_bytes(topic_id.into_bytes()),
            )
            .map_err(|refusal| match refusal {
                ObservationRefusal::InvalidTarget => RefusalV2::UnknownTopic,
                other => map_observation_refusal(other),
            })?;
        Ok(OutcomeV2::TopicStatus(TopicStatusV1 {
            revision: status.revision.0,
            room_id: status.room_id.as_str().to_owned(),
            topic_id: TopicIdV1::new(status.topic_id.into_bytes()),
            resolved: status.resolved,
        }))
    }

    fn topic_supersession_status(
        &self,
        observer: &ParticipantId,
        room_id: &str,
        topic_id: TopicIdV1,
    ) -> Result<OutcomeV2, RefusalV2> {
        let status = self
            .engine
            .topic_supersession_status(
                observer,
                &RoomId::new(room_id),
                TopicId::from_bytes(topic_id.into_bytes()),
            )
            .map_err(|refusal| match refusal {
                ObservationRefusal::InvalidTarget => RefusalV2::UnknownTopic,
                other => map_observation_refusal(other),
            })?;
        Ok(OutcomeV2::TopicSupersessionStatus(
            TopicSupersessionStatusV1 {
                revision: status.revision.0,
                room_id: status.room_id.as_str().to_owned(),
                topic_id: TopicIdV1::new(status.topic_id.into_bytes()),
                successor_topic_id: status
                    .successor_topic_id
                    .map(|id| TopicIdV1::new(id.into_bytes())),
            },
        ))
    }

    fn topic_alias_status(
        &self,
        observer: &ParticipantId,
        room_id: &str,
        topic_id: TopicIdV1,
    ) -> Result<OutcomeV2, RefusalV2> {
        let status = self
            .engine
            .topic_alias_status(
                observer,
                &RoomId::new(room_id),
                TopicId::from_bytes(topic_id.into_bytes()),
            )
            .map_err(|refusal| match refusal {
                ObservationRefusal::InvalidTarget => RefusalV2::UnknownTopic,
                other => map_observation_refusal(other),
            })?;
        Ok(OutcomeV2::TopicAliasStatus(TopicAliasStatusV1 {
            revision: status.revision.0,
            room_id: status.room_id.as_str().to_owned(),
            topic_id: TopicIdV1::new(status.topic_id.into_bytes()),
            aliases: status.aliases,
        }))
    }

    fn topic_tag_status(
        &self,
        observer: &ParticipantId,
        room_id: &str,
        topic_id: TopicIdV1,
    ) -> Result<OutcomeV2, RefusalV2> {
        let status = self
            .engine
            .topic_tag_status(
                observer,
                &RoomId::new(room_id),
                TopicId::from_bytes(topic_id.into_bytes()),
            )
            .map_err(|refusal| match refusal {
                ObservationRefusal::InvalidTarget => RefusalV2::UnknownTopic,
                other => map_observation_refusal(other),
            })?;
        Ok(OutcomeV2::TopicTagStatus(TopicTagStatusV1 {
            revision: status.revision.0,
            room_id: status.room_id.as_str().to_owned(),
            topic_id: TopicIdV1::new(status.topic_id.into_bytes()),
            tags: status.tags,
        }))
    }

    fn topic_messages(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        room_id: &str,
        topic_id: TopicIdV1,
        cursor_id: Option<PageCursorV2>,
        limit: u32,
    ) -> Result<OutcomeV2, RefusalV2> {
        let room = RoomId::new(room_id);
        let topic = TopicId::from_bytes(topic_id.into_bytes());
        let cursor = match cursor_id {
            Some(id) => match self.cursor_for(client, id)? {
                CursorPage::TopicMessages {
                    room_id: bound_room,
                    topic_id: bound_topic,
                    cursor,
                } if bound_room == room && bound_topic == topic => Some(cursor),
                _ => return Err(RefusalV2::InvalidCursor),
            },
            None => None,
        };
        let page = self
            .engine
            .topic_message_page(observer, &room, topic, cursor.as_ref(), limit as usize)
            .map_err(map_observation_refusal)?;
        let next_page = page.next.map(|cursor| CursorPage::TopicMessages {
            room_id: room.clone(),
            topic_id: topic,
            cursor,
        });
        let next = self.replace_cursor(client, cursor_id, next_page)?;
        Ok(OutcomeV2::TopicMessages(TopicMessagePageV1 {
            revision: page.revision.0,
            room_id: page.room_id.as_str().to_owned(),
            topic_id: TopicIdV1::new(page.topic_id.into_bytes()),
            messages: page
                .messages
                .into_iter()
                .map(map_topic_message_from_engine)
                .collect(),
            oldest_retained_sequence: page.oldest_retained_sequence,
            next_message_sequence: page.next_message_sequence,
            next,
        }))
    }

    fn read_position(
        &self,
        participant: &ParticipantId,
        room_id: &str,
        scope: ReadScopeV1,
    ) -> Result<OutcomeV2, RefusalV2> {
        let engine_scope = map_read_scope_to_engine(scope);
        let position = self
            .engine
            .read_position(participant, &RoomId::new(room_id), &engine_scope)
            .map_err(map_read_position_refusal)?
            .map(map_read_position_from_engine);
        Ok(OutcomeV2::ReadPosition(position))
    }

    fn advance_read_position(
        &mut self,
        participant: &ParticipantId,
        room_id: &str,
        observed_revision: u64,
        scope: ReadScopeV1,
        target_message_id: MessageIdV1,
    ) -> Result<OutcomeV2, RefusalV2> {
        let position = self
            .engine
            .advance_read_position(
                participant,
                &RoomId::new(room_id),
                map_read_scope_to_engine(scope),
                Revision(observed_revision),
                MessageId::from_bytes(target_message_id.into_bytes()),
            )
            .map_err(map_read_position_refusal)?;
        Ok(OutcomeV2::ReadPositionAdvanced(
            map_read_position_from_engine(&position),
        ))
    }

    fn attention_preference(
        &self,
        participant: &ParticipantId,
        room_id: &str,
        scope: AttentionScopeV1,
    ) -> Result<OutcomeV2, RefusalV2> {
        let preference = self
            .engine
            .attention_preference(
                participant,
                &RoomId::new(room_id),
                map_attention_scope_to_engine(scope),
            )
            .map_err(map_attention_refusal)?;
        Ok(OutcomeV2::AttentionPreference(
            map_attention_preference_from_engine(preference),
        ))
    }

    fn set_attention_preference(
        &mut self,
        participant: &ParticipantId,
        room_id: &str,
        observed_revision: u64,
        scope: AttentionScopeV1,
        preference: AttentionPreferenceV1,
    ) -> Result<OutcomeV2, RefusalV2> {
        let preference = self
            .engine
            .set_attention_preference(
                participant,
                &RoomId::new(room_id),
                map_attention_scope_to_engine(scope),
                Revision(observed_revision),
                map_attention_preference_to_engine(preference),
            )
            .map_err(map_attention_refusal)?;
        Ok(OutcomeV2::AttentionPreferenceSet(
            map_attention_preference_from_engine(preference),
        ))
    }

    fn pins(&self, participant: &ParticipantId, room_id: &str) -> Result<OutcomeV2, RefusalV2> {
        let pins = self
            .engine
            .pinned_references(participant, &RoomId::new(room_id))
            .map_err(map_observation_refusal)?
            .into_iter()
            .map(|pin| PinnedReferenceV1 {
                message_id: MessageIdV1::new(pin.message_id().into_bytes()),
                original_sequence: pin.original_sequence(),
                pinned_by: pin.pinned_by().as_str().to_owned(),
                availability: map_stable_reference_availability(pin.availability()),
            })
            .collect();
        Ok(OutcomeV2::Pins(pins))
    }

    fn saved_reference(
        &self,
        participant: &ParticipantId,
        room_id: &str,
        message_id: MessageIdV1,
    ) -> Result<OutcomeV2, RefusalV2> {
        let reference = self
            .engine
            .saved_reference(
                participant,
                &RoomId::new(room_id),
                MessageId::from_bytes(message_id.into_bytes()),
            )
            .map_err(map_private_save_refusal)?
            .map(map_saved_reference_from_engine);
        Ok(OutcomeV2::SavedReference(reference))
    }

    fn saved_references(
        &self,
        participant: &ParticipantId,
        room_id: &str,
    ) -> Result<OutcomeV2, RefusalV2> {
        let references = self
            .engine
            .saved_references(participant, &RoomId::new(room_id))
            .map_err(map_private_save_refusal)?
            .into_iter()
            .map(map_saved_reference_from_engine)
            .collect();
        Ok(OutcomeV2::SavedReferences(references))
    }

    fn set_saved_reference(
        &mut self,
        participant: &ParticipantId,
        room_id: &str,
        observed_revision: u64,
        message_id: MessageIdV1,
        saved: bool,
    ) -> Result<OutcomeV2, RefusalV2> {
        let reference = self
            .engine
            .set_saved_reference(
                participant,
                &RoomId::new(room_id),
                MessageId::from_bytes(message_id.into_bytes()),
                Revision(observed_revision),
                saved,
            )
            .map_err(map_private_save_refusal)?;
        Ok(OutcomeV2::SavedReferenceSet {
            reference: map_saved_reference_from_engine(reference),
            saved,
        })
    }

    fn thread_roots(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        room_id: &str,
        cursor_id: Option<PageCursorV2>,
        limit: u32,
    ) -> Result<OutcomeV2, RefusalV2> {
        let room = RoomId::new(room_id);
        let cursor = match cursor_id {
            Some(id) => match self.cursor_for(client, id)? {
                CursorPage::ThreadRoots {
                    room_id: bound_room,
                    cursor,
                } if bound_room == room => Some(cursor),
                _ => return Err(RefusalV2::InvalidCursor),
            },
            None => None,
        };
        let page = self
            .engine
            .thread_root_page(observer, &room, cursor.as_ref(), limit as usize)
            .map_err(map_observation_refusal)?;
        let next_page = page.next.clone().map(|cursor| CursorPage::ThreadRoots {
            room_id: room.clone(),
            cursor,
        });
        let next = self.replace_cursor(client, cursor_id, next_page)?;
        Ok(OutcomeV2::ThreadRoots(map_thread_page(page, next)))
    }

    fn thread_replies(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        room_id: &str,
        root_message_id: MessageIdV1,
        cursor_id: Option<PageCursorV2>,
        limit: u32,
    ) -> Result<OutcomeV2, RefusalV2> {
        let room = RoomId::new(room_id);
        let root = MessageId::from_bytes(root_message_id.into_bytes());
        let cursor = match cursor_id {
            Some(id) => match self.cursor_for(client, id)? {
                CursorPage::ThreadReplies {
                    room_id: bound_room,
                    root_message_id: bound_root,
                    cursor,
                } if bound_room == room && bound_root == root => Some(cursor),
                _ => return Err(RefusalV2::InvalidCursor),
            },
            None => None,
        };
        let page = self
            .engine
            .thread_reply_page(observer, &room, root, cursor.as_ref(), limit as usize)
            .map_err(map_observation_refusal)?;
        let next_page = page.next.clone().map(|cursor| CursorPage::ThreadReplies {
            room_id: room.clone(),
            root_message_id: root,
            cursor,
        });
        let next = self.replace_cursor(client, cursor_id, next_page)?;
        Ok(OutcomeV2::ThreadReplies(map_thread_page(page, next)))
    }

    fn bans(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        room_id: &str,
        cursor_id: Option<PageCursorV2>,
        limit: u32,
    ) -> Result<OutcomeV2, RefusalV2> {
        let room = RoomId::new(room_id);
        let cursor = match cursor_id {
            Some(id) => match self.cursor_for(client, id)? {
                CursorPage::Bans {
                    room_id: bound_room,
                    cursor,
                } if bound_room == room => Some(cursor),
                _ => return Err(RefusalV2::InvalidCursor),
            },
            None => None,
        };
        let page = self
            .engine
            .ban_page(observer, &room, cursor.as_ref(), limit as usize)
            .map_err(map_observation_refusal)?;
        let next_page = page.next.map(|cursor| CursorPage::Bans {
            room_id: room.clone(),
            cursor,
        });
        let next = self.replace_cursor(client, cursor_id, next_page)?;
        let banned = page
            .banned
            .into_iter()
            .map(|entry| BannedParticipantV2 {
                participant_id: entry.participant_id.as_str().to_owned(),
                display_name: entry.display_name,
                kind: match entry.kind {
                    ParticipantKind::Human => ParticipantKindV2::Human,
                    ParticipantKind::Agent => ParticipantKindV2::Agent,
                },
                prior_role: map_role_from_engine(entry.prior_role),
            })
            .collect();
        Ok(OutcomeV2::Bans(BanPageV2 {
            revision: page.revision.0,
            room_id: page.room_id.as_str().to_owned(),
            banned,
            next,
        }))
    }

    fn open_changes(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        room_id: &str,
        observed_revision: u64,
    ) -> Result<OutcomeV2, RefusalV2> {
        let room = RoomId::new(room_id);
        let position = self
            .engine
            .room_change_position(observer, &room, Revision(observed_revision))
            .map_err(map_observation_refusal)?;
        let cursor = self
            .replace_cursor(
                client,
                None,
                Some(CursorPage::Changes {
                    room_id: room,
                    position,
                }),
            )?
            .ok_or(RefusalV2::Unavailable)?;
        Ok(OutcomeV2::ChangesOpened {
            revision: observed_revision,
            cursor,
        })
    }

    fn changes(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        room_id: &str,
        cursor_id: PageCursorV2,
        limit: u32,
    ) -> Result<OutcomeV2, RefusalV2> {
        let room = RoomId::new(room_id);
        let position = match self.cursor_for(client, cursor_id)? {
            CursorPage::Changes {
                room_id: bound_room,
                position,
            } if bound_room == room => position,
            _ => return Err(RefusalV2::InvalidCursor),
        };
        match self
            .engine
            .room_change_page(observer, &room, &position, limit as usize)
            .map_err(map_observation_refusal)?
        {
            RoomChangeResult::Gap {
                cause,
                current_revision,
            } => {
                self.replace_cursor(client, Some(cursor_id), None)?;
                Ok(OutcomeV2::ChangeGap {
                    current_revision: current_revision.0,
                    cause: match cause {
                        RoomChangeGap::ChangeRetention { discarded_through } => {
                            ChangeGapV2::ChangeRetention {
                                discarded_through: discarded_through.0,
                            }
                        }
                        RoomChangeGap::MembershipChanged => ChangeGapV2::MembershipChanged,
                    },
                })
            }
            RoomChangeResult::Page(page) => {
                let next_position = page.next;
                let next = self
                    .replace_cursor(
                        client,
                        Some(cursor_id),
                        Some(CursorPage::Changes {
                            room_id: room,
                            position: next_position,
                        }),
                    )?
                    .ok_or(RefusalV2::Unavailable)?;
                let changes = page
                    .changes
                    .into_iter()
                    .map(|change| {
                        let event = match change.event {
                            agora_engine::DomainEventV2::ParticipantRegistered { .. } => {
                                return Err(RefusalV2::Unavailable);
                            }
                            agora_engine::DomainEventV2::PollCreated {
                                actor,
                                poll_id,
                                prompt,
                                options,
                                eligible_participants,
                                max_choices,
                                ..
                            } => RoomEventV2::PollCreated {
                                actor_id: actor.as_str().to_owned(),
                                poll_id: PollIdV1::new(poll_id.into_bytes()),
                                prompt,
                                options,
                                eligible_participant_ids: eligible_participants
                                    .into_iter()
                                    .map(|participant| participant.as_str().to_owned())
                                    .collect(),
                                max_choices,
                            },
                            agora_engine::DomainEventV2::PollBallotChanged {
                                poll_id,
                                totals,
                                total_ballots,
                                ..
                            } => RoomEventV2::PollBallotChanged {
                                poll_id: PollIdV1::new(poll_id.into_bytes()),
                                totals,
                                total_ballots,
                            },
                            agora_engine::DomainEventV2::PollLifecycleChanged {
                                actor,
                                poll_id,
                                open,
                                ..
                            } => RoomEventV2::PollLifecycleChanged {
                                actor_id: actor.as_str().to_owned(),
                                poll_id: PollIdV1::new(poll_id.into_bytes()),
                                open,
                            },
                            agora_engine::DomainEventV2::PinChanged {
                                actor,
                                message_id,
                                original_sequence,
                                pinned,
                                ..
                            } => RoomEventV2::PinChanged {
                                actor_id: actor.as_str().to_owned(),
                                message_id: MessageIdV1::new(message_id.into_bytes()),
                                original_sequence,
                                pinned,
                            },
                            agora_engine::DomainEventV2::RoomCreated {
                                owner, admission, ..
                            } => RoomEventV2::RoomCreated {
                                owner: owner.as_str().to_owned(),
                                admission: map_admission_from_engine(admission),
                            },
                            agora_engine::DomainEventV2::PeerConversationCreated {
                                initiator,
                                profile,
                                founding_members,
                                ..
                            } => RoomEventV2::PeerConversationCreated(map_peer_conversation_event(
                                initiator,
                                profile,
                                founding_members,
                            )),
                            agora_engine::DomainEventV2::TopicCreated {
                                actor,
                                topic_id,
                                name,
                                ..
                            } => RoomEventV2::TopicCreated {
                                actor_id: actor.as_str().to_owned(),
                                topic_id: TopicIdV1::new(topic_id.into_bytes()),
                                name,
                            },
                            agora_engine::DomainEventV2::TopicRenamed {
                                actor,
                                topic_id,
                                previous,
                                name,
                                ..
                            } => RoomEventV2::TopicRenamed {
                                actor_id: actor.as_str().to_owned(),
                                topic_id: TopicIdV1::new(topic_id.into_bytes()),
                                previous,
                                name,
                            },
                            agora_engine::DomainEventV2::TopicLifecycleChanged {
                                actor,
                                topic_id,
                                previous_resolved,
                                resolved,
                                ..
                            } => RoomEventV2::TopicLifecycleChanged {
                                actor_id: actor.as_str().to_owned(),
                                topic_id: TopicIdV1::new(topic_id.into_bytes()),
                                previous_resolved,
                                resolved,
                            },
                            agora_engine::DomainEventV2::TopicSupersessionChanged {
                                actor,
                                topic_id,
                                previous_successor_topic_id,
                                successor_topic_id,
                                ..
                            } => RoomEventV2::TopicSupersessionChanged {
                                actor_id: actor.as_str().to_owned(),
                                topic_id: TopicIdV1::new(topic_id.into_bytes()),
                                previous_successor_topic_id: previous_successor_topic_id
                                    .map(|id| TopicIdV1::new(id.into_bytes())),
                                successor_topic_id: successor_topic_id
                                    .map(|id| TopicIdV1::new(id.into_bytes())),
                            },
                            agora_engine::DomainEventV2::TopicAliasChanged {
                                actor,
                                topic_id,
                                alias,
                                added,
                                ..
                            } => RoomEventV2::TopicAliasChanged {
                                actor_id: actor.as_str().to_owned(),
                                topic_id: TopicIdV1::new(topic_id.into_bytes()),
                                alias,
                                added,
                            },
                            agora_engine::DomainEventV2::TopicTagChanged {
                                actor,
                                topic_id,
                                tag,
                                added,
                                ..
                            } => RoomEventV2::TopicTagChanged {
                                actor_id: actor.as_str().to_owned(),
                                topic_id: TopicIdV1::new(topic_id.into_bytes()),
                                tag,
                                added,
                            },
                            agora_engine::DomainEventV2::InvitationRecorded {
                                participant, ..
                            } => RoomEventV2::InvitationRecorded {
                                participant_id: participant.as_str().to_owned(),
                            },
                            agora_engine::DomainEventV2::ParticipantJoined {
                                participant, ..
                            } => RoomEventV2::ParticipantJoined {
                                participant_id: participant.as_str().to_owned(),
                            },
                            agora_engine::DomainEventV2::ParticipantLeft {
                                participant, ..
                            } => RoomEventV2::ParticipantLeft {
                                participant_id: participant.as_str().to_owned(),
                            },
                            agora_engine::DomainEventV2::RoleChanged {
                                participant, role, ..
                            } => RoomEventV2::RoleChanged {
                                participant_id: participant.as_str().to_owned(),
                                role: map_role_from_engine(role),
                            },
                            agora_engine::DomainEventV2::RoomPolicyChanged {
                                actor,
                                previous,
                                admission,
                                ..
                            } => RoomEventV2::RoomPolicyChanged {
                                actor_id: actor.as_str().to_owned(),
                                previous: map_admission_from_engine(previous),
                                admission: map_admission_from_engine(admission),
                            },
                            agora_engine::DomainEventV2::MessageIntervalChanged {
                                actor,
                                previous_interval_ms,
                                minimum_interval_ms,
                                ..
                            } => RoomEventV2::MessageIntervalChanged {
                                actor_id: actor.as_str().to_owned(),
                                previous_interval_ms,
                                minimum_interval_ms,
                            },
                            agora_engine::DomainEventV2::ParticipantKicked {
                                actor,
                                participant,
                                reason,
                                causation,
                                ..
                            } => RoomEventV2::ParticipantKicked {
                                actor_id: actor.as_str().to_owned(),
                                participant_id: participant.as_str().to_owned(),
                                reason: reason.as_str().to_owned(),
                                causation: causation.into_bytes(),
                            },
                            agora_engine::DomainEventV2::ParticipantBanned {
                                actor,
                                participant,
                                reason,
                                causation,
                                ..
                            } => RoomEventV2::ParticipantBanned {
                                actor_id: actor.as_str().to_owned(),
                                participant_id: participant.as_str().to_owned(),
                                reason: reason.as_str().to_owned(),
                                causation: causation.into_bytes(),
                            },
                            agora_engine::DomainEventV2::ParticipantUnbanned {
                                actor,
                                participant,
                                reason,
                                causation,
                                ..
                            } => RoomEventV2::ParticipantUnbanned {
                                actor_id: actor.as_str().to_owned(),
                                participant_id: participant.as_str().to_owned(),
                                reason: reason.as_str().to_owned(),
                                causation: causation.into_bytes(),
                            },
                            agora_engine::DomainEventV2::PostingRestrictionChanged {
                                actor,
                                participant,
                                restricted,
                                reason,
                                causation,
                                ..
                            } => RoomEventV2::PostingRestrictionChanged {
                                actor_id: actor.as_str().to_owned(),
                                participant_id: participant.as_str().to_owned(),
                                restricted,
                                reason: reason.as_str().to_owned(),
                                causation: causation.into_bytes(),
                            },
                            agora_engine::DomainEventV2::MessageAccepted { message, .. } => {
                                RoomEventV2::MessageAccepted(map_message_from_engine(message))
                            }
                            agora_engine::DomainEventV2::MessageWithdrawn {
                                actor,
                                message_id,
                                sequence,
                                ..
                            } => RoomEventV2::MessageWithdrawn {
                                actor_id: actor.as_str().to_owned(),
                                message_id: MessageIdV1::new(message_id.into_bytes()),
                                sequence,
                            },
                            agora_engine::DomainEventV2::MessageRedacted {
                                actor,
                                author,
                                message_id,
                                sequence,
                                reason,
                                causation,
                                ..
                            } => RoomEventV2::MessageRedacted {
                                actor_id: actor.as_str().to_owned(),
                                author_id: author.as_str().to_owned(),
                                message_id: MessageIdV1::new(message_id.into_bytes()),
                                sequence,
                                reason: reason.as_str().to_owned(),
                                causation: causation.into_bytes(),
                            },
                            agora_engine::DomainEventV2::ThreadLifecycleChanged {
                                actor,
                                root_message_id,
                                archived,
                                reason,
                                causation,
                                ..
                            } => RoomEventV2::ThreadLifecycleChanged {
                                actor_id: actor.as_str().to_owned(),
                                root_message_id: MessageIdV1::new(root_message_id.into_bytes()),
                                archived,
                                reason: reason.as_str().to_owned(),
                                causation: causation.into_bytes(),
                            },
                            agora_engine::DomainEventV2::ReactionChanged {
                                actor,
                                message_id,
                                key,
                                reacted,
                                ..
                            } => RoomEventV2::ReactionChanged {
                                actor_id: actor.as_str().to_owned(),
                                message_id: MessageIdV1::new(message_id.into_bytes()),
                                key,
                                reacted,
                            },
                            agora_engine::DomainEventV2::MessageEdited {
                                actor,
                                message_id,
                                message_revision,
                                at,
                                body,
                                ..
                            } => RoomEventV2::MessageEdited {
                                actor_id: actor.as_str().to_owned(),
                                message_id: MessageIdV1::new(message_id.into_bytes()),
                                message_revision,
                                logical_time: at.0,
                                body,
                            },
                        };
                        Ok(RoomChangeV2 {
                            event_sequence: change.sequence.0,
                            revision: change.revision.0,
                            event,
                        })
                    })
                    .collect::<Result<Vec<_>, RefusalV2>>()?;
                Ok(OutcomeV2::Changes(RoomChangePageV2 {
                    revision: page.revision.0,
                    changes,
                    next,
                    has_more: page.has_more,
                }))
            }
        }
    }

    fn open_identified_changes(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        room_id: &str,
        observed_revision: u64,
    ) -> Result<OutcomeV2, RefusalV2> {
        let room = RoomId::new(room_id);
        let position = self
            .engine
            .room_change_position(observer, &room, Revision(observed_revision))
            .map_err(map_observation_refusal)?;
        let cursor = self
            .replace_cursor(
                client,
                None,
                Some(CursorPage::IdentifiedChanges {
                    room_id: room,
                    position,
                }),
            )?
            .ok_or(RefusalV2::Unavailable)?;
        Ok(OutcomeV2::ChangesOpened {
            revision: observed_revision,
            cursor,
        })
    }

    fn identified_changes(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        room_id: &str,
        cursor_id: PageCursorV2,
        limit: u32,
    ) -> Result<OutcomeV2, RefusalV2> {
        let room = RoomId::new(room_id);
        let position = match self.cursor_for(client, cursor_id)? {
            CursorPage::IdentifiedChanges {
                room_id: bound_room,
                position,
            } if bound_room == room => position,
            _ => return Err(RefusalV2::InvalidCursor),
        };
        match self
            .engine
            .room_change_page(observer, &room, &position, limit as usize)
            .map_err(map_observation_refusal)?
        {
            RoomChangeResult::Gap {
                cause,
                current_revision,
            } => {
                self.replace_cursor(client, Some(cursor_id), None)?;
                Ok(OutcomeV2::ChangeGap {
                    current_revision: current_revision.0,
                    cause: match cause {
                        RoomChangeGap::ChangeRetention { discarded_through } => {
                            ChangeGapV2::ChangeRetention {
                                discarded_through: discarded_through.0,
                            }
                        }
                        RoomChangeGap::MembershipChanged => ChangeGapV2::MembershipChanged,
                    },
                })
            }
            RoomChangeResult::Page(page) => {
                let next = self
                    .replace_cursor(
                        client,
                        Some(cursor_id),
                        Some(CursorPage::IdentifiedChanges {
                            room_id: room,
                            position: page.next,
                        }),
                    )?
                    .ok_or(RefusalV2::Unavailable)?;
                let changes = page
                    .changes
                    .into_iter()
                    .map(|change| {
                        Ok(IdentifiedRoomChangeV1 {
                            event_sequence: change.sequence.0,
                            revision: change.revision.0,
                            event: map_identified_room_event(change.event)?,
                        })
                    })
                    .collect::<Result<Vec<_>, RefusalV2>>()?;
                Ok(OutcomeV2::IdentifiedChanges(IdentifiedRoomChangePageV1 {
                    revision: page.revision.0,
                    changes,
                    next,
                    has_more: page.has_more,
                }))
            }
        }
    }

    fn open_attachment_changes(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        room_id: &str,
        observed_revision: u64,
    ) -> Result<OutcomeV2, RefusalV2> {
        let room = RoomId::new(room_id);
        let position = self
            .engine
            .room_change_position(observer, &room, Revision(observed_revision))
            .map_err(map_observation_refusal)?;
        let cursor = self
            .replace_cursor(
                client,
                None,
                Some(CursorPage::AttachmentChanges {
                    room_id: room,
                    position,
                }),
            )?
            .ok_or(RefusalV2::Unavailable)?;
        Ok(OutcomeV2::ChangesOpened {
            revision: observed_revision,
            cursor,
        })
    }

    fn attachment_changes(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        room_id: &str,
        cursor_id: PageCursorV2,
        limit: u32,
    ) -> Result<OutcomeV2, RefusalV2> {
        let room = RoomId::new(room_id);
        let position = match self.cursor_for(client, cursor_id)? {
            CursorPage::AttachmentChanges {
                room_id: bound_room,
                position,
            } if bound_room == room => position,
            _ => return Err(RefusalV2::InvalidCursor),
        };
        match self
            .engine
            .room_change_page(observer, &room, &position, limit as usize)
            .map_err(map_observation_refusal)?
        {
            RoomChangeResult::Gap {
                cause,
                current_revision,
            } => {
                self.replace_cursor(client, Some(cursor_id), None)?;
                Ok(OutcomeV2::ChangeGap {
                    current_revision: current_revision.0,
                    cause: match cause {
                        RoomChangeGap::ChangeRetention { discarded_through } => {
                            ChangeGapV2::ChangeRetention {
                                discarded_through: discarded_through.0,
                            }
                        }
                        RoomChangeGap::MembershipChanged => ChangeGapV2::MembershipChanged,
                    },
                })
            }
            RoomChangeResult::Page(page) => {
                let next = self
                    .replace_cursor(
                        client,
                        Some(cursor_id),
                        Some(CursorPage::AttachmentChanges {
                            room_id: room,
                            position: page.next,
                        }),
                    )?
                    .ok_or(RefusalV2::Unavailable)?;
                let mut changes = Vec::with_capacity(page.changes.len());
                let mut accepted_messages = Vec::new();
                for change in page.changes {
                    if let agora_engine::DomainEventV2::MessageAccepted { message, .. } =
                        &change.event
                    {
                        accepted_messages.push(map_attachment_message_from_engine(message.clone()));
                    }
                    changes.push(IdentifiedRoomChangeV1 {
                        event_sequence: change.sequence.0,
                        revision: change.revision.0,
                        event: map_identified_room_event(change.event)?,
                    });
                }
                Ok(OutcomeV2::AttachmentChanges(AttachmentRoomChangePageV1 {
                    identified: IdentifiedRoomChangePageV1 {
                        revision: page.revision.0,
                        changes,
                        next,
                        has_more: page.has_more,
                    },
                    accepted_messages,
                }))
            }
        }
    }

    fn open_threaded_changes(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        room_id: &str,
        observed_revision: u64,
    ) -> Result<OutcomeV2, RefusalV2> {
        let room = RoomId::new(room_id);
        let position = self
            .engine
            .room_change_position(observer, &room, Revision(observed_revision))
            .map_err(map_observation_refusal)?;
        let cursor = self
            .replace_cursor(
                client,
                None,
                Some(CursorPage::ThreadedChanges {
                    room_id: room,
                    position,
                }),
            )?
            .ok_or(RefusalV2::Unavailable)?;
        Ok(OutcomeV2::ChangesOpened {
            revision: observed_revision,
            cursor,
        })
    }

    fn threaded_changes(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        room_id: &str,
        cursor_id: PageCursorV2,
        limit: u32,
    ) -> Result<OutcomeV2, RefusalV2> {
        let room = RoomId::new(room_id);
        let position = match self.cursor_for(client, cursor_id)? {
            CursorPage::ThreadedChanges {
                room_id: bound_room,
                position,
            } if bound_room == room => position,
            _ => return Err(RefusalV2::InvalidCursor),
        };
        match self
            .engine
            .room_change_page(observer, &room, &position, limit as usize)
            .map_err(map_observation_refusal)?
        {
            RoomChangeResult::Gap {
                cause,
                current_revision,
            } => {
                self.replace_cursor(client, Some(cursor_id), None)?;
                Ok(OutcomeV2::ChangeGap {
                    current_revision: current_revision.0,
                    cause: match cause {
                        RoomChangeGap::ChangeRetention { discarded_through } => {
                            ChangeGapV2::ChangeRetention {
                                discarded_through: discarded_through.0,
                            }
                        }
                        RoomChangeGap::MembershipChanged => ChangeGapV2::MembershipChanged,
                    },
                })
            }
            RoomChangeResult::Page(page) => {
                let next = self
                    .replace_cursor(
                        client,
                        Some(cursor_id),
                        Some(CursorPage::ThreadedChanges {
                            room_id: room,
                            position: page.next,
                        }),
                    )?
                    .ok_or(RefusalV2::Unavailable)?;
                let changes = page
                    .changes
                    .into_iter()
                    .map(|change| {
                        Ok(ThreadedRoomChangeV1 {
                            event_sequence: change.sequence.0,
                            revision: change.revision.0,
                            event: map_threaded_room_event(change.event)?,
                        })
                    })
                    .collect::<Result<Vec<_>, RefusalV2>>()?;
                Ok(OutcomeV2::ThreadedChanges(ThreadedRoomChangePageV1 {
                    revision: page.revision.0,
                    changes,
                    next,
                    has_more: page.has_more,
                }))
            }
        }
    }

    fn open_topic_changes(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        room_id: &str,
        observed_revision: u64,
    ) -> Result<OutcomeV2, RefusalV2> {
        let room = RoomId::new(room_id);
        let position = self
            .engine
            .room_change_position(observer, &room, Revision(observed_revision))
            .map_err(map_observation_refusal)?;
        let cursor = self
            .replace_cursor(
                client,
                None,
                Some(CursorPage::TopicChanges {
                    room_id: room,
                    position,
                }),
            )?
            .ok_or(RefusalV2::Unavailable)?;
        Ok(OutcomeV2::ChangesOpened {
            revision: observed_revision,
            cursor,
        })
    }

    fn topic_changes(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        room_id: &str,
        cursor_id: PageCursorV2,
        limit: u32,
    ) -> Result<OutcomeV2, RefusalV2> {
        let room = RoomId::new(room_id);
        let position = match self.cursor_for(client, cursor_id)? {
            CursorPage::TopicChanges {
                room_id: bound_room,
                position,
            } if bound_room == room => position,
            _ => return Err(RefusalV2::InvalidCursor),
        };
        match self
            .engine
            .room_change_page(observer, &room, &position, limit as usize)
            .map_err(map_observation_refusal)?
        {
            RoomChangeResult::Gap {
                cause,
                current_revision,
            } => {
                self.replace_cursor(client, Some(cursor_id), None)?;
                Ok(OutcomeV2::ChangeGap {
                    current_revision: current_revision.0,
                    cause: match cause {
                        RoomChangeGap::ChangeRetention { discarded_through } => {
                            ChangeGapV2::ChangeRetention {
                                discarded_through: discarded_through.0,
                            }
                        }
                        RoomChangeGap::MembershipChanged => ChangeGapV2::MembershipChanged,
                    },
                })
            }
            RoomChangeResult::Page(page) => {
                let next = self
                    .replace_cursor(
                        client,
                        Some(cursor_id),
                        Some(CursorPage::TopicChanges {
                            room_id: room,
                            position: page.next,
                        }),
                    )?
                    .ok_or(RefusalV2::Unavailable)?;
                let changes = page
                    .changes
                    .into_iter()
                    .map(|change| {
                        Ok(TopicRoomChangeV1 {
                            event_sequence: change.sequence.0,
                            revision: change.revision.0,
                            event: map_topic_room_event(change.event)?,
                        })
                    })
                    .collect::<Result<Vec<_>, RefusalV2>>()?;
                Ok(OutcomeV2::TopicChanges(TopicRoomChangePageV1 {
                    revision: page.revision.0,
                    changes,
                    next,
                    has_more: page.has_more,
                }))
            }
        }
    }

    fn cursor_for(
        &self,
        client: ClientIdV2,
        cursor: PageCursorV2,
    ) -> Result<CursorPage, RefusalV2> {
        let record = self.cursors.get(&cursor).ok_or(RefusalV2::InvalidCursor)?;
        if record.owner != client {
            return Err(RefusalV2::InvalidCursor);
        }
        Ok(record.page.clone())
    }

    fn replace_cursor(
        &mut self,
        client: ClientIdV2,
        old: Option<PageCursorV2>,
        next: Option<CursorPage>,
    ) -> Result<Option<PageCursorV2>, RefusalV2> {
        if next.is_some() && old.is_none() && self.cursors.len() >= self.limits.max_retained_cursors
        {
            return Err(RefusalV2::CursorLimit);
        }
        let next_id = if next.is_some() {
            let id = PageCursorV2(self.next_cursor);
            self.next_cursor = self
                .next_cursor
                .checked_add(1)
                .ok_or(RefusalV2::Exhausted)?;
            Some(id)
        } else {
            None
        };
        if let Some(old) = old {
            self.cursors.remove(&old);
        }
        if let (Some(id), Some(page)) = (next_id, next) {
            self.cursors.insert(
                id,
                CursorRecord {
                    owner: client,
                    page,
                },
            );
        }
        Ok(next_id)
    }

    fn valid_command_shape(&self, command: &CommandV2) -> bool {
        let valid_page = |limit: u32| limit > 0 && limit <= self.limits.max_page_items;
        let valid_room = |room: &str| valid_text(room, self.limits.max_room_id_bytes);
        let valid_participant =
            |participant: &str| valid_text(participant, self.limits.max_participant_id_bytes);
        match command {
            CommandV2::Capabilities => true,
            CommandV2::ConversationProfile { room_id } => valid_room(room_id),
            CommandV2::AdmissionControl { room_id } => valid_room(room_id),
            CommandV2::MessageReports { room_id }
            | CommandV2::ModerationReports { room_id }
            | CommandV2::ReviewMessageReport { room_id, .. } => valid_room(room_id),
            CommandV2::SubmitMessageReport {
                room_id, reason, ..
            } => valid_room(room_id) && valid_text(reason, self.limits.max_moderation_reason_bytes),
            CommandV2::Rooms { limit, .. } => valid_page(*limit),
            CommandV2::Members { room_id, limit, .. }
            | CommandV2::Bans { room_id, limit, .. }
            | CommandV2::Messages { room_id, limit, .. }
            | CommandV2::IdentifiedMessages { room_id, limit, .. }
            | CommandV2::AttachmentMessages { room_id, limit, .. }
            | CommandV2::VellumArtifactMessages { room_id, limit, .. }
            | CommandV2::ThreadRoots { room_id, limit, .. }
            | CommandV2::ThreadReplies { room_id, limit, .. }
            | CommandV2::MessageRevisions { room_id, limit, .. }
            | CommandV2::Topics { room_id, limit, .. }
            | CommandV2::TopicMessages { room_id, limit, .. } => {
                valid_room(room_id) && valid_page(*limit)
            }
            CommandV2::TopicCollection {
                room_id,
                filter,
                limit,
                ..
            } => {
                valid_room(room_id)
                    && valid_page(*limit)
                    && filter.tags.len() <= 8
                    && filter
                        .tags
                        .iter()
                        .all(|tag| valid_text(tag, self.limits.max_room_name_bytes))
                    && filter.tags.windows(2).all(|pair| pair[0] < pair[1])
            }
            CommandV2::OpenChanges { room_id, .. }
            | CommandV2::OpenIdentifiedChanges { room_id, .. }
            | CommandV2::OpenAttachmentChanges { room_id, .. }
            | CommandV2::OpenThreadedChanges { room_id, .. }
            | CommandV2::OpenTopicChanges { room_id, .. }
            | CommandV2::ThreadStatus { room_id, .. }
            | CommandV2::ReactionStatus { room_id, .. }
            | CommandV2::ReadPosition { room_id, .. }
            | CommandV2::AdvanceReadPosition { room_id, .. }
            | CommandV2::AttentionPreference { room_id, .. }
            | CommandV2::SetAttentionPreference { room_id, .. }
            | CommandV2::Pins { room_id }
            | CommandV2::SetPin { room_id, .. }
            | CommandV2::SavedReference { room_id, .. }
            | CommandV2::SavedReferences { room_id }
            | CommandV2::SetSavedReference { room_id, .. }
            | CommandV2::Polls { room_id }
            | CommandV2::Poll { room_id, .. }
            | CommandV2::PollBallot { room_id, .. }
            | CommandV2::SetPollOpen { room_id, .. } => valid_room(room_id),
            CommandV2::SetPollBallot {
                room_id,
                selections,
                ..
            } => {
                valid_room(room_id)
                    && selections.len() <= 32
                    && selections.iter().all(|selection| *selection < 32)
                    && selections.windows(2).all(|pair| pair[0] < pair[1])
            }
            CommandV2::Changes { room_id, limit, .. }
            | CommandV2::IdentifiedChanges { room_id, limit, .. }
            | CommandV2::AttachmentChanges { room_id, limit, .. }
            | CommandV2::ThreadedChanges { room_id, limit, .. }
            | CommandV2::TopicChanges { room_id, limit, .. } => {
                valid_room(room_id) && valid_page(*limit)
            }
            CommandV2::CreateRoom { room_id, name, .. } => {
                valid_room(room_id) && valid_text(name, self.limits.max_room_name_bytes)
            }
            CommandV2::CreatePoll {
                room_id,
                prompt,
                options,
                eligible_participant_ids,
                max_choices,
                ..
            } => {
                valid_room(room_id)
                    && valid_text(prompt, 1_024)
                    && (2..=32).contains(&options.len())
                    && options.iter().all(|option| valid_text(option, 256))
                    && !eligible_participant_ids.is_empty()
                    && eligible_participant_ids.len() <= 256
                    && eligible_participant_ids
                        .iter()
                        .all(|participant| valid_participant(participant))
                    && *max_choices > 0
                    && usize::from(*max_choices) <= options.len()
            }
            CommandV2::CreatePeerConversation {
                room_id,
                name,
                profile,
                founding_participant_ids,
                ..
            } => {
                let valid_cardinality = match profile {
                    ConversationProfileV1::Ordinary => false,
                    ConversationProfileV1::Direct => founding_participant_ids.len() == 2,
                    ConversationProfileV1::GroupDirect => founding_participant_ids.len() >= 3,
                };
                valid_room(room_id)
                    && valid_text(name, self.limits.max_room_name_bytes)
                    && valid_cardinality
                    && founding_participant_ids
                        .windows(2)
                        .all(|pair| pair[0] < pair[1])
                    && founding_participant_ids
                        .iter()
                        .all(|participant| valid_participant(participant))
            }
            CommandV2::CreateTopic { room_id, name, .. }
            | CommandV2::RenameTopic { room_id, name, .. } => {
                valid_room(room_id) && valid_text(name, self.limits.max_room_name_bytes)
            }
            CommandV2::ResolveTopic { room_id, .. }
            | CommandV2::ReopenTopic { room_id, .. }
            | CommandV2::TopicStatus { room_id, .. }
            | CommandV2::SupersedeTopic { room_id, .. }
            | CommandV2::ClearTopicSupersession { room_id, .. }
            | CommandV2::TopicSupersessionStatus { room_id, .. } => valid_room(room_id),
            CommandV2::AddTopicAlias { room_id, alias, .. }
            | CommandV2::RemoveTopicAlias { room_id, alias, .. } => {
                valid_room(room_id) && valid_text(alias, self.limits.max_room_name_bytes)
            }
            CommandV2::TopicAliasStatus { room_id, .. } => valid_room(room_id),
            CommandV2::AddTopicTag { room_id, tag, .. }
            | CommandV2::RemoveTopicTag { room_id, tag, .. } => {
                valid_room(room_id) && valid_text(tag, self.limits.max_room_name_bytes)
            }
            CommandV2::TopicTagStatus { room_id, .. } => valid_room(room_id),
            CommandV2::Invite {
                room_id,
                participant_id,
                ..
            }
            | CommandV2::SetRole {
                room_id,
                participant_id,
                ..
            } => valid_room(room_id) && valid_participant(participant_id),
            CommandV2::Kick {
                room_id,
                participant_id,
                reason,
                ..
            }
            | CommandV2::Ban {
                room_id,
                participant_id,
                reason,
                ..
            }
            | CommandV2::Unban {
                room_id,
                participant_id,
                reason,
                ..
            } => {
                valid_room(room_id)
                    && valid_participant(participant_id)
                    && valid_text(reason, self.limits.max_moderation_reason_bytes)
            }
            CommandV2::SetPostingRestricted {
                room_id,
                participant_id,
                reason,
                ..
            } => {
                valid_room(room_id)
                    && valid_participant(participant_id)
                    && valid_text(reason, self.limits.max_moderation_reason_bytes)
            }
            CommandV2::Join { room_id, .. } | CommandV2::Leave { room_id, .. } => {
                valid_room(room_id)
            }
            CommandV2::SetAdmissionPolicy { room_id, .. }
            | CommandV2::SetMinimumMessageInterval { room_id, .. } => valid_room(room_id),
            CommandV2::WithdrawMessage { room_id, .. } => valid_room(room_id),
            CommandV2::RedactMessage {
                room_id, reason, ..
            }
            | CommandV2::ArchiveThread {
                room_id, reason, ..
            }
            | CommandV2::ReopenThread {
                room_id, reason, ..
            } => valid_room(room_id) && valid_text(reason, self.limits.max_moderation_reason_bytes),
            CommandV2::AddReaction { room_id, key, .. }
            | CommandV2::RemoveReaction { room_id, key, .. } => {
                valid_room(room_id) && valid_text(key, MAX_REACTION_KEY_BYTES_V1)
            }
            CommandV2::EditMessage { room_id, body, .. } => {
                valid_room(room_id) && valid_text(body, self.limits.max_message_body_bytes)
            }
            CommandV2::Speak {
                room_id,
                body,
                addressed_to,
                ..
            }
            | CommandV2::SpeakIdentified {
                room_id,
                body,
                addressed_to,
                ..
            }
            | CommandV2::SpeakReply {
                room_id,
                body,
                addressed_to,
                ..
            }
            | CommandV2::SpeakInTopic {
                room_id,
                body,
                addressed_to,
                ..
            } => {
                valid_room(room_id)
                    && valid_text(body, self.limits.max_message_body_bytes)
                    && addressed_to.len() <= self.limits.max_addressed_participants
                    && addressed_to
                        .iter()
                        .all(|participant| valid_participant(participant))
            }
            CommandV2::SpeakWithAttachments {
                room_id,
                body,
                addressed_to,
                attachments,
                ..
            } => {
                valid_room(room_id)
                    && valid_text(body, self.limits.max_message_body_bytes)
                    && addressed_to.len() <= self.limits.max_addressed_participants
                    && addressed_to
                        .iter()
                        .all(|participant| valid_participant(participant))
                    && (1..=8).contains(&attachments.len())
            }
            CommandV2::SpeakWithVellumArtifacts {
                room_id,
                body,
                addressed_to,
                references,
                ..
            } => {
                valid_room(room_id)
                    && valid_text(body, self.limits.max_message_body_bytes)
                    && addressed_to.len() <= self.limits.max_addressed_participants
                    && addressed_to
                        .iter()
                        .all(|participant| valid_participant(participant))
                    && (1..=8).contains(&references.len())
            }
        }
    }
}

fn command_needs_admission_time(
    engine: &Engine,
    participant: &ParticipantId,
    command: &CommandV2,
) -> bool {
    let room_id = match command {
        CommandV2::Speak { room_id, .. }
        | CommandV2::SpeakIdentified { room_id, .. }
        | CommandV2::SpeakReply { room_id, .. }
        | CommandV2::SpeakInTopic { room_id, .. }
        | CommandV2::SpeakWithAttachments { room_id, .. }
        | CommandV2::SpeakWithVellumArtifacts { room_id, .. } => room_id,
        _ => return false,
    };
    engine
        .admission_control(participant, &RoomId::new(room_id))
        .is_ok_and(|status| status.minimum_message_interval_ms > 0)
}

fn apply_revisioned_with_optional_time(
    engine: &mut Engine,
    expected_revision: u64,
    command: EngineCommand,
    admission_time: Option<&AdmissionTimeV1>,
) -> Result<agora_engine::TransitionV2, RefusalV2> {
    let envelope = RevisionedCommandV2 {
        expected_revision: Revision(expected_revision),
        command,
    };
    match admission_time {
        Some(time) => engine.apply_revisioned_at(envelope, time),
        None => engine.apply_revisioned(envelope),
    }
    .map_err(map_engine_refusal)
}

fn map_admission_time_refusal(refusal: AdmissionTimeRefusalV1) -> RefusalV2 {
    match refusal {
        AdmissionTimeRefusalV1::IdentityExhausted => RefusalV2::Exhausted,
        AdmissionTimeRefusalV1::Regressed
        | AdmissionTimeRefusalV1::ForeignObservation
        | AdmissionTimeRefusalV1::StaleObservation => RefusalV2::Unavailable,
    }
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.len() <= maximum
}

fn map_read_scope_to_engine(scope: ReadScopeV1) -> EngineReadScopeV1 {
    match scope {
        ReadScopeV1::Room => EngineReadScopeV1::Room,
        ReadScopeV1::Thread(root) => {
            EngineReadScopeV1::Thread(MessageId::from_bytes(root.into_bytes()))
        }
        ReadScopeV1::Topic(topic) => {
            EngineReadScopeV1::Topic(TopicId::from_bytes(topic.into_bytes()))
        }
    }
}

fn map_read_scope_from_engine(scope: &EngineReadScopeV1) -> ReadScopeV1 {
    match scope {
        EngineReadScopeV1::Room => ReadScopeV1::Room,
        EngineReadScopeV1::Thread(root) => ReadScopeV1::Thread(MessageIdV1::new(root.into_bytes())),
        EngineReadScopeV1::Topic(topic) => ReadScopeV1::Topic(TopicIdV1::new(topic.into_bytes())),
    }
}

fn map_read_position_from_engine(position: &agora_engine::ReadPositionV1) -> ReadPositionV1 {
    ReadPositionV1 {
        room_id: position.room_id().as_str().to_owned(),
        scope: map_read_scope_from_engine(position.scope()),
        target_message_id: MessageIdV1::new(position.target_message_id().into_bytes()),
        target_sequence: position.target_sequence(),
        observed_revision: position.observed_revision().0,
    }
}

fn map_read_position_refusal(refusal: ReadPositionRefusalV1) -> RefusalV2 {
    match refusal {
        ReadPositionRefusalV1::UnknownParticipant => RefusalV2::UnknownParticipant,
        ReadPositionRefusalV1::UnknownRoom => RefusalV2::UnknownRoom,
        ReadPositionRefusalV1::NotMember => RefusalV2::NotMember,
        ReadPositionRefusalV1::Stale { actual_revision } => RefusalV2::Stale {
            actual_revision: actual_revision.0,
        },
        ReadPositionRefusalV1::InvalidScope => RefusalV2::InvalidReadScope,
        ReadPositionRefusalV1::InvalidTarget => RefusalV2::InvalidReadTarget,
        ReadPositionRefusalV1::Regression => RefusalV2::ReadPositionRegression,
        ReadPositionRefusalV1::Capacity => RefusalV2::Capacity,
    }
}

fn map_attention_scope_to_engine(scope: AttentionScopeV1) -> EngineAttentionScopeV1 {
    match scope {
        AttentionScopeV1::Room => EngineAttentionScopeV1::Room,
        AttentionScopeV1::Thread(root) => {
            EngineAttentionScopeV1::Thread(MessageId::from_bytes(root.into_bytes()))
        }
        AttentionScopeV1::Topic(topic) => {
            EngineAttentionScopeV1::Topic(TopicId::from_bytes(topic.into_bytes()))
        }
    }
}

fn map_attention_preference_to_engine(
    preference: AttentionPreferenceV1,
) -> EngineAttentionPreferenceV1 {
    EngineAttentionPreferenceV1::new(
        preference.following,
        preference.muted,
        match preference.notification_intent {
            NotificationIntentV1::Default => EngineNotificationIntentV1::Default,
            NotificationIntentV1::AllActivity => EngineNotificationIntentV1::AllActivity,
            NotificationIntentV1::AddressedOnly => EngineNotificationIntentV1::AddressedOnly,
            NotificationIntentV1::None => EngineNotificationIntentV1::None,
        },
    )
}

fn map_attention_preference_from_engine(
    preference: EngineAttentionPreferenceV1,
) -> AttentionPreferenceV1 {
    AttentionPreferenceV1 {
        following: preference.following(),
        muted: preference.muted(),
        notification_intent: match preference.notification_intent() {
            EngineNotificationIntentV1::Default => NotificationIntentV1::Default,
            EngineNotificationIntentV1::AllActivity => NotificationIntentV1::AllActivity,
            EngineNotificationIntentV1::AddressedOnly => NotificationIntentV1::AddressedOnly,
            EngineNotificationIntentV1::None => NotificationIntentV1::None,
        },
    }
}

fn map_attention_refusal(refusal: AttentionPreferenceRefusalV1) -> RefusalV2 {
    match refusal {
        AttentionPreferenceRefusalV1::UnknownParticipant => RefusalV2::UnknownParticipant,
        AttentionPreferenceRefusalV1::UnknownRoom => RefusalV2::UnknownRoom,
        AttentionPreferenceRefusalV1::NotMember => RefusalV2::NotMember,
        AttentionPreferenceRefusalV1::Stale { actual_revision } => RefusalV2::Stale {
            actual_revision: actual_revision.0,
        },
        AttentionPreferenceRefusalV1::InvalidScope => RefusalV2::InvalidAttentionScope,
        AttentionPreferenceRefusalV1::Capacity => RefusalV2::Capacity,
    }
}

fn map_stable_reference_availability(
    availability: agora_engine::StableReferenceAvailabilityV1,
) -> StableReferenceAvailabilityV1 {
    match availability {
        agora_engine::StableReferenceAvailabilityV1::Available => {
            StableReferenceAvailabilityV1::Available
        }
        agora_engine::StableReferenceAvailabilityV1::Tombstoned => {
            StableReferenceAvailabilityV1::Tombstoned
        }
        agora_engine::StableReferenceAvailabilityV1::Evicted => {
            StableReferenceAvailabilityV1::Evicted
        }
    }
}

fn map_saved_reference_from_engine(reference: agora_engine::SavedReferenceV1) -> SavedReferenceV1 {
    SavedReferenceV1 {
        message_id: MessageIdV1::new(reference.message_id().into_bytes()),
        original_sequence: reference.original_sequence(),
        availability: map_stable_reference_availability(reference.availability()),
    }
}

fn map_private_save_refusal(refusal: PrivateSaveRefusalV1) -> RefusalV2 {
    match refusal {
        PrivateSaveRefusalV1::UnknownParticipant => RefusalV2::UnknownParticipant,
        PrivateSaveRefusalV1::UnknownRoom => RefusalV2::UnknownRoom,
        PrivateSaveRefusalV1::NotMember => RefusalV2::NotMember,
        PrivateSaveRefusalV1::Stale { actual_revision } => RefusalV2::Stale {
            actual_revision: actual_revision.0,
        },
        PrivateSaveRefusalV1::UnknownMessage => RefusalV2::InvalidInput,
        PrivateSaveRefusalV1::InvalidChange => RefusalV2::InvalidSaveChange,
        PrivateSaveRefusalV1::Capacity => RefusalV2::Capacity,
    }
}

fn map_message_from_engine(message: agora_engine::Message) -> MessageV2 {
    MessageV2 {
        sequence: message.sequence(),
        author: message.author().as_str().to_owned(),
        logical_time: message.at().0,
        body: message.body().to_owned(),
        addressed_to: message
            .addressed_to()
            .iter()
            .map(|participant| participant.as_str().to_owned())
            .collect(),
    }
}

fn map_identified_message_from_engine(message: agora_engine::Message) -> IdentifiedMessageV1 {
    IdentifiedMessageV1 {
        message_id: MessageIdV1::new(message.id().into_bytes()),
        sequence: message.sequence(),
        author: message.author().as_str().to_owned(),
        logical_time: message.at().0,
        body: message.body().to_owned(),
        addressed_to: message
            .addressed_to()
            .iter()
            .map(|participant| participant.as_str().to_owned())
            .collect(),
    }
}

fn map_attachment_target_to_engine(
    target: AttachmentMessageTargetV1,
) -> EngineAttachmentMessageTargetV1 {
    match target {
        AttachmentMessageTargetV1::Room => EngineAttachmentMessageTargetV1::Room,
        AttachmentMessageTargetV1::Reply(message_id) => {
            EngineAttachmentMessageTargetV1::Reply(MessageId::from_bytes(message_id.into_bytes()))
        }
        AttachmentMessageTargetV1::Topic(topic_id) => {
            EngineAttachmentMessageTargetV1::Topic(TopicId::from_bytes(topic_id.into_bytes()))
        }
    }
}

fn map_vellum_artifact_target_to_engine(
    target: VellumArtifactMessageTargetV1,
) -> EngineVellumArtifactMessageTargetV1 {
    match target {
        VellumArtifactMessageTargetV1::Room => EngineVellumArtifactMessageTargetV1::Room,
        VellumArtifactMessageTargetV1::Reply(message_id) => {
            EngineVellumArtifactMessageTargetV1::Reply(MessageId::from_bytes(
                message_id.into_bytes(),
            ))
        }
        VellumArtifactMessageTargetV1::Topic(topic_id) => {
            EngineVellumArtifactMessageTargetV1::Topic(TopicId::from_bytes(topic_id.into_bytes()))
        }
    }
}

fn map_vellum_artifact_reference_to_engine(
    reference: VellumArtifactReferenceV1,
) -> EngineVellumArtifactReferenceV1 {
    EngineVellumArtifactReferenceV1::new(
        EngineVellumArtifactReferenceIdV1::new(reference.reference_id.into_bytes()),
        EngineVellumDocumentTargetV1::new(
            reference.target.workspace_id,
            reference.target.document_id,
            reference.target.source_revision,
        )
        .expect("wire validation rejects zero Vellum identities"),
    )
}

fn map_vellum_artifact_message_from_engine(
    message: agora_engine::Message,
) -> VellumArtifactMessageV1 {
    VellumArtifactMessageV1 {
        message_id: MessageIdV1::new(message.id().into_bytes()),
        sequence: message.sequence(),
        author: message.author().as_str().to_owned(),
        logical_time: message.at().0,
        body: message.body().to_owned(),
        addressed_to: message
            .addressed_to()
            .iter()
            .map(|participant| participant.as_str().to_owned())
            .collect(),
        reply_to_root: message
            .reply_to_root()
            .map(|message_id| MessageIdV1::new(message_id.into_bytes())),
        topic_id: message
            .topic_id()
            .map(|topic_id| TopicIdV1::new(topic_id.into_bytes())),
        references: message
            .vellum_artifacts()
            .iter()
            .map(|reference| MessageVellumArtifactReferenceV1 {
                reference_id: VellumArtifactReferenceIdV1::new(
                    reference.reference_id().into_bytes(),
                ),
                target: reference.target().map(|target| VellumDocumentTargetV1 {
                    workspace_id: target.workspace_id(),
                    document_id: target.document_id(),
                    source_revision: target.source_revision(),
                }),
            })
            .collect(),
    }
}

fn map_attachment_descriptor_to_engine(
    descriptor: AttachmentDescriptorV1,
) -> EngineAttachmentDescriptorV1 {
    EngineAttachmentDescriptorV1::new(
        EngineAttachmentIdV1::new(descriptor.attachment_id.into_bytes()),
        EngineAttachmentStoreIdV1::new(descriptor.store_id.into_bytes()),
        EngineAttachmentObjectKeyV1::new(descriptor.object_key.into_bytes()),
        descriptor.declared_bytes,
        descriptor.media_type,
        descriptor.display_label,
        match descriptor.integrity {
            AttachmentIntegrityV1::Sha256(digest) => {
                EngineAttachmentIntegrityV1::Sha256(EngineSha256DigestV1::new(digest.into_bytes()))
            }
        },
    )
}

fn map_attachment_descriptor_from_engine(
    descriptor: &agora_engine::AttachmentDescriptorV1,
) -> AttachmentDescriptorV1 {
    AttachmentDescriptorV1 {
        attachment_id: AttachmentIdV1::new(descriptor.attachment_id().into_bytes()),
        store_id: AttachmentStoreIdV1::new(descriptor.store_id().into_bytes()),
        object_key: AttachmentObjectKeyV1::new(descriptor.object_key().into_bytes()),
        declared_bytes: descriptor.declared_bytes(),
        media_type: descriptor.media_type().to_owned(),
        display_label: descriptor.display_label().to_owned(),
        integrity: match descriptor.integrity() {
            agora_engine::AttachmentIntegrityV1::Sha256(digest) => {
                AttachmentIntegrityV1::Sha256(Sha256DigestV1::new(digest.into_bytes()))
            }
        },
    }
}

fn map_attachment_message_from_engine(message: agora_engine::Message) -> AttachmentMessageV1 {
    AttachmentMessageV1 {
        message_id: MessageIdV1::new(message.id().into_bytes()),
        sequence: message.sequence(),
        author: message.author().as_str().to_owned(),
        logical_time: message.at().0,
        body: message.body().to_owned(),
        addressed_to: message
            .addressed_to()
            .iter()
            .map(|participant| participant.as_str().to_owned())
            .collect(),
        reply_to_root: message
            .reply_to_root()
            .map(|message_id| MessageIdV1::new(message_id.into_bytes())),
        topic_id: message
            .topic_id()
            .map(|topic_id| TopicIdV1::new(topic_id.into_bytes())),
        attachments: message
            .attachments()
            .iter()
            .map(|attachment| MessageAttachmentV1 {
                attachment_id: AttachmentIdV1::new(attachment.attachment_id().into_bytes()),
                descriptor: attachment
                    .descriptor()
                    .map(map_attachment_descriptor_from_engine),
            })
            .collect(),
    }
}

fn map_threaded_message_from_engine(message: agora_engine::Message) -> ThreadedMessageV1 {
    ThreadedMessageV1 {
        message_id: MessageIdV1::new(message.id().into_bytes()),
        sequence: message.sequence(),
        author: message.author().as_str().to_owned(),
        logical_time: message.at().0,
        body: message.body().to_owned(),
        addressed_to: message
            .addressed_to()
            .iter()
            .map(|participant| participant.as_str().to_owned())
            .collect(),
        reply_to_root: message
            .reply_to_root()
            .map(|root| MessageIdV1::new(root.into_bytes())),
    }
}

fn map_topic_message_from_engine(message: agora_engine::Message) -> TopicMessageV1 {
    TopicMessageV1 {
        message_id: MessageIdV1::new(message.id().into_bytes()),
        sequence: message.sequence(),
        author: message.author().as_str().to_owned(),
        logical_time: message.at().0,
        body: message.body().to_owned(),
        addressed_to: message
            .addressed_to()
            .iter()
            .map(|participant| participant.as_str().to_owned())
            .collect(),
        reply_to_root: message
            .reply_to_root()
            .map(|root| MessageIdV1::new(root.into_bytes())),
        topic_id: message
            .topic_id()
            .map(|topic| TopicIdV1::new(topic.into_bytes())),
    }
}

fn map_thread_page(
    page: agora_engine::ThreadMessagePage,
    next: Option<PageCursorV2>,
) -> ThreadMessagePageV1 {
    ThreadMessagePageV1 {
        revision: page.revision.0,
        room_id: page.room_id.as_str().to_owned(),
        root_message_id: page
            .root_message_id
            .map(|root| MessageIdV1::new(root.into_bytes())),
        messages: page
            .messages
            .into_iter()
            .map(map_threaded_message_from_engine)
            .collect(),
        oldest_retained_sequence: page.oldest_retained_sequence,
        next_message_sequence: page.next_message_sequence,
        next,
    }
}

fn map_identified_room_event(
    event: agora_engine::DomainEventV2,
) -> Result<IdentifiedRoomEventV1, RefusalV2> {
    Ok(match event {
        agora_engine::DomainEventV2::ParticipantRegistered { .. } => {
            return Err(RefusalV2::Unavailable);
        }
        agora_engine::DomainEventV2::PollCreated {
            actor,
            poll_id,
            prompt,
            options,
            eligible_participants,
            max_choices,
            ..
        } => IdentifiedRoomEventV1::PollCreated {
            actor_id: actor.as_str().to_owned(),
            poll_id: PollIdV1::new(poll_id.into_bytes()),
            prompt,
            options,
            eligible_participant_ids: eligible_participants
                .into_iter()
                .map(|participant| participant.as_str().to_owned())
                .collect(),
            max_choices,
        },
        agora_engine::DomainEventV2::PollBallotChanged {
            poll_id,
            totals,
            total_ballots,
            ..
        } => IdentifiedRoomEventV1::PollBallotChanged {
            poll_id: PollIdV1::new(poll_id.into_bytes()),
            totals,
            total_ballots,
        },
        agora_engine::DomainEventV2::PollLifecycleChanged {
            actor,
            poll_id,
            open,
            ..
        } => IdentifiedRoomEventV1::PollLifecycleChanged {
            actor_id: actor.as_str().to_owned(),
            poll_id: PollIdV1::new(poll_id.into_bytes()),
            open,
        },
        agora_engine::DomainEventV2::PinChanged {
            actor,
            message_id,
            original_sequence,
            pinned,
            ..
        } => IdentifiedRoomEventV1::PinChanged {
            actor_id: actor.as_str().to_owned(),
            message_id: MessageIdV1::new(message_id.into_bytes()),
            original_sequence,
            pinned,
        },
        agora_engine::DomainEventV2::RoomCreated {
            owner, admission, ..
        } => IdentifiedRoomEventV1::RoomCreated {
            owner: owner.as_str().to_owned(),
            admission: map_admission_from_engine(admission),
        },
        agora_engine::DomainEventV2::PeerConversationCreated {
            initiator,
            profile,
            founding_members,
            ..
        } => IdentifiedRoomEventV1::PeerConversationCreated(map_peer_conversation_event(
            initiator,
            profile,
            founding_members,
        )),
        agora_engine::DomainEventV2::TopicCreated {
            actor,
            topic_id,
            name,
            ..
        } => IdentifiedRoomEventV1::TopicCreated {
            actor_id: actor.as_str().to_owned(),
            topic_id: TopicIdV1::new(topic_id.into_bytes()),
            name,
        },
        agora_engine::DomainEventV2::TopicRenamed {
            actor,
            topic_id,
            previous,
            name,
            ..
        } => IdentifiedRoomEventV1::TopicRenamed {
            actor_id: actor.as_str().to_owned(),
            topic_id: TopicIdV1::new(topic_id.into_bytes()),
            previous,
            name,
        },
        agora_engine::DomainEventV2::TopicLifecycleChanged {
            actor,
            topic_id,
            previous_resolved,
            resolved,
            ..
        } => IdentifiedRoomEventV1::TopicLifecycleChanged {
            actor_id: actor.as_str().to_owned(),
            topic_id: TopicIdV1::new(topic_id.into_bytes()),
            previous_resolved,
            resolved,
        },
        agora_engine::DomainEventV2::TopicSupersessionChanged {
            actor,
            topic_id,
            previous_successor_topic_id,
            successor_topic_id,
            ..
        } => IdentifiedRoomEventV1::TopicSupersessionChanged {
            actor_id: actor.as_str().to_owned(),
            topic_id: TopicIdV1::new(topic_id.into_bytes()),
            previous_successor_topic_id: previous_successor_topic_id
                .map(|id| TopicIdV1::new(id.into_bytes())),
            successor_topic_id: successor_topic_id.map(|id| TopicIdV1::new(id.into_bytes())),
        },
        agora_engine::DomainEventV2::TopicAliasChanged {
            actor,
            topic_id,
            alias,
            added,
            ..
        } => IdentifiedRoomEventV1::TopicAliasChanged {
            actor_id: actor.as_str().to_owned(),
            topic_id: TopicIdV1::new(topic_id.into_bytes()),
            alias,
            added,
        },
        agora_engine::DomainEventV2::TopicTagChanged {
            actor,
            topic_id,
            tag,
            added,
            ..
        } => IdentifiedRoomEventV1::TopicTagChanged {
            actor_id: actor.as_str().to_owned(),
            topic_id: TopicIdV1::new(topic_id.into_bytes()),
            tag,
            added,
        },
        agora_engine::DomainEventV2::InvitationRecorded { participant, .. } => {
            IdentifiedRoomEventV1::InvitationRecorded {
                participant_id: participant.as_str().to_owned(),
            }
        }
        agora_engine::DomainEventV2::ParticipantJoined { participant, .. } => {
            IdentifiedRoomEventV1::ParticipantJoined {
                participant_id: participant.as_str().to_owned(),
            }
        }
        agora_engine::DomainEventV2::ParticipantLeft { participant, .. } => {
            IdentifiedRoomEventV1::ParticipantLeft {
                participant_id: participant.as_str().to_owned(),
            }
        }
        agora_engine::DomainEventV2::RoleChanged {
            participant, role, ..
        } => IdentifiedRoomEventV1::RoleChanged {
            participant_id: participant.as_str().to_owned(),
            role: map_role_from_engine(role),
        },
        agora_engine::DomainEventV2::RoomPolicyChanged {
            actor,
            previous,
            admission,
            ..
        } => IdentifiedRoomEventV1::RoomPolicyChanged {
            actor_id: actor.as_str().to_owned(),
            previous: map_admission_from_engine(previous),
            admission: map_admission_from_engine(admission),
        },
        agora_engine::DomainEventV2::MessageIntervalChanged {
            actor,
            previous_interval_ms,
            minimum_interval_ms,
            ..
        } => IdentifiedRoomEventV1::MessageIntervalChanged {
            actor_id: actor.as_str().to_owned(),
            previous_interval_ms,
            minimum_interval_ms,
        },
        agora_engine::DomainEventV2::ParticipantKicked {
            actor,
            participant,
            reason,
            causation,
            ..
        } => IdentifiedRoomEventV1::ParticipantKicked {
            actor_id: actor.as_str().to_owned(),
            participant_id: participant.as_str().to_owned(),
            reason: reason.as_str().to_owned(),
            causation: causation.into_bytes(),
        },
        agora_engine::DomainEventV2::ParticipantBanned {
            actor,
            participant,
            reason,
            causation,
            ..
        } => IdentifiedRoomEventV1::ParticipantBanned {
            actor_id: actor.as_str().to_owned(),
            participant_id: participant.as_str().to_owned(),
            reason: reason.as_str().to_owned(),
            causation: causation.into_bytes(),
        },
        agora_engine::DomainEventV2::ParticipantUnbanned {
            actor,
            participant,
            reason,
            causation,
            ..
        } => IdentifiedRoomEventV1::ParticipantUnbanned {
            actor_id: actor.as_str().to_owned(),
            participant_id: participant.as_str().to_owned(),
            reason: reason.as_str().to_owned(),
            causation: causation.into_bytes(),
        },
        agora_engine::DomainEventV2::PostingRestrictionChanged {
            actor,
            participant,
            restricted,
            reason,
            causation,
            ..
        } => IdentifiedRoomEventV1::PostingRestrictionChanged {
            actor_id: actor.as_str().to_owned(),
            participant_id: participant.as_str().to_owned(),
            restricted,
            reason: reason.as_str().to_owned(),
            causation: causation.into_bytes(),
        },
        agora_engine::DomainEventV2::MessageAccepted { message, .. } => {
            IdentifiedRoomEventV1::MessageAccepted(map_identified_message_from_engine(message))
        }
        agora_engine::DomainEventV2::MessageWithdrawn {
            actor,
            message_id,
            sequence,
            ..
        } => IdentifiedRoomEventV1::MessageWithdrawn {
            actor_id: actor.as_str().to_owned(),
            message_id: MessageIdV1::new(message_id.into_bytes()),
            sequence,
        },
        agora_engine::DomainEventV2::MessageRedacted {
            actor,
            author,
            message_id,
            sequence,
            reason,
            causation,
            ..
        } => IdentifiedRoomEventV1::MessageRedacted {
            actor_id: actor.as_str().to_owned(),
            author_id: author.as_str().to_owned(),
            message_id: MessageIdV1::new(message_id.into_bytes()),
            sequence,
            reason: reason.as_str().to_owned(),
            causation: causation.into_bytes(),
        },
        agora_engine::DomainEventV2::ThreadLifecycleChanged {
            actor,
            root_message_id,
            archived,
            reason,
            causation,
            ..
        } => IdentifiedRoomEventV1::ThreadLifecycleChanged {
            actor_id: actor.as_str().to_owned(),
            root_message_id: MessageIdV1::new(root_message_id.into_bytes()),
            archived,
            reason: reason.as_str().to_owned(),
            causation: causation.into_bytes(),
        },
        agora_engine::DomainEventV2::ReactionChanged {
            actor,
            message_id,
            key,
            reacted,
            ..
        } => IdentifiedRoomEventV1::ReactionChanged {
            actor_id: actor.as_str().to_owned(),
            message_id: MessageIdV1::new(message_id.into_bytes()),
            key,
            reacted,
        },
        agora_engine::DomainEventV2::MessageEdited {
            actor,
            message_id,
            message_revision,
            at,
            body,
            ..
        } => IdentifiedRoomEventV1::MessageEdited {
            actor_id: actor.as_str().to_owned(),
            message_id: MessageIdV1::new(message_id.into_bytes()),
            message_revision,
            logical_time: at.0,
            body,
        },
    })
}

fn map_threaded_room_event(
    event: agora_engine::DomainEventV2,
) -> Result<ThreadedRoomEventV1, RefusalV2> {
    Ok(match event {
        agora_engine::DomainEventV2::ParticipantRegistered { .. } => {
            return Err(RefusalV2::Unavailable);
        }
        agora_engine::DomainEventV2::PollCreated {
            actor,
            poll_id,
            prompt,
            options,
            eligible_participants,
            max_choices,
            ..
        } => ThreadedRoomEventV1::PollCreated {
            actor_id: actor.as_str().to_owned(),
            poll_id: PollIdV1::new(poll_id.into_bytes()),
            prompt,
            options,
            eligible_participant_ids: eligible_participants
                .into_iter()
                .map(|participant| participant.as_str().to_owned())
                .collect(),
            max_choices,
        },
        agora_engine::DomainEventV2::PollBallotChanged {
            poll_id,
            totals,
            total_ballots,
            ..
        } => ThreadedRoomEventV1::PollBallotChanged {
            poll_id: PollIdV1::new(poll_id.into_bytes()),
            totals,
            total_ballots,
        },
        agora_engine::DomainEventV2::PollLifecycleChanged {
            actor,
            poll_id,
            open,
            ..
        } => ThreadedRoomEventV1::PollLifecycleChanged {
            actor_id: actor.as_str().to_owned(),
            poll_id: PollIdV1::new(poll_id.into_bytes()),
            open,
        },
        agora_engine::DomainEventV2::PinChanged {
            actor,
            message_id,
            original_sequence,
            pinned,
            ..
        } => ThreadedRoomEventV1::PinChanged {
            actor_id: actor.as_str().to_owned(),
            message_id: MessageIdV1::new(message_id.into_bytes()),
            original_sequence,
            pinned,
        },
        agora_engine::DomainEventV2::RoomCreated {
            owner, admission, ..
        } => ThreadedRoomEventV1::RoomCreated {
            owner: owner.as_str().to_owned(),
            admission: map_admission_from_engine(admission),
        },
        agora_engine::DomainEventV2::PeerConversationCreated {
            initiator,
            profile,
            founding_members,
            ..
        } => ThreadedRoomEventV1::PeerConversationCreated(map_peer_conversation_event(
            initiator,
            profile,
            founding_members,
        )),
        agora_engine::DomainEventV2::TopicCreated {
            actor,
            topic_id,
            name,
            ..
        } => ThreadedRoomEventV1::TopicCreated {
            actor_id: actor.as_str().to_owned(),
            topic_id: TopicIdV1::new(topic_id.into_bytes()),
            name,
        },
        agora_engine::DomainEventV2::TopicRenamed {
            actor,
            topic_id,
            previous,
            name,
            ..
        } => ThreadedRoomEventV1::TopicRenamed {
            actor_id: actor.as_str().to_owned(),
            topic_id: TopicIdV1::new(topic_id.into_bytes()),
            previous,
            name,
        },
        agora_engine::DomainEventV2::TopicLifecycleChanged {
            actor,
            topic_id,
            previous_resolved,
            resolved,
            ..
        } => ThreadedRoomEventV1::TopicLifecycleChanged {
            actor_id: actor.as_str().to_owned(),
            topic_id: TopicIdV1::new(topic_id.into_bytes()),
            previous_resolved,
            resolved,
        },
        agora_engine::DomainEventV2::TopicSupersessionChanged {
            actor,
            topic_id,
            previous_successor_topic_id,
            successor_topic_id,
            ..
        } => ThreadedRoomEventV1::TopicSupersessionChanged {
            actor_id: actor.as_str().to_owned(),
            topic_id: TopicIdV1::new(topic_id.into_bytes()),
            previous_successor_topic_id: previous_successor_topic_id
                .map(|id| TopicIdV1::new(id.into_bytes())),
            successor_topic_id: successor_topic_id.map(|id| TopicIdV1::new(id.into_bytes())),
        },
        agora_engine::DomainEventV2::TopicAliasChanged {
            actor,
            topic_id,
            alias,
            added,
            ..
        } => ThreadedRoomEventV1::TopicAliasChanged {
            actor_id: actor.as_str().to_owned(),
            topic_id: TopicIdV1::new(topic_id.into_bytes()),
            alias,
            added,
        },
        agora_engine::DomainEventV2::TopicTagChanged {
            actor,
            topic_id,
            tag,
            added,
            ..
        } => ThreadedRoomEventV1::TopicTagChanged {
            actor_id: actor.as_str().to_owned(),
            topic_id: TopicIdV1::new(topic_id.into_bytes()),
            tag,
            added,
        },
        agora_engine::DomainEventV2::InvitationRecorded { participant, .. } => {
            ThreadedRoomEventV1::InvitationRecorded {
                participant_id: participant.as_str().to_owned(),
            }
        }
        agora_engine::DomainEventV2::ParticipantJoined { participant, .. } => {
            ThreadedRoomEventV1::ParticipantJoined {
                participant_id: participant.as_str().to_owned(),
            }
        }
        agora_engine::DomainEventV2::ParticipantLeft { participant, .. } => {
            ThreadedRoomEventV1::ParticipantLeft {
                participant_id: participant.as_str().to_owned(),
            }
        }
        agora_engine::DomainEventV2::RoleChanged {
            participant, role, ..
        } => ThreadedRoomEventV1::RoleChanged {
            participant_id: participant.as_str().to_owned(),
            role: map_role_from_engine(role),
        },
        agora_engine::DomainEventV2::RoomPolicyChanged {
            actor,
            previous,
            admission,
            ..
        } => ThreadedRoomEventV1::RoomPolicyChanged {
            actor_id: actor.as_str().to_owned(),
            previous: map_admission_from_engine(previous),
            admission: map_admission_from_engine(admission),
        },
        agora_engine::DomainEventV2::MessageIntervalChanged {
            actor,
            previous_interval_ms,
            minimum_interval_ms,
            ..
        } => ThreadedRoomEventV1::MessageIntervalChanged {
            actor_id: actor.as_str().to_owned(),
            previous_interval_ms,
            minimum_interval_ms,
        },
        agora_engine::DomainEventV2::ParticipantKicked {
            actor,
            participant,
            reason,
            causation,
            ..
        } => ThreadedRoomEventV1::ParticipantKicked {
            actor_id: actor.as_str().to_owned(),
            participant_id: participant.as_str().to_owned(),
            reason: reason.as_str().to_owned(),
            causation: causation.into_bytes(),
        },
        agora_engine::DomainEventV2::ParticipantBanned {
            actor,
            participant,
            reason,
            causation,
            ..
        } => ThreadedRoomEventV1::ParticipantBanned {
            actor_id: actor.as_str().to_owned(),
            participant_id: participant.as_str().to_owned(),
            reason: reason.as_str().to_owned(),
            causation: causation.into_bytes(),
        },
        agora_engine::DomainEventV2::ParticipantUnbanned {
            actor,
            participant,
            reason,
            causation,
            ..
        } => ThreadedRoomEventV1::ParticipantUnbanned {
            actor_id: actor.as_str().to_owned(),
            participant_id: participant.as_str().to_owned(),
            reason: reason.as_str().to_owned(),
            causation: causation.into_bytes(),
        },
        agora_engine::DomainEventV2::PostingRestrictionChanged {
            actor,
            participant,
            restricted,
            reason,
            causation,
            ..
        } => ThreadedRoomEventV1::PostingRestrictionChanged {
            actor_id: actor.as_str().to_owned(),
            participant_id: participant.as_str().to_owned(),
            restricted,
            reason: reason.as_str().to_owned(),
            causation: causation.into_bytes(),
        },
        agora_engine::DomainEventV2::MessageAccepted { message, .. } => {
            ThreadedRoomEventV1::MessageAccepted(map_threaded_message_from_engine(message))
        }
        agora_engine::DomainEventV2::MessageWithdrawn {
            actor,
            message_id,
            sequence,
            ..
        } => ThreadedRoomEventV1::MessageWithdrawn {
            actor_id: actor.as_str().to_owned(),
            message_id: MessageIdV1::new(message_id.into_bytes()),
            sequence,
        },
        agora_engine::DomainEventV2::MessageRedacted {
            actor,
            author,
            message_id,
            sequence,
            reason,
            causation,
            ..
        } => ThreadedRoomEventV1::MessageRedacted {
            actor_id: actor.as_str().to_owned(),
            author_id: author.as_str().to_owned(),
            message_id: MessageIdV1::new(message_id.into_bytes()),
            sequence,
            reason: reason.as_str().to_owned(),
            causation: causation.into_bytes(),
        },
        agora_engine::DomainEventV2::ThreadLifecycleChanged {
            actor,
            root_message_id,
            archived,
            reason,
            causation,
            ..
        } => ThreadedRoomEventV1::ThreadLifecycleChanged {
            actor_id: actor.as_str().to_owned(),
            root_message_id: MessageIdV1::new(root_message_id.into_bytes()),
            archived,
            reason: reason.as_str().to_owned(),
            causation: causation.into_bytes(),
        },
        agora_engine::DomainEventV2::ReactionChanged {
            actor,
            message_id,
            key,
            reacted,
            ..
        } => ThreadedRoomEventV1::ReactionChanged {
            actor_id: actor.as_str().to_owned(),
            message_id: MessageIdV1::new(message_id.into_bytes()),
            key,
            reacted,
        },
        agora_engine::DomainEventV2::MessageEdited {
            actor,
            message_id,
            message_revision,
            at,
            body,
            ..
        } => ThreadedRoomEventV1::MessageEdited {
            actor_id: actor.as_str().to_owned(),
            message_id: MessageIdV1::new(message_id.into_bytes()),
            message_revision,
            logical_time: at.0,
            body,
        },
    })
}

fn map_topic_room_event(event: agora_engine::DomainEventV2) -> Result<TopicRoomEventV1, RefusalV2> {
    if let agora_engine::DomainEventV2::MessageAccepted { message, .. } = event {
        return Ok(TopicRoomEventV1::MessageAccepted(
            map_topic_message_from_engine(message),
        ));
    }
    Ok(match map_threaded_room_event(event)? {
        ThreadedRoomEventV1::RoomCreated { owner, admission } => {
            TopicRoomEventV1::RoomCreated { owner, admission }
        }
        ThreadedRoomEventV1::PeerConversationCreated(event) => {
            TopicRoomEventV1::PeerConversationCreated(event)
        }
        ThreadedRoomEventV1::TopicCreated {
            actor_id,
            topic_id,
            name,
        } => TopicRoomEventV1::TopicCreated {
            actor_id,
            topic_id,
            name,
        },
        ThreadedRoomEventV1::TopicRenamed {
            actor_id,
            topic_id,
            previous,
            name,
        } => TopicRoomEventV1::TopicRenamed {
            actor_id,
            topic_id,
            previous,
            name,
        },
        ThreadedRoomEventV1::TopicLifecycleChanged {
            actor_id,
            topic_id,
            previous_resolved,
            resolved,
        } => TopicRoomEventV1::TopicLifecycleChanged {
            actor_id,
            topic_id,
            previous_resolved,
            resolved,
        },
        ThreadedRoomEventV1::TopicSupersessionChanged {
            actor_id,
            topic_id,
            previous_successor_topic_id,
            successor_topic_id,
        } => TopicRoomEventV1::TopicSupersessionChanged {
            actor_id,
            topic_id,
            previous_successor_topic_id,
            successor_topic_id,
        },
        ThreadedRoomEventV1::TopicAliasChanged {
            actor_id,
            topic_id,
            alias,
            added,
        } => TopicRoomEventV1::TopicAliasChanged {
            actor_id,
            topic_id,
            alias,
            added,
        },
        ThreadedRoomEventV1::TopicTagChanged {
            actor_id,
            topic_id,
            tag,
            added,
        } => TopicRoomEventV1::TopicTagChanged {
            actor_id,
            topic_id,
            tag,
            added,
        },
        ThreadedRoomEventV1::InvitationRecorded { participant_id } => {
            TopicRoomEventV1::InvitationRecorded { participant_id }
        }
        ThreadedRoomEventV1::ParticipantJoined { participant_id } => {
            TopicRoomEventV1::ParticipantJoined { participant_id }
        }
        ThreadedRoomEventV1::ParticipantLeft { participant_id } => {
            TopicRoomEventV1::ParticipantLeft { participant_id }
        }
        ThreadedRoomEventV1::RoleChanged {
            participant_id,
            role,
        } => TopicRoomEventV1::RoleChanged {
            participant_id,
            role,
        },
        ThreadedRoomEventV1::RoomPolicyChanged {
            actor_id,
            previous,
            admission,
        } => TopicRoomEventV1::RoomPolicyChanged {
            actor_id,
            previous,
            admission,
        },
        ThreadedRoomEventV1::MessageIntervalChanged {
            actor_id,
            previous_interval_ms,
            minimum_interval_ms,
        } => TopicRoomEventV1::MessageIntervalChanged {
            actor_id,
            previous_interval_ms,
            minimum_interval_ms,
        },
        ThreadedRoomEventV1::ParticipantKicked {
            actor_id,
            participant_id,
            reason,
            causation,
        } => TopicRoomEventV1::ParticipantKicked {
            actor_id,
            participant_id,
            reason,
            causation,
        },
        ThreadedRoomEventV1::ParticipantBanned {
            actor_id,
            participant_id,
            reason,
            causation,
        } => TopicRoomEventV1::ParticipantBanned {
            actor_id,
            participant_id,
            reason,
            causation,
        },
        ThreadedRoomEventV1::ParticipantUnbanned {
            actor_id,
            participant_id,
            reason,
            causation,
        } => TopicRoomEventV1::ParticipantUnbanned {
            actor_id,
            participant_id,
            reason,
            causation,
        },
        ThreadedRoomEventV1::PostingRestrictionChanged {
            actor_id,
            participant_id,
            restricted,
            reason,
            causation,
        } => TopicRoomEventV1::PostingRestrictionChanged {
            actor_id,
            participant_id,
            restricted,
            reason,
            causation,
        },
        ThreadedRoomEventV1::MessageAccepted(message) => {
            TopicRoomEventV1::MessageAccepted(TopicMessageV1 {
                message_id: message.message_id,
                sequence: message.sequence,
                author: message.author,
                logical_time: message.logical_time,
                body: message.body,
                addressed_to: message.addressed_to,
                reply_to_root: message.reply_to_root,
                topic_id: None,
            })
        }
        ThreadedRoomEventV1::MessageWithdrawn {
            actor_id,
            message_id,
            sequence,
        } => TopicRoomEventV1::MessageWithdrawn {
            actor_id,
            message_id,
            sequence,
        },
        ThreadedRoomEventV1::MessageRedacted {
            actor_id,
            author_id,
            message_id,
            sequence,
            reason,
            causation,
        } => TopicRoomEventV1::MessageRedacted {
            actor_id,
            author_id,
            message_id,
            sequence,
            reason,
            causation,
        },
        ThreadedRoomEventV1::ThreadLifecycleChanged {
            actor_id,
            root_message_id,
            archived,
            reason,
            causation,
        } => TopicRoomEventV1::ThreadLifecycleChanged {
            actor_id,
            root_message_id,
            archived,
            reason,
            causation,
        },
        ThreadedRoomEventV1::ReactionChanged {
            actor_id,
            message_id,
            key,
            reacted,
        } => TopicRoomEventV1::ReactionChanged {
            actor_id,
            message_id,
            key,
            reacted,
        },
        ThreadedRoomEventV1::PinChanged {
            actor_id,
            message_id,
            original_sequence,
            pinned,
        } => TopicRoomEventV1::PinChanged {
            actor_id,
            message_id,
            original_sequence,
            pinned,
        },
        ThreadedRoomEventV1::PollCreated {
            actor_id,
            poll_id,
            prompt,
            options,
            eligible_participant_ids,
            max_choices,
        } => TopicRoomEventV1::PollCreated {
            actor_id,
            poll_id,
            prompt,
            options,
            eligible_participant_ids,
            max_choices,
        },
        ThreadedRoomEventV1::PollBallotChanged {
            poll_id,
            totals,
            total_ballots,
        } => TopicRoomEventV1::PollBallotChanged {
            poll_id,
            totals,
            total_ballots,
        },
        ThreadedRoomEventV1::PollLifecycleChanged {
            actor_id,
            poll_id,
            open,
        } => TopicRoomEventV1::PollLifecycleChanged {
            actor_id,
            poll_id,
            open,
        },
        ThreadedRoomEventV1::MessageEdited {
            actor_id,
            message_id,
            message_revision,
            logical_time,
            body,
        } => TopicRoomEventV1::MessageEdited {
            actor_id,
            message_id,
            message_revision,
            logical_time,
            body,
        },
    })
}

fn map_admission_to_engine(value: AdmissionPolicyV2) -> AdmissionPolicy {
    match value {
        AdmissionPolicyV2::Open => AdmissionPolicy::Open,
        AdmissionPolicyV2::InviteOnly => AdmissionPolicy::InviteOnly,
    }
}

fn map_admission_from_engine(value: AdmissionPolicy) -> AdmissionPolicyV2 {
    match value {
        AdmissionPolicy::Open => AdmissionPolicyV2::Open,
        AdmissionPolicy::InviteOnly => AdmissionPolicyV2::InviteOnly,
    }
}

fn map_conversation_profile_to_engine(value: ConversationProfileV1) -> EngineConversationProfileV1 {
    match value {
        ConversationProfileV1::Ordinary => EngineConversationProfileV1::Ordinary,
        ConversationProfileV1::Direct => EngineConversationProfileV1::Direct,
        ConversationProfileV1::GroupDirect => EngineConversationProfileV1::GroupDirect,
    }
}

fn map_conversation_profile_from_engine(
    value: EngineConversationProfileV1,
) -> ConversationProfileV1 {
    match value {
        EngineConversationProfileV1::Ordinary => ConversationProfileV1::Ordinary,
        EngineConversationProfileV1::Direct => ConversationProfileV1::Direct,
        EngineConversationProfileV1::GroupDirect => ConversationProfileV1::GroupDirect,
    }
}

fn map_peer_conversation_event(
    initiator: ParticipantId,
    profile: EngineConversationProfileV1,
    founding_members: Vec<ParticipantId>,
) -> PeerConversationCreatedV1 {
    PeerConversationCreatedV1 {
        initiator_id: initiator.as_str().to_owned(),
        profile: map_conversation_profile_from_engine(profile),
        founding_participant_ids: founding_members
            .into_iter()
            .map(|participant| participant.as_str().to_owned())
            .collect(),
    }
}

fn map_role_to_engine(value: RoleV2) -> Role {
    match value {
        RoleV2::Owner => Role::Owner,
        RoleV2::Moderator => Role::Moderator,
        RoleV2::Member => Role::Member,
    }
}

fn map_role_from_engine(value: Role) -> RoleV2 {
    match value {
        Role::Owner => RoleV2::Owner,
        Role::Moderator => RoleV2::Moderator,
        Role::Member => RoleV2::Member,
    }
}

fn map_observation_refusal(refusal: ObservationRefusal) -> RefusalV2 {
    match refusal {
        ObservationRefusal::UnknownParticipant => RefusalV2::UnknownParticipant,
        ObservationRefusal::UnknownRoom => RefusalV2::UnknownRoom,
        ObservationRefusal::NotMember => RefusalV2::NotMember,
        ObservationRefusal::NotAuthorized => RefusalV2::NotAuthorized,
        ObservationRefusal::InvalidLimit => RefusalV2::InvalidInput,
        ObservationRefusal::InvalidTarget => RefusalV2::InvalidInput,
        ObservationRefusal::ForeignCursor => RefusalV2::InvalidCursor,
        ObservationRefusal::Stale { actual_revision } => RefusalV2::Stale {
            actual_revision: actual_revision.0,
        },
        ObservationRefusal::Capacity => RefusalV2::Capacity,
        ObservationRefusal::InconsistentState => RefusalV2::Unavailable,
    }
}

fn map_poll_status(status: agora_engine::PollStatusV1) -> PollStatusV1 {
    PollStatusV1 {
        poll_id: PollIdV1::new(status.poll_id().into_bytes()),
        creator_id: status.creator().as_str().to_owned(),
        prompt: status.prompt().to_owned(),
        options: status.options().to_vec(),
        eligible_participant_ids: status
            .eligible_participants()
            .iter()
            .map(|participant| participant.as_str().to_owned())
            .collect(),
        max_choices: status.max_choices(),
        open: status.is_open(),
        totals: status.totals().to_vec(),
        total_ballots: status.total_ballots(),
    }
}

fn map_poll_observation_refusal(refusal: PollObservationRefusalV1) -> RefusalV2 {
    match refusal {
        PollObservationRefusalV1::UnknownParticipant => RefusalV2::UnknownParticipant,
        PollObservationRefusalV1::UnknownRoom => RefusalV2::UnknownRoom,
        PollObservationRefusalV1::NotMember => RefusalV2::NotMember,
        PollObservationRefusalV1::UnknownPoll => RefusalV2::UnknownPoll,
        PollObservationRefusalV1::NotEligible => RefusalV2::NotPollEligible,
    }
}

fn map_report_disposition_to_engine(
    disposition: MessageReportDispositionV1,
) -> EngineMessageReportDispositionV1 {
    match disposition {
        MessageReportDispositionV1::Pending => EngineMessageReportDispositionV1::Pending,
        MessageReportDispositionV1::Dismissed => EngineMessageReportDispositionV1::Dismissed,
        MessageReportDispositionV1::Escalated => EngineMessageReportDispositionV1::Escalated,
    }
}

fn map_message_report_from_engine(report: EngineMessageReportV1) -> MessageReportV1 {
    MessageReportV1 {
        report_id: MessageReportIdV1(report.report_id.get()),
        room_id: report.room_id.as_str().to_owned(),
        message_id: MessageIdV1::new(report.message_id.into_bytes()),
        original_sequence: report.original_sequence,
        target_author_id: report.target_author.as_str().to_owned(),
        target_availability: map_stable_reference_availability(report.target_availability),
        reporter_id: report.reporter.as_str().to_owned(),
        reason: report.reason.as_str().to_owned(),
        causation: report.causation.into_bytes(),
        creation_order: report.creation_order,
        disposition: match report.disposition {
            EngineMessageReportDispositionV1::Pending => MessageReportDispositionV1::Pending,
            EngineMessageReportDispositionV1::Dismissed => MessageReportDispositionV1::Dismissed,
            EngineMessageReportDispositionV1::Escalated => MessageReportDispositionV1::Escalated,
        },
        review_reason: report
            .review_reason
            .map(|reason| reason.as_str().to_owned()),
        review_causation: report
            .review_causation
            .map(ModerationCausationV1::into_bytes),
    }
}

fn map_message_report_refusal(refusal: MessageReportRefusalV1) -> RefusalV2 {
    match refusal {
        MessageReportRefusalV1::UnknownParticipant => RefusalV2::UnknownParticipant,
        MessageReportRefusalV1::UnknownRoom => RefusalV2::UnknownRoom,
        MessageReportRefusalV1::NotMember => RefusalV2::NotMember,
        MessageReportRefusalV1::NotAuthorized => RefusalV2::NotAuthorized,
        MessageReportRefusalV1::Stale { actual_revision } => RefusalV2::Stale {
            actual_revision: actual_revision.0,
        },
        MessageReportRefusalV1::UnknownMessage
        | MessageReportRefusalV1::InvalidReason
        | MessageReportRefusalV1::DuplicateReport => RefusalV2::InvalidMessageReport,
        MessageReportRefusalV1::UnknownReport => RefusalV2::UnknownMessageReport,
        MessageReportRefusalV1::InvalidChange => RefusalV2::InvalidMessageReportChange,
        MessageReportRefusalV1::Capacity => RefusalV2::MessageReportLimit,
        MessageReportRefusalV1::IdentityExhausted => RefusalV2::Exhausted,
    }
}

fn map_engine_refusal(refusal: EngineRefusal) -> RefusalV2 {
    match refusal {
        EngineRefusal::InvalidPinChange => RefusalV2::InvalidPinChange,
        EngineRefusal::RevisionConflict { actual, .. } => RefusalV2::Stale {
            actual_revision: actual.0,
        },
        EngineRefusal::UnknownParticipant(_) => RefusalV2::UnknownParticipant,
        EngineRefusal::UnknownRoom(_) => RefusalV2::UnknownRoom,
        EngineRefusal::NotMember(_) => RefusalV2::NotMember,
        EngineRefusal::NotAuthorized(_)
        | EngineRefusal::InvitationRequired(_)
        | EngineRefusal::OwnerCannotLeave => RefusalV2::NotAuthorized,
        EngineRefusal::ParticipantBanned(_) => RefusalV2::Banned,
        EngineRefusal::PostingRestricted => RefusalV2::PostingRestricted,
        EngineRefusal::MessageThrottled { retry_after_ms } => {
            RefusalV2::MessageThrottled { retry_after_ms }
        }
        EngineRefusal::AdmissionTimeRequired
        | EngineRefusal::AdmissionTimeForeign
        | EngineRefusal::AdmissionTimeStale => RefusalV2::Unavailable,
        EngineRefusal::ThreadArchived => RefusalV2::ThreadArchived,
        EngineRefusal::TopicResolved => RefusalV2::TopicResolved,
        EngineRefusal::InvalidTopicSupersession => RefusalV2::InvalidTopicSupersession,
        EngineRefusal::TopicMustBeResolved => RefusalV2::TopicMustBeResolved,
        EngineRefusal::TopicAliasLimit => RefusalV2::TopicAliasLimit,
        EngineRefusal::UnknownTopicAlias => RefusalV2::UnknownTopicAlias,
        EngineRefusal::InvalidTopicTag => RefusalV2::InvalidTopicTag,
        EngineRefusal::DuplicateTopicTag => RefusalV2::DuplicateTopicTag,
        EngineRefusal::TopicTagLimit => RefusalV2::TopicTagLimit,
        EngineRefusal::UnknownTopicTag => RefusalV2::UnknownTopicTag,
        EngineRefusal::ImmutableConversationCohort => RefusalV2::ImmutableConversationCohort,
        EngineRefusal::ParticipantLimit
        | EngineRefusal::RoomLimit
        | EngineRefusal::MemberLimit
        | EngineRefusal::InvitationLimit
        | EngineRefusal::BanLimit
        | EngineRefusal::ReactionLimit
        | EngineRefusal::PinLimit
        | EngineRefusal::MessageRevisionLimit
        | EngineRefusal::TopicLimit
        | EngineRefusal::MessageRetentionDisabled
        | EngineRefusal::AddressedParticipantLimit
        | EngineRefusal::EventOutboxFull => RefusalV2::Capacity,
        EngineRefusal::MessageIdentityExhausted
        | EngineRefusal::MessageSequenceExhausted
        | EngineRefusal::RevisionExhausted
        | EngineRefusal::EventSequenceExhausted => RefusalV2::Exhausted,
        EngineRefusal::TopicIdentityExhausted | EngineRefusal::PollIdentityExhausted => {
            RefusalV2::Exhausted
        }
        EngineRefusal::PollLimit => RefusalV2::PollLimit,
        EngineRefusal::UnknownPoll => RefusalV2::UnknownPoll,
        EngineRefusal::NotPollEligible => RefusalV2::NotPollEligible,
        EngineRefusal::PollClosed => RefusalV2::PollClosed,
        EngineRefusal::InvalidPollPrompt
        | EngineRefusal::InvalidPollOptions
        | EngineRefusal::InvalidPollEligibility
        | EngineRefusal::InvalidPollChoiceLimit
        | EngineRefusal::InvalidPollBallot => RefusalV2::InvalidPoll,
        EngineRefusal::InvalidPollBallotChange | EngineRefusal::InvalidPollLifecycleChange => {
            RefusalV2::InvalidPollChange
        }
        EngineRefusal::InvalidParticipantId
        | EngineRefusal::InvalidConversationProfile
        | EngineRefusal::InvalidConversationCohort
        | EngineRefusal::InvalidRoomId
        | EngineRefusal::InvalidDisplayName
        | EngineRefusal::InvalidRoomName
        | EngineRefusal::InvalidMessageBody
        | EngineRefusal::InvalidAttachmentCount
        | EngineRefusal::InvalidAttachmentDeclaredLength
        | EngineRefusal::InvalidAttachmentMediaType
        | EngineRefusal::InvalidAttachmentDisplayLabel
        | EngineRefusal::DuplicateAttachmentId
        | EngineRefusal::DuplicateAttachmentObject
        | EngineRefusal::InvalidVellumArtifactReferenceCount
        | EngineRefusal::DuplicateVellumArtifactReferenceId
        | EngineRefusal::DuplicateVellumArtifactTarget
        | EngineRefusal::NonCanonicalAddressing
        | EngineRefusal::InvalidModerationReason
        | EngineRefusal::InvalidRoleChange
        | EngineRefusal::InvalidPolicyChange
        | EngineRefusal::InvalidMessageInterval
        | EngineRefusal::InvalidPostingRestrictionChange
        | EngineRefusal::UnknownMessage
        | EngineRefusal::InvalidReplyTarget
        | EngineRefusal::InvalidThreadLifecycleChange
        | EngineRefusal::InvalidReaction
        | EngineRefusal::InvalidReactionChange
        | EngineRefusal::InvalidMessageEdit
        | EngineRefusal::InvalidTopicName
        | EngineRefusal::InvalidTopicLifecycleChange
        | EngineRefusal::DuplicateTopicName
        | EngineRefusal::MessageAlreadyTombstoned
        | EngineRefusal::DuplicateParticipant(_)
        | EngineRefusal::DuplicateRoom(_)
        | EngineRefusal::AlreadyMember(_)
        | EngineRefusal::NotBanned(_) => RefusalV2::InvalidInput,
        EngineRefusal::UnknownTopic => RefusalV2::UnknownTopic,
    }
}

fn map_scheduled_publication_refusal(
    refusal: PublicationRefusalV2,
) -> ScheduledPublicationRefusalV1 {
    match refusal {
        PublicationRefusalV2::IdempotencyConflict => ScheduledPublicationRefusalV1::Conflict,
        PublicationRefusalV2::PublicationReceiptLimit => ScheduledPublicationRefusalV1::Capacity,
        PublicationRefusalV2::AdmissionTime(AdmissionTimeRefusalV1::IdentityExhausted) => {
            ScheduledPublicationRefusalV1::Exhausted
        }
        PublicationRefusalV2::AdmissionTime(
            AdmissionTimeRefusalV1::Regressed
            | AdmissionTimeRefusalV1::ForeignObservation
            | AdmissionTimeRefusalV1::StaleObservation,
        ) => ScheduledPublicationRefusalV1::Unavailable,
        PublicationRefusalV2::Core(EngineRefusal::MessageThrottled { retry_after_ms }) => {
            ScheduledPublicationRefusalV1::AdmissionDeferred { retry_after_ms }
        }
        PublicationRefusalV2::Core(
            EngineRefusal::NotMember(_)
            | EngineRefusal::NotAuthorized(_)
            | EngineRefusal::ParticipantBanned(_)
            | EngineRefusal::PostingRestricted
            | EngineRefusal::InvitationRequired(_)
            | EngineRefusal::OwnerCannotLeave,
        ) => ScheduledPublicationRefusalV1::NotAuthorized,
        PublicationRefusalV2::Core(
            EngineRefusal::EventOutboxFull
            | EngineRefusal::MessageRetentionDisabled
            | EngineRefusal::ParticipantLimit
            | EngineRefusal::RoomLimit
            | EngineRefusal::MemberLimit
            | EngineRefusal::InvitationLimit
            | EngineRefusal::BanLimit
            | EngineRefusal::ReactionLimit
            | EngineRefusal::PinLimit
            | EngineRefusal::MessageRevisionLimit
            | EngineRefusal::TopicLimit
            | EngineRefusal::AddressedParticipantLimit,
        ) => ScheduledPublicationRefusalV1::Capacity,
        PublicationRefusalV2::Core(
            EngineRefusal::MessageIdentityExhausted
            | EngineRefusal::MessageSequenceExhausted
            | EngineRefusal::RevisionExhausted
            | EngineRefusal::EventSequenceExhausted
            | EngineRefusal::TopicIdentityExhausted
            | EngineRefusal::PollIdentityExhausted,
        ) => ScheduledPublicationRefusalV1::Exhausted,
        PublicationRefusalV2::Core(
            EngineRefusal::UnknownParticipant(_) | EngineRefusal::UnknownRoom(_),
        ) => ScheduledPublicationRefusalV1::NotAuthorized,
        PublicationRefusalV2::Core(
            EngineRefusal::AdmissionTimeRequired
            | EngineRefusal::AdmissionTimeForeign
            | EngineRefusal::AdmissionTimeStale
            | EngineRefusal::RevisionConflict { .. },
        ) => ScheduledPublicationRefusalV1::Unavailable,
        PublicationRefusalV2::Core(_) => ScheduledPublicationRefusalV1::InvalidInput,
    }
}

fn map_federated_publication_refusal(
    refusal: PublicationRefusalV2,
) -> FederatedPublicationRefusalV1 {
    match map_scheduled_publication_refusal(refusal) {
        ScheduledPublicationRefusalV1::InvalidInput => FederatedPublicationRefusalV1::InvalidInput,
        ScheduledPublicationRefusalV1::NotAuthorized => {
            FederatedPublicationRefusalV1::NotAuthorized
        }
        ScheduledPublicationRefusalV1::AdmissionDeferred { retry_after_ms } => {
            FederatedPublicationRefusalV1::AdmissionDeferred { retry_after_ms }
        }
        ScheduledPublicationRefusalV1::Capacity => FederatedPublicationRefusalV1::Capacity,
        ScheduledPublicationRefusalV1::Conflict => FederatedPublicationRefusalV1::Conflict,
        ScheduledPublicationRefusalV1::Unavailable => FederatedPublicationRefusalV1::Unavailable,
        ScheduledPublicationRefusalV1::Exhausted => FederatedPublicationRefusalV1::Exhausted,
    }
}

fn map_interaction_promotion_refusal(
    refusal: PublicationRefusalV2,
) -> InteractionPromotionRefusalV1 {
    match map_scheduled_publication_refusal(refusal) {
        ScheduledPublicationRefusalV1::InvalidInput => InteractionPromotionRefusalV1::InvalidInput,
        ScheduledPublicationRefusalV1::NotAuthorized => {
            InteractionPromotionRefusalV1::NotAuthorized
        }
        ScheduledPublicationRefusalV1::AdmissionDeferred { retry_after_ms } => {
            InteractionPromotionRefusalV1::AdmissionDeferred { retry_after_ms }
        }
        ScheduledPublicationRefusalV1::Capacity => InteractionPromotionRefusalV1::Capacity,
        ScheduledPublicationRefusalV1::Conflict => InteractionPromotionRefusalV1::Conflict,
        ScheduledPublicationRefusalV1::Unavailable => InteractionPromotionRefusalV1::Unavailable,
        ScheduledPublicationRefusalV1::Exhausted => InteractionPromotionRefusalV1::Exhausted,
    }
}

#[cfg(test)]
mod tests {
    use super::{ServiceLimitsV2, ServiceV2};
    use agora_engine::Engine;

    #[test]
    fn receipt_revision_saturation_fences_identity_without_wrapping() {
        let mut service = ServiceV2::new(Engine::new(), ServiceLimitsV2::default());
        service.receipt_revision = u64::MAX - 1;

        service.advance_receipt_revision();
        assert_eq!(service.receipt_revision(), Some(u64::MAX));
        assert!(!service.receipt_revision_exhausted());

        service.advance_receipt_revision();
        assert_eq!(service.receipt_revision(), None);
        assert!(service.receipt_revision_exhausted());
        assert_eq!(service.receipt_revision, u64::MAX);

        service.advance_receipt_revision();
        assert_eq!(service.receipt_revision, u64::MAX);
        assert!(service.receipt_revision_exhausted());
    }
}
