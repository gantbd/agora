//! A deterministic, bounded chat-room core for humans and software agents.
//!
//! Agora performs no I/O and does not authenticate callers. Hosts must bind an
//! authenticated principal to the [`ParticipantId`] used in commands.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
pub mod persistence;

pub const MAX_TOPIC_ALIASES_PER_TOPIC_V1: usize = 16;
pub const MAX_TOPIC_TAGS_PER_TOPIC_V1: usize = 8;
pub const MAX_ATTACHMENTS_PER_MESSAGE_V1: usize = 8;
pub const MAX_VELLUM_ARTIFACT_REFERENCES_PER_MESSAGE_V1: usize = 8;

/// Resource ceilings for one engine profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_participants: usize,
    pub max_rooms: usize,
    pub max_members_per_room: usize,
    pub max_invitations_per_room: usize,
    pub max_bans_per_room: usize,
    pub max_messages_per_room: usize,
    pub max_topics_per_room: usize,
    pub max_pending_events: usize,
    pub max_retained_changes: usize,
    pub max_publication_receipts: usize,
    pub max_addressed_participants_per_message: usize,
    pub max_participant_id_bytes: usize,
    pub max_room_id_bytes: usize,
    pub max_display_name_bytes: usize,
    pub max_room_name_bytes: usize,
    pub max_topic_name_bytes: usize,
    pub max_message_body_bytes: usize,
    pub max_moderation_reason_bytes: usize,
    pub max_attachment_declared_bytes: usize,
    pub max_attachment_media_type_bytes: usize,
    pub max_attachment_display_label_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_participants: 1_024,
            max_rooms: 256,
            max_members_per_room: 256,
            max_invitations_per_room: 256,
            max_bans_per_room: 256,
            max_messages_per_room: 1_024,
            max_topics_per_room: 256,
            max_pending_events: 4_096,
            max_retained_changes: 4_096,
            max_publication_receipts: 4_096,
            max_addressed_participants_per_message: 64,
            max_participant_id_bytes: 128,
            max_room_id_bytes: 128,
            max_display_name_bytes: 256,
            max_room_name_bytes: 256,
            max_topic_name_bytes: 256,
            max_message_body_bytes: 16 * 1_024,
            max_moderation_reason_bytes: 1_024,
            max_attachment_declared_bytes: 1024 * 1024 * 1024,
            max_attachment_media_type_bytes: 127,
            max_attachment_display_label_bytes: 256,
        }
    }
}

macro_rules! string_id {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self::new(value)
            }
        }
    };
}

string_id!(ParticipantId);
string_id!(RoomId);

/// Whether a participant is operated directly by a person or by software.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParticipantKind {
    Human,
    Agent,
}

/// A registered identity. Registration is not authentication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Participant {
    id: ParticipantId,
    display_name: String,
    kind: ParticipantKind,
}

impl Participant {
    pub fn new(
        id: impl Into<ParticipantId>,
        display_name: impl Into<String>,
        kind: ParticipantKind,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            kind,
        }
    }

    pub fn id(&self) -> &ParticipantId {
        &self.id
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn kind(&self) -> ParticipantKind {
        self.kind
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionPolicy {
    Open,
    InviteOnly,
}

/// Authoritative local cardinality and membership law for one room.
///
/// A shared room is intentionally absent: it is a federation composition over
/// an ordinary room, not local trust or membership evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConversationProfileV1 {
    Ordinary,
    Direct,
    GroupDirect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    Owner,
    Moderator,
    Member,
}

/// A caller-supplied timestamp in a host-defined logical epoch.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LogicalTime(pub u64);

/// Opaque room-local identity for one immutable attachment relation.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AttachmentIdV1([u8; 16]);

/// Opaque logical namespace for an external object store. This is not a URL,
/// path, credential, or authority decision.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AttachmentStoreIdV1([u8; 16]);

/// Opaque logical object key. Its bytes are correlation data, not a bearer
/// credential or proof that an object exists.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AttachmentObjectKeyV1([u8; 32]);

/// Caller-asserted SHA-256 value. Agora does not verify referenced bytes.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Sha256DigestV1([u8; 32]);

macro_rules! opaque_attachment_value {
    ($name:ident, $length:literal) => {
        impl $name {
            pub const fn new(bytes: [u8; $length]) -> Self {
                Self(bytes)
            }

            pub const fn into_bytes(self) -> [u8; $length] {
                self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(concat!(stringify!($name), "(<opaque>)"))
            }
        }
    };
}

opaque_attachment_value!(AttachmentIdV1, 16);
opaque_attachment_value!(AttachmentStoreIdV1, 16);
opaque_attachment_value!(AttachmentObjectKeyV1, 32);
opaque_attachment_value!(Sha256DigestV1, 32);

/// Opaque room-local identity for one Vellum artifact relation.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VellumArtifactReferenceIdV1([u8; 16]);

opaque_attachment_value!(VellumArtifactReferenceIdV1, 16);

/// Exact restart-stable Vellum document revision named by an Agora relation.
/// These scalar values are correlation data, never Vellum authority.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VellumDocumentTargetV1 {
    workspace_id: u64,
    document_id: u64,
    source_revision: u64,
}

impl VellumDocumentTargetV1 {
    pub const fn new(workspace_id: u64, document_id: u64, source_revision: u64) -> Option<Self> {
        if workspace_id == 0 || document_id == 0 {
            None
        } else {
            Some(Self {
                workspace_id,
                document_id,
                source_revision,
            })
        }
    }

    pub const fn workspace_id(self) -> u64 {
        self.workspace_id
    }

    pub const fn document_id(self) -> u64 {
        self.document_id
    }

    pub const fn source_revision(self) -> u64 {
        self.source_revision
    }
}

impl fmt::Debug for VellumDocumentTargetV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("VellumDocumentTargetV1(<opaque>)")
    }
}

/// Immutable typed reference supplied with one accepted Agora message.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VellumArtifactReferenceV1 {
    reference_id: VellumArtifactReferenceIdV1,
    target: VellumDocumentTargetV1,
}

impl VellumArtifactReferenceV1 {
    pub const fn new(
        reference_id: VellumArtifactReferenceIdV1,
        target: VellumDocumentTargetV1,
    ) -> Self {
        Self {
            reference_id,
            target,
        }
    }

    pub const fn reference_id(self) -> VellumArtifactReferenceIdV1 {
        self.reference_id
    }

    pub const fn target(self) -> VellumDocumentTargetV1 {
        self.target
    }
}

impl fmt::Debug for VellumArtifactReferenceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VellumArtifactReferenceV1")
            .field("reference_id", &self.reference_id)
            .field("target", &self.target)
            .finish()
    }
}

/// Closed placement vocabulary for a Vellum-artifact-bearing message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VellumArtifactMessageTargetV1 {
    Room,
    Reply(MessageId),
    Topic(TopicId),
}

/// Retained relation. A tombstone preserves only its room-local identity.
#[derive(Clone, Eq, PartialEq)]
pub struct MessageVellumArtifactReferenceV1 {
    reference_id: VellumArtifactReferenceIdV1,
    target: Option<VellumDocumentTargetV1>,
}

impl MessageVellumArtifactReferenceV1 {
    fn available(reference: VellumArtifactReferenceV1) -> Self {
        Self {
            reference_id: reference.reference_id(),
            target: Some(reference.target()),
        }
    }

    pub const fn reference_id(&self) -> VellumArtifactReferenceIdV1 {
        self.reference_id
    }

    pub const fn target(&self) -> Option<VellumDocumentTargetV1> {
        self.target
    }

    pub const fn is_tombstoned(&self) -> bool {
        self.target.is_none()
    }
}

impl fmt::Debug for MessageVellumArtifactReferenceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MessageVellumArtifactReferenceV1")
            .field("reference_id", &self.reference_id)
            .field("is_tombstoned", &self.is_tombstoned())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttachmentIntegrityV1 {
    Sha256(Sha256DigestV1),
}

/// Closed placement vocabulary for an attachment-bearing message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttachmentMessageTargetV1 {
    Room,
    Reply(MessageId),
    Topic(TopicId),
}

/// Bounds for validating one attachment descriptor before message mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttachmentDescriptorLimitsV1 {
    pub max_declared_bytes: u64,
    pub max_media_type_bytes: usize,
    pub max_display_label_bytes: usize,
}

impl Default for AttachmentDescriptorLimitsV1 {
    fn default() -> Self {
        Self {
            max_declared_bytes: 1024 * 1024 * 1024,
            max_media_type_bytes: 127,
            max_display_label_bytes: 256,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttachmentDescriptorRefusalV1 {
    InvalidDeclaredLength,
    InvalidMediaType,
    InvalidDisplayLabel,
}

/// Immutable logical reference metadata. Construction records caller input;
/// [`AttachmentDescriptorV1::validate`] must succeed before admission.
#[derive(Clone, Eq, PartialEq)]
pub struct AttachmentDescriptorV1 {
    attachment_id: AttachmentIdV1,
    store_id: AttachmentStoreIdV1,
    object_key: AttachmentObjectKeyV1,
    declared_bytes: u64,
    media_type: String,
    display_label: String,
    integrity: AttachmentIntegrityV1,
}

impl AttachmentDescriptorV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        attachment_id: AttachmentIdV1,
        store_id: AttachmentStoreIdV1,
        object_key: AttachmentObjectKeyV1,
        declared_bytes: u64,
        media_type: impl Into<String>,
        display_label: impl Into<String>,
        integrity: AttachmentIntegrityV1,
    ) -> Self {
        Self {
            attachment_id,
            store_id,
            object_key,
            declared_bytes,
            media_type: media_type.into(),
            display_label: display_label.into(),
            integrity,
        }
    }

    pub fn attachment_id(&self) -> AttachmentIdV1 {
        self.attachment_id
    }

    pub fn store_id(&self) -> AttachmentStoreIdV1 {
        self.store_id
    }

    pub fn object_key(&self) -> AttachmentObjectKeyV1 {
        self.object_key
    }

    pub fn declared_bytes(&self) -> u64 {
        self.declared_bytes
    }

    pub fn media_type(&self) -> &str {
        &self.media_type
    }

    pub fn display_label(&self) -> &str {
        &self.display_label
    }

    pub fn integrity(&self) -> AttachmentIntegrityV1 {
        self.integrity
    }

    pub fn validate(
        &self,
        limits: AttachmentDescriptorLimitsV1,
    ) -> Result<(), AttachmentDescriptorRefusalV1> {
        if self.declared_bytes == 0 || self.declared_bytes > limits.max_declared_bytes {
            return Err(AttachmentDescriptorRefusalV1::InvalidDeclaredLength);
        }
        if self.media_type.len() > limits.max_media_type_bytes
            || !canonical_media_type_v1(&self.media_type)
        {
            return Err(AttachmentDescriptorRefusalV1::InvalidMediaType);
        }
        if self.display_label.is_empty()
            || self.display_label.len() > limits.max_display_label_bytes
            || self.display_label.chars().any(char::is_control)
        {
            return Err(AttachmentDescriptorRefusalV1::InvalidDisplayLabel);
        }
        Ok(())
    }
}

impl fmt::Debug for AttachmentDescriptorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AttachmentDescriptorV1")
            .field("attachment_id", &self.attachment_id)
            .field("store_id", &self.store_id)
            .field("object_key", &self.object_key)
            .field("declared_bytes", &self.declared_bytes)
            .field("media_type_bytes", &self.media_type.len())
            .field("display_label_bytes", &self.display_label.len())
            .field("integrity", &self.integrity)
            .finish()
    }
}

/// Retained relation between a message and one attachment identity. A
/// tombstone retains only the identity and original ordering slot.
#[derive(Clone, Eq, PartialEq)]
pub struct MessageAttachmentV1 {
    attachment_id: AttachmentIdV1,
    descriptor: Option<AttachmentDescriptorV1>,
}

impl MessageAttachmentV1 {
    fn available(descriptor: AttachmentDescriptorV1) -> Self {
        Self {
            attachment_id: descriptor.attachment_id(),
            descriptor: Some(descriptor),
        }
    }

    pub fn attachment_id(&self) -> AttachmentIdV1 {
        self.attachment_id
    }

    pub fn descriptor(&self) -> Option<&AttachmentDescriptorV1> {
        self.descriptor.as_ref()
    }

    pub fn is_tombstoned(&self) -> bool {
        self.descriptor.is_none()
    }
}

impl fmt::Debug for MessageAttachmentV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MessageAttachmentV1")
            .field("attachment_id", &self.attachment_id)
            .field("is_tombstoned", &self.is_tombstoned())
            .finish()
    }
}

fn canonical_media_type_v1(value: &str) -> bool {
    let Some((top_level, subtype)) = value.split_once('/') else {
        return false;
    };
    !top_level.is_empty()
        && !subtype.is_empty()
        && !subtype.contains('/')
        && top_level.bytes().all(canonical_media_type_token_byte_v1)
        && subtype.bytes().all(canonical_media_type_token_byte_v1)
}

fn canonical_media_type_token_byte_v1(value: u8) -> bool {
    value.is_ascii_lowercase()
        || value.is_ascii_digit()
        || matches!(
            value,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
}

/// Stable room-local message reference. The byte representation is opaque and
/// carries no ordering contract; use message sequence or event position to order.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MessageId([u8; 16]);

/// Stable room-local topic reference. Its bytes are opaque and grant no authority.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TopicId([u8; 16]);

/// Stable room-local poll reference. Its bytes are opaque and grant no authority.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PollId([u8; 16]);

pub const MAX_REACTION_KEY_BYTES_V1: usize = 64;
pub const MAX_REACTION_KEYS_PER_MESSAGE_V1: usize = 64;
pub const MAX_MESSAGE_REVISIONS_V1: usize = 64;
pub const MAX_READ_POSITIONS_V1: usize = 4_096;
pub const MAX_ATTENTION_PREFERENCES_V1: usize = 4_096;
pub const MAX_PRIVATE_SAVES_V1: usize = 4_096;
pub const MAX_ROOM_PINS_V1: usize = 256;
pub const MAX_POLLS_PER_ROOM_V1: usize = 256;
pub const MAX_POLL_OPTIONS_V1: usize = 32;
pub const MAX_POLL_ELIGIBLE_PARTICIPANTS_V1: usize = 256;
pub const MAX_POLL_PROMPT_BYTES_V1: usize = 1_024;
pub const MAX_POLL_OPTION_BYTES_V1: usize = 256;

/// Public poll definition and aggregate. Individual ballots are excluded.
#[derive(Clone, Eq, PartialEq)]
pub struct PollStatusV1 {
    poll_id: PollId,
    creator: ParticipantId,
    prompt: String,
    options: Vec<String>,
    eligible_participants: Vec<ParticipantId>,
    max_choices: u8,
    open: bool,
    totals: Vec<u16>,
    total_ballots: u16,
}

impl PollStatusV1 {
    pub const fn poll_id(&self) -> PollId {
        self.poll_id
    }
    pub fn creator(&self) -> &ParticipantId {
        &self.creator
    }
    pub fn prompt(&self) -> &str {
        &self.prompt
    }
    pub fn options(&self) -> &[String] {
        &self.options
    }
    pub fn eligible_participants(&self) -> &[ParticipantId] {
        &self.eligible_participants
    }
    pub const fn max_choices(&self) -> u8 {
        self.max_choices
    }
    pub const fn is_open(&self) -> bool {
        self.open
    }
    pub fn totals(&self) -> &[u16] {
        &self.totals
    }
    pub const fn total_ballots(&self) -> u16 {
        self.total_ballots
    }
}

impl fmt::Debug for PollStatusV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PollStatusV1")
            .field("poll_id", &self.poll_id)
            .field("creator_id_bytes", &self.creator.as_str().len())
            .field("prompt_bytes", &self.prompt.len())
            .field("option_count", &self.options.len())
            .field("eligible_count", &self.eligible_participants.len())
            .field("max_choices", &self.max_choices)
            .field("open", &self.open)
            .field("totals", &self.totals)
            .field("total_ballots", &self.total_ballots)
            .finish()
    }
}

/// One caller-owned private ballot. Debug output never exposes selections.
#[derive(Clone, Eq, PartialEq)]
pub struct PollBallotV1 {
    poll_id: PollId,
    selections: Vec<u8>,
}

impl PollBallotV1 {
    pub const fn poll_id(&self) -> PollId {
        self.poll_id
    }
    pub fn selections(&self) -> &[u8] {
        &self.selections
    }
}

impl fmt::Debug for PollBallotV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PollBallotV1")
            .field("poll_id", &self.poll_id)
            .field("selection_count", &self.selections.len())
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PollObservationRefusalV1 {
    UnknownParticipant,
    UnknownRoom,
    NotMember,
    UnknownPoll,
    NotEligible,
}

/// Closed selector for one participant's private read position.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReadScopeV1 {
    Room,
    Thread(MessageId),
    Topic(TopicId),
}

/// One private, monotone marker. This is not a delivery or attention receipt.
#[derive(Clone, Eq, PartialEq)]
pub struct ReadPositionV1 {
    room_id: RoomId,
    scope: ReadScopeV1,
    target_message_id: MessageId,
    target_sequence: u64,
    observed_revision: Revision,
}

impl ReadPositionV1 {
    pub fn room_id(&self) -> &RoomId {
        &self.room_id
    }
    pub fn scope(&self) -> &ReadScopeV1 {
        &self.scope
    }
    pub fn target_message_id(&self) -> MessageId {
        self.target_message_id
    }
    pub fn target_sequence(&self) -> u64 {
        self.target_sequence
    }
    pub fn observed_revision(&self) -> Revision {
        self.observed_revision
    }
}

impl fmt::Debug for ReadPositionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReadPositionV1")
            .field("room_id_bytes", &self.room_id.as_str().len())
            .field("scope", &self.scope)
            .field("target_message_id", &self.target_message_id)
            .field("target_sequence", &self.target_sequence)
            .field("observed_revision", &self.observed_revision)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadPositionRefusalV1 {
    UnknownParticipant,
    UnknownRoom,
    NotMember,
    Stale { actual_revision: Revision },
    InvalidScope,
    InvalidTarget,
    Regression,
    Capacity,
}

/// Closed target for one participant's private attention intent.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AttentionScopeV1 {
    Room,
    Thread(MessageId),
    Topic(TopicId),
}

/// A delivery-neutral notification preference.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NotificationIntentV1 {
    #[default]
    Default,
    AllActivity,
    AddressedOnly,
    None,
}

/// One participant-owned preference product. All-default means no stored row.
#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub struct AttentionPreferenceV1 {
    following: bool,
    muted: bool,
    notification_intent: NotificationIntentV1,
}

impl fmt::Debug for AttentionPreferenceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AttentionPreferenceV1(<redacted>)")
    }
}

impl AttentionPreferenceV1 {
    pub const fn new(
        following: bool,
        muted: bool,
        notification_intent: NotificationIntentV1,
    ) -> Self {
        Self {
            following,
            muted,
            notification_intent,
        }
    }

    pub const fn following(self) -> bool {
        self.following
    }

    pub const fn muted(self) -> bool {
        self.muted
    }

    pub const fn notification_intent(self) -> NotificationIntentV1 {
        self.notification_intent
    }

    pub fn is_default(self) -> bool {
        self == Self::default()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttentionPreferenceRefusalV1 {
    UnknownParticipant,
    UnknownRoom,
    NotMember,
    Stale { actual_revision: Revision },
    InvalidScope,
    Capacity,
}

/// Current availability of a stable message relation. This carries no payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StableReferenceAvailabilityV1 {
    Available,
    Tombstoned,
    Evicted,
}

/// Participant-owned stable message relation with no copied message metadata.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct SavedReferenceV1 {
    message_id: MessageId,
    original_sequence: u64,
    availability: StableReferenceAvailabilityV1,
}

impl SavedReferenceV1 {
    pub const fn message_id(self) -> MessageId {
        self.message_id
    }

    pub const fn original_sequence(self) -> u64 {
        self.original_sequence
    }

    pub const fn availability(self) -> StableReferenceAvailabilityV1 {
        self.availability
    }
}

impl fmt::Debug for SavedReferenceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SavedReferenceV1")
            .field("message_id", &self.message_id)
            .field("original_sequence", &self.original_sequence)
            .field("availability", &self.availability)
            .finish()
    }
}

/// Room-visible stable message relation. Payload remains in message retention.
#[derive(Clone, Eq, PartialEq)]
pub struct PinnedReferenceV1 {
    message_id: MessageId,
    original_sequence: u64,
    pinned_by: ParticipantId,
    availability: StableReferenceAvailabilityV1,
}

impl PinnedReferenceV1 {
    pub fn message_id(&self) -> MessageId {
        self.message_id
    }

    pub fn original_sequence(&self) -> u64 {
        self.original_sequence
    }

    pub fn pinned_by(&self) -> &ParticipantId {
        &self.pinned_by
    }

    pub fn availability(&self) -> StableReferenceAvailabilityV1 {
        self.availability
    }
}

impl fmt::Debug for PinnedReferenceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PinnedReferenceV1")
            .field("message_id", &self.message_id)
            .field("original_sequence", &self.original_sequence)
            .field("pinned_by_id_bytes", &self.pinned_by.as_str().len())
            .field("availability", &self.availability)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrivateSaveRefusalV1 {
    UnknownParticipant,
    UnknownRoom,
    NotMember,
    Stale { actual_revision: Revision },
    UnknownMessage,
    InvalidChange,
    Capacity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageRevisionAvailabilityV1 {
    Available,
    Tombstoned,
}

#[derive(Clone, Eq, PartialEq)]
pub struct MessageRevisionV1 {
    pub number: u32,
    pub editor: ParticipantId,
    pub at: LogicalTime,
    pub availability: MessageRevisionAvailabilityV1,
    pub body: String,
}

impl fmt::Debug for MessageRevisionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MessageRevisionV1")
            .field("number", &self.number)
            .field("editor_id_bytes", &self.editor.as_str().len())
            .field("at", &self.at)
            .field("availability", &self.availability)
            .field("body_bytes", &self.body.len())
            .finish()
    }
}

impl MessageId {
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn into_bytes(self) -> [u8; 16] {
        self.0
    }

    fn allocated(room_incarnation: u64, room_counter: u64) -> Self {
        let mut bytes = [0; 16];
        bytes[..8].copy_from_slice(&room_incarnation.to_be_bytes());
        bytes[8..].copy_from_slice(&room_counter.to_be_bytes());
        Self(bytes)
    }
}

impl TopicId {
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn into_bytes(self) -> [u8; 16] {
        self.0
    }

    fn allocated(room_incarnation: u64, room_counter: u64) -> Self {
        let mut bytes = [0; 16];
        bytes[..8].copy_from_slice(&room_incarnation.to_be_bytes());
        bytes[8..].copy_from_slice(&room_counter.to_be_bytes());
        Self(bytes)
    }
}

impl PollId {
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn into_bytes(self) -> [u8; 16] {
        self.0
    }

    fn allocated(room_incarnation: u64, room_counter: u64) -> Self {
        let mut bytes = [0; 16];
        bytes[..8].copy_from_slice(&room_incarnation.to_be_bytes());
        bytes[8..].copy_from_slice(&room_counter.to_be_bytes());
        Self(bytes)
    }
}

impl fmt::Debug for PollId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PollId(<opaque>)")
    }
}

impl fmt::Debug for TopicId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TopicId(<opaque>)")
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct TopicV1 {
    id: TopicId,
    name: String,
    creator: ParticipantId,
    resolved: bool,
    aliases: BTreeSet<String>,
    tags: BTreeSet<String>,
}

impl TopicV1 {
    pub fn id(&self) -> TopicId {
        self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn creator(&self) -> &ParticipantId {
        &self.creator
    }
    pub fn is_resolved(&self) -> bool {
        self.resolved
    }

    pub fn aliases(&self) -> &BTreeSet<String> {
        &self.aliases
    }
    pub fn tags(&self) -> &BTreeSet<String> {
        &self.tags
    }
}

impl fmt::Debug for TopicV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TopicV1")
            .field("id", &self.id)
            .field("name_bytes", &self.name.len())
            .field("creator_id_bytes", &self.creator.as_str().len())
            .field("resolved", &self.resolved)
            .finish()
    }
}

impl fmt::Debug for MessageId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("MessageId(<opaque>)")
    }
}

/// A retained room message. Debug output deliberately redacts the body.
#[derive(Clone, Eq, PartialEq)]
pub struct Message {
    id: MessageId,
    sequence: u64,
    author: ParticipantId,
    at: LogicalTime,
    body: String,
    addressed_to: Vec<ParticipantId>,
    reply_to_root: Option<MessageId>,
    topic_id: Option<TopicId>,
    attachments: Vec<MessageAttachmentV1>,
    vellum_artifacts: Vec<MessageVellumArtifactReferenceV1>,
    revisions: Vec<MessageRevisionV1>,
}

impl Message {
    pub fn id(&self) -> MessageId {
        self.id
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn author(&self) -> &ParticipantId {
        &self.author
    }

    pub fn at(&self) -> LogicalTime {
        self.at
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    /// True when the author has withdrawn the body. Identity, order, author,
    /// logical time, and addressing remain as the reference-preserving tombstone.
    pub fn is_withdrawn(&self) -> bool {
        self.body.is_empty()
    }

    /// Explicit room members addressed by this message in canonical ID order.
    /// Addressing is conversation data, not permission or proof of execution.
    pub fn addressed_to(&self) -> &[ParticipantId] {
        &self.addressed_to
    }

    /// Stable root reference for a one-level reply. Absence identifies a root.
    pub fn reply_to_root(&self) -> Option<MessageId> {
        self.reply_to_root
    }

    pub fn topic_id(&self) -> Option<TopicId> {
        self.topic_id
    }

    pub fn attachments(&self) -> &[MessageAttachmentV1] {
        &self.attachments
    }

    pub fn vellum_artifacts(&self) -> &[MessageVellumArtifactReferenceV1] {
        &self.vellum_artifacts
    }

    pub fn current_revision(&self) -> u32 {
        self.revisions
            .last()
            .expect("admitted messages have one revision")
            .number
    }

    pub fn revisions(&self) -> &[MessageRevisionV1] {
        &self.revisions
    }
}

impl fmt::Debug for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Message")
            .field("id", &self.id)
            .field("sequence", &self.sequence)
            .field("author", &self.author)
            .field("at", &self.at)
            .field("body_bytes", &self.body.len())
            .field("addressed_count", &self.addressed_to.len())
            .field("is_reply", &self.reply_to_root.is_some())
            .field("has_topic", &self.topic_id.is_some())
            .field("attachment_count", &self.attachments.len())
            .field("vellum_artifact_count", &self.vellum_artifacts.len())
            .field("revision_count", &self.revisions.len())
            .finish()
    }
}

/// Immutable public room observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoomView {
    pub id: RoomId,
    pub name: String,
    pub profile: ConversationProfileV1,
    pub admission: AdmissionPolicy,
    pub founding_members: Vec<ParticipantId>,
    pub members: Vec<(ParticipantId, Role)>,
    pub pending_invitations: Vec<ParticipantId>,
    pub banned_participants: Vec<ParticipantId>,
    pub messages: Vec<Message>,
    pub next_message_sequence: u64,
}

/// Bounded room-directory entry visible to one current room member.
#[derive(Clone, Eq, PartialEq)]
pub struct RoomSummary {
    pub id: RoomId,
    pub name: String,
    pub profile: ConversationProfileV1,
    pub admission: AdmissionPolicy,
    pub observer_role: Role,
    pub member_count: usize,
    pub retained_message_count: usize,
    pub next_message_sequence: u64,
}

/// Current-member-only observation of one room's immutable conversation law.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationProfileStatusV1 {
    pub revision: Revision,
    pub room_id: RoomId,
    pub profile: ConversationProfileV1,
    pub founding_members: Vec<ParticipantId>,
}

impl fmt::Debug for RoomSummary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RoomSummary")
            .field("room_id_bytes", &self.id.as_str().len())
            .field("room_name_bytes", &self.name.len())
            .field("profile", &self.profile)
            .field("admission", &self.admission)
            .field("observer_role", &self.observer_role)
            .field("member_count", &self.member_count)
            .field("retained_message_count", &self.retained_message_count)
            .field("next_message_sequence", &self.next_message_sequence)
            .finish()
    }
}

/// Opaque continuation for one participant's room-directory observation.
#[derive(Clone, Eq, PartialEq)]
pub struct RoomPageCursor {
    revision: Revision,
    observer: ParticipantId,
    after: RoomId,
}

impl fmt::Debug for RoomPageCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RoomPageCursor")
            .field("revision", &self.revision)
            .field("observer_id_bytes", &self.observer.as_str().len())
            .field("after_room_id_bytes", &self.after.as_str().len())
            .finish()
    }
}

impl RoomPageCursor {
    /// Checks a recovered private cursor binding without exposing its contents.
    pub fn is_bound_to(&self, observer: &ParticipantId) -> bool {
        &self.observer == observer
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct RoomPage {
    pub revision: Revision,
    pub rooms: Vec<RoomSummary>,
    pub next: Option<RoomPageCursor>,
}

impl fmt::Debug for RoomPage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RoomPage")
            .field("revision", &self.revision)
            .field("room_count", &self.rooms.len())
            .field("has_next", &self.next.is_some())
            .finish()
    }
}

/// One bounded member projection. Registration is not authentication.
#[derive(Clone, Eq, PartialEq)]
pub struct MemberView {
    pub participant_id: ParticipantId,
    pub display_name: String,
    pub kind: ParticipantKind,
    pub role: Role,
}

impl fmt::Debug for MemberView {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MemberView")
            .field("participant_id_bytes", &self.participant_id.as_str().len())
            .field("display_name_bytes", &self.display_name.len())
            .field("kind", &self.kind)
            .field("role", &self.role)
            .finish()
    }
}

/// Opaque continuation for one participant's observation of one member set.
#[derive(Clone, Eq, PartialEq)]
pub struct MemberPageCursor {
    revision: Revision,
    observer: ParticipantId,
    room_id: RoomId,
    after: ParticipantId,
}

impl fmt::Debug for MemberPageCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MemberPageCursor")
            .field("revision", &self.revision)
            .field("observer_id_bytes", &self.observer.as_str().len())
            .field("room_id_bytes", &self.room_id.as_str().len())
            .field("after_participant_id_bytes", &self.after.as_str().len())
            .finish()
    }
}

impl MemberPageCursor {
    /// Checks a recovered private cursor binding without exposing its contents.
    pub fn is_bound_to(&self, observer: &ParticipantId, room_id: &RoomId) -> bool {
        &self.observer == observer && &self.room_id == room_id
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct MemberPage {
    pub revision: Revision,
    pub room_id: RoomId,
    pub members: Vec<MemberView>,
    pub next: Option<MemberPageCursor>,
}

/// One authority-scoped banned-participant projection.
#[derive(Clone, Eq, PartialEq)]
pub struct BannedParticipantView {
    pub participant_id: ParticipantId,
    pub display_name: String,
    pub kind: ParticipantKind,
    pub prior_role: Role,
}

impl fmt::Debug for BannedParticipantView {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BannedParticipantView")
            .field("participant_id_bytes", &self.participant_id.as_str().len())
            .field("display_name_bytes", &self.display_name.len())
            .field("kind", &self.kind)
            .field("prior_role", &self.prior_role)
            .finish()
    }
}

/// Opaque continuation for one moderator's observation of one room ban set.
#[derive(Clone, Eq, PartialEq)]
pub struct BanPageCursor {
    revision: Revision,
    observer: ParticipantId,
    room_id: RoomId,
    after: ParticipantId,
}

impl BanPageCursor {
    pub fn is_bound_to(&self, observer: &ParticipantId, room_id: &RoomId) -> bool {
        &self.observer == observer && &self.room_id == room_id
    }
}

impl fmt::Debug for BanPageCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BanPageCursor")
            .field("revision", &self.revision)
            .field("observer_id_bytes", &self.observer.as_str().len())
            .field("room_id_bytes", &self.room_id.as_str().len())
            .field("after_participant_id_bytes", &self.after.as_str().len())
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct BanPage {
    pub revision: Revision,
    pub room_id: RoomId,
    pub banned: Vec<BannedParticipantView>,
    pub next: Option<BanPageCursor>,
}

impl fmt::Debug for BanPage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BanPage")
            .field("revision", &self.revision)
            .field("room_id_bytes", &self.room_id.as_str().len())
            .field("banned_count", &self.banned.len())
            .field("has_next", &self.next.is_some())
            .finish()
    }
}

impl fmt::Debug for MemberPage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MemberPage")
            .field("revision", &self.revision)
            .field("room_id_bytes", &self.room_id.as_str().len())
            .field("member_count", &self.members.len())
            .field("has_next", &self.next.is_some())
            .finish()
    }
}

/// Opaque continuation for one participant's retained-message observation.
#[derive(Clone, Eq, PartialEq)]
pub struct MessagePageCursor {
    revision: Revision,
    observer: ParticipantId,
    room_id: RoomId,
    after_sequence: u64,
}

impl fmt::Debug for MessagePageCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MessagePageCursor")
            .field("revision", &self.revision)
            .field("observer_id_bytes", &self.observer.as_str().len())
            .field("room_id_bytes", &self.room_id.as_str().len())
            .field("after_sequence", &self.after_sequence)
            .finish()
    }
}

impl MessagePageCursor {
    /// Checks a recovered private cursor binding without exposing its contents.
    pub fn is_bound_to(&self, observer: &ParticipantId, room_id: &RoomId) -> bool {
        &self.observer == observer && &self.room_id == room_id
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct MessagePage {
    pub revision: Revision,
    pub room_id: RoomId,
    pub messages: Vec<Message>,
    pub oldest_retained_sequence: u64,
    pub next_message_sequence: u64,
    pub next: Option<MessagePageCursor>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct MessageRevisionPageCursorV1 {
    revision: Revision,
    observer: ParticipantId,
    room_id: RoomId,
    message_id: MessageId,
    after_revision: u32,
}

impl MessageRevisionPageCursorV1 {
    pub fn is_bound_to(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
        message_id: MessageId,
    ) -> bool {
        &self.observer == observer && &self.room_id == room_id && self.message_id == message_id
    }
}

impl fmt::Debug for MessageRevisionPageCursorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MessageRevisionPageCursorV1")
            .field("revision", &self.revision)
            .field("observer_id_bytes", &self.observer.as_str().len())
            .field("room_id_bytes", &self.room_id.as_str().len())
            .field("message_id", &self.message_id)
            .field("after_revision", &self.after_revision)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct MessageRevisionPageV1 {
    pub revision: Revision,
    pub room_id: RoomId,
    pub message_id: MessageId,
    pub revisions: Vec<MessageRevisionV1>,
    pub next: Option<MessageRevisionPageCursorV1>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct TopicPageCursorV1 {
    revision: Revision,
    observer: ParticipantId,
    room_id: RoomId,
    after: TopicId,
}

impl TopicPageCursorV1 {
    pub fn is_bound_to(&self, observer: &ParticipantId, room_id: &RoomId) -> bool {
        &self.observer == observer && &self.room_id == room_id
    }
}

impl fmt::Debug for TopicPageCursorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TopicPageCursorV1")
            .field("revision", &self.revision)
            .field("observer_id_bytes", &self.observer.as_str().len())
            .field("room_id_bytes", &self.room_id.as_str().len())
            .field("after", &self.after)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct TopicPageV1 {
    pub revision: Revision,
    pub room_id: RoomId,
    pub topics: Vec<TopicV1>,
    pub next: Option<TopicPageCursorV1>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TopicCollectionLifecycleV1 {
    Any,
    Active,
    Resolved,
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct TopicCollectionFilterV1 {
    pub lifecycle: TopicCollectionLifecycleV1,
    pub tags: Vec<String>,
}

impl fmt::Debug for TopicCollectionFilterV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TopicCollectionFilterV1")
            .field("lifecycle", &self.lifecycle)
            .field("tag_count", &self.tags.len())
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct TopicCollectionEntryV1 {
    pub topic_id: TopicId,
    pub name: String,
    pub creator: ParticipantId,
    pub resolved: bool,
    pub aliases: Vec<String>,
    pub tags: Vec<String>,
    pub successor_topic_id: Option<TopicId>,
}

impl fmt::Debug for TopicCollectionEntryV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TopicCollectionEntryV1")
            .field("topic_id", &self.topic_id)
            .field("name_bytes", &self.name.len())
            .field("creator_id_bytes", &self.creator.as_str().len())
            .field("resolved", &self.resolved)
            .field("alias_count", &self.aliases.len())
            .field("tag_count", &self.tags.len())
            .field("has_successor", &self.successor_topic_id.is_some())
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct TopicCollectionCursorV1 {
    revision: Revision,
    observer: ParticipantId,
    room_id: RoomId,
    filter: TopicCollectionFilterV1,
    after: TopicId,
}

impl TopicCollectionCursorV1 {
    pub fn is_bound_to_observer_room(&self, observer: &ParticipantId, room_id: &RoomId) -> bool {
        &self.observer == observer && &self.room_id == room_id
    }

    pub fn is_bound_to(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
        filter: &TopicCollectionFilterV1,
    ) -> bool {
        &self.observer == observer && &self.room_id == room_id && &self.filter == filter
    }
}

impl fmt::Debug for TopicCollectionCursorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TopicCollectionCursorV1")
            .field("revision", &self.revision)
            .field("observer_id_bytes", &self.observer.as_str().len())
            .field("room_id_bytes", &self.room_id.as_str().len())
            .field("filter", &self.filter)
            .field("after", &self.after)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct TopicCollectionPageV1 {
    pub revision: Revision,
    pub room_id: RoomId,
    pub filter: TopicCollectionFilterV1,
    pub topics: Vec<TopicCollectionEntryV1>,
    pub next: Option<TopicCollectionCursorV1>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct TopicStatusV1 {
    pub revision: Revision,
    pub room_id: RoomId,
    pub topic_id: TopicId,
    pub resolved: bool,
}

#[derive(Clone, Eq, PartialEq)]
pub struct TopicSupersessionStatusV1 {
    pub revision: Revision,
    pub room_id: RoomId,
    pub topic_id: TopicId,
    pub successor_topic_id: Option<TopicId>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct TopicAliasStatusV1 {
    pub revision: Revision,
    pub room_id: RoomId,
    pub topic_id: TopicId,
    pub aliases: Vec<String>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct TopicTagStatusV1 {
    pub revision: Revision,
    pub room_id: RoomId,
    pub topic_id: TopicId,
    pub tags: Vec<String>,
}

impl fmt::Debug for TopicTagStatusV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TopicTagStatusV1")
            .field("revision", &self.revision)
            .field("room_id_bytes", &self.room_id.as_str().len())
            .field("topic_id", &self.topic_id)
            .field("tag_count", &self.tags.len())
            .finish()
    }
}

impl fmt::Debug for TopicAliasStatusV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TopicAliasStatusV1")
            .field("revision", &self.revision)
            .field("room_id_bytes", &self.room_id.as_str().len())
            .field("topic_id", &self.topic_id)
            .field("alias_count", &self.aliases.len())
            .finish()
    }
}

impl fmt::Debug for TopicSupersessionStatusV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TopicSupersessionStatusV1")
            .field("revision", &self.revision)
            .field("room_id_bytes", &self.room_id.as_str().len())
            .field("topic_id", &self.topic_id)
            .field("successor_topic_id", &self.successor_topic_id)
            .finish()
    }
}

impl fmt::Debug for TopicStatusV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TopicStatusV1")
            .field("revision", &self.revision)
            .field("room_id_bytes", &self.room_id.as_str().len())
            .field("topic_id", &self.topic_id)
            .field("resolved", &self.resolved)
            .finish()
    }
}

impl fmt::Debug for TopicPageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TopicPageV1")
            .field("revision", &self.revision)
            .field("room_id_bytes", &self.room_id.as_str().len())
            .field("topic_count", &self.topics.len())
            .field("has_next", &self.next.is_some())
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct TopicMessagePageCursorV1 {
    revision: Revision,
    observer: ParticipantId,
    room_id: RoomId,
    topic_id: TopicId,
    after_sequence: u64,
}

impl TopicMessagePageCursorV1 {
    pub fn is_bound_to(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
        topic_id: TopicId,
    ) -> bool {
        &self.observer == observer && &self.room_id == room_id && self.topic_id == topic_id
    }
}

impl fmt::Debug for TopicMessagePageCursorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TopicMessagePageCursorV1")
            .field("revision", &self.revision)
            .field("observer_id_bytes", &self.observer.as_str().len())
            .field("room_id_bytes", &self.room_id.as_str().len())
            .field("topic_id", &self.topic_id)
            .field("after_sequence", &self.after_sequence)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct TopicMessagePageV1 {
    pub revision: Revision,
    pub room_id: RoomId,
    pub topic_id: TopicId,
    pub messages: Vec<Message>,
    pub oldest_retained_sequence: u64,
    pub next_message_sequence: u64,
    pub next: Option<TopicMessagePageCursorV1>,
}

impl fmt::Debug for TopicMessagePageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TopicMessagePageV1")
            .field("revision", &self.revision)
            .field("room_id_bytes", &self.room_id.as_str().len())
            .field("topic_id", &self.topic_id)
            .field("message_count", &self.messages.len())
            .field("oldest_retained_sequence", &self.oldest_retained_sequence)
            .field("next_message_sequence", &self.next_message_sequence)
            .field("has_next", &self.next.is_some())
            .finish()
    }
}

impl fmt::Debug for MessageRevisionPageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MessageRevisionPageV1")
            .field("revision", &self.revision)
            .field("room_id_bytes", &self.room_id.as_str().len())
            .field("message_id", &self.message_id)
            .field("revision_count", &self.revisions.len())
            .field("has_next", &self.next.is_some())
            .finish()
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ThreadMessageSelector {
    Roots,
    Replies(MessageId),
}

/// Opaque continuation for a root page or one root's reply page.
#[derive(Clone, Eq, PartialEq)]
pub struct ThreadMessagePageCursor {
    revision: Revision,
    observer: ParticipantId,
    room_id: RoomId,
    selector: ThreadMessageSelector,
    after_sequence: u64,
}

impl ThreadMessagePageCursor {
    pub fn is_bound_to_roots(&self, observer: &ParticipantId, room_id: &RoomId) -> bool {
        &self.observer == observer
            && &self.room_id == room_id
            && self.selector == ThreadMessageSelector::Roots
    }

    pub fn is_bound_to_replies(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
        root_message_id: MessageId,
    ) -> bool {
        &self.observer == observer
            && &self.room_id == room_id
            && self.selector == ThreadMessageSelector::Replies(root_message_id)
    }
}

impl fmt::Debug for ThreadMessagePageCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ThreadMessagePageCursor")
            .field("revision", &self.revision)
            .field("observer_id_bytes", &self.observer.as_str().len())
            .field("room_id_bytes", &self.room_id.as_str().len())
            .field(
                "selector",
                &match self.selector {
                    ThreadMessageSelector::Roots => "roots",
                    ThreadMessageSelector::Replies(_) => "replies",
                },
            )
            .field("after_sequence", &self.after_sequence)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct ThreadMessagePage {
    pub revision: Revision,
    pub room_id: RoomId,
    pub root_message_id: Option<MessageId>,
    pub messages: Vec<Message>,
    pub oldest_retained_sequence: u64,
    pub next_message_sequence: u64,
    pub next: Option<ThreadMessagePageCursor>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThreadLifecycleStateV1 {
    Active,
    Archived,
}

#[derive(Clone, Eq, PartialEq)]
pub struct ThreadStatusV1 {
    pub revision: Revision,
    pub room_id: RoomId,
    pub root_message_id: MessageId,
    pub state: ThreadLifecycleStateV1,
}

/// One canonical aggregate in a participant-bound reaction observation.
#[derive(Clone, Eq, PartialEq)]
pub struct ReactionSummaryV1 {
    pub key: String,
    pub count: u32,
    pub caller_reacted: bool,
}

impl fmt::Debug for ReactionSummaryV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReactionSummaryV1")
            .field("key_bytes", &self.key.len())
            .field("count", &self.count)
            .field("caller_reacted", &self.caller_reacted)
            .finish()
    }
}

/// Complete bounded reaction state for one retained message.
#[derive(Clone, Eq, PartialEq)]
pub struct ReactionStatusV1 {
    pub revision: Revision,
    pub room_id: RoomId,
    pub message_id: MessageId,
    pub reactions: Vec<ReactionSummaryV1>,
}

impl fmt::Debug for ReactionStatusV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReactionStatusV1")
            .field("revision", &self.revision)
            .field("room_id_bytes", &self.room_id.as_str().len())
            .field("message_id", &self.message_id)
            .field("reaction_count", &self.reactions.len())
            .finish()
    }
}

impl fmt::Debug for ThreadStatusV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ThreadStatusV1")
            .field("revision", &self.revision)
            .field("room_id_bytes", &self.room_id.as_str().len())
            .field("root_message_id", &self.root_message_id)
            .field("state", &self.state)
            .finish()
    }
}

impl fmt::Debug for ThreadMessagePage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ThreadMessagePage")
            .field("revision", &self.revision)
            .field("room_id_bytes", &self.room_id.as_str().len())
            .field("is_reply_page", &self.root_message_id.is_some())
            .field("message_count", &self.messages.len())
            .field("has_next", &self.next.is_some())
            .finish()
    }
}

/// Opaque exclusive position in one member-visible room change stream.
///
/// Positions are stable across canonical engine recovery. They are not bearer
/// credentials: every use rechecks the observer's current room membership.
#[derive(Clone, Eq, PartialEq)]
pub struct RoomChangePosition {
    observer: ParticipantId,
    room_id: RoomId,
    observed_revision: Revision,
    after: Option<EventSequence>,
}

impl RoomChangePosition {
    pub fn revision(&self) -> Revision {
        self.observed_revision
    }

    pub fn is_bound_to(&self, observer: &ParticipantId, room_id: &RoomId) -> bool {
        &self.observer == observer && &self.room_id == room_id
    }
}

impl fmt::Debug for RoomChangePosition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RoomChangePosition")
            .field("observer_id_bytes", &self.observer.as_str().len())
            .field("room_id_bytes", &self.room_id.as_str().len())
            .field("observed_revision", &self.observed_revision)
            .field("has_event_position", &self.after.is_some())
            .finish()
    }
}

/// One bounded, exclusive-after page of retained room-scoped domain events.
#[derive(Clone, Eq, PartialEq)]
pub struct RoomChangePage {
    pub revision: Revision,
    pub changes: Vec<EventEnvelopeV2>,
    pub next: RoomChangePosition,
    pub has_more: bool,
}

impl fmt::Debug for RoomChangePage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RoomChangePage")
            .field("revision", &self.revision)
            .field("change_count", &self.changes.len())
            .field("next", &self.next)
            .field("has_more", &self.has_more)
            .finish()
    }
}

/// Why a room feed can no longer prove continuous visibility.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RoomChangeGap {
    ChangeRetention { discarded_through: EventSequence },
    MembershipChanged,
}

/// Pull-feed result. A gap requires a fresh bounded room snapshot and position.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RoomChangeResult {
    Page(RoomChangePage),
    Gap {
        cause: RoomChangeGap,
        current_revision: Revision,
    },
}

impl fmt::Debug for MessagePage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MessagePage")
            .field("revision", &self.revision)
            .field("room_id_bytes", &self.room_id.as_str().len())
            .field("message_count", &self.messages.len())
            .field("oldest_retained_sequence", &self.oldest_retained_sequence)
            .field("next_message_sequence", &self.next_message_sequence)
            .field("has_next", &self.next.is_some())
            .finish()
    }
}

/// Refusal vocabulary for bounded observation. Refusals expose no foreign IDs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservationRefusal {
    UnknownParticipant,
    UnknownRoom,
    NotMember,
    NotAuthorized,
    InvalidLimit,
    InvalidTarget,
    ForeignCursor,
    Stale { actual_revision: Revision },
    Capacity,
    InconsistentState,
}

#[derive(Debug)]
struct PollV1 {
    id: PollId,
    creator: ParticipantId,
    prompt: String,
    options: Vec<String>,
    eligible_participants: BTreeSet<ParticipantId>,
    max_choices: u8,
    open: bool,
    ballots: BTreeMap<ParticipantId, BTreeSet<u8>>,
}

impl PollV1 {
    fn status(&self) -> PollStatusV1 {
        let mut totals = vec![0_u16; self.options.len()];
        for selections in self.ballots.values() {
            for selection in selections {
                totals[usize::from(*selection)] += 1;
            }
        }
        PollStatusV1 {
            poll_id: self.id,
            creator: self.creator.clone(),
            prompt: self.prompt.clone(),
            options: self.options.clone(),
            eligible_participants: self.eligible_participants.iter().cloned().collect(),
            max_choices: self.max_choices,
            open: self.open,
            totals,
            total_ballots: self
                .ballots
                .len()
                .try_into()
                .expect("poll cohort bound fits u16"),
        }
    }
}

#[derive(Debug)]
struct Room {
    id: RoomId,
    name: String,
    profile: ConversationProfileV1,
    admission: AdmissionPolicy,
    founding_members: BTreeSet<ParticipantId>,
    members: BTreeMap<ParticipantId, Role>,
    invitations: BTreeSet<ParticipantId>,
    bans: BTreeMap<ParticipantId, Role>,
    messages: VecDeque<Message>,
    thread_roots: BTreeSet<MessageId>,
    archived_threads: BTreeSet<MessageId>,
    reactions: BTreeMap<MessageId, BTreeMap<String, BTreeSet<ParticipantId>>>,
    pins: BTreeMap<MessageId, (u64, ParticipantId)>,
    polls: BTreeMap<PollId, PollV1>,
    poll_creation_order: VecDeque<PollId>,
    topics: BTreeMap<TopicId, TopicV1>,
    topic_successors: BTreeMap<TopicId, TopicId>,
    topic_identity_incarnation: u64,
    next_topic_identity: u64,
    poll_identity_incarnation: u64,
    next_poll_identity: u64,
    message_identity_incarnation: u64,
    next_message_identity: u64,
    next_message_sequence: u64,
}

impl Room {
    fn view(&self) -> RoomView {
        RoomView {
            id: self.id.clone(),
            name: self.name.clone(),
            profile: self.profile,
            admission: self.admission,
            founding_members: self.founding_members.iter().cloned().collect(),
            members: self
                .members
                .iter()
                .map(|(id, role)| (id.clone(), *role))
                .collect(),
            pending_invitations: self.invitations.iter().cloned().collect(),
            banned_participants: self.bans.keys().cloned().collect(),
            messages: self.messages.iter().cloned().collect(),
            next_message_sequence: self.next_message_sequence,
        }
    }
}

/// Opaque caller-supplied correlation for one moderation decision.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ModerationCausationV1([u8; 16]);

impl ModerationCausationV1 {
    pub const fn new(value: [u8; 16]) -> Self {
        Self(value)
    }

    pub const fn into_bytes(self) -> [u8; 16] {
        self.0
    }
}

impl fmt::Debug for ModerationCausationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ModerationCausationV1(<opaque>)")
    }
}

/// Human-readable moderation rationale. Debug output is content-free.
#[derive(Clone, Eq, PartialEq)]
pub struct ModerationReasonV1(String);

impl ModerationReasonV1 {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ModerationReasonV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ModerationReasonV1")
            .field("bytes", &self.0.len())
            .finish()
    }
}

/// Finite room action vocabulary represented by the current engine profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoomActionV1 {
    CreateRoom,
    CreatePeerConversation,
    Invite,
    Join,
    Leave,
    Speak,
    Reply,
    React,
    Pin,
    CreatePoll,
    VotePoll,
    ManagePoll,
    EditOwn,
    ManageThread,
    WithdrawOwn,
    RedactAny,
    ManageRole,
    ManageRoomPolicy,
    Kick,
    Ban,
    Unban,
    InspectHistory,
    ManageTopics,
}

#[derive(Clone, Copy)]
enum MessagePlacement {
    Unscoped,
    Reply(MessageId),
    Topic(TopicId),
}

struct MessageAdmission {
    room_id: RoomId,
    author: ParticipantId,
    at: LogicalTime,
    body: String,
    addressed_to: Vec<ParticipantId>,
    placement: MessagePlacement,
    attachments: Option<Vec<AttachmentDescriptorV1>>,
    vellum_artifacts: Option<Vec<VellumArtifactReferenceV1>>,
}

impl CommandV2 {
    /// Passive authorization vocabulary for a trusted host or Aegis adapter.
    /// Agora still enforces its room invariants after any external decision.
    pub fn room_action(&self) -> Option<RoomActionV1> {
        match self {
            Self::RegisterParticipant(_) => None,
            Self::CreateRoom { .. } => Some(RoomActionV1::CreateRoom),
            Self::CreatePeerConversation { .. } => Some(RoomActionV1::CreatePeerConversation),
            Self::Invite { .. } => Some(RoomActionV1::Invite),
            Self::Join { .. } => Some(RoomActionV1::Join),
            Self::Leave { .. } => Some(RoomActionV1::Leave),
            Self::SetRole { .. } => Some(RoomActionV1::ManageRole),
            Self::SetAdmissionPolicy { .. } => Some(RoomActionV1::ManageRoomPolicy),
            Self::Kick { .. } => Some(RoomActionV1::Kick),
            Self::Ban { .. } => Some(RoomActionV1::Ban),
            Self::Unban { .. } => Some(RoomActionV1::Unban),
            Self::Speak { .. } | Self::SpeakAddressed { .. } | Self::SpeakInTopic { .. } => {
                Some(RoomActionV1::Speak)
            }
            Self::SpeakReply { .. } => Some(RoomActionV1::Reply),
            Self::SpeakWithAttachments { target, .. } => Some(match target {
                AttachmentMessageTargetV1::Reply(_) => RoomActionV1::Reply,
                AttachmentMessageTargetV1::Room | AttachmentMessageTargetV1::Topic(_) => {
                    RoomActionV1::Speak
                }
            }),
            Self::SpeakWithVellumArtifacts { target, .. } => Some(match target {
                VellumArtifactMessageTargetV1::Reply(_) => RoomActionV1::Reply,
                VellumArtifactMessageTargetV1::Room | VellumArtifactMessageTargetV1::Topic(_) => {
                    RoomActionV1::Speak
                }
            }),
            Self::SetReaction { .. } => Some(RoomActionV1::React),
            Self::SetPin { .. } => Some(RoomActionV1::Pin),
            Self::CreatePoll { .. } => Some(RoomActionV1::CreatePoll),
            Self::SetPollBallot { .. } => Some(RoomActionV1::VotePoll),
            Self::SetPollOpen { .. } => Some(RoomActionV1::ManagePoll),
            Self::EditMessage { .. } => Some(RoomActionV1::EditOwn),
            Self::SetThreadArchived { .. } => Some(RoomActionV1::ManageThread),
            Self::WithdrawMessage { .. } => Some(RoomActionV1::WithdrawOwn),
            Self::RedactMessage { .. } => Some(RoomActionV1::RedactAny),
            Self::CreateTopic { .. } => Some(RoomActionV1::ManageTopics),
            Self::RenameTopic { .. } => Some(RoomActionV1::ManageTopics),
            Self::SetTopicResolved { .. } => Some(RoomActionV1::ManageTopics),
            Self::SetTopicSuccessor { .. } => Some(RoomActionV1::ManageTopics),
            Self::SetTopicAlias { .. } => Some(RoomActionV1::ManageTopics),
            Self::SetTopicTag { .. } => Some(RoomActionV1::ManageTopics),
        }
    }
}

/// The closed command vocabulary for the v0.3 participant-neutral profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandV2 {
    RegisterParticipant(Participant),
    CreateRoom {
        room_id: RoomId,
        name: String,
        owner: ParticipantId,
        admission: AdmissionPolicy,
    },
    CreatePeerConversation {
        room_id: RoomId,
        name: String,
        initiator: ParticipantId,
        profile: ConversationProfileV1,
        founding_members: Vec<ParticipantId>,
    },
    Invite {
        room_id: RoomId,
        actor: ParticipantId,
        participant: ParticipantId,
    },
    Join {
        room_id: RoomId,
        participant: ParticipantId,
    },
    Leave {
        room_id: RoomId,
        participant: ParticipantId,
    },
    SetRole {
        room_id: RoomId,
        actor: ParticipantId,
        participant: ParticipantId,
        role: Role,
    },
    SetAdmissionPolicy {
        room_id: RoomId,
        actor: ParticipantId,
        admission: AdmissionPolicy,
    },
    Kick {
        room_id: RoomId,
        actor: ParticipantId,
        participant: ParticipantId,
        reason: ModerationReasonV1,
        causation: ModerationCausationV1,
    },
    Ban {
        room_id: RoomId,
        actor: ParticipantId,
        participant: ParticipantId,
        reason: ModerationReasonV1,
        causation: ModerationCausationV1,
    },
    Unban {
        room_id: RoomId,
        actor: ParticipantId,
        participant: ParticipantId,
        reason: ModerationReasonV1,
        causation: ModerationCausationV1,
    },
    Speak {
        room_id: RoomId,
        author: ParticipantId,
        at: LogicalTime,
        body: String,
    },
    /// Atomically publishes a message with explicit participant-neutral
    /// addressing. `addressed_to` must be strictly increasing by participant ID.
    SpeakAddressed {
        room_id: RoomId,
        author: ParticipantId,
        at: LogicalTime,
        body: String,
        addressed_to: Vec<ParticipantId>,
    },
    SpeakReply {
        room_id: RoomId,
        author: ParticipantId,
        root_message_id: MessageId,
        at: LogicalTime,
        body: String,
        addressed_to: Vec<ParticipantId>,
    },
    CreateTopic {
        room_id: RoomId,
        actor: ParticipantId,
        name: String,
    },
    RenameTopic {
        room_id: RoomId,
        actor: ParticipantId,
        topic_id: TopicId,
        name: String,
    },
    SetTopicResolved {
        room_id: RoomId,
        actor: ParticipantId,
        topic_id: TopicId,
        resolved: bool,
    },
    SetTopicSuccessor {
        room_id: RoomId,
        actor: ParticipantId,
        topic_id: TopicId,
        successor_topic_id: Option<TopicId>,
    },
    SetTopicAlias {
        room_id: RoomId,
        actor: ParticipantId,
        topic_id: TopicId,
        alias: String,
        added: bool,
    },
    SetTopicTag {
        room_id: RoomId,
        actor: ParticipantId,
        topic_id: TopicId,
        tag: String,
        added: bool,
    },
    SpeakInTopic {
        room_id: RoomId,
        author: ParticipantId,
        topic_id: TopicId,
        at: LogicalTime,
        body: String,
        addressed_to: Vec<ParticipantId>,
    },
    SpeakWithAttachments {
        room_id: RoomId,
        author: ParticipantId,
        target: AttachmentMessageTargetV1,
        at: LogicalTime,
        body: String,
        addressed_to: Vec<ParticipantId>,
        attachments: Vec<AttachmentDescriptorV1>,
    },
    SpeakWithVellumArtifacts {
        room_id: RoomId,
        author: ParticipantId,
        target: VellumArtifactMessageTargetV1,
        at: LogicalTime,
        body: String,
        addressed_to: Vec<ParticipantId>,
        references: Vec<VellumArtifactReferenceV1>,
    },
    WithdrawMessage {
        room_id: RoomId,
        actor: ParticipantId,
        message_id: MessageId,
    },
    RedactMessage {
        room_id: RoomId,
        actor: ParticipantId,
        message_id: MessageId,
        reason: ModerationReasonV1,
        causation: ModerationCausationV1,
    },
    SetThreadArchived {
        room_id: RoomId,
        actor: ParticipantId,
        root_message_id: MessageId,
        archived: bool,
        reason: ModerationReasonV1,
        causation: ModerationCausationV1,
    },
    SetReaction {
        room_id: RoomId,
        actor: ParticipantId,
        message_id: MessageId,
        key: String,
        reacted: bool,
    },
    SetPin {
        room_id: RoomId,
        actor: ParticipantId,
        message_id: MessageId,
        pinned: bool,
    },
    CreatePoll {
        room_id: RoomId,
        actor: ParticipantId,
        prompt: String,
        options: Vec<String>,
        eligible_participants: Vec<ParticipantId>,
        max_choices: u8,
    },
    SetPollBallot {
        room_id: RoomId,
        actor: ParticipantId,
        poll_id: PollId,
        selections: Vec<u8>,
    },
    SetPollOpen {
        room_id: RoomId,
        actor: ParticipantId,
        poll_id: PollId,
        open: bool,
    },
    EditMessage {
        room_id: RoomId,
        actor: ParticipantId,
        message_id: MessageId,
        at: LogicalTime,
        body: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutcomeV2 {
    ParticipantRegistered(ParticipantId),
    RoomCreated(RoomId),
    PeerConversationCreated {
        room_id: RoomId,
        profile: ConversationProfileV1,
    },
    InvitationRecorded {
        room_id: RoomId,
        participant: ParticipantId,
    },
    Joined {
        room_id: RoomId,
        participant: ParticipantId,
    },
    Left {
        room_id: RoomId,
        participant: ParticipantId,
    },
    RoleChanged {
        room_id: RoomId,
        participant: ParticipantId,
        role: Role,
    },
    RoomPolicyChanged {
        room_id: RoomId,
        actor: ParticipantId,
        previous: AdmissionPolicy,
        admission: AdmissionPolicy,
    },
    Kicked {
        room_id: RoomId,
        actor: ParticipantId,
        participant: ParticipantId,
        reason: ModerationReasonV1,
        causation: ModerationCausationV1,
    },
    Banned {
        room_id: RoomId,
        actor: ParticipantId,
        participant: ParticipantId,
        reason: ModerationReasonV1,
        causation: ModerationCausationV1,
    },
    Unbanned {
        room_id: RoomId,
        actor: ParticipantId,
        participant: ParticipantId,
        reason: ModerationReasonV1,
        causation: ModerationCausationV1,
    },
    MessageAccepted {
        room_id: RoomId,
        sequence: u64,
    },
    TopicCreated {
        room_id: RoomId,
        actor: ParticipantId,
        topic_id: TopicId,
        name: String,
    },
    TopicRenamed {
        room_id: RoomId,
        actor: ParticipantId,
        topic_id: TopicId,
        previous: String,
        name: String,
    },
    TopicLifecycleChanged {
        room_id: RoomId,
        actor: ParticipantId,
        topic_id: TopicId,
        previous_resolved: bool,
        resolved: bool,
    },
    TopicSupersessionChanged {
        room_id: RoomId,
        actor: ParticipantId,
        topic_id: TopicId,
        previous_successor_topic_id: Option<TopicId>,
        successor_topic_id: Option<TopicId>,
    },
    TopicAliasChanged {
        room_id: RoomId,
        actor: ParticipantId,
        topic_id: TopicId,
        alias: String,
        added: bool,
    },
    TopicTagChanged {
        room_id: RoomId,
        actor: ParticipantId,
        topic_id: TopicId,
        tag: String,
        added: bool,
    },
    TopicMessageAccepted {
        room_id: RoomId,
        topic_id: TopicId,
        sequence: u64,
    },
    AttachmentMessageAccepted {
        room_id: RoomId,
        message_id: MessageId,
        sequence: u64,
        attachment_count: u8,
    },
    VellumArtifactMessageAccepted {
        room_id: RoomId,
        message_id: MessageId,
        sequence: u64,
        reference_count: u8,
    },
    ReplyAccepted {
        room_id: RoomId,
        root_message_id: MessageId,
        sequence: u64,
    },
    MessageWithdrawn {
        room_id: RoomId,
        actor: ParticipantId,
        message_id: MessageId,
        sequence: u64,
    },
    MessageRedacted {
        room_id: RoomId,
        actor: ParticipantId,
        author: ParticipantId,
        message_id: MessageId,
        sequence: u64,
        reason: ModerationReasonV1,
        causation: ModerationCausationV1,
    },
    ThreadLifecycleChanged {
        room_id: RoomId,
        actor: ParticipantId,
        root_message_id: MessageId,
        archived: bool,
        reason: ModerationReasonV1,
        causation: ModerationCausationV1,
    },
    ReactionChanged {
        room_id: RoomId,
        actor: ParticipantId,
        message_id: MessageId,
        key: String,
        reacted: bool,
    },
    PinChanged {
        room_id: RoomId,
        actor: ParticipantId,
        message_id: MessageId,
        original_sequence: u64,
        pinned: bool,
    },
    PollCreated {
        room_id: RoomId,
        poll_id: PollId,
    },
    PollBallotChanged {
        room_id: RoomId,
        poll_id: PollId,
        totals: Vec<u16>,
        total_ballots: u16,
    },
    PollLifecycleChanged {
        room_id: RoomId,
        actor: ParticipantId,
        poll_id: PollId,
        open: bool,
    },
    MessageEdited {
        room_id: RoomId,
        actor: ParticipantId,
        message_id: MessageId,
        message_revision: u32,
        at: LogicalTime,
        body: String,
    },
}

/// Typed refusal reasons. Every refusal preserves authoritative state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Refusal {
    InvalidParticipantId,
    InvalidRoomId,
    InvalidDisplayName,
    InvalidRoomName,
    InvalidConversationProfile,
    InvalidConversationCohort,
    InvalidMessageBody,
    InvalidAttachmentCount,
    InvalidAttachmentDeclaredLength,
    InvalidAttachmentMediaType,
    InvalidAttachmentDisplayLabel,
    DuplicateAttachmentId,
    DuplicateAttachmentObject,
    InvalidVellumArtifactReferenceCount,
    DuplicateVellumArtifactReferenceId,
    DuplicateVellumArtifactTarget,
    ParticipantLimit,
    RoomLimit,
    MemberLimit,
    InvitationLimit,
    BanLimit,
    MessageRetentionDisabled,
    MessageIdentityExhausted,
    MessageSequenceExhausted,
    EventOutboxFull,
    RevisionExhausted,
    EventSequenceExhausted,
    AddressedParticipantLimit,
    NonCanonicalAddressing,
    InvalidModerationReason,
    DuplicateParticipant(ParticipantId),
    DuplicateRoom(RoomId),
    UnknownParticipant(ParticipantId),
    UnknownRoom(RoomId),
    AlreadyMember(ParticipantId),
    NotMember(ParticipantId),
    InvitationRequired(ParticipantId),
    ParticipantBanned(ParticipantId),
    NotBanned(ParticipantId),
    NotAuthorized(ParticipantId),
    OwnerCannotLeave,
    ImmutableConversationCohort,
    InvalidRoleChange,
    InvalidPolicyChange,
    UnknownMessage,
    InvalidReplyTarget,
    ThreadArchived,
    InvalidThreadLifecycleChange,
    InvalidReaction,
    ReactionLimit,
    InvalidReactionChange,
    PinLimit,
    InvalidPinChange,
    InvalidPollPrompt,
    InvalidPollOptions,
    InvalidPollEligibility,
    InvalidPollChoiceLimit,
    PollLimit,
    PollIdentityExhausted,
    UnknownPoll,
    NotPollEligible,
    PollClosed,
    InvalidPollBallot,
    InvalidPollBallotChange,
    InvalidPollLifecycleChange,
    MessageRevisionLimit,
    InvalidMessageEdit,
    InvalidTopicName,
    TopicLimit,
    TopicIdentityExhausted,
    DuplicateTopicName,
    UnknownTopic,
    TopicResolved,
    InvalidTopicLifecycleChange,
    InvalidTopicSupersession,
    TopicMustBeResolved,
    TopicAliasLimit,
    UnknownTopicAlias,
    InvalidTopicTag,
    DuplicateTopicTag,
    TopicTagLimit,
    UnknownTopicTag,
    MessageAlreadyTombstoned,
    RevisionConflict {
        expected: Revision,
        actual: Revision,
    },
}

/// Monotone engine state revision.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Revision(pub u64);

/// Monotone outbox position.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct EventSequence(pub u64);

/// Optimistic command envelope used by concurrent or remote hosts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionedCommandV2 {
    pub expected_revision: Revision,
    pub command: CommandV2,
}

/// Opaque adapter-supplied identity for one response publication.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PublicationKeyV2([u8; 16]);

impl PublicationKeyV2 {
    pub const fn new(value: [u8; 16]) -> Self {
        Self(value)
    }
}

impl fmt::Debug for PublicationKeyV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PublicationKeyV2(<opaque>)")
    }
}

/// Replay-safe response publication profile layered over the closed v0.3
/// command vocabulary. The expected revision applies only to first acceptance;
/// matching replay observes the retained transition.
#[derive(Clone, Eq, PartialEq)]
pub struct PublicationCommandV2 {
    expected_revision: Revision,
    key: PublicationKeyV2,
    room_id: RoomId,
    author: ParticipantId,
    at: LogicalTime,
    body: String,
}

impl PublicationCommandV2 {
    pub fn new(
        expected_revision: Revision,
        key: PublicationKeyV2,
        room_id: RoomId,
        author: ParticipantId,
        at: LogicalTime,
        body: impl Into<String>,
    ) -> Self {
        Self {
            expected_revision,
            key,
            room_id,
            author,
            at,
            body: body.into(),
        }
    }
}

impl fmt::Debug for PublicationCommandV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PublicationCommandV2")
            .field("expected_revision", &self.expected_revision)
            .field("key", &self.key)
            .field("room_id", &self.room_id)
            .field("author", &self.author)
            .field("at", &self.at)
            .field("body_bytes", &self.body.len())
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublicationRefusalV2 {
    IdempotencyConflict,
    PublicationReceiptLimit,
    Core(Refusal),
}

impl From<Refusal> for PublicationRefusalV2 {
    fn from(value: Refusal) -> Self {
        Self::Core(value)
    }
}

/// Accepted transition metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionV2 {
    pub revision: Revision,
    pub event_sequence: EventSequence,
    pub outcome: OutcomeV2,
}

/// Ordered facts emitted after accepted commands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DomainEventV2 {
    ParticipantRegistered {
        participant_id: ParticipantId,
        kind: ParticipantKind,
    },
    RoomCreated {
        room_id: RoomId,
        owner: ParticipantId,
        admission: AdmissionPolicy,
    },
    PeerConversationCreated {
        room_id: RoomId,
        initiator: ParticipantId,
        profile: ConversationProfileV1,
        founding_members: Vec<ParticipantId>,
    },
    TopicCreated {
        room_id: RoomId,
        actor: ParticipantId,
        topic_id: TopicId,
        name: String,
    },
    TopicRenamed {
        room_id: RoomId,
        actor: ParticipantId,
        topic_id: TopicId,
        previous: String,
        name: String,
    },
    TopicLifecycleChanged {
        room_id: RoomId,
        actor: ParticipantId,
        topic_id: TopicId,
        previous_resolved: bool,
        resolved: bool,
    },
    TopicSupersessionChanged {
        room_id: RoomId,
        actor: ParticipantId,
        topic_id: TopicId,
        previous_successor_topic_id: Option<TopicId>,
        successor_topic_id: Option<TopicId>,
    },
    TopicAliasChanged {
        room_id: RoomId,
        actor: ParticipantId,
        topic_id: TopicId,
        alias: String,
        added: bool,
    },
    TopicTagChanged {
        room_id: RoomId,
        actor: ParticipantId,
        topic_id: TopicId,
        tag: String,
        added: bool,
    },
    InvitationRecorded {
        room_id: RoomId,
        participant: ParticipantId,
    },
    ParticipantJoined {
        room_id: RoomId,
        participant: ParticipantId,
    },
    ParticipantLeft {
        room_id: RoomId,
        participant: ParticipantId,
    },
    RoleChanged {
        room_id: RoomId,
        participant: ParticipantId,
        role: Role,
    },
    RoomPolicyChanged {
        room_id: RoomId,
        actor: ParticipantId,
        previous: AdmissionPolicy,
        admission: AdmissionPolicy,
    },
    ParticipantKicked {
        room_id: RoomId,
        actor: ParticipantId,
        participant: ParticipantId,
        reason: ModerationReasonV1,
        causation: ModerationCausationV1,
    },
    ParticipantBanned {
        room_id: RoomId,
        actor: ParticipantId,
        participant: ParticipantId,
        reason: ModerationReasonV1,
        causation: ModerationCausationV1,
    },
    ParticipantUnbanned {
        room_id: RoomId,
        actor: ParticipantId,
        participant: ParticipantId,
        reason: ModerationReasonV1,
        causation: ModerationCausationV1,
    },
    MessageAccepted {
        room_id: RoomId,
        message: Message,
    },
    MessageWithdrawn {
        room_id: RoomId,
        actor: ParticipantId,
        message_id: MessageId,
        sequence: u64,
    },
    MessageRedacted {
        room_id: RoomId,
        actor: ParticipantId,
        author: ParticipantId,
        message_id: MessageId,
        sequence: u64,
        reason: ModerationReasonV1,
        causation: ModerationCausationV1,
    },
    ThreadLifecycleChanged {
        room_id: RoomId,
        actor: ParticipantId,
        root_message_id: MessageId,
        archived: bool,
        reason: ModerationReasonV1,
        causation: ModerationCausationV1,
    },
    ReactionChanged {
        room_id: RoomId,
        actor: ParticipantId,
        message_id: MessageId,
        key: String,
        reacted: bool,
    },
    PinChanged {
        room_id: RoomId,
        actor: ParticipantId,
        message_id: MessageId,
        original_sequence: u64,
        pinned: bool,
    },
    PollCreated {
        room_id: RoomId,
        actor: ParticipantId,
        poll_id: PollId,
        prompt: String,
        options: Vec<String>,
        eligible_participants: Vec<ParticipantId>,
        max_choices: u8,
    },
    PollBallotChanged {
        room_id: RoomId,
        poll_id: PollId,
        totals: Vec<u16>,
        total_ballots: u16,
    },
    PollLifecycleChanged {
        room_id: RoomId,
        actor: ParticipantId,
        poll_id: PollId,
        open: bool,
    },
    MessageEdited {
        room_id: RoomId,
        actor: ParticipantId,
        message_id: MessageId,
        message_revision: u32,
        at: LogicalTime,
        body: String,
    },
}

impl DomainEventV2 {
    fn room_id(&self) -> Option<&RoomId> {
        match self {
            Self::ParticipantRegistered { .. } => None,
            Self::RoomCreated { room_id, .. }
            | Self::PeerConversationCreated { room_id, .. }
            | Self::TopicCreated { room_id, .. }
            | Self::TopicRenamed { room_id, .. }
            | Self::TopicLifecycleChanged { room_id, .. }
            | Self::TopicSupersessionChanged { room_id, .. }
            | Self::TopicAliasChanged { room_id, .. }
            | Self::TopicTagChanged { room_id, .. }
            | Self::InvitationRecorded { room_id, .. }
            | Self::ParticipantJoined { room_id, .. }
            | Self::ParticipantLeft { room_id, .. }
            | Self::RoleChanged { room_id, .. }
            | Self::RoomPolicyChanged { room_id, .. }
            | Self::ParticipantKicked { room_id, .. }
            | Self::ParticipantBanned { room_id, .. }
            | Self::ParticipantUnbanned { room_id, .. }
            | Self::MessageAccepted { room_id, .. }
            | Self::MessageWithdrawn { room_id, .. }
            | Self::MessageRedacted { room_id, .. }
            | Self::ThreadLifecycleChanged { room_id, .. }
            | Self::ReactionChanged { room_id, .. }
            | Self::PinChanged { room_id, .. }
            | Self::PollCreated { room_id, .. }
            | Self::PollBallotChanged { room_id, .. }
            | Self::PollLifecycleChanged { room_id, .. }
            | Self::MessageEdited { room_id, .. } => Some(room_id),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventEnvelopeV2 {
    pub sequence: EventSequence,
    pub revision: Revision,
    pub event: DomainEventV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcknowledgeError {
    Regressed {
        requested: EventSequence,
        acknowledged_through: EventSequence,
    },
    BeyondPublished {
        requested: EventSequence,
        last_published: Option<EventSequence>,
    },
}

#[derive(Clone, Eq, PartialEq)]
struct PublicationRecordV2 {
    room_id: RoomId,
    author: ParticipantId,
    at: LogicalTime,
    body: String,
    transition: TransitionV2,
}

impl fmt::Debug for PublicationRecordV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PublicationRecordV2")
            .field("room_id", &self.room_id)
            .field("author", &self.author)
            .field("at", &self.at)
            .field("body_bytes", &self.body.len())
            .field("transition", &self.transition)
            .finish()
    }
}

/// The authoritative, serial in-process engine.
#[derive(Debug)]
pub struct Engine {
    limits: Limits,
    participants: BTreeMap<ParticipantId, Participant>,
    rooms: BTreeMap<RoomId, Room>,
    revision: Revision,
    next_event_sequence: EventSequence,
    last_published_event: Option<EventSequence>,
    acknowledged_through: Option<EventSequence>,
    pending_events: VecDeque<EventEnvelopeV2>,
    discarded_changes_through: Option<EventSequence>,
    retained_changes: VecDeque<EventEnvelopeV2>,
    publications: BTreeMap<PublicationKeyV2, PublicationRecordV2>,
    read_positions: BTreeMap<(ParticipantId, RoomId, ReadScopeV1), ReadPositionV1>,
    attention_preferences:
        BTreeMap<(ParticipantId, RoomId, AttentionScopeV1), AttentionPreferenceV1>,
    private_saves: BTreeMap<(ParticipantId, RoomId, MessageId), u64>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Self::with_limits(Limits::default())
    }

    pub fn with_limits(limits: Limits) -> Self {
        Self {
            limits,
            participants: BTreeMap::new(),
            rooms: BTreeMap::new(),
            revision: Revision(0),
            next_event_sequence: EventSequence(0),
            last_published_event: None,
            acknowledged_through: None,
            pending_events: VecDeque::new(),
            discarded_changes_through: None,
            retained_changes: VecDeque::new(),
            publications: BTreeMap::new(),
            read_positions: BTreeMap::new(),
            attention_preferences: BTreeMap::new(),
            private_saves: BTreeMap::new(),
        }
    }

    pub fn limits(&self) -> Limits {
        self.limits
    }

    pub fn participant(&self, id: &ParticipantId) -> Option<&Participant> {
        self.participants.get(id)
    }

    pub fn participants(&self) -> impl ExactSizeIterator<Item = &Participant> {
        self.participants.values()
    }

    pub fn room(&self, id: &RoomId) -> Option<RoomView> {
        self.rooms.get(id).map(Room::view)
    }

    pub fn conversation_profile(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
    ) -> Result<ConversationProfileStatusV1, ObservationRefusal> {
        if !self.participants.contains_key(observer) {
            return Err(ObservationRefusal::UnknownParticipant);
        }
        let room = self
            .rooms
            .get(room_id)
            .ok_or(ObservationRefusal::UnknownRoom)?;
        if !room.members.contains_key(observer) {
            return Err(ObservationRefusal::NotMember);
        }
        let founding_members = match room.profile {
            ConversationProfileV1::Ordinary => room
                .members
                .iter()
                .filter(|(_, role)| **role == Role::Owner)
                .map(|(id, _)| id.clone())
                .collect(),
            ConversationProfileV1::Direct | ConversationProfileV1::GroupDirect => {
                room.founding_members.iter().cloned().collect()
            }
        };
        Ok(ConversationProfileStatusV1 {
            revision: self.revision,
            room_id: room_id.clone(),
            profile: room.profile,
            founding_members,
        })
    }

    /// Returns the current room role without creating a durable observation.
    ///
    /// `None` deliberately does not distinguish an unknown room from a
    /// non-member. Composition layers may use this to bind transient facts to
    /// current membership without caching room internals.
    pub fn member_role(&self, room_id: &RoomId, participant: &ParticipantId) -> Option<Role> {
        self.rooms.get(room_id)?.members.get(participant).copied()
    }

    pub fn topic(&self, room_id: &RoomId, topic_id: TopicId) -> Option<&TopicV1> {
        self.rooms.get(room_id)?.topics.get(&topic_id)
    }

    /// Exact retained-message observation without copying the room history.
    pub fn message(&self, id: &RoomId, sequence: u64) -> Option<&Message> {
        let room = self.rooms.get(id)?;
        let start = room
            .next_message_sequence
            .checked_sub(room.messages.len() as u64)?;
        let index: usize = sequence.checked_sub(start)?.try_into().ok()?;
        room.messages.get(index).filter(|m| m.sequence == sequence)
    }

    /// Finds a retained message by stable identity without interpreting its
    /// opaque bytes as an ordering coordinate.
    pub fn message_by_id(&self, room_id: &RoomId, message_id: MessageId) -> Option<&Message> {
        self.rooms
            .get(room_id)?
            .messages
            .iter()
            .find(|message| message.id == message_id)
    }

    /// Returns only the caller's private marker. Membership is checked on every
    /// observation even when retained marker state survives a leave or ban.
    pub fn read_position(
        &self,
        actor: &ParticipantId,
        room_id: &RoomId,
        scope: &ReadScopeV1,
    ) -> Result<Option<&ReadPositionV1>, ReadPositionRefusalV1> {
        if !self.participants.contains_key(actor) {
            return Err(ReadPositionRefusalV1::UnknownParticipant);
        }
        let room = self
            .rooms
            .get(room_id)
            .ok_or(ReadPositionRefusalV1::UnknownRoom)?;
        if !room.members.contains_key(actor) {
            return Err(ReadPositionRefusalV1::NotMember);
        }
        Ok(self
            .read_positions
            .get(&(actor.clone(), room_id.clone(), *scope)))
    }

    /// Advances one caller-owned marker without changing domain revision or
    /// publishing an event. The exact retained target anchors the claim.
    pub fn advance_read_position(
        &mut self,
        actor: &ParticipantId,
        room_id: &RoomId,
        scope: ReadScopeV1,
        observed_revision: Revision,
        target_message_id: MessageId,
    ) -> Result<ReadPositionV1, ReadPositionRefusalV1> {
        if !self.participants.contains_key(actor) {
            return Err(ReadPositionRefusalV1::UnknownParticipant);
        }
        if observed_revision != self.revision {
            return Err(ReadPositionRefusalV1::Stale {
                actual_revision: self.revision,
            });
        }
        let room = self
            .rooms
            .get(room_id)
            .ok_or(ReadPositionRefusalV1::UnknownRoom)?;
        if !room.members.contains_key(actor) {
            return Err(ReadPositionRefusalV1::NotMember);
        }
        match scope {
            ReadScopeV1::Room => {}
            ReadScopeV1::Thread(root) => {
                let root = room
                    .messages
                    .iter()
                    .find(|message| message.id == root)
                    .ok_or(ReadPositionRefusalV1::InvalidScope)?;
                if root.reply_to_root.is_some() {
                    return Err(ReadPositionRefusalV1::InvalidScope);
                }
            }
            ReadScopeV1::Topic(topic) => {
                if !room.topics.contains_key(&topic) {
                    return Err(ReadPositionRefusalV1::InvalidScope);
                }
            }
        }
        let target = room
            .messages
            .iter()
            .find(|message| message.id == target_message_id)
            .ok_or(ReadPositionRefusalV1::InvalidTarget)?;
        let in_scope = match scope {
            ReadScopeV1::Room => true,
            ReadScopeV1::Thread(root) => target.id == root || target.reply_to_root == Some(root),
            ReadScopeV1::Topic(topic) => target.topic_id == Some(topic),
        };
        if !in_scope {
            return Err(ReadPositionRefusalV1::InvalidTarget);
        }
        let key = (actor.clone(), room_id.clone(), scope);
        if let Some(current) = self.read_positions.get(&key) {
            if current.target_message_id == target_message_id {
                return Ok(current.clone());
            }
            if target.sequence <= current.target_sequence {
                return Err(ReadPositionRefusalV1::Regression);
            }
        } else if self.read_positions.len() == MAX_READ_POSITIONS_V1 {
            return Err(ReadPositionRefusalV1::Capacity);
        }
        let position = ReadPositionV1 {
            room_id: room_id.clone(),
            scope,
            target_message_id,
            target_sequence: target.sequence,
            observed_revision,
        };
        self.read_positions.insert(key, position.clone());
        Ok(position)
    }

    /// Returns only the caller's private attention intent.
    pub fn attention_preference(
        &self,
        actor: &ParticipantId,
        room_id: &RoomId,
        scope: AttentionScopeV1,
    ) -> Result<AttentionPreferenceV1, AttentionPreferenceRefusalV1> {
        self.validate_attention_actor(actor, room_id)?;
        Ok(self
            .attention_preferences
            .get(&(actor.clone(), room_id.clone(), scope))
            .copied()
            .unwrap_or_default())
    }

    /// Replaces one caller-owned intent without changing revision or events.
    pub fn set_attention_preference(
        &mut self,
        actor: &ParticipantId,
        room_id: &RoomId,
        scope: AttentionScopeV1,
        observed_revision: Revision,
        preference: AttentionPreferenceV1,
    ) -> Result<AttentionPreferenceV1, AttentionPreferenceRefusalV1> {
        if observed_revision != self.revision {
            return Err(AttentionPreferenceRefusalV1::Stale {
                actual_revision: self.revision,
            });
        }
        let key = (actor.clone(), room_id.clone(), scope);
        self.validate_attention_actor(actor, room_id)?;
        if preference.is_default() {
            self.attention_preferences.remove(&key);
        } else {
            if !self.attention_preferences.contains_key(&key) {
                self.validate_attention_scope(room_id, scope)?;
                if self.attention_preferences.len() == MAX_ATTENTION_PREFERENCES_V1 {
                    return Err(AttentionPreferenceRefusalV1::Capacity);
                }
            }
            self.attention_preferences.insert(key, preference);
        }
        Ok(preference)
    }

    fn validate_attention_actor(
        &self,
        actor: &ParticipantId,
        room_id: &RoomId,
    ) -> Result<&Room, AttentionPreferenceRefusalV1> {
        if !self.participants.contains_key(actor) {
            return Err(AttentionPreferenceRefusalV1::UnknownParticipant);
        }
        let room = self
            .rooms
            .get(room_id)
            .ok_or(AttentionPreferenceRefusalV1::UnknownRoom)?;
        if !room.members.contains_key(actor) {
            return Err(AttentionPreferenceRefusalV1::NotMember);
        }
        Ok(room)
    }

    fn validate_attention_scope(
        &self,
        room_id: &RoomId,
        scope: AttentionScopeV1,
    ) -> Result<(), AttentionPreferenceRefusalV1> {
        let room = self
            .rooms
            .get(room_id)
            .ok_or(AttentionPreferenceRefusalV1::UnknownRoom)?;
        match scope {
            AttentionScopeV1::Room => {}
            AttentionScopeV1::Thread(root) => {
                let root = room
                    .messages
                    .iter()
                    .find(|message| message.id == root)
                    .ok_or(AttentionPreferenceRefusalV1::InvalidScope)?;
                if root.reply_to_root.is_some() {
                    return Err(AttentionPreferenceRefusalV1::InvalidScope);
                }
            }
            AttentionScopeV1::Topic(topic) => {
                if !room.topics.contains_key(&topic) {
                    return Err(AttentionPreferenceRefusalV1::InvalidScope);
                }
            }
        }
        Ok(())
    }

    /// Observes only the caller's private saved relation.
    pub fn saved_reference(
        &self,
        actor: &ParticipantId,
        room_id: &RoomId,
        message_id: MessageId,
    ) -> Result<Option<SavedReferenceV1>, PrivateSaveRefusalV1> {
        let room = self.validate_private_save_actor(actor, room_id)?;
        let Some(original_sequence) = self
            .private_saves
            .get(&(actor.clone(), room_id.clone(), message_id))
            .copied()
        else {
            return Ok(None);
        };
        Ok(Some(SavedReferenceV1 {
            message_id,
            original_sequence,
            availability: stable_reference_availability(room, message_id),
        }))
    }

    /// Observes the caller's bounded private saved set in one room.
    pub fn saved_references(
        &self,
        actor: &ParticipantId,
        room_id: &RoomId,
    ) -> Result<Vec<SavedReferenceV1>, PrivateSaveRefusalV1> {
        let room = self.validate_private_save_actor(actor, room_id)?;
        Ok(self
            .private_saves
            .iter()
            .filter_map(|((owner, saved_room_id, message_id), original_sequence)| {
                (owner == actor && saved_room_id == room_id).then_some(SavedReferenceV1 {
                    message_id: *message_id,
                    original_sequence: *original_sequence,
                    availability: stable_reference_availability(room, *message_id),
                })
            })
            .collect())
    }

    /// Returns the bounded shared pin set to one current room member.
    pub fn pinned_references(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
    ) -> Result<Vec<PinnedReferenceV1>, ObservationRefusal> {
        if !self.participants.contains_key(observer) {
            return Err(ObservationRefusal::UnknownParticipant);
        }
        let room = self
            .rooms
            .get(room_id)
            .ok_or(ObservationRefusal::UnknownRoom)?;
        if !room.members.contains_key(observer) {
            return Err(ObservationRefusal::NotMember);
        }
        Ok(room
            .pins
            .iter()
            .map(
                |(message_id, (original_sequence, pinned_by))| PinnedReferenceV1 {
                    message_id: *message_id,
                    original_sequence: *original_sequence,
                    pinned_by: pinned_by.clone(),
                    availability: stable_reference_availability(room, *message_id),
                },
            )
            .collect())
    }

    /// Returns one public poll definition and aggregate to a current member.
    pub fn poll(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
        poll_id: PollId,
    ) -> Result<PollStatusV1, PollObservationRefusalV1> {
        let room = self.validate_poll_observer(observer, room_id)?;
        room.polls
            .get(&poll_id)
            .map(PollV1::status)
            .ok_or(PollObservationRefusalV1::UnknownPoll)
    }

    /// Returns retained polls in creation order without private ballots.
    pub fn polls(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
    ) -> Result<Vec<PollStatusV1>, PollObservationRefusalV1> {
        let room = self.validate_poll_observer(observer, room_id)?;
        Ok(room
            .poll_creation_order
            .iter()
            .map(|poll_id| {
                room.polls
                    .get(poll_id)
                    .expect("poll order and map remain equal")
                    .status()
            })
            .collect())
    }

    /// Returns only the caller's current private ballot.
    pub fn poll_ballot(
        &self,
        actor: &ParticipantId,
        room_id: &RoomId,
        poll_id: PollId,
    ) -> Result<PollBallotV1, PollObservationRefusalV1> {
        let room = self.validate_poll_observer(actor, room_id)?;
        let poll = room
            .polls
            .get(&poll_id)
            .ok_or(PollObservationRefusalV1::UnknownPoll)?;
        if !poll.eligible_participants.contains(actor) {
            return Err(PollObservationRefusalV1::NotEligible);
        }
        Ok(PollBallotV1 {
            poll_id,
            selections: poll
                .ballots
                .get(actor)
                .map(|selections| selections.iter().copied().collect())
                .unwrap_or_default(),
        })
    }

    fn validate_poll_observer(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
    ) -> Result<&Room, PollObservationRefusalV1> {
        if !self.participants.contains_key(observer) {
            return Err(PollObservationRefusalV1::UnknownParticipant);
        }
        let room = self
            .rooms
            .get(room_id)
            .ok_or(PollObservationRefusalV1::UnknownRoom)?;
        if !room.members.contains_key(observer) {
            return Err(PollObservationRefusalV1::NotMember);
        }
        Ok(room)
    }

    /// Changes one caller-owned saved relation without a revision or room event.
    pub fn set_saved_reference(
        &mut self,
        actor: &ParticipantId,
        room_id: &RoomId,
        message_id: MessageId,
        observed_revision: Revision,
        saved: bool,
    ) -> Result<SavedReferenceV1, PrivateSaveRefusalV1> {
        let room = self.validate_private_save_actor(actor, room_id)?;
        if observed_revision != self.revision {
            return Err(PrivateSaveRefusalV1::Stale {
                actual_revision: self.revision,
            });
        }
        let key = (actor.clone(), room_id.clone(), message_id);
        let current = self.private_saves.get(&key).copied();
        if current.is_some() == saved {
            return Err(PrivateSaveRefusalV1::InvalidChange);
        }
        if saved {
            if self.private_saves.len() == MAX_PRIVATE_SAVES_V1 {
                return Err(PrivateSaveRefusalV1::Capacity);
            }
            let sequence = room
                .messages
                .iter()
                .find(|message| message.id == message_id)
                .map(|message| message.sequence)
                .ok_or(PrivateSaveRefusalV1::UnknownMessage)?;
            let availability = stable_reference_availability(room, message_id);
            self.private_saves.insert(key, sequence);
            Ok(SavedReferenceV1 {
                message_id,
                original_sequence: sequence,
                availability,
            })
        } else {
            let original_sequence = current.expect("change validation established saved relation");
            let availability = stable_reference_availability(room, message_id);
            self.private_saves.remove(&key);
            Ok(SavedReferenceV1 {
                message_id,
                original_sequence,
                availability,
            })
        }
    }

    fn validate_private_save_actor(
        &self,
        actor: &ParticipantId,
        room_id: &RoomId,
    ) -> Result<&Room, PrivateSaveRefusalV1> {
        if !self.participants.contains_key(actor) {
            return Err(PrivateSaveRefusalV1::UnknownParticipant);
        }
        let room = self
            .rooms
            .get(room_id)
            .ok_or(PrivateSaveRefusalV1::UnknownRoom)?;
        if !room.members.contains_key(actor) {
            return Err(PrivateSaveRefusalV1::NotMember);
        }
        Ok(room)
    }

    pub fn rooms(&self) -> impl Iterator<Item = RoomView> + '_ {
        self.rooms.values().map(Room::view)
    }
    pub fn room_ids(&self) -> impl ExactSizeIterator<Item = &RoomId> {
        self.rooms.keys()
    }

    /// Returns one revision-bound page of rooms containing `observer`.
    ///
    /// The cursor is intentionally opaque and is valid only for the same
    /// observer at the exact engine revision that produced it.
    pub fn room_page(
        &self,
        observer: &ParticipantId,
        cursor: Option<&RoomPageCursor>,
        limit: usize,
    ) -> Result<RoomPage, ObservationRefusal> {
        if !self.participants.contains_key(observer) {
            return Err(ObservationRefusal::UnknownParticipant);
        }
        if limit == 0 || limit > self.limits.max_rooms {
            return Err(ObservationRefusal::InvalidLimit);
        }
        let after = if let Some(cursor) = cursor {
            if &cursor.observer != observer {
                return Err(ObservationRefusal::ForeignCursor);
            }
            if cursor.revision != self.revision {
                return Err(ObservationRefusal::Stale {
                    actual_revision: self.revision,
                });
            }
            Some(&cursor.after)
        } else {
            None
        };

        let mut rooms = Vec::new();
        rooms
            .try_reserve_exact(limit)
            .map_err(|_| ObservationRefusal::Capacity)?;
        let mut has_more = false;
        for room in self.rooms.values() {
            if after.is_some_and(|after| &room.id <= after) {
                continue;
            }
            let Some(role) = room.members.get(observer).copied() else {
                continue;
            };
            if rooms.len() == limit {
                has_more = true;
                break;
            }
            rooms.push(RoomSummary {
                id: room.id.clone(),
                name: room.name.clone(),
                profile: room.profile,
                admission: room.admission,
                observer_role: role,
                member_count: room.members.len(),
                retained_message_count: room.messages.len(),
                next_message_sequence: room.next_message_sequence,
            });
        }
        let next = if has_more {
            Some(RoomPageCursor {
                revision: self.revision,
                observer: observer.clone(),
                after: rooms
                    .last()
                    .ok_or(ObservationRefusal::InconsistentState)?
                    .id
                    .clone(),
            })
        } else {
            None
        };
        Ok(RoomPage {
            revision: self.revision,
            rooms,
            next,
        })
    }

    /// Returns one revision-bound page of members in canonical participant-ID
    /// order. Only a current room member may observe the page.
    pub fn member_page(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
        cursor: Option<&MemberPageCursor>,
        limit: usize,
    ) -> Result<MemberPage, ObservationRefusal> {
        if !self.participants.contains_key(observer) {
            return Err(ObservationRefusal::UnknownParticipant);
        }
        if limit == 0 || limit > self.limits.max_members_per_room {
            return Err(ObservationRefusal::InvalidLimit);
        }
        let room = self
            .rooms
            .get(room_id)
            .ok_or(ObservationRefusal::UnknownRoom)?;
        if !room.members.contains_key(observer) {
            return Err(ObservationRefusal::NotMember);
        }
        let after = if let Some(cursor) = cursor {
            if &cursor.observer != observer || &cursor.room_id != room_id {
                return Err(ObservationRefusal::ForeignCursor);
            }
            if cursor.revision != self.revision {
                return Err(ObservationRefusal::Stale {
                    actual_revision: self.revision,
                });
            }
            Some(&cursor.after)
        } else {
            None
        };

        let mut members = Vec::new();
        members
            .try_reserve_exact(limit)
            .map_err(|_| ObservationRefusal::Capacity)?;
        let mut has_more = false;
        for (participant_id, role) in &room.members {
            if after.is_some_and(|after| participant_id <= after) {
                continue;
            }
            if members.len() == limit {
                has_more = true;
                break;
            }
            let participant = self
                .participants
                .get(participant_id)
                .ok_or(ObservationRefusal::InconsistentState)?;
            members.push(MemberView {
                participant_id: participant_id.clone(),
                display_name: participant.display_name.clone(),
                kind: participant.kind,
                role: *role,
            });
        }
        let next = if has_more {
            Some(MemberPageCursor {
                revision: self.revision,
                observer: observer.clone(),
                room_id: room_id.clone(),
                after: members
                    .last()
                    .ok_or(ObservationRefusal::InconsistentState)?
                    .participant_id
                    .clone(),
            })
        } else {
            None
        };
        Ok(MemberPage {
            revision: self.revision,
            room_id: room_id.clone(),
            members,
            next,
        })
    }

    /// Returns one revision-bound page of banned participants. Only a current
    /// owner or moderator may observe this authority-sensitive set.
    pub fn ban_page(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
        cursor: Option<&BanPageCursor>,
        limit: usize,
    ) -> Result<BanPage, ObservationRefusal> {
        if !self.participants.contains_key(observer) {
            return Err(ObservationRefusal::UnknownParticipant);
        }
        if limit == 0 || limit > self.limits.max_bans_per_room {
            return Err(ObservationRefusal::InvalidLimit);
        }
        let room = self
            .rooms
            .get(room_id)
            .ok_or(ObservationRefusal::UnknownRoom)?;
        if !matches!(
            room.members.get(observer),
            Some(Role::Owner | Role::Moderator)
        ) {
            return Err(if room.members.contains_key(observer) {
                ObservationRefusal::NotAuthorized
            } else {
                ObservationRefusal::NotMember
            });
        }
        let after = if let Some(cursor) = cursor {
            if &cursor.observer != observer || &cursor.room_id != room_id {
                return Err(ObservationRefusal::ForeignCursor);
            }
            if cursor.revision != self.revision {
                return Err(ObservationRefusal::Stale {
                    actual_revision: self.revision,
                });
            }
            Some(&cursor.after)
        } else {
            None
        };
        let mut banned = Vec::new();
        banned
            .try_reserve_exact(limit)
            .map_err(|_| ObservationRefusal::Capacity)?;
        let mut has_more = false;
        for (participant_id, prior_role) in &room.bans {
            if after.is_some_and(|after| participant_id <= after) {
                continue;
            }
            if banned.len() == limit {
                has_more = true;
                break;
            }
            let participant = self
                .participants
                .get(participant_id)
                .ok_or(ObservationRefusal::InconsistentState)?;
            banned.push(BannedParticipantView {
                participant_id: participant_id.clone(),
                display_name: participant.display_name.clone(),
                kind: participant.kind,
                prior_role: *prior_role,
            });
        }
        let next = if has_more {
            Some(BanPageCursor {
                revision: self.revision,
                observer: observer.clone(),
                room_id: room_id.clone(),
                after: banned
                    .last()
                    .ok_or(ObservationRefusal::InconsistentState)?
                    .participant_id
                    .clone(),
            })
        } else {
            None
        };
        Ok(BanPage {
            revision: self.revision,
            room_id: room_id.clone(),
            banned,
            next,
        })
    }

    /// Returns one revision-bound page of retained messages in sequence order.
    /// The page is a retained suffix observation, not a complete-history claim.
    pub fn message_page(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
        cursor: Option<&MessagePageCursor>,
        limit: usize,
    ) -> Result<MessagePage, ObservationRefusal> {
        if !self.participants.contains_key(observer) {
            return Err(ObservationRefusal::UnknownParticipant);
        }
        if limit == 0 || limit > self.limits.max_messages_per_room {
            return Err(ObservationRefusal::InvalidLimit);
        }
        let room = self
            .rooms
            .get(room_id)
            .ok_or(ObservationRefusal::UnknownRoom)?;
        if !room.members.contains_key(observer) {
            return Err(ObservationRefusal::NotMember);
        }
        let after_sequence = if let Some(cursor) = cursor {
            if &cursor.observer != observer || &cursor.room_id != room_id {
                return Err(ObservationRefusal::ForeignCursor);
            }
            if cursor.revision != self.revision {
                return Err(ObservationRefusal::Stale {
                    actual_revision: self.revision,
                });
            }
            Some(cursor.after_sequence)
        } else {
            None
        };

        let mut messages = Vec::new();
        messages
            .try_reserve_exact(limit)
            .map_err(|_| ObservationRefusal::Capacity)?;
        let mut has_more = false;
        for message in &room.messages {
            if after_sequence.is_some_and(|after| message.sequence <= after) {
                continue;
            }
            if messages.len() == limit {
                has_more = true;
                break;
            }
            messages.push(message.clone());
        }
        let next = if has_more {
            Some(MessagePageCursor {
                revision: self.revision,
                observer: observer.clone(),
                room_id: room_id.clone(),
                after_sequence: messages
                    .last()
                    .ok_or(ObservationRefusal::InconsistentState)?
                    .sequence,
            })
        } else {
            None
        };
        Ok(MessagePage {
            revision: self.revision,
            room_id: room_id.clone(),
            messages,
            oldest_retained_sequence: room
                .next_message_sequence
                .checked_sub(room.messages.len() as u64)
                .ok_or(ObservationRefusal::InconsistentState)?,
            next_message_sequence: room.next_message_sequence,
            next,
        })
    }

    pub fn topic_page(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
        cursor: Option<&TopicPageCursorV1>,
        limit: usize,
    ) -> Result<TopicPageV1, ObservationRefusal> {
        if !self.participants.contains_key(observer) {
            return Err(ObservationRefusal::UnknownParticipant);
        }
        if limit == 0 || limit > self.limits.max_topics_per_room {
            return Err(ObservationRefusal::InvalidLimit);
        }
        let room = self
            .rooms
            .get(room_id)
            .ok_or(ObservationRefusal::UnknownRoom)?;
        if !room.members.contains_key(observer) {
            return Err(ObservationRefusal::NotMember);
        }
        let after = if let Some(cursor) = cursor {
            if &cursor.observer != observer || &cursor.room_id != room_id {
                return Err(ObservationRefusal::ForeignCursor);
            }
            if cursor.revision != self.revision {
                return Err(ObservationRefusal::Stale {
                    actual_revision: self.revision,
                });
            }
            Some(cursor.after)
        } else {
            None
        };
        let mut topics = Vec::new();
        topics
            .try_reserve_exact(limit)
            .map_err(|_| ObservationRefusal::Capacity)?;
        let mut has_more = false;
        for (id, topic) in &room.topics {
            if after.is_some_and(|after| *id <= after) {
                continue;
            }
            if topics.len() == limit {
                has_more = true;
                break;
            }
            topics.push(topic.clone());
        }
        let next = if has_more {
            Some(TopicPageCursorV1 {
                revision: self.revision,
                observer: observer.clone(),
                room_id: room_id.clone(),
                after: topics
                    .last()
                    .ok_or(ObservationRefusal::InconsistentState)?
                    .id,
            })
        } else {
            None
        };
        Ok(TopicPageV1 {
            revision: self.revision,
            room_id: room_id.clone(),
            topics,
            next,
        })
    }

    pub fn topic_collection_page(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
        filter: &TopicCollectionFilterV1,
        cursor: Option<&TopicCollectionCursorV1>,
        limit: usize,
    ) -> Result<TopicCollectionPageV1, ObservationRefusal> {
        if !self.participants.contains_key(observer) {
            return Err(ObservationRefusal::UnknownParticipant);
        }
        if limit == 0 || limit > self.limits.max_topics_per_room {
            return Err(ObservationRefusal::InvalidLimit);
        }
        if filter.tags.len() > MAX_TOPIC_TAGS_PER_TOPIC_V1
            || filter
                .tags
                .iter()
                .any(|tag| tag.is_empty() || tag.len() > self.limits.max_topic_name_bytes)
            || !filter.tags.windows(2).all(|pair| pair[0] < pair[1])
        {
            return Err(ObservationRefusal::InvalidTarget);
        }
        let room = self
            .rooms
            .get(room_id)
            .ok_or(ObservationRefusal::UnknownRoom)?;
        if !room.members.contains_key(observer) {
            return Err(ObservationRefusal::NotMember);
        }
        let after = if let Some(cursor) = cursor {
            if !cursor.is_bound_to(observer, room_id, filter) {
                return Err(ObservationRefusal::ForeignCursor);
            }
            if cursor.revision != self.revision {
                return Err(ObservationRefusal::Stale {
                    actual_revision: self.revision,
                });
            }
            Some(cursor.after)
        } else {
            None
        };
        let mut topics = Vec::new();
        topics
            .try_reserve_exact(limit)
            .map_err(|_| ObservationRefusal::Capacity)?;
        let mut has_more = false;
        for (topic_id, topic) in &room.topics {
            if after.is_some_and(|after| *topic_id <= after)
                || !match filter.lifecycle {
                    TopicCollectionLifecycleV1::Any => true,
                    TopicCollectionLifecycleV1::Active => !topic.resolved,
                    TopicCollectionLifecycleV1::Resolved => topic.resolved,
                }
                || !filter.tags.iter().all(|tag| topic.tags.contains(tag))
            {
                continue;
            }
            if topics.len() == limit {
                has_more = true;
                break;
            }
            topics.push(TopicCollectionEntryV1 {
                topic_id: *topic_id,
                name: topic.name.clone(),
                creator: topic.creator.clone(),
                resolved: topic.resolved,
                aliases: topic.aliases.iter().cloned().collect(),
                tags: topic.tags.iter().cloned().collect(),
                successor_topic_id: room.topic_successors.get(topic_id).copied(),
            });
        }
        let next = if has_more {
            Some(TopicCollectionCursorV1 {
                revision: self.revision,
                observer: observer.clone(),
                room_id: room_id.clone(),
                filter: filter.clone(),
                after: topics
                    .last()
                    .ok_or(ObservationRefusal::InconsistentState)?
                    .topic_id,
            })
        } else {
            None
        };
        Ok(TopicCollectionPageV1 {
            revision: self.revision,
            room_id: room_id.clone(),
            filter: filter.clone(),
            topics,
            next,
        })
    }

    pub fn topic_status(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
        topic_id: TopicId,
    ) -> Result<TopicStatusV1, ObservationRefusal> {
        if !self.participants.contains_key(observer) {
            return Err(ObservationRefusal::UnknownParticipant);
        }
        let room = self
            .rooms
            .get(room_id)
            .ok_or(ObservationRefusal::UnknownRoom)?;
        if !room.members.contains_key(observer) {
            return Err(ObservationRefusal::NotMember);
        }
        let topic = room
            .topics
            .get(&topic_id)
            .ok_or(ObservationRefusal::InvalidTarget)?;
        Ok(TopicStatusV1 {
            revision: self.revision,
            room_id: room_id.clone(),
            topic_id,
            resolved: topic.resolved,
        })
    }

    pub fn topic_supersession_status(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
        topic_id: TopicId,
    ) -> Result<TopicSupersessionStatusV1, ObservationRefusal> {
        if !self.participants.contains_key(observer) {
            return Err(ObservationRefusal::UnknownParticipant);
        }
        let room = self
            .rooms
            .get(room_id)
            .ok_or(ObservationRefusal::UnknownRoom)?;
        if !room.members.contains_key(observer) {
            return Err(ObservationRefusal::NotMember);
        }
        if !room.topics.contains_key(&topic_id) {
            return Err(ObservationRefusal::InvalidTarget);
        }
        Ok(TopicSupersessionStatusV1 {
            revision: self.revision,
            room_id: room_id.clone(),
            topic_id,
            successor_topic_id: room.topic_successors.get(&topic_id).copied(),
        })
    }

    pub fn topic_alias_status(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
        topic_id: TopicId,
    ) -> Result<TopicAliasStatusV1, ObservationRefusal> {
        if !self.participants.contains_key(observer) {
            return Err(ObservationRefusal::UnknownParticipant);
        }
        let room = self
            .rooms
            .get(room_id)
            .ok_or(ObservationRefusal::UnknownRoom)?;
        if !room.members.contains_key(observer) {
            return Err(ObservationRefusal::NotMember);
        }
        let topic = room
            .topics
            .get(&topic_id)
            .ok_or(ObservationRefusal::InvalidTarget)?;
        Ok(TopicAliasStatusV1 {
            revision: self.revision,
            room_id: room_id.clone(),
            topic_id,
            aliases: topic.aliases.iter().cloned().collect(),
        })
    }

    pub fn topic_tag_status(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
        topic_id: TopicId,
    ) -> Result<TopicTagStatusV1, ObservationRefusal> {
        if !self.participants.contains_key(observer) {
            return Err(ObservationRefusal::UnknownParticipant);
        }
        let room = self
            .rooms
            .get(room_id)
            .ok_or(ObservationRefusal::UnknownRoom)?;
        if !room.members.contains_key(observer) {
            return Err(ObservationRefusal::NotMember);
        }
        let topic = room
            .topics
            .get(&topic_id)
            .ok_or(ObservationRefusal::InvalidTarget)?;
        Ok(TopicTagStatusV1 {
            revision: self.revision,
            room_id: room_id.clone(),
            topic_id,
            tags: topic.tags.iter().cloned().collect(),
        })
    }

    pub fn topic_message_page(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
        topic_id: TopicId,
        cursor: Option<&TopicMessagePageCursorV1>,
        limit: usize,
    ) -> Result<TopicMessagePageV1, ObservationRefusal> {
        if !self.participants.contains_key(observer) {
            return Err(ObservationRefusal::UnknownParticipant);
        }
        if limit == 0 || limit > self.limits.max_messages_per_room {
            return Err(ObservationRefusal::InvalidLimit);
        }
        let room = self
            .rooms
            .get(room_id)
            .ok_or(ObservationRefusal::UnknownRoom)?;
        if !room.members.contains_key(observer) {
            return Err(ObservationRefusal::NotMember);
        }
        if !room.topics.contains_key(&topic_id) {
            return Err(ObservationRefusal::InvalidTarget);
        }
        let after = if let Some(cursor) = cursor {
            if &cursor.observer != observer
                || &cursor.room_id != room_id
                || cursor.topic_id != topic_id
            {
                return Err(ObservationRefusal::ForeignCursor);
            }
            if cursor.revision != self.revision {
                return Err(ObservationRefusal::Stale {
                    actual_revision: self.revision,
                });
            }
            Some(cursor.after_sequence)
        } else {
            None
        };
        let mut messages = Vec::new();
        messages
            .try_reserve_exact(limit)
            .map_err(|_| ObservationRefusal::Capacity)?;
        let mut has_more = false;
        for message in &room.messages {
            if message.topic_id != Some(topic_id)
                || after.is_some_and(|after| message.sequence <= after)
            {
                continue;
            }
            if messages.len() == limit {
                has_more = true;
                break;
            }
            messages.push(message.clone());
        }
        let next = if has_more {
            Some(TopicMessagePageCursorV1 {
                revision: self.revision,
                observer: observer.clone(),
                room_id: room_id.clone(),
                topic_id,
                after_sequence: messages
                    .last()
                    .ok_or(ObservationRefusal::InconsistentState)?
                    .sequence,
            })
        } else {
            None
        };
        let oldest_retained_sequence = room
            .next_message_sequence
            .checked_sub(room.messages.len() as u64)
            .ok_or(ObservationRefusal::InconsistentState)?;
        Ok(TopicMessagePageV1 {
            revision: self.revision,
            room_id: room_id.clone(),
            topic_id,
            messages,
            oldest_retained_sequence,
            next_message_sequence: room.next_message_sequence,
            next,
        })
    }

    pub fn message_revision_page(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
        message_id: MessageId,
        cursor: Option<&MessageRevisionPageCursorV1>,
        limit: usize,
    ) -> Result<MessageRevisionPageV1, ObservationRefusal> {
        if !self.participants.contains_key(observer) {
            return Err(ObservationRefusal::UnknownParticipant);
        }
        if limit == 0 || limit > MAX_MESSAGE_REVISIONS_V1 {
            return Err(ObservationRefusal::InvalidLimit);
        }
        let room = self
            .rooms
            .get(room_id)
            .ok_or(ObservationRefusal::UnknownRoom)?;
        if !room.members.contains_key(observer) {
            return Err(ObservationRefusal::NotMember);
        }
        let message = room
            .messages
            .iter()
            .find(|message| message.id == message_id)
            .ok_or(ObservationRefusal::InvalidTarget)?;
        let after = if let Some(cursor) = cursor {
            if !cursor.is_bound_to(observer, room_id, message_id) {
                return Err(ObservationRefusal::ForeignCursor);
            }
            if cursor.revision != self.revision {
                return Err(ObservationRefusal::Stale {
                    actual_revision: self.revision,
                });
            }
            Some(cursor.after_revision)
        } else {
            None
        };
        let start = after.map_or(0, |number| number as usize + 1);
        if start > message.revisions.len() {
            return Err(ObservationRefusal::InconsistentState);
        }
        let end = start.saturating_add(limit).min(message.revisions.len());
        let revisions = message.revisions[start..end].to_vec();
        let next = if end < message.revisions.len() {
            Some(MessageRevisionPageCursorV1 {
                revision: self.revision,
                observer: observer.clone(),
                room_id: room_id.clone(),
                message_id,
                after_revision: revisions
                    .last()
                    .ok_or(ObservationRefusal::InconsistentState)?
                    .number,
            })
        } else {
            None
        };
        Ok(MessageRevisionPageV1 {
            revision: self.revision,
            room_id: room_id.clone(),
            message_id,
            revisions,
            next,
        })
    }

    /// Returns retained root messages in room sequence order.
    pub fn thread_root_page(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
        cursor: Option<&ThreadMessagePageCursor>,
        limit: usize,
    ) -> Result<ThreadMessagePage, ObservationRefusal> {
        self.thread_message_page(
            observer,
            room_id,
            ThreadMessageSelector::Roots,
            cursor,
            limit,
        )
    }

    /// Returns retained one-level replies to one stable root in sequence order.
    pub fn thread_reply_page(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
        root_message_id: MessageId,
        cursor: Option<&ThreadMessagePageCursor>,
        limit: usize,
    ) -> Result<ThreadMessagePage, ObservationRefusal> {
        self.thread_message_page(
            observer,
            room_id,
            ThreadMessageSelector::Replies(root_message_id),
            cursor,
            limit,
        )
    }

    pub fn thread_status(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
        root_message_id: MessageId,
    ) -> Result<ThreadStatusV1, ObservationRefusal> {
        if !self.participants.contains_key(observer) {
            return Err(ObservationRefusal::UnknownParticipant);
        }
        let room = self
            .rooms
            .get(room_id)
            .ok_or(ObservationRefusal::UnknownRoom)?;
        if !room.members.contains_key(observer) {
            return Err(ObservationRefusal::NotMember);
        }
        let root = room
            .messages
            .iter()
            .find(|message| message.id == root_message_id)
            .ok_or(ObservationRefusal::InvalidTarget)?;
        if root.reply_to_root.is_some() {
            return Err(ObservationRefusal::InvalidTarget);
        }
        Ok(ThreadStatusV1 {
            revision: self.revision,
            room_id: room_id.clone(),
            root_message_id,
            state: if room.archived_threads.contains(&root_message_id) {
                ThreadLifecycleStateV1::Archived
            } else {
                ThreadLifecycleStateV1::Active
            },
        })
    }

    pub fn reaction_status(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
        message_id: MessageId,
    ) -> Result<ReactionStatusV1, ObservationRefusal> {
        if !self.participants.contains_key(observer) {
            return Err(ObservationRefusal::UnknownParticipant);
        }
        let room = self
            .rooms
            .get(room_id)
            .ok_or(ObservationRefusal::UnknownRoom)?;
        if !room.members.contains_key(observer) {
            return Err(ObservationRefusal::NotMember);
        }
        if !room.messages.iter().any(|message| message.id == message_id) {
            return Err(ObservationRefusal::InvalidTarget);
        }
        let reactions = room
            .reactions
            .get(&message_id)
            .into_iter()
            .flat_map(|by_key| by_key.iter())
            .map(|(key, participants)| {
                Ok(ReactionSummaryV1 {
                    key: key.clone(),
                    count: participants
                        .len()
                        .try_into()
                        .map_err(|_| ObservationRefusal::InconsistentState)?,
                    caller_reacted: participants.contains(observer),
                })
            })
            .collect::<Result<Vec<_>, ObservationRefusal>>()?;
        Ok(ReactionStatusV1 {
            revision: self.revision,
            room_id: room_id.clone(),
            message_id,
            reactions,
        })
    }

    fn thread_message_page(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
        selector: ThreadMessageSelector,
        cursor: Option<&ThreadMessagePageCursor>,
        limit: usize,
    ) -> Result<ThreadMessagePage, ObservationRefusal> {
        if !self.participants.contains_key(observer) {
            return Err(ObservationRefusal::UnknownParticipant);
        }
        if limit == 0 || limit > self.limits.max_messages_per_room {
            return Err(ObservationRefusal::InvalidLimit);
        }
        let room = self
            .rooms
            .get(room_id)
            .ok_or(ObservationRefusal::UnknownRoom)?;
        if !room.members.contains_key(observer) {
            return Err(ObservationRefusal::NotMember);
        }
        let after_sequence = if let Some(cursor) = cursor {
            if &cursor.observer != observer
                || &cursor.room_id != room_id
                || cursor.selector != selector
            {
                return Err(ObservationRefusal::ForeignCursor);
            }
            if cursor.revision != self.revision {
                return Err(ObservationRefusal::Stale {
                    actual_revision: self.revision,
                });
            }
            Some(cursor.after_sequence)
        } else {
            None
        };

        let matches = |message: &Message| match selector {
            ThreadMessageSelector::Roots => message.reply_to_root.is_none(),
            ThreadMessageSelector::Replies(root) => message.reply_to_root == Some(root),
        };
        let mut messages = Vec::new();
        messages
            .try_reserve_exact(limit)
            .map_err(|_| ObservationRefusal::Capacity)?;
        let mut has_more = false;
        for message in &room.messages {
            if after_sequence.is_some_and(|after| message.sequence <= after) || !matches(message) {
                continue;
            }
            if messages.len() == limit {
                has_more = true;
                break;
            }
            messages.push(message.clone());
        }
        let next = if has_more {
            Some(ThreadMessagePageCursor {
                revision: self.revision,
                observer: observer.clone(),
                room_id: room_id.clone(),
                selector,
                after_sequence: messages
                    .last()
                    .ok_or(ObservationRefusal::InconsistentState)?
                    .sequence,
            })
        } else {
            None
        };
        Ok(ThreadMessagePage {
            revision: self.revision,
            room_id: room_id.clone(),
            root_message_id: match selector {
                ThreadMessageSelector::Roots => None,
                ThreadMessageSelector::Replies(root) => Some(root),
            },
            messages,
            oldest_retained_sequence: room
                .next_message_sequence
                .checked_sub(room.messages.len() as u64)
                .ok_or(ObservationRefusal::InconsistentState)?,
            next_message_sequence: room.next_message_sequence,
            next,
        })
    }

    /// Captures the exact stream position corresponding to a completed room
    /// snapshot at `observed_revision`.
    ///
    /// A mutation between snapshot completion and this call is reported as
    /// stale, allowing the caller to repeat the bounded snapshot without a
    /// silent observation window.
    pub fn room_change_position(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
        observed_revision: Revision,
    ) -> Result<RoomChangePosition, ObservationRefusal> {
        if !self.participants.contains_key(observer) {
            return Err(ObservationRefusal::UnknownParticipant);
        }
        let room = self
            .rooms
            .get(room_id)
            .ok_or(ObservationRefusal::UnknownRoom)?;
        if !room.members.contains_key(observer) {
            return Err(ObservationRefusal::NotMember);
        }
        if observed_revision != self.revision {
            return Err(ObservationRefusal::Stale {
                actual_revision: self.revision,
            });
        }
        Ok(RoomChangePosition {
            observer: observer.clone(),
            room_id: room_id.clone(),
            observed_revision,
            after: self.last_published_event,
        })
    }

    /// Pulls retained room-scoped events strictly after `position`.
    ///
    /// Reusing the same position returns the same retained prefix. Advancing to
    /// `page.next` yields no overlap. Independent bounded change retention may
    /// remove a position's required prefix; that is an explicit `Gap`, never an
    /// empty page that implies continuity. Delivery-outbox acknowledgement has
    /// no effect on this journal.
    pub fn room_change_page(
        &self,
        observer: &ParticipantId,
        room_id: &RoomId,
        position: &RoomChangePosition,
        limit: usize,
    ) -> Result<RoomChangeResult, ObservationRefusal> {
        if !self.participants.contains_key(observer) {
            return Err(ObservationRefusal::UnknownParticipant);
        }
        if limit == 0 || limit > self.limits.max_retained_changes {
            return Err(ObservationRefusal::InvalidLimit);
        }
        let room = self
            .rooms
            .get(room_id)
            .ok_or(ObservationRefusal::UnknownRoom)?;
        if !room.members.contains_key(observer) {
            return Err(ObservationRefusal::NotMember);
        }
        if !position.is_bound_to(observer, room_id) {
            return Err(ObservationRefusal::ForeignCursor);
        }
        let position_is_canonical = match position.after {
            Some(after) => after.0.checked_add(1) == Some(position.observed_revision.0),
            None => position.observed_revision == Revision(0),
        };
        if !position_is_canonical || position.observed_revision > self.revision {
            return Err(ObservationRefusal::InconsistentState);
        }
        if let Some(discarded_through) = self.discarded_changes_through
            && position.after.is_none_or(|after| after < discarded_through)
        {
            return Ok(RoomChangeResult::Gap {
                cause: RoomChangeGap::ChangeRetention { discarded_through },
                current_revision: self.revision,
            });
        }

        let mut changes: Vec<EventEnvelopeV2> = Vec::new();
        changes
            .try_reserve_exact(limit)
            .map_err(|_| ObservationRefusal::Capacity)?;
        for event in &self.retained_changes {
            if position.after.is_some_and(|after| event.sequence <= after) {
                continue;
            }
            if event.event.room_id() != Some(room_id) {
                continue;
            }
            if matches!(
                &event.event,
                DomainEventV2::ParticipantJoined { participant, .. }
                    | DomainEventV2::ParticipantLeft { participant, .. }
                    | DomainEventV2::ParticipantKicked { participant, .. }
                    | DomainEventV2::ParticipantBanned { participant, .. }
                    if participant == observer
            ) {
                return Ok(RoomChangeResult::Gap {
                    cause: RoomChangeGap::MembershipChanged,
                    current_revision: self.revision,
                });
            }
            if changes.len() == limit {
                let last = changes
                    .last()
                    .ok_or(ObservationRefusal::InconsistentState)?;
                let after = last.sequence;
                let observed_revision = last.revision;
                return Ok(RoomChangeResult::Page(RoomChangePage {
                    revision: self.revision,
                    changes,
                    next: RoomChangePosition {
                        observer: observer.clone(),
                        room_id: room_id.clone(),
                        observed_revision,
                        after: Some(after),
                    },
                    has_more: true,
                }));
            }
            changes.push(event.clone());
        }
        Ok(RoomChangeResult::Page(RoomChangePage {
            revision: self.revision,
            changes,
            next: RoomChangePosition {
                observer: observer.clone(),
                room_id: room_id.clone(),
                observed_revision: self.revision,
                after: self.last_published_event,
            },
            has_more: false,
        }))
    }

    pub fn revision(&self) -> Revision {
        self.revision
    }

    pub fn pending_events(&self) -> impl ExactSizeIterator<Item = &EventEnvelopeV2> {
        self.pending_events.iter()
    }

    pub fn acknowledged_through(&self) -> Option<EventSequence> {
        self.acknowledged_through
    }

    pub fn retained_changes(&self) -> impl ExactSizeIterator<Item = &EventEnvelopeV2> {
        self.retained_changes.iter()
    }

    pub fn discarded_changes_through(&self) -> Option<EventSequence> {
        self.discarded_changes_through
    }

    /// Publishes one message under a durable-store-compatible idempotency key.
    /// Matching replay returns the original transition even when its original
    /// optimistic revision is now stale.
    pub fn publish(
        &mut self,
        command: PublicationCommandV2,
    ) -> Result<TransitionV2, PublicationRefusalV2> {
        if let Some(existing) = self.publications.get(&command.key) {
            if existing.room_id == command.room_id
                && existing.author == command.author
                && existing.at == command.at
                && existing.body == command.body
            {
                return Ok(existing.transition.clone());
            }
            return Err(PublicationRefusalV2::IdempotencyConflict);
        }
        if self.publications.len() >= self.limits.max_publication_receipts {
            return Err(PublicationRefusalV2::PublicationReceiptLimit);
        }

        let record = PublicationRecordV2 {
            room_id: command.room_id.clone(),
            author: command.author.clone(),
            at: command.at,
            body: command.body.clone(),
            transition: self.apply_revisioned(RevisionedCommandV2 {
                expected_revision: command.expected_revision,
                command: CommandV2::Speak {
                    room_id: command.room_id,
                    author: command.author,
                    at: command.at,
                    body: command.body,
                },
            })?,
        };
        let transition = record.transition.clone();
        self.publications.insert(command.key, record);
        Ok(transition)
    }

    /// Acknowledges and removes the published prefix through `sequence`.
    /// Repeated acknowledgment of an already removed prefix is an identity.
    pub fn acknowledge_events(
        &mut self,
        sequence: EventSequence,
    ) -> Result<usize, AcknowledgeError> {
        if let Some(acknowledged_through) = self.acknowledged_through
            && sequence < acknowledged_through
        {
            return Err(AcknowledgeError::Regressed {
                requested: sequence,
                acknowledged_through,
            });
        }
        if self.last_published_event.is_none_or(|last| sequence > last) {
            return Err(AcknowledgeError::BeyondPublished {
                requested: sequence,
                last_published: self.last_published_event,
            });
        }
        let mut removed = 0;
        while self
            .pending_events
            .front()
            .is_some_and(|event| event.sequence <= sequence)
        {
            self.pending_events.pop_front();
            removed += 1;
        }
        self.acknowledged_through = Some(sequence);
        Ok(removed)
    }

    /// Applies a command against the current revision.
    ///
    /// Hosts coordinating concurrent writers should use [`Engine::apply_revisioned`]
    /// and persist the returned revision with the accepted event.
    pub fn apply(&mut self, command: CommandV2) -> Result<OutcomeV2, Refusal> {
        self.apply_revisioned(RevisionedCommandV2 {
            expected_revision: self.revision,
            command,
        })
        .map(|transition| transition.outcome)
    }

    /// Applies exactly one optimistic command and publishes exactly one event.
    pub fn apply_revisioned(
        &mut self,
        envelope: RevisionedCommandV2,
    ) -> Result<TransitionV2, Refusal> {
        if envelope.expected_revision != self.revision {
            return Err(Refusal::RevisionConflict {
                expected: envelope.expected_revision,
                actual: self.revision,
            });
        }
        if self.pending_events.len() >= self.limits.max_pending_events {
            return Err(Refusal::EventOutboxFull);
        }
        let next_revision = Revision(
            self.revision
                .0
                .checked_add(1)
                .ok_or(Refusal::RevisionExhausted)?,
        );
        let event_sequence = self.next_event_sequence;
        let next_event_sequence = EventSequence(
            event_sequence
                .0
                .checked_add(1)
                .ok_or(Refusal::EventSequenceExhausted)?,
        );
        let event_source = envelope.command.clone();
        let outcome = self.apply_command(envelope.command)?;
        let event = self.event_for(&event_source, &outcome);

        self.revision = next_revision;
        self.next_event_sequence = next_event_sequence;
        self.last_published_event = Some(event_sequence);
        let event = EventEnvelopeV2 {
            sequence: event_sequence,
            revision: next_revision,
            event,
        };
        self.pending_events.push_back(event.clone());
        self.retained_changes.push_back(event);
        while self.retained_changes.len() > self.limits.max_retained_changes {
            let discarded = self
                .retained_changes
                .pop_front()
                .expect("retained change length exceeded its limit");
            self.discarded_changes_through = Some(discarded.sequence);
        }
        Ok(TransitionV2 {
            revision: next_revision,
            event_sequence,
            outcome,
        })
    }

    fn apply_command(&mut self, command: CommandV2) -> Result<OutcomeV2, Refusal> {
        match command {
            CommandV2::RegisterParticipant(participant) => self.register(participant),
            CommandV2::CreateRoom {
                room_id,
                name,
                owner,
                admission,
            } => self.create_room(room_id, name, owner, admission),
            CommandV2::CreatePeerConversation {
                room_id,
                name,
                initiator,
                profile,
                founding_members,
            } => self.create_peer_conversation(room_id, name, initiator, profile, founding_members),
            CommandV2::Invite {
                room_id,
                actor,
                participant,
            } => self.invite(room_id, actor, participant),
            CommandV2::Join {
                room_id,
                participant,
            } => self.join(room_id, participant),
            CommandV2::Leave {
                room_id,
                participant,
            } => self.leave(room_id, participant),
            CommandV2::SetRole {
                room_id,
                actor,
                participant,
                role,
            } => self.set_role(room_id, actor, participant, role),
            CommandV2::SetAdmissionPolicy {
                room_id,
                actor,
                admission,
            } => self.set_admission_policy(room_id, actor, admission),
            CommandV2::Kick {
                room_id,
                actor,
                participant,
                reason,
                causation,
            } => self.kick(room_id, actor, participant, reason, causation),
            CommandV2::Ban {
                room_id,
                actor,
                participant,
                reason,
                causation,
            } => self.ban(room_id, actor, participant, reason, causation),
            CommandV2::Unban {
                room_id,
                actor,
                participant,
                reason,
                causation,
            } => self.unban(room_id, actor, participant, reason, causation),
            CommandV2::Speak {
                room_id,
                author,
                at,
                body,
            } => self.speak(MessageAdmission {
                room_id,
                author,
                at,
                body,
                addressed_to: Vec::new(),
                placement: MessagePlacement::Unscoped,
                attachments: None,
                vellum_artifacts: None,
            }),
            CommandV2::SpeakAddressed {
                room_id,
                author,
                at,
                body,
                addressed_to,
            } => self.speak(MessageAdmission {
                room_id,
                author,
                at,
                body,
                addressed_to,
                placement: MessagePlacement::Unscoped,
                attachments: None,
                vellum_artifacts: None,
            }),
            CommandV2::SpeakReply {
                room_id,
                author,
                root_message_id,
                at,
                body,
                addressed_to,
            } => self.speak(MessageAdmission {
                room_id,
                author,
                at,
                body,
                addressed_to,
                placement: MessagePlacement::Reply(root_message_id),
                attachments: None,
                vellum_artifacts: None,
            }),
            CommandV2::CreateTopic {
                room_id,
                actor,
                name,
            } => self.create_topic(room_id, actor, name),
            CommandV2::RenameTopic {
                room_id,
                actor,
                topic_id,
                name,
            } => self.rename_topic(room_id, actor, topic_id, name),
            CommandV2::SetTopicResolved {
                room_id,
                actor,
                topic_id,
                resolved,
            } => self.set_topic_resolved(room_id, actor, topic_id, resolved),
            CommandV2::SetTopicSuccessor {
                room_id,
                actor,
                topic_id,
                successor_topic_id,
            } => self.set_topic_successor(room_id, actor, topic_id, successor_topic_id),
            CommandV2::SetTopicAlias {
                room_id,
                actor,
                topic_id,
                alias,
                added,
            } => self.set_topic_alias(room_id, actor, topic_id, alias, added),
            CommandV2::SetTopicTag {
                room_id,
                actor,
                topic_id,
                tag,
                added,
            } => self.set_topic_tag(room_id, actor, topic_id, tag, added),
            CommandV2::SpeakInTopic {
                room_id,
                author,
                topic_id,
                at,
                body,
                addressed_to,
            } => self.speak(MessageAdmission {
                room_id,
                author,
                at,
                body,
                addressed_to,
                placement: MessagePlacement::Topic(topic_id),
                attachments: None,
                vellum_artifacts: None,
            }),
            CommandV2::SpeakWithAttachments {
                room_id,
                author,
                target,
                at,
                body,
                addressed_to,
                attachments,
            } => self.speak(MessageAdmission {
                room_id,
                author,
                at,
                body,
                addressed_to,
                placement: match target {
                    AttachmentMessageTargetV1::Room => MessagePlacement::Unscoped,
                    AttachmentMessageTargetV1::Reply(message_id) => {
                        MessagePlacement::Reply(message_id)
                    }
                    AttachmentMessageTargetV1::Topic(topic_id) => MessagePlacement::Topic(topic_id),
                },
                attachments: Some(attachments),
                vellum_artifacts: None,
            }),
            CommandV2::SpeakWithVellumArtifacts {
                room_id,
                author,
                target,
                at,
                body,
                addressed_to,
                references,
            } => self.speak(MessageAdmission {
                room_id,
                author,
                at,
                body,
                addressed_to,
                placement: match target {
                    VellumArtifactMessageTargetV1::Room => MessagePlacement::Unscoped,
                    VellumArtifactMessageTargetV1::Reply(message_id) => {
                        MessagePlacement::Reply(message_id)
                    }
                    VellumArtifactMessageTargetV1::Topic(topic_id) => {
                        MessagePlacement::Topic(topic_id)
                    }
                },
                attachments: None,
                vellum_artifacts: Some(references),
            }),
            CommandV2::WithdrawMessage {
                room_id,
                actor,
                message_id,
            } => self.withdraw_message(room_id, actor, message_id),
            CommandV2::RedactMessage {
                room_id,
                actor,
                message_id,
                reason,
                causation,
            } => self.redact_message(room_id, actor, message_id, reason, causation),
            CommandV2::SetThreadArchived {
                room_id,
                actor,
                root_message_id,
                archived,
                reason,
                causation,
            } => self.set_thread_archived(
                room_id,
                actor,
                root_message_id,
                archived,
                reason,
                causation,
            ),
            CommandV2::SetReaction {
                room_id,
                actor,
                message_id,
                key,
                reacted,
            } => self.set_reaction(room_id, actor, message_id, key, reacted),
            CommandV2::SetPin {
                room_id,
                actor,
                message_id,
                pinned,
            } => self.set_pin(room_id, actor, message_id, pinned),
            CommandV2::CreatePoll {
                room_id,
                actor,
                prompt,
                options,
                eligible_participants,
                max_choices,
            } => self.create_poll(
                room_id,
                actor,
                prompt,
                options,
                eligible_participants,
                max_choices,
            ),
            CommandV2::SetPollBallot {
                room_id,
                actor,
                poll_id,
                selections,
            } => self.set_poll_ballot(room_id, actor, poll_id, selections),
            CommandV2::SetPollOpen {
                room_id,
                actor,
                poll_id,
                open,
            } => self.set_poll_open(room_id, actor, poll_id, open),
            CommandV2::EditMessage {
                room_id,
                actor,
                message_id,
                at,
                body,
            } => self.edit_message(room_id, actor, message_id, at, body),
        }
    }

    fn event_for(&self, command: &CommandV2, outcome: &OutcomeV2) -> DomainEventV2 {
        match (command, outcome) {
            (
                CommandV2::RegisterParticipant(participant),
                OutcomeV2::ParticipantRegistered(participant_id),
            ) => DomainEventV2::ParticipantRegistered {
                participant_id: participant_id.clone(),
                kind: participant.kind,
            },
            (
                CommandV2::CreateRoom {
                    owner, admission, ..
                },
                OutcomeV2::RoomCreated(room_id),
            ) => DomainEventV2::RoomCreated {
                room_id: room_id.clone(),
                owner: owner.clone(),
                admission: *admission,
            },
            (
                CommandV2::CreatePeerConversation {
                    initiator,
                    founding_members,
                    ..
                },
                OutcomeV2::PeerConversationCreated { room_id, profile },
            ) => DomainEventV2::PeerConversationCreated {
                room_id: room_id.clone(),
                initiator: initiator.clone(),
                profile: *profile,
                founding_members: founding_members.clone(),
            },
            (
                CommandV2::Invite { .. },
                OutcomeV2::InvitationRecorded {
                    room_id,
                    participant,
                },
            ) => DomainEventV2::InvitationRecorded {
                room_id: room_id.clone(),
                participant: participant.clone(),
            },
            (
                CommandV2::CreateTopic { .. },
                OutcomeV2::TopicCreated {
                    room_id,
                    actor,
                    topic_id,
                    name,
                },
            ) => DomainEventV2::TopicCreated {
                room_id: room_id.clone(),
                actor: actor.clone(),
                topic_id: *topic_id,
                name: name.clone(),
            },
            (
                CommandV2::RenameTopic { .. },
                OutcomeV2::TopicRenamed {
                    room_id,
                    actor,
                    topic_id,
                    previous,
                    name,
                },
            ) => DomainEventV2::TopicRenamed {
                room_id: room_id.clone(),
                actor: actor.clone(),
                topic_id: *topic_id,
                previous: previous.clone(),
                name: name.clone(),
            },
            (
                CommandV2::SetTopicResolved { .. },
                OutcomeV2::TopicLifecycleChanged {
                    room_id,
                    actor,
                    topic_id,
                    previous_resolved,
                    resolved,
                },
            ) => DomainEventV2::TopicLifecycleChanged {
                room_id: room_id.clone(),
                actor: actor.clone(),
                topic_id: *topic_id,
                previous_resolved: *previous_resolved,
                resolved: *resolved,
            },
            (
                CommandV2::SetTopicSuccessor { .. },
                OutcomeV2::TopicSupersessionChanged {
                    room_id,
                    actor,
                    topic_id,
                    previous_successor_topic_id,
                    successor_topic_id,
                },
            ) => DomainEventV2::TopicSupersessionChanged {
                room_id: room_id.clone(),
                actor: actor.clone(),
                topic_id: *topic_id,
                previous_successor_topic_id: *previous_successor_topic_id,
                successor_topic_id: *successor_topic_id,
            },
            (
                CommandV2::SetTopicAlias { .. },
                OutcomeV2::TopicAliasChanged {
                    room_id,
                    actor,
                    topic_id,
                    alias,
                    added,
                },
            ) => DomainEventV2::TopicAliasChanged {
                room_id: room_id.clone(),
                actor: actor.clone(),
                topic_id: *topic_id,
                alias: alias.clone(),
                added: *added,
            },
            (
                CommandV2::SetTopicTag { .. },
                OutcomeV2::TopicTagChanged {
                    room_id,
                    actor,
                    topic_id,
                    tag,
                    added,
                },
            ) => DomainEventV2::TopicTagChanged {
                room_id: room_id.clone(),
                actor: actor.clone(),
                topic_id: *topic_id,
                tag: tag.clone(),
                added: *added,
            },
            (
                CommandV2::Join { .. },
                OutcomeV2::Joined {
                    room_id,
                    participant,
                },
            ) => DomainEventV2::ParticipantJoined {
                room_id: room_id.clone(),
                participant: participant.clone(),
            },
            (
                CommandV2::Leave { .. },
                OutcomeV2::Left {
                    room_id,
                    participant,
                },
            ) => DomainEventV2::ParticipantLeft {
                room_id: room_id.clone(),
                participant: participant.clone(),
            },
            (
                CommandV2::SetRole { .. },
                OutcomeV2::RoleChanged {
                    room_id,
                    participant,
                    role,
                },
            ) => DomainEventV2::RoleChanged {
                room_id: room_id.clone(),
                participant: participant.clone(),
                role: *role,
            },
            (
                CommandV2::SetAdmissionPolicy { .. },
                OutcomeV2::RoomPolicyChanged {
                    room_id,
                    actor,
                    previous,
                    admission,
                },
            ) => DomainEventV2::RoomPolicyChanged {
                room_id: room_id.clone(),
                actor: actor.clone(),
                previous: *previous,
                admission: *admission,
            },
            (
                CommandV2::Kick { .. },
                OutcomeV2::Kicked {
                    room_id,
                    actor,
                    participant,
                    reason,
                    causation,
                },
            ) => DomainEventV2::ParticipantKicked {
                room_id: room_id.clone(),
                actor: actor.clone(),
                participant: participant.clone(),
                reason: reason.clone(),
                causation: *causation,
            },
            (
                CommandV2::Ban { .. },
                OutcomeV2::Banned {
                    room_id,
                    actor,
                    participant,
                    reason,
                    causation,
                },
            ) => DomainEventV2::ParticipantBanned {
                room_id: room_id.clone(),
                actor: actor.clone(),
                participant: participant.clone(),
                reason: reason.clone(),
                causation: *causation,
            },
            (
                CommandV2::Unban { .. },
                OutcomeV2::Unbanned {
                    room_id,
                    actor,
                    participant,
                    reason,
                    causation,
                },
            ) => DomainEventV2::ParticipantUnbanned {
                room_id: room_id.clone(),
                actor: actor.clone(),
                participant: participant.clone(),
                reason: reason.clone(),
                causation: *causation,
            },
            (CommandV2::Speak { room_id, .. }, OutcomeV2::MessageAccepted { sequence, .. }) => {
                DomainEventV2::MessageAccepted {
                    room_id: room_id.clone(),
                    message: self
                        .message(room_id, *sequence)
                        .expect("accepted message remains retained")
                        .clone(),
                }
            }
            (
                CommandV2::SpeakAddressed { room_id, .. },
                OutcomeV2::MessageAccepted { sequence, .. },
            ) => DomainEventV2::MessageAccepted {
                room_id: room_id.clone(),
                message: self
                    .message(room_id, *sequence)
                    .expect("accepted message remains retained")
                    .clone(),
            },
            (
                CommandV2::SpeakWithVellumArtifacts { room_id, .. },
                OutcomeV2::VellumArtifactMessageAccepted { sequence, .. },
            ) => DomainEventV2::MessageAccepted {
                room_id: room_id.clone(),
                message: self
                    .message(room_id, *sequence)
                    .expect("accepted Vellum artifact message remains retained")
                    .clone(),
            },
            (
                CommandV2::SpeakInTopic { room_id, .. },
                OutcomeV2::TopicMessageAccepted { sequence, .. },
            ) => DomainEventV2::MessageAccepted {
                room_id: room_id.clone(),
                message: self
                    .message(room_id, *sequence)
                    .expect("accepted topic message remains retained")
                    .clone(),
            },
            (
                CommandV2::SpeakReply { .. },
                OutcomeV2::ReplyAccepted {
                    room_id, sequence, ..
                },
            ) => DomainEventV2::MessageAccepted {
                room_id: room_id.clone(),
                message: self
                    .message(room_id, *sequence)
                    .expect("accepted reply remains retained")
                    .clone(),
            },
            (
                CommandV2::SpeakWithAttachments { room_id, .. },
                OutcomeV2::AttachmentMessageAccepted { sequence, .. },
            ) => DomainEventV2::MessageAccepted {
                room_id: room_id.clone(),
                message: self
                    .message(room_id, *sequence)
                    .expect("accepted attachment message remains retained")
                    .clone(),
            },
            (
                CommandV2::WithdrawMessage { .. },
                OutcomeV2::MessageWithdrawn {
                    room_id,
                    actor,
                    message_id,
                    sequence,
                },
            ) => DomainEventV2::MessageWithdrawn {
                room_id: room_id.clone(),
                actor: actor.clone(),
                message_id: *message_id,
                sequence: *sequence,
            },
            (
                CommandV2::RedactMessage { .. },
                OutcomeV2::MessageRedacted {
                    room_id,
                    actor,
                    author,
                    message_id,
                    sequence,
                    reason,
                    causation,
                },
            ) => DomainEventV2::MessageRedacted {
                room_id: room_id.clone(),
                actor: actor.clone(),
                author: author.clone(),
                message_id: *message_id,
                sequence: *sequence,
                reason: reason.clone(),
                causation: *causation,
            },
            (
                CommandV2::SetThreadArchived { .. },
                OutcomeV2::ThreadLifecycleChanged {
                    room_id,
                    actor,
                    root_message_id,
                    archived,
                    reason,
                    causation,
                },
            ) => DomainEventV2::ThreadLifecycleChanged {
                room_id: room_id.clone(),
                actor: actor.clone(),
                root_message_id: *root_message_id,
                archived: *archived,
                reason: reason.clone(),
                causation: *causation,
            },
            (
                CommandV2::SetReaction { .. },
                OutcomeV2::ReactionChanged {
                    room_id,
                    actor,
                    message_id,
                    key,
                    reacted,
                },
            ) => DomainEventV2::ReactionChanged {
                room_id: room_id.clone(),
                actor: actor.clone(),
                message_id: *message_id,
                key: key.clone(),
                reacted: *reacted,
            },
            (
                CommandV2::SetPin { .. },
                OutcomeV2::PinChanged {
                    room_id,
                    actor,
                    message_id,
                    original_sequence,
                    pinned,
                },
            ) => DomainEventV2::PinChanged {
                room_id: room_id.clone(),
                actor: actor.clone(),
                message_id: *message_id,
                original_sequence: *original_sequence,
                pinned: *pinned,
            },
            (
                CommandV2::CreatePoll {
                    actor,
                    prompt,
                    options,
                    eligible_participants,
                    max_choices,
                    ..
                },
                OutcomeV2::PollCreated { room_id, poll_id },
            ) => DomainEventV2::PollCreated {
                room_id: room_id.clone(),
                actor: actor.clone(),
                poll_id: *poll_id,
                prompt: prompt.clone(),
                options: options.clone(),
                eligible_participants: eligible_participants.clone(),
                max_choices: *max_choices,
            },
            (
                CommandV2::SetPollBallot { .. },
                OutcomeV2::PollBallotChanged {
                    room_id,
                    poll_id,
                    totals,
                    total_ballots,
                },
            ) => DomainEventV2::PollBallotChanged {
                room_id: room_id.clone(),
                poll_id: *poll_id,
                totals: totals.clone(),
                total_ballots: *total_ballots,
            },
            (
                CommandV2::SetPollOpen { .. },
                OutcomeV2::PollLifecycleChanged {
                    room_id,
                    actor,
                    poll_id,
                    open,
                },
            ) => DomainEventV2::PollLifecycleChanged {
                room_id: room_id.clone(),
                actor: actor.clone(),
                poll_id: *poll_id,
                open: *open,
            },
            (
                CommandV2::EditMessage { .. },
                OutcomeV2::MessageEdited {
                    room_id,
                    actor,
                    message_id,
                    message_revision,
                    at,
                    body,
                },
            ) => DomainEventV2::MessageEdited {
                room_id: room_id.clone(),
                actor: actor.clone(),
                message_id: *message_id,
                message_revision: *message_revision,
                at: *at,
                body: body.clone(),
            },
            _ => unreachable!("command and outcome are produced by one exhaustive transition"),
        }
    }

    fn register(&mut self, participant: Participant) -> Result<OutcomeV2, Refusal> {
        self.validate_nonempty_bounded(
            participant.id.as_str(),
            self.limits.max_participant_id_bytes,
            Refusal::InvalidParticipantId,
        )?;
        self.validate_nonempty_bounded(
            &participant.display_name,
            self.limits.max_display_name_bytes,
            Refusal::InvalidDisplayName,
        )?;
        if self.participants.contains_key(&participant.id) {
            return Err(Refusal::DuplicateParticipant(participant.id));
        }
        if self.participants.len() >= self.limits.max_participants {
            return Err(Refusal::ParticipantLimit);
        }
        let id = participant.id.clone();
        self.participants.insert(id.clone(), participant);
        Ok(OutcomeV2::ParticipantRegistered(id))
    }

    fn create_room(
        &mut self,
        room_id: RoomId,
        name: String,
        owner: ParticipantId,
        admission: AdmissionPolicy,
    ) -> Result<OutcomeV2, Refusal> {
        self.validate_nonempty_bounded(
            room_id.as_str(),
            self.limits.max_room_id_bytes,
            Refusal::InvalidRoomId,
        )?;
        self.validate_nonempty_bounded(
            &name,
            self.limits.max_room_name_bytes,
            Refusal::InvalidRoomName,
        )?;
        if !self.participants.contains_key(&owner) {
            return Err(Refusal::UnknownParticipant(owner));
        }
        if self.rooms.contains_key(&room_id) {
            return Err(Refusal::DuplicateRoom(room_id));
        }
        if self.rooms.len() >= self.limits.max_rooms {
            return Err(Refusal::RoomLimit);
        }
        if self.limits.max_members_per_room == 0 {
            return Err(Refusal::MemberLimit);
        }
        let mut members = BTreeMap::new();
        members.insert(owner, Role::Owner);
        self.rooms.insert(
            room_id.clone(),
            Room {
                id: room_id.clone(),
                name,
                profile: ConversationProfileV1::Ordinary,
                admission,
                founding_members: BTreeSet::new(),
                members,
                invitations: BTreeSet::new(),
                bans: BTreeMap::new(),
                messages: VecDeque::new(),
                thread_roots: BTreeSet::new(),
                archived_threads: BTreeSet::new(),
                reactions: BTreeMap::new(),
                pins: BTreeMap::new(),
                polls: BTreeMap::new(),
                poll_creation_order: VecDeque::new(),
                topics: BTreeMap::new(),
                topic_successors: BTreeMap::new(),
                topic_identity_incarnation: self.next_event_sequence.0,
                next_topic_identity: 0,
                poll_identity_incarnation: self.next_event_sequence.0,
                next_poll_identity: 0,
                message_identity_incarnation: self.next_event_sequence.0,
                next_message_identity: 0,
                next_message_sequence: 0,
            },
        );
        Ok(OutcomeV2::RoomCreated(room_id))
    }

    fn create_peer_conversation(
        &mut self,
        room_id: RoomId,
        name: String,
        initiator: ParticipantId,
        profile: ConversationProfileV1,
        founding_members: Vec<ParticipantId>,
    ) -> Result<OutcomeV2, Refusal> {
        self.validate_nonempty_bounded(
            room_id.as_str(),
            self.limits.max_room_id_bytes,
            Refusal::InvalidRoomId,
        )?;
        self.validate_nonempty_bounded(
            &name,
            self.limits.max_room_name_bytes,
            Refusal::InvalidRoomName,
        )?;
        let valid_cardinality = match profile {
            ConversationProfileV1::Ordinary => return Err(Refusal::InvalidConversationProfile),
            ConversationProfileV1::Direct => founding_members.len() == 2,
            ConversationProfileV1::GroupDirect => founding_members.len() >= 3,
        };
        if !valid_cardinality
            || founding_members.len() > self.limits.max_members_per_room
            || founding_members.windows(2).any(|pair| pair[0] >= pair[1])
            || founding_members.binary_search(&initiator).is_err()
        {
            return Err(Refusal::InvalidConversationCohort);
        }
        for participant in &founding_members {
            if !self.participants.contains_key(participant) {
                return Err(Refusal::UnknownParticipant(participant.clone()));
            }
        }
        if self.rooms.contains_key(&room_id) {
            return Err(Refusal::DuplicateRoom(room_id));
        }
        if self.rooms.len() >= self.limits.max_rooms {
            return Err(Refusal::RoomLimit);
        }

        let founding_members: BTreeSet<_> = founding_members.into_iter().collect();
        let members = founding_members
            .iter()
            .cloned()
            .map(|participant| (participant, Role::Member))
            .collect();
        self.rooms.insert(
            room_id.clone(),
            Room {
                id: room_id.clone(),
                name,
                profile,
                admission: AdmissionPolicy::InviteOnly,
                founding_members,
                members,
                invitations: BTreeSet::new(),
                bans: BTreeMap::new(),
                messages: VecDeque::new(),
                thread_roots: BTreeSet::new(),
                archived_threads: BTreeSet::new(),
                reactions: BTreeMap::new(),
                pins: BTreeMap::new(),
                polls: BTreeMap::new(),
                poll_creation_order: VecDeque::new(),
                topics: BTreeMap::new(),
                topic_successors: BTreeMap::new(),
                topic_identity_incarnation: self.next_event_sequence.0,
                next_topic_identity: 0,
                poll_identity_incarnation: self.next_event_sequence.0,
                next_poll_identity: 0,
                message_identity_incarnation: self.next_event_sequence.0,
                next_message_identity: 0,
                next_message_sequence: 0,
            },
        );
        Ok(OutcomeV2::PeerConversationCreated { room_id, profile })
    }

    fn invite(
        &mut self,
        room_id: RoomId,
        actor: ParticipantId,
        participant: ParticipantId,
    ) -> Result<OutcomeV2, Refusal> {
        if !self.participants.contains_key(&participant) {
            return Err(Refusal::UnknownParticipant(participant));
        }
        let room = self.room_ref(&room_id)?;
        if room.profile != ConversationProfileV1::Ordinary {
            return Err(Refusal::ImmutableConversationCohort);
        }
        if !matches!(
            room.members.get(&actor),
            Some(Role::Owner | Role::Moderator)
        ) {
            return Err(Refusal::NotAuthorized(actor));
        }
        if room.members.contains_key(&participant) {
            return Err(Refusal::AlreadyMember(participant));
        }
        if room.bans.contains_key(&participant) {
            return Err(Refusal::ParticipantBanned(participant));
        }
        if !room.invitations.contains(&participant)
            && room.invitations.len() >= self.limits.max_invitations_per_room
        {
            return Err(Refusal::InvitationLimit);
        }
        self.rooms
            .get_mut(&room_id)
            .expect("room established by validation")
            .invitations
            .insert(participant.clone());
        Ok(OutcomeV2::InvitationRecorded {
            room_id,
            participant,
        })
    }

    fn join(&mut self, room_id: RoomId, participant: ParticipantId) -> Result<OutcomeV2, Refusal> {
        if !self.participants.contains_key(&participant) {
            return Err(Refusal::UnknownParticipant(participant));
        }
        let room = self.room_ref(&room_id)?;
        if room.profile != ConversationProfileV1::Ordinary {
            return Err(Refusal::ImmutableConversationCohort);
        }
        if room.members.contains_key(&participant) {
            return Err(Refusal::AlreadyMember(participant));
        }
        if room.bans.contains_key(&participant) {
            return Err(Refusal::ParticipantBanned(participant));
        }
        if room.members.len() >= self.limits.max_members_per_room {
            return Err(Refusal::MemberLimit);
        }
        if room.admission == AdmissionPolicy::InviteOnly && !room.invitations.contains(&participant)
        {
            return Err(Refusal::InvitationRequired(participant));
        }
        let room = self
            .rooms
            .get_mut(&room_id)
            .expect("room established by validation");
        room.invitations.remove(&participant);
        room.members.insert(participant.clone(), Role::Member);
        Ok(OutcomeV2::Joined {
            room_id,
            participant,
        })
    }

    fn leave(&mut self, room_id: RoomId, participant: ParticipantId) -> Result<OutcomeV2, Refusal> {
        let room = self.room_ref(&room_id)?;
        let Some(role) = room.members.get(&participant) else {
            return Err(Refusal::NotMember(participant));
        };
        if *role == Role::Owner {
            return Err(Refusal::OwnerCannotLeave);
        }
        self.rooms
            .get_mut(&room_id)
            .expect("room established by validation")
            .members
            .remove(&participant);
        Ok(OutcomeV2::Left {
            room_id,
            participant,
        })
    }

    fn set_role(
        &mut self,
        room_id: RoomId,
        actor: ParticipantId,
        participant: ParticipantId,
        role: Role,
    ) -> Result<OutcomeV2, Refusal> {
        let room = self.room_ref(&room_id)?;
        if room.profile != ConversationProfileV1::Ordinary {
            return Err(Refusal::ImmutableConversationCohort);
        }
        if room.members.get(&actor) != Some(&Role::Owner) {
            return Err(Refusal::NotAuthorized(actor));
        }
        let Some(current) = room.members.get(&participant) else {
            return Err(Refusal::NotMember(participant));
        };
        if *current == Role::Owner || role == Role::Owner {
            return Err(Refusal::InvalidRoleChange);
        }
        self.rooms
            .get_mut(&room_id)
            .expect("room established by validation")
            .members
            .insert(participant.clone(), role);
        Ok(OutcomeV2::RoleChanged {
            room_id,
            participant,
            role,
        })
    }

    fn set_admission_policy(
        &mut self,
        room_id: RoomId,
        actor: ParticipantId,
        admission: AdmissionPolicy,
    ) -> Result<OutcomeV2, Refusal> {
        let room = self.room_ref(&room_id)?;
        if room.profile != ConversationProfileV1::Ordinary {
            return Err(Refusal::ImmutableConversationCohort);
        }
        if room.members.get(&actor) != Some(&Role::Owner) {
            return Err(Refusal::NotAuthorized(actor));
        }
        let previous = room.admission;
        if previous == admission {
            return Err(Refusal::InvalidPolicyChange);
        }
        self.rooms
            .get_mut(&room_id)
            .expect("room established by validation")
            .admission = admission;
        Ok(OutcomeV2::RoomPolicyChanged {
            room_id,
            actor,
            previous,
            admission,
        })
    }

    fn validate_moderation_reason(&self, reason: &ModerationReasonV1) -> Result<(), Refusal> {
        self.validate_nonempty_bounded(
            reason.as_str(),
            self.limits.max_moderation_reason_bytes,
            Refusal::InvalidModerationReason,
        )
    }

    fn moderation_authorized(
        room: &Room,
        actor: &ParticipantId,
        participant: &ParticipantId,
    ) -> bool {
        if actor == participant {
            return false;
        }
        match room.members.get(actor) {
            Some(Role::Owner) => room.members.get(participant) != Some(&Role::Owner),
            Some(Role::Moderator) => !matches!(
                room.members.get(participant),
                Some(Role::Owner | Role::Moderator)
            ),
            _ => false,
        }
    }

    fn kick(
        &mut self,
        room_id: RoomId,
        actor: ParticipantId,
        participant: ParticipantId,
        reason: ModerationReasonV1,
        causation: ModerationCausationV1,
    ) -> Result<OutcomeV2, Refusal> {
        self.validate_moderation_reason(&reason)?;
        let room = self.room_ref(&room_id)?;
        if room.profile != ConversationProfileV1::Ordinary {
            return Err(Refusal::ImmutableConversationCohort);
        }
        if !Self::moderation_authorized(room, &actor, &participant) {
            return Err(Refusal::NotAuthorized(actor));
        }
        if !room.members.contains_key(&participant) {
            return Err(Refusal::NotMember(participant));
        }
        let room = self
            .rooms
            .get_mut(&room_id)
            .expect("room established by validation");
        room.members.remove(&participant);
        room.invitations.remove(&participant);
        Ok(OutcomeV2::Kicked {
            room_id,
            actor,
            participant,
            reason,
            causation,
        })
    }

    fn ban(
        &mut self,
        room_id: RoomId,
        actor: ParticipantId,
        participant: ParticipantId,
        reason: ModerationReasonV1,
        causation: ModerationCausationV1,
    ) -> Result<OutcomeV2, Refusal> {
        self.validate_moderation_reason(&reason)?;
        if !self.participants.contains_key(&participant) {
            return Err(Refusal::UnknownParticipant(participant));
        }
        let room = self.room_ref(&room_id)?;
        if room.profile != ConversationProfileV1::Ordinary {
            return Err(Refusal::ImmutableConversationCohort);
        }
        if !Self::moderation_authorized(room, &actor, &participant) {
            return Err(Refusal::NotAuthorized(actor));
        }
        if room.bans.contains_key(&participant) {
            return Err(Refusal::ParticipantBanned(participant));
        }
        if room.bans.len() >= self.limits.max_bans_per_room {
            return Err(Refusal::BanLimit);
        }
        let room = self
            .rooms
            .get_mut(&room_id)
            .expect("room established by validation");
        let prior_role = room
            .members
            .get(&participant)
            .copied()
            .unwrap_or(Role::Member);
        room.members.remove(&participant);
        room.invitations.remove(&participant);
        room.bans.insert(participant.clone(), prior_role);
        Ok(OutcomeV2::Banned {
            room_id,
            actor,
            participant,
            reason,
            causation,
        })
    }

    fn unban(
        &mut self,
        room_id: RoomId,
        actor: ParticipantId,
        participant: ParticipantId,
        reason: ModerationReasonV1,
        causation: ModerationCausationV1,
    ) -> Result<OutcomeV2, Refusal> {
        self.validate_moderation_reason(&reason)?;
        let room = self.room_ref(&room_id)?;
        if room.profile != ConversationProfileV1::Ordinary {
            return Err(Refusal::ImmutableConversationCohort);
        }
        let Some(prior_role) = room.bans.get(&participant).copied() else {
            return Err(Refusal::NotBanned(participant));
        };
        let authorized = actor != participant
            && match room.members.get(&actor) {
                Some(Role::Owner) => prior_role != Role::Owner,
                Some(Role::Moderator) => prior_role == Role::Member,
                _ => false,
            };
        if !authorized {
            return Err(Refusal::NotAuthorized(actor));
        }
        self.rooms
            .get_mut(&room_id)
            .expect("room established by validation")
            .bans
            .remove(&participant);
        Ok(OutcomeV2::Unbanned {
            room_id,
            actor,
            participant,
            reason,
            causation,
        })
    }

    fn create_topic(
        &mut self,
        room_id: RoomId,
        actor: ParticipantId,
        name: String,
    ) -> Result<OutcomeV2, Refusal> {
        self.validate_nonempty_bounded(
            &name,
            self.limits.max_topic_name_bytes,
            Refusal::InvalidTopicName,
        )?;
        let room = self.room_ref(&room_id)?;
        if !matches!(
            room.members.get(&actor),
            Some(Role::Owner | Role::Moderator)
        ) {
            return Err(Refusal::NotAuthorized(actor));
        }
        if room
            .topics
            .values()
            .any(|topic| topic.name == name || topic.aliases.contains(&name))
        {
            return Err(Refusal::DuplicateTopicName);
        }
        if room.topics.len() >= self.limits.max_topics_per_room {
            return Err(Refusal::TopicLimit);
        }
        let topic_id =
            TopicId::allocated(room.topic_identity_incarnation, room.next_topic_identity);
        let next_topic_identity = room
            .next_topic_identity
            .checked_add(1)
            .ok_or(Refusal::TopicIdentityExhausted)?;
        let room = self
            .rooms
            .get_mut(&room_id)
            .expect("room established by validation");
        room.topics.insert(
            topic_id,
            TopicV1 {
                id: topic_id,
                name: name.clone(),
                creator: actor.clone(),
                resolved: false,
                aliases: BTreeSet::new(),
                tags: BTreeSet::new(),
            },
        );
        room.next_topic_identity = next_topic_identity;
        Ok(OutcomeV2::TopicCreated {
            room_id,
            actor,
            topic_id,
            name,
        })
    }

    fn rename_topic(
        &mut self,
        room_id: RoomId,
        actor: ParticipantId,
        topic_id: TopicId,
        name: String,
    ) -> Result<OutcomeV2, Refusal> {
        self.validate_nonempty_bounded(
            &name,
            self.limits.max_topic_name_bytes,
            Refusal::InvalidTopicName,
        )?;
        let room = self.room_ref(&room_id)?;
        if !matches!(
            room.members.get(&actor),
            Some(Role::Owner | Role::Moderator)
        ) {
            return Err(Refusal::NotAuthorized(actor));
        }
        let topic = room.topics.get(&topic_id).ok_or(Refusal::UnknownTopic)?;
        if topic.name == name
            || room
                .topics
                .values()
                .any(|topic| topic.name == name || topic.aliases.contains(&name))
        {
            return Err(Refusal::DuplicateTopicName);
        }
        let previous = topic.name.clone();
        self.rooms
            .get_mut(&room_id)
            .expect("room established by validation")
            .topics
            .get_mut(&topic_id)
            .expect("topic established by validation")
            .name
            .clone_from(&name);
        Ok(OutcomeV2::TopicRenamed {
            room_id,
            actor,
            topic_id,
            previous,
            name,
        })
    }

    fn set_topic_resolved(
        &mut self,
        room_id: RoomId,
        actor: ParticipantId,
        topic_id: TopicId,
        resolved: bool,
    ) -> Result<OutcomeV2, Refusal> {
        let room = self.room_ref(&room_id)?;
        if !matches!(
            room.members.get(&actor),
            Some(Role::Owner | Role::Moderator)
        ) {
            return Err(Refusal::NotAuthorized(actor));
        }
        let topic = room.topics.get(&topic_id).ok_or(Refusal::UnknownTopic)?;
        if topic.resolved == resolved {
            return Err(Refusal::InvalidTopicLifecycleChange);
        }
        if !resolved && room.topic_successors.contains_key(&topic_id) {
            return Err(Refusal::InvalidTopicLifecycleChange);
        }
        let previous_resolved = topic.resolved;
        self.rooms
            .get_mut(&room_id)
            .expect("room established by validation")
            .topics
            .get_mut(&topic_id)
            .expect("topic established by validation")
            .resolved = resolved;
        Ok(OutcomeV2::TopicLifecycleChanged {
            room_id,
            actor,
            topic_id,
            previous_resolved,
            resolved,
        })
    }

    fn set_topic_successor(
        &mut self,
        room_id: RoomId,
        actor: ParticipantId,
        topic_id: TopicId,
        successor_topic_id: Option<TopicId>,
    ) -> Result<OutcomeV2, Refusal> {
        let room = self.room_ref(&room_id)?;
        if !matches!(
            room.members.get(&actor),
            Some(Role::Owner | Role::Moderator)
        ) {
            return Err(Refusal::NotAuthorized(actor));
        }
        let topic = room.topics.get(&topic_id).ok_or(Refusal::UnknownTopic)?;
        let previous_successor_topic_id = room.topic_successors.get(&topic_id).copied();
        if previous_successor_topic_id == successor_topic_id {
            return Err(Refusal::InvalidTopicSupersession);
        }
        if let Some(successor_topic_id) = successor_topic_id {
            if !topic.resolved {
                return Err(Refusal::TopicMustBeResolved);
            }
            if successor_topic_id == topic_id || !room.topics.contains_key(&successor_topic_id) {
                return Err(if successor_topic_id == topic_id {
                    Refusal::InvalidTopicSupersession
                } else {
                    Refusal::UnknownTopic
                });
            }
            if room.topic_successors.contains_key(&successor_topic_id)
                || room
                    .topic_successors
                    .values()
                    .any(|candidate| *candidate == topic_id)
            {
                return Err(Refusal::InvalidTopicSupersession);
            }
        } else if previous_successor_topic_id.is_none() {
            return Err(Refusal::InvalidTopicSupersession);
        }

        let successors = &mut self
            .rooms
            .get_mut(&room_id)
            .expect("room established by validation")
            .topic_successors;
        if let Some(successor_topic_id) = successor_topic_id {
            successors.insert(topic_id, successor_topic_id);
        } else {
            successors.remove(&topic_id);
        }
        Ok(OutcomeV2::TopicSupersessionChanged {
            room_id,
            actor,
            topic_id,
            previous_successor_topic_id,
            successor_topic_id,
        })
    }

    fn set_topic_alias(
        &mut self,
        room_id: RoomId,
        actor: ParticipantId,
        topic_id: TopicId,
        alias: String,
        added: bool,
    ) -> Result<OutcomeV2, Refusal> {
        self.validate_nonempty_bounded(
            &alias,
            self.limits.max_topic_name_bytes,
            Refusal::InvalidTopicName,
        )?;
        let room = self.room_ref(&room_id)?;
        if !matches!(
            room.members.get(&actor),
            Some(Role::Owner | Role::Moderator)
        ) {
            return Err(Refusal::NotAuthorized(actor));
        }
        let topic = room.topics.get(&topic_id).ok_or(Refusal::UnknownTopic)?;
        if added {
            if room
                .topics
                .values()
                .any(|candidate| candidate.name == alias || candidate.aliases.contains(&alias))
            {
                return Err(Refusal::DuplicateTopicName);
            }
            if topic.aliases.len() >= MAX_TOPIC_ALIASES_PER_TOPIC_V1 {
                return Err(Refusal::TopicAliasLimit);
            }
        } else if !topic.aliases.contains(&alias) {
            return Err(Refusal::UnknownTopicAlias);
        }
        let aliases = &mut self
            .rooms
            .get_mut(&room_id)
            .expect("room established by validation")
            .topics
            .get_mut(&topic_id)
            .expect("topic established by validation")
            .aliases;
        if added {
            aliases.insert(alias.clone());
        } else {
            aliases.remove(&alias);
        }
        Ok(OutcomeV2::TopicAliasChanged {
            room_id,
            actor,
            topic_id,
            alias,
            added,
        })
    }

    fn set_topic_tag(
        &mut self,
        room_id: RoomId,
        actor: ParticipantId,
        topic_id: TopicId,
        tag: String,
        added: bool,
    ) -> Result<OutcomeV2, Refusal> {
        self.validate_nonempty_bounded(
            &tag,
            self.limits.max_topic_name_bytes,
            Refusal::InvalidTopicTag,
        )?;
        let room = self.room_ref(&room_id)?;
        if !matches!(
            room.members.get(&actor),
            Some(Role::Owner | Role::Moderator)
        ) {
            return Err(Refusal::NotAuthorized(actor));
        }
        let topic = room.topics.get(&topic_id).ok_or(Refusal::UnknownTopic)?;
        if added {
            if topic.tags.contains(&tag) {
                return Err(Refusal::DuplicateTopicTag);
            }
            if topic.tags.len() >= MAX_TOPIC_TAGS_PER_TOPIC_V1 {
                return Err(Refusal::TopicTagLimit);
            }
        } else if !topic.tags.contains(&tag) {
            return Err(Refusal::UnknownTopicTag);
        }
        let tags = &mut self
            .rooms
            .get_mut(&room_id)
            .expect("room established by validation")
            .topics
            .get_mut(&topic_id)
            .expect("topic established by validation")
            .tags;
        if added {
            tags.insert(tag.clone());
        } else {
            tags.remove(&tag);
        }
        Ok(OutcomeV2::TopicTagChanged {
            room_id,
            actor,
            topic_id,
            tag,
            added,
        })
    }

    fn speak(&mut self, admission: MessageAdmission) -> Result<OutcomeV2, Refusal> {
        let MessageAdmission {
            room_id,
            author,
            at,
            body,
            addressed_to,
            placement,
            attachments,
            vellum_artifacts,
        } = admission;
        self.validate_nonempty_bounded(
            &body,
            self.limits.max_message_body_bytes,
            Refusal::InvalidMessageBody,
        )?;
        if self.limits.max_messages_per_room == 0 {
            return Err(Refusal::MessageRetentionDisabled);
        }
        let room = self.room_ref(&room_id)?;
        if !room.members.contains_key(&author) {
            return Err(Refusal::NotMember(author));
        }
        let reply_to_root = match placement {
            MessagePlacement::Reply(root) => Some(root),
            MessagePlacement::Unscoped | MessagePlacement::Topic(_) => None,
        };
        let topic_id = if let Some(root_message_id) = reply_to_root {
            let root = room
                .messages
                .iter()
                .find(|message| message.id == root_message_id)
                .ok_or(Refusal::UnknownMessage)?;
            if root.reply_to_root.is_some() {
                return Err(Refusal::InvalidReplyTarget);
            }
            if room.archived_threads.contains(&root_message_id) {
                return Err(Refusal::ThreadArchived);
            }
            if let Some(topic_id) = root.topic_id
                && room.topics.get(&topic_id).is_some_and(TopicV1::is_resolved)
            {
                return Err(Refusal::TopicResolved);
            }
            root.topic_id
        } else {
            let requested_topic = match placement {
                MessagePlacement::Topic(topic) => Some(topic),
                MessagePlacement::Unscoped | MessagePlacement::Reply(_) => None,
            };
            if let Some(topic_id) = requested_topic {
                let topic = room.topics.get(&topic_id).ok_or(Refusal::UnknownTopic)?;
                if topic.is_resolved() {
                    return Err(Refusal::TopicResolved);
                }
            }
            requested_topic
        };
        if addressed_to.len() > self.limits.max_addressed_participants_per_message {
            return Err(Refusal::AddressedParticipantLimit);
        }
        if addressed_to.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(Refusal::NonCanonicalAddressing);
        }
        if let Some(id) = addressed_to
            .iter()
            .find(|id| !room.members.contains_key(*id))
        {
            return Err(Refusal::NotMember(id.clone()));
        }
        if let Some(attachments) = &attachments {
            if attachments.is_empty() || attachments.len() > MAX_ATTACHMENTS_PER_MESSAGE_V1 {
                return Err(Refusal::InvalidAttachmentCount);
            }
            let limits = AttachmentDescriptorLimitsV1 {
                max_declared_bytes: self
                    .limits
                    .max_attachment_declared_bytes
                    .try_into()
                    .map_err(|_| Refusal::InvalidAttachmentDeclaredLength)?,
                max_media_type_bytes: self.limits.max_attachment_media_type_bytes,
                max_display_label_bytes: self.limits.max_attachment_display_label_bytes,
            };
            let mut attachment_ids = BTreeSet::new();
            let mut object_refs = BTreeSet::new();
            for attachment in attachments {
                attachment
                    .validate(limits)
                    .map_err(|refusal| match refusal {
                        AttachmentDescriptorRefusalV1::InvalidDeclaredLength => {
                            Refusal::InvalidAttachmentDeclaredLength
                        }
                        AttachmentDescriptorRefusalV1::InvalidMediaType => {
                            Refusal::InvalidAttachmentMediaType
                        }
                        AttachmentDescriptorRefusalV1::InvalidDisplayLabel => {
                            Refusal::InvalidAttachmentDisplayLabel
                        }
                    })?;
                if !attachment_ids.insert(attachment.attachment_id()) {
                    return Err(Refusal::DuplicateAttachmentId);
                }
                if !object_refs.insert((attachment.store_id(), attachment.object_key())) {
                    return Err(Refusal::DuplicateAttachmentObject);
                }
            }
        }
        if let Some(references) = &vellum_artifacts {
            if references.is_empty()
                || references.len() > MAX_VELLUM_ARTIFACT_REFERENCES_PER_MESSAGE_V1
            {
                return Err(Refusal::InvalidVellumArtifactReferenceCount);
            }
            let mut reference_ids = BTreeSet::new();
            let mut targets = BTreeSet::new();
            for reference in references {
                if !reference_ids.insert(reference.reference_id()) {
                    return Err(Refusal::DuplicateVellumArtifactReferenceId);
                }
                if !targets.insert(reference.target()) {
                    return Err(Refusal::DuplicateVellumArtifactTarget);
                }
            }
        }
        let message_id = MessageId::allocated(
            room.message_identity_incarnation,
            room.next_message_identity,
        );
        let next_message_identity = room
            .next_message_identity
            .checked_add(1)
            .ok_or(Refusal::MessageIdentityExhausted)?;
        let sequence = room.next_message_sequence;
        let next = sequence
            .checked_add(1)
            .ok_or(Refusal::MessageSequenceExhausted)?;
        let room = self
            .rooms
            .get_mut(&room_id)
            .expect("room established by validation");
        if room.messages.len() == self.limits.max_messages_per_room
            && let Some(evicted) = room.messages.pop_front()
        {
            room.archived_threads.remove(&evicted.id);
            room.reactions.remove(&evicted.id);
            if let Some(root) = evicted.reply_to_root
                && !room
                    .messages
                    .iter()
                    .any(|message| message.reply_to_root == Some(root))
            {
                room.thread_roots.remove(&root);
            }
        }
        let attachment_count = attachments.as_ref().map_or(0, Vec::len);
        let vellum_artifact_count = vellum_artifacts.as_ref().map_or(0, Vec::len);
        room.messages.push_back(Message {
            id: message_id,
            sequence,
            author: author.clone(),
            at,
            body: body.clone(),
            addressed_to,
            reply_to_root,
            topic_id,
            attachments: attachments
                .unwrap_or_default()
                .into_iter()
                .map(MessageAttachmentV1::available)
                .collect(),
            vellum_artifacts: vellum_artifacts
                .unwrap_or_default()
                .into_iter()
                .map(MessageVellumArtifactReferenceV1::available)
                .collect(),
            revisions: vec![MessageRevisionV1 {
                number: 0,
                editor: author,
                at,
                availability: MessageRevisionAvailabilityV1::Available,
                body,
            }],
        });
        room.next_message_identity = next_message_identity;
        room.next_message_sequence = next;
        if let Some(root) = reply_to_root {
            room.thread_roots.insert(root);
        }
        if attachment_count != 0 {
            return Ok(OutcomeV2::AttachmentMessageAccepted {
                room_id,
                message_id,
                sequence,
                attachment_count: attachment_count
                    .try_into()
                    .expect("attachment count is bounded by eight"),
            });
        }
        if vellum_artifact_count != 0 {
            return Ok(OutcomeV2::VellumArtifactMessageAccepted {
                room_id,
                message_id,
                sequence,
                reference_count: vellum_artifact_count
                    .try_into()
                    .expect("Vellum artifact reference count is bounded by eight"),
            });
        }
        Ok(match reply_to_root {
            Some(root_message_id) => OutcomeV2::ReplyAccepted {
                room_id,
                root_message_id,
                sequence,
            },
            None => match topic_id {
                Some(topic_id) => OutcomeV2::TopicMessageAccepted {
                    room_id,
                    topic_id,
                    sequence,
                },
                None => OutcomeV2::MessageAccepted { room_id, sequence },
            },
        })
    }

    fn set_thread_archived(
        &mut self,
        room_id: RoomId,
        actor: ParticipantId,
        root_message_id: MessageId,
        archived: bool,
        reason: ModerationReasonV1,
        causation: ModerationCausationV1,
    ) -> Result<OutcomeV2, Refusal> {
        self.validate_moderation_reason(&reason)?;
        let room = self.room_ref(&room_id)?;
        let Some(root) = room
            .messages
            .iter()
            .find(|message| message.id == root_message_id)
        else {
            return Err(Refusal::UnknownMessage);
        };
        if root.reply_to_root.is_some() {
            return Err(Refusal::InvalidReplyTarget);
        }
        let actor_role = room
            .members
            .get(&actor)
            .copied()
            .ok_or_else(|| Refusal::NotMember(actor.clone()))?;
        if root.author != actor && !matches!(actor_role, Role::Owner | Role::Moderator) {
            return Err(Refusal::NotAuthorized(actor));
        }
        if room.archived_threads.contains(&root_message_id) == archived {
            return Err(Refusal::InvalidThreadLifecycleChange);
        }
        let room = self
            .rooms
            .get_mut(&room_id)
            .expect("room established by validation");
        if archived {
            room.archived_threads.insert(root_message_id);
        } else {
            room.archived_threads.remove(&root_message_id);
        }
        Ok(OutcomeV2::ThreadLifecycleChanged {
            room_id,
            actor,
            root_message_id,
            archived,
            reason,
            causation,
        })
    }

    fn set_reaction(
        &mut self,
        room_id: RoomId,
        actor: ParticipantId,
        message_id: MessageId,
        key: String,
        reacted: bool,
    ) -> Result<OutcomeV2, Refusal> {
        self.validate_nonempty_bounded(&key, MAX_REACTION_KEY_BYTES_V1, Refusal::InvalidReaction)?;
        let room = self.room_ref(&room_id)?;
        if !room.members.contains_key(&actor) {
            return Err(Refusal::NotMember(actor));
        }
        if !room.messages.iter().any(|message| message.id == message_id) {
            return Err(Refusal::UnknownMessage);
        }
        let by_key = room.reactions.get(&message_id);
        let exists = by_key
            .and_then(|values| values.get(&key))
            .is_some_and(|participants| participants.contains(&actor));
        if exists == reacted {
            return Err(Refusal::InvalidReactionChange);
        }
        if reacted
            && by_key.is_none_or(|values| !values.contains_key(&key))
            && by_key.is_some_and(|values| values.len() == MAX_REACTION_KEYS_PER_MESSAGE_V1)
        {
            return Err(Refusal::ReactionLimit);
        }

        let room = self
            .rooms
            .get_mut(&room_id)
            .expect("room established by validation");
        if reacted {
            room.reactions
                .entry(message_id)
                .or_default()
                .entry(key.clone())
                .or_default()
                .insert(actor.clone());
        } else {
            let by_key = room
                .reactions
                .get_mut(&message_id)
                .expect("existing relation established by validation");
            let participants = by_key
                .get_mut(&key)
                .expect("existing relation established by validation");
            participants.remove(&actor);
            if participants.is_empty() {
                by_key.remove(&key);
            }
            if by_key.is_empty() {
                room.reactions.remove(&message_id);
            }
        }
        Ok(OutcomeV2::ReactionChanged {
            room_id,
            actor,
            message_id,
            key,
            reacted,
        })
    }

    fn set_pin(
        &mut self,
        room_id: RoomId,
        actor: ParticipantId,
        message_id: MessageId,
        pinned: bool,
    ) -> Result<OutcomeV2, Refusal> {
        let room = self.room_ref(&room_id)?;
        if !room.members.contains_key(&actor) {
            return Err(Refusal::NotMember(actor));
        }
        let current = room.pins.get(&message_id);
        if current.is_some() == pinned {
            return Err(Refusal::InvalidPinChange);
        }
        let original_sequence = if pinned {
            if room.pins.len() == MAX_ROOM_PINS_V1 {
                return Err(Refusal::PinLimit);
            }
            room.messages
                .iter()
                .find(|message| message.id == message_id)
                .map(|message| message.sequence)
                .ok_or(Refusal::UnknownMessage)?
        } else {
            current
                .map(|(sequence, _)| *sequence)
                .expect("change validation established pin")
        };
        let room = self
            .rooms
            .get_mut(&room_id)
            .expect("room established by validation");
        if pinned {
            room.pins
                .insert(message_id, (original_sequence, actor.clone()));
        } else {
            room.pins.remove(&message_id);
        }
        Ok(OutcomeV2::PinChanged {
            room_id,
            actor,
            message_id,
            original_sequence,
            pinned,
        })
    }

    fn create_poll(
        &mut self,
        room_id: RoomId,
        actor: ParticipantId,
        prompt: String,
        options: Vec<String>,
        eligible_participants: Vec<ParticipantId>,
        max_choices: u8,
    ) -> Result<OutcomeV2, Refusal> {
        self.validate_nonempty_bounded(
            &prompt,
            MAX_POLL_PROMPT_BYTES_V1,
            Refusal::InvalidPollPrompt,
        )?;
        if !(2..=MAX_POLL_OPTIONS_V1).contains(&options.len())
            || options
                .iter()
                .any(|option| option.is_empty() || option.len() > MAX_POLL_OPTION_BYTES_V1)
            || options.iter().collect::<BTreeSet<_>>().len() != options.len()
        {
            return Err(Refusal::InvalidPollOptions);
        }
        if max_choices == 0 || usize::from(max_choices) > options.len() {
            return Err(Refusal::InvalidPollChoiceLimit);
        }
        if eligible_participants.is_empty()
            || eligible_participants.len() > MAX_POLL_ELIGIBLE_PARTICIPANTS_V1
            || eligible_participants
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(Refusal::InvalidPollEligibility);
        }

        let room = self.room_ref(&room_id)?;
        if !room.members.contains_key(&actor) {
            return Err(Refusal::NotMember(actor));
        }
        for participant in &eligible_participants {
            if !self.participants.contains_key(participant) {
                return Err(Refusal::UnknownParticipant(participant.clone()));
            }
            if !room.members.contains_key(participant) {
                return Err(Refusal::InvalidPollEligibility);
            }
        }
        let evicted = if room.polls.len() == MAX_POLLS_PER_ROOM_V1 {
            Some(
                room.poll_creation_order
                    .iter()
                    .copied()
                    .find(|poll_id| room.polls.get(poll_id).is_some_and(|poll| !poll.open))
                    .ok_or(Refusal::PollLimit)?,
            )
        } else {
            None
        };
        let poll_id = PollId::allocated(room.poll_identity_incarnation, room.next_poll_identity);
        let next_poll_identity = room
            .next_poll_identity
            .checked_add(1)
            .ok_or(Refusal::PollIdentityExhausted)?;

        let room = self
            .rooms
            .get_mut(&room_id)
            .expect("room established by validation");
        if let Some(evicted) = evicted {
            room.polls.remove(&evicted);
            let index = room
                .poll_creation_order
                .iter()
                .position(|poll_id| *poll_id == evicted)
                .expect("poll order and map remain equal");
            room.poll_creation_order.remove(index);
        }
        room.polls.insert(
            poll_id,
            PollV1 {
                id: poll_id,
                creator: actor,
                prompt,
                options,
                eligible_participants: eligible_participants.into_iter().collect(),
                max_choices,
                open: true,
                ballots: BTreeMap::new(),
            },
        );
        room.poll_creation_order.push_back(poll_id);
        room.next_poll_identity = next_poll_identity;
        Ok(OutcomeV2::PollCreated { room_id, poll_id })
    }

    fn set_poll_ballot(
        &mut self,
        room_id: RoomId,
        actor: ParticipantId,
        poll_id: PollId,
        selections: Vec<u8>,
    ) -> Result<OutcomeV2, Refusal> {
        let room = self.room_ref(&room_id)?;
        if !room.members.contains_key(&actor) {
            return Err(Refusal::NotMember(actor));
        }
        let poll = room.polls.get(&poll_id).ok_or(Refusal::UnknownPoll)?;
        if !poll.open {
            return Err(Refusal::PollClosed);
        }
        if !poll.eligible_participants.contains(&actor) {
            return Err(Refusal::NotPollEligible);
        }
        if selections.len() > usize::from(poll.max_choices)
            || selections.windows(2).any(|pair| pair[0] >= pair[1])
            || selections
                .iter()
                .any(|selection| usize::from(*selection) >= poll.options.len())
        {
            return Err(Refusal::InvalidPollBallot);
        }
        let replacement: BTreeSet<_> = selections.into_iter().collect();
        if poll.ballots.get(&actor).cloned().unwrap_or_default() == replacement {
            return Err(Refusal::InvalidPollBallotChange);
        }

        let room = self
            .rooms
            .get_mut(&room_id)
            .expect("room established by validation");
        let poll = room
            .polls
            .get_mut(&poll_id)
            .expect("poll established by validation");
        if replacement.is_empty() {
            poll.ballots.remove(&actor);
        } else {
            poll.ballots.insert(actor, replacement);
        }
        let status = poll.status();
        Ok(OutcomeV2::PollBallotChanged {
            room_id,
            poll_id,
            totals: status.totals,
            total_ballots: status.total_ballots,
        })
    }

    fn set_poll_open(
        &mut self,
        room_id: RoomId,
        actor: ParticipantId,
        poll_id: PollId,
        open: bool,
    ) -> Result<OutcomeV2, Refusal> {
        let room = self.room_ref(&room_id)?;
        let role = room
            .members
            .get(&actor)
            .ok_or_else(|| Refusal::NotMember(actor.clone()))?;
        let poll = room.polls.get(&poll_id).ok_or(Refusal::UnknownPoll)?;
        if poll.creator != actor && !matches!(role, Role::Owner | Role::Moderator) {
            return Err(Refusal::NotAuthorized(actor));
        }
        if poll.open == open {
            return Err(Refusal::InvalidPollLifecycleChange);
        }
        self.rooms
            .get_mut(&room_id)
            .expect("room established by validation")
            .polls
            .get_mut(&poll_id)
            .expect("poll established by validation")
            .open = open;
        Ok(OutcomeV2::PollLifecycleChanged {
            room_id,
            actor,
            poll_id,
            open,
        })
    }

    fn edit_message(
        &mut self,
        room_id: RoomId,
        actor: ParticipantId,
        message_id: MessageId,
        at: LogicalTime,
        body: String,
    ) -> Result<OutcomeV2, Refusal> {
        self.validate_nonempty_bounded(
            &body,
            self.limits.max_message_body_bytes,
            Refusal::InvalidMessageBody,
        )?;
        let room = self.room_ref(&room_id)?;
        if !room.members.contains_key(&actor) {
            return Err(Refusal::NotMember(actor));
        }
        let message = room
            .messages
            .iter()
            .find(|message| message.id == message_id)
            .ok_or(Refusal::UnknownMessage)?;
        if message.author != actor {
            return Err(Refusal::NotAuthorized(actor));
        }
        if message.is_withdrawn() || message.body == body {
            return Err(Refusal::InvalidMessageEdit);
        }
        if message.revisions.len() == MAX_MESSAGE_REVISIONS_V1 {
            return Err(Refusal::MessageRevisionLimit);
        }
        let message_revision: u32 = message
            .revisions
            .len()
            .try_into()
            .map_err(|_| Refusal::MessageRevisionLimit)?;
        let message = self
            .rooms
            .get_mut(&room_id)
            .expect("room established by validation")
            .messages
            .iter_mut()
            .find(|message| message.id == message_id)
            .expect("message established by validation");
        message.body.clone_from(&body);
        message.revisions.push(MessageRevisionV1 {
            number: message_revision,
            editor: actor.clone(),
            at,
            availability: MessageRevisionAvailabilityV1::Available,
            body: body.clone(),
        });
        Ok(OutcomeV2::MessageEdited {
            room_id,
            actor,
            message_id,
            message_revision,
            at,
            body,
        })
    }

    fn withdraw_message(
        &mut self,
        room_id: RoomId,
        actor: ParticipantId,
        message_id: MessageId,
    ) -> Result<OutcomeV2, Refusal> {
        let room = self.room_ref(&room_id)?;
        if !room.members.contains_key(&actor) {
            return Err(Refusal::NotMember(actor));
        }
        let message = room
            .messages
            .iter()
            .find(|message| message.id == message_id)
            .ok_or(Refusal::UnknownMessage)?;
        if message.author != actor {
            return Err(Refusal::NotAuthorized(actor));
        }
        if message.is_withdrawn() {
            return Err(Refusal::MessageAlreadyTombstoned);
        }
        let sequence = message.sequence;

        self.tombstone_message(&room_id, message_id);
        Ok(OutcomeV2::MessageWithdrawn {
            room_id,
            actor,
            message_id,
            sequence,
        })
    }

    fn redact_message(
        &mut self,
        room_id: RoomId,
        actor: ParticipantId,
        message_id: MessageId,
        reason: ModerationReasonV1,
        causation: ModerationCausationV1,
    ) -> Result<OutcomeV2, Refusal> {
        self.validate_moderation_reason(&reason)?;
        let room = self.room_ref(&room_id)?;
        let message = room
            .messages
            .iter()
            .find(|message| message.id == message_id)
            .ok_or(Refusal::UnknownMessage)?;
        let author = message.author.clone();
        if !Self::moderation_authorized(room, &actor, &author) {
            return Err(Refusal::NotAuthorized(actor));
        }
        if message.is_withdrawn() {
            return Err(Refusal::MessageAlreadyTombstoned);
        }
        let sequence = message.sequence;

        self.tombstone_message(&room_id, message_id);
        Ok(OutcomeV2::MessageRedacted {
            room_id,
            actor,
            author,
            message_id,
            sequence,
            reason,
            causation,
        })
    }

    fn tombstone_message(&mut self, room_id: &RoomId, message_id: MessageId) {
        let room = self
            .rooms
            .get_mut(room_id)
            .expect("room established by validation");
        let message = room
            .messages
            .iter_mut()
            .find(|message| message.id == message_id)
            .expect("message established by validation");
        message.body.clear();
        for attachment in &mut message.attachments {
            attachment.descriptor = None;
        }
        for artifact in &mut message.vellum_artifacts {
            artifact.target = None;
        }
        for revision in &mut message.revisions {
            revision.body.clear();
            revision.availability = MessageRevisionAvailabilityV1::Tombstoned;
        }
        for envelope in self
            .pending_events
            .iter_mut()
            .chain(self.retained_changes.iter_mut())
        {
            match &mut envelope.event {
                DomainEventV2::MessageAccepted {
                    room_id: event_room,
                    message,
                } if event_room == room_id && message.id == message_id => {
                    message.body.clear();
                    for attachment in &mut message.attachments {
                        attachment.descriptor = None;
                    }
                    for artifact in &mut message.vellum_artifacts {
                        artifact.target = None;
                    }
                    for revision in &mut message.revisions {
                        revision.body.clear();
                        revision.availability = MessageRevisionAvailabilityV1::Tombstoned;
                    }
                }
                DomainEventV2::MessageEdited {
                    room_id: event_room,
                    message_id: event_message,
                    body,
                    ..
                } if event_room == room_id && *event_message == message_id => body.clear(),
                _ => {}
            }
        }
    }

    fn room_ref(&self, room_id: &RoomId) -> Result<&Room, Refusal> {
        self.rooms
            .get(room_id)
            .ok_or_else(|| Refusal::UnknownRoom(room_id.clone()))
    }

    fn validate_nonempty_bounded(
        &self,
        value: &str,
        max_bytes: usize,
        refusal: Refusal,
    ) -> Result<(), Refusal> {
        if value.is_empty() || value.len() > max_bytes {
            Err(refusal)
        } else {
            Ok(())
        }
    }
}

fn stable_reference_availability(
    room: &Room,
    message_id: MessageId,
) -> StableReferenceAvailabilityV1 {
    room.messages
        .iter()
        .find(|message| message.id == message_id)
        .map_or(StableReferenceAvailabilityV1::Evicted, |message| {
            if message.is_withdrawn() {
                StableReferenceAvailabilityV1::Tombstoned
            } else {
                StableReferenceAvailabilityV1::Available
            }
        })
}
