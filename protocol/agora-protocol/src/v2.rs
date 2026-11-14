//! Transport-neutral values for Agora's additive V2 service profile.
//!
//! IDs and cursor values are correlation handles, not credentials or authority.

use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ClientIdV2(pub u64);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RequestIdV2(pub u64);

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct PageCursorV2(pub u64);

impl fmt::Debug for PageCursorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PageCursorV2(<opaque>)")
    }
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MessageIdV1([u8; 16]);

impl MessageIdV1 {
    pub const fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn into_bytes(self) -> [u8; 16] {
        self.0
    }
}

impl fmt::Debug for MessageIdV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("MessageIdV1(<opaque>)")
    }
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TopicIdV1([u8; 16]);

impl TopicIdV1 {
    pub const fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }
    pub const fn into_bytes(self) -> [u8; 16] {
        self.0
    }
}

impl fmt::Debug for TopicIdV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TopicIdV1(<opaque>)")
    }
}

macro_rules! opaque_attachment_value {
    ($name:ident, $bytes:expr) => {
        #[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; $bytes]);

        impl $name {
            pub const fn new(bytes: [u8; $bytes]) -> Self {
                Self(bytes)
            }

            pub const fn into_bytes(self) -> [u8; $bytes] {
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
opaque_attachment_value!(VellumArtifactReferenceIdV1, 16);

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VellumDocumentTargetV1 {
    pub workspace_id: u64,
    pub document_id: u64,
    pub source_revision: u64,
}

impl fmt::Debug for VellumDocumentTargetV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("VellumDocumentTargetV1(<opaque>)")
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VellumArtifactReferenceV1 {
    pub reference_id: VellumArtifactReferenceIdV1,
    pub target: VellumDocumentTargetV1,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VellumArtifactMessageTargetV1 {
    Room,
    Reply(MessageIdV1),
    Topic(TopicIdV1),
}

#[derive(Clone, Eq, PartialEq)]
pub struct MessageVellumArtifactReferenceV1 {
    pub reference_id: VellumArtifactReferenceIdV1,
    pub target: Option<VellumDocumentTargetV1>,
}

impl fmt::Debug for MessageVellumArtifactReferenceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MessageVellumArtifactReferenceV1")
            .field("reference_id", &self.reference_id)
            .field("available", &self.target.is_some())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttachmentIntegrityV1 {
    Sha256(Sha256DigestV1),
}

#[derive(Clone, Eq, PartialEq)]
pub struct AttachmentDescriptorV1 {
    pub attachment_id: AttachmentIdV1,
    pub store_id: AttachmentStoreIdV1,
    pub object_key: AttachmentObjectKeyV1,
    pub declared_bytes: u64,
    pub media_type: String,
    pub display_label: String,
    pub integrity: AttachmentIntegrityV1,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttachmentMessageTargetV1 {
    Room,
    Reply(MessageIdV1),
    Topic(TopicIdV1),
}

#[derive(Clone, Eq, PartialEq)]
pub struct MessageAttachmentV1 {
    pub attachment_id: AttachmentIdV1,
    pub descriptor: Option<AttachmentDescriptorV1>,
}

impl fmt::Debug for MessageAttachmentV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MessageAttachmentV1")
            .field("attachment_id", &self.attachment_id)
            .field("available", &self.descriptor.is_some())
            .finish()
    }
}

/// Closed selector for one participant's private read position.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReadScopeV1 {
    Room,
    Thread(MessageIdV1),
    Topic(TopicIdV1),
}

/// Private marker returned only for the bound participant.
#[derive(Clone, Eq, PartialEq)]
pub struct ReadPositionV1 {
    pub room_id: String,
    pub scope: ReadScopeV1,
    pub target_message_id: MessageIdV1,
    pub target_sequence: u64,
    pub observed_revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AttentionScopeV1 {
    Room,
    Thread(MessageIdV1),
    Topic(TopicIdV1),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NotificationIntentV1 {
    #[default]
    Default,
    AllActivity,
    AddressedOnly,
    None,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub struct AttentionPreferenceV1 {
    pub following: bool,
    pub muted: bool,
    pub notification_intent: NotificationIntentV1,
}

impl fmt::Debug for AttentionPreferenceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AttentionPreferenceV1(<redacted>)")
    }
}

impl fmt::Debug for ReadPositionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReadPositionV1")
            .field("room_id_bytes", &self.room_id.len())
            .field("scope", &self.scope)
            .field("target_message_id", &self.target_message_id)
            .field("target_sequence", &self.target_sequence)
            .field("observed_revision", &self.observed_revision)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionPolicyV2 {
    Open,
    InviteOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConversationProfileV1 {
    Ordinary,
    Direct,
    GroupDirect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParticipantKindV2 {
    Human,
    Agent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoleV2 {
    Owner,
    Moderator,
    Member,
}

#[derive(Clone, Eq, PartialEq)]
pub enum CommandV2 {
    Capabilities,
    Rooms {
        cursor: Option<PageCursorV2>,
        limit: u32,
    },
    Members {
        room_id: String,
        cursor: Option<PageCursorV2>,
        limit: u32,
    },
    Bans {
        room_id: String,
        cursor: Option<PageCursorV2>,
        limit: u32,
    },
    Messages {
        room_id: String,
        cursor: Option<PageCursorV2>,
        limit: u32,
    },
    IdentifiedMessages {
        room_id: String,
        cursor: Option<PageCursorV2>,
        limit: u32,
    },
    AttachmentMessages {
        room_id: String,
        cursor: Option<PageCursorV2>,
        limit: u32,
    },
    VellumArtifactMessages {
        room_id: String,
        cursor: Option<PageCursorV2>,
        limit: u32,
    },
    OpenChanges {
        room_id: String,
        observed_revision: u64,
    },
    Changes {
        room_id: String,
        cursor: PageCursorV2,
        limit: u32,
    },
    OpenIdentifiedChanges {
        room_id: String,
        observed_revision: u64,
    },
    IdentifiedChanges {
        room_id: String,
        cursor: PageCursorV2,
        limit: u32,
    },
    OpenAttachmentChanges {
        room_id: String,
        observed_revision: u64,
    },
    AttachmentChanges {
        room_id: String,
        cursor: PageCursorV2,
        limit: u32,
    },
    ThreadRoots {
        room_id: String,
        cursor: Option<PageCursorV2>,
        limit: u32,
    },
    ThreadReplies {
        room_id: String,
        root_message_id: MessageIdV1,
        cursor: Option<PageCursorV2>,
        limit: u32,
    },
    OpenThreadedChanges {
        room_id: String,
        observed_revision: u64,
    },
    ThreadedChanges {
        room_id: String,
        cursor: PageCursorV2,
        limit: u32,
    },
    OpenTopicChanges {
        room_id: String,
        observed_revision: u64,
    },
    TopicChanges {
        room_id: String,
        cursor: PageCursorV2,
        limit: u32,
    },
    ArchiveThread {
        room_id: String,
        expected_revision: u64,
        root_message_id: MessageIdV1,
        reason: String,
        causation: [u8; 16],
    },
    ReopenThread {
        room_id: String,
        expected_revision: u64,
        root_message_id: MessageIdV1,
        reason: String,
        causation: [u8; 16],
    },
    ThreadStatus {
        room_id: String,
        root_message_id: MessageIdV1,
    },
    AddReaction {
        room_id: String,
        expected_revision: u64,
        message_id: MessageIdV1,
        key: String,
    },
    RemoveReaction {
        room_id: String,
        expected_revision: u64,
        message_id: MessageIdV1,
        key: String,
    },
    ReactionStatus {
        room_id: String,
        message_id: MessageIdV1,
    },
    EditMessage {
        room_id: String,
        expected_revision: u64,
        message_id: MessageIdV1,
        logical_time: u64,
        body: String,
    },
    MessageRevisions {
        room_id: String,
        message_id: MessageIdV1,
        cursor: Option<PageCursorV2>,
        limit: u32,
    },
    Topics {
        room_id: String,
        cursor: Option<PageCursorV2>,
        limit: u32,
    },
    TopicMessages {
        room_id: String,
        topic_id: TopicIdV1,
        cursor: Option<PageCursorV2>,
        limit: u32,
    },
    CreateTopic {
        room_id: String,
        expected_revision: u64,
        name: String,
    },
    RenameTopic {
        room_id: String,
        expected_revision: u64,
        topic_id: TopicIdV1,
        name: String,
    },
    ResolveTopic {
        room_id: String,
        expected_revision: u64,
        topic_id: TopicIdV1,
    },
    ReopenTopic {
        room_id: String,
        expected_revision: u64,
        topic_id: TopicIdV1,
    },
    TopicStatus {
        room_id: String,
        topic_id: TopicIdV1,
    },
    SupersedeTopic {
        room_id: String,
        expected_revision: u64,
        topic_id: TopicIdV1,
        successor_topic_id: TopicIdV1,
    },
    ClearTopicSupersession {
        room_id: String,
        expected_revision: u64,
        topic_id: TopicIdV1,
    },
    TopicSupersessionStatus {
        room_id: String,
        topic_id: TopicIdV1,
    },
    AddTopicAlias {
        room_id: String,
        expected_revision: u64,
        topic_id: TopicIdV1,
        alias: String,
    },
    RemoveTopicAlias {
        room_id: String,
        expected_revision: u64,
        topic_id: TopicIdV1,
        alias: String,
    },
    TopicAliasStatus {
        room_id: String,
        topic_id: TopicIdV1,
    },
    AddTopicTag {
        room_id: String,
        expected_revision: u64,
        topic_id: TopicIdV1,
        tag: String,
    },
    RemoveTopicTag {
        room_id: String,
        expected_revision: u64,
        topic_id: TopicIdV1,
        tag: String,
    },
    TopicTagStatus {
        room_id: String,
        topic_id: TopicIdV1,
    },
    SpeakInTopic {
        room_id: String,
        expected_revision: u64,
        topic_id: TopicIdV1,
        logical_time: u64,
        body: String,
        addressed_to: Vec<String>,
    },
    ReadPosition {
        room_id: String,
        scope: ReadScopeV1,
    },
    AdvanceReadPosition {
        room_id: String,
        observed_revision: u64,
        scope: ReadScopeV1,
        target_message_id: MessageIdV1,
    },
    AttentionPreference {
        room_id: String,
        scope: AttentionScopeV1,
    },
    SetAttentionPreference {
        room_id: String,
        observed_revision: u64,
        scope: AttentionScopeV1,
        preference: AttentionPreferenceV1,
    },
    CreateRoom {
        room_id: String,
        name: String,
        admission: AdmissionPolicyV2,
        expected_revision: u64,
    },
    CreatePeerConversation {
        room_id: String,
        name: String,
        profile: ConversationProfileV1,
        founding_participant_ids: Vec<String>,
        expected_revision: u64,
    },
    ConversationProfile {
        room_id: String,
    },
    Invite {
        room_id: String,
        participant_id: String,
        expected_revision: u64,
    },
    Join {
        room_id: String,
        expected_revision: u64,
    },
    Leave {
        room_id: String,
        expected_revision: u64,
    },
    SetRole {
        room_id: String,
        participant_id: String,
        role: RoleV2,
        expected_revision: u64,
    },
    SetAdmissionPolicy {
        room_id: String,
        admission: AdmissionPolicyV2,
        expected_revision: u64,
    },
    Kick {
        room_id: String,
        participant_id: String,
        reason: String,
        causation: [u8; 16],
        expected_revision: u64,
    },
    Ban {
        room_id: String,
        participant_id: String,
        reason: String,
        causation: [u8; 16],
        expected_revision: u64,
    },
    Unban {
        room_id: String,
        participant_id: String,
        reason: String,
        causation: [u8; 16],
        expected_revision: u64,
    },
    Speak {
        room_id: String,
        expected_revision: u64,
        logical_time: u64,
        body: String,
        addressed_to: Vec<String>,
    },
    SpeakIdentified {
        room_id: String,
        expected_revision: u64,
        logical_time: u64,
        body: String,
        addressed_to: Vec<String>,
    },
    SpeakReply {
        room_id: String,
        expected_revision: u64,
        root_message_id: MessageIdV1,
        logical_time: u64,
        body: String,
        addressed_to: Vec<String>,
    },
    SpeakWithAttachments {
        room_id: String,
        expected_revision: u64,
        target: AttachmentMessageTargetV1,
        logical_time: u64,
        body: String,
        addressed_to: Vec<String>,
        attachments: Vec<AttachmentDescriptorV1>,
    },
    SpeakWithVellumArtifacts {
        room_id: String,
        expected_revision: u64,
        target: VellumArtifactMessageTargetV1,
        logical_time: u64,
        body: String,
        addressed_to: Vec<String>,
        references: Vec<VellumArtifactReferenceV1>,
    },
    WithdrawMessage {
        room_id: String,
        expected_revision: u64,
        message_id: MessageIdV1,
    },
    RedactMessage {
        room_id: String,
        expected_revision: u64,
        message_id: MessageIdV1,
        reason: String,
        causation: [u8; 16],
    },
}

impl fmt::Debug for CommandV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capabilities => formatter.write_str("Capabilities"),
            Self::Rooms { cursor, limit } => formatter
                .debug_struct("Rooms")
                .field("has_cursor", &cursor.is_some())
                .field("limit", limit)
                .finish(),
            Self::Members {
                room_id,
                cursor,
                limit,
            } => formatter
                .debug_struct("Members")
                .field("room_id_bytes", &room_id.len())
                .field("has_cursor", &cursor.is_some())
                .field("limit", limit)
                .finish(),
            Self::Bans {
                room_id,
                cursor,
                limit,
            } => formatter
                .debug_struct("Bans")
                .field("room_id_bytes", &room_id.len())
                .field("has_cursor", &cursor.is_some())
                .field("limit", limit)
                .finish(),
            Self::Messages {
                room_id,
                cursor,
                limit,
            } => formatter
                .debug_struct("Messages")
                .field("room_id_bytes", &room_id.len())
                .field("has_cursor", &cursor.is_some())
                .field("limit", limit)
                .finish(),
            Self::IdentifiedMessages {
                room_id,
                cursor,
                limit,
            } => formatter
                .debug_struct("IdentifiedMessages")
                .field("room_id_bytes", &room_id.len())
                .field("has_cursor", &cursor.is_some())
                .field("limit", limit)
                .finish(),
            Self::AttachmentMessages {
                room_id,
                cursor,
                limit,
            } => formatter
                .debug_struct("AttachmentMessages")
                .field("room_id_bytes", &room_id.len())
                .field("has_cursor", &cursor.is_some())
                .field("limit", limit)
                .finish(),
            Self::VellumArtifactMessages {
                room_id,
                cursor,
                limit,
            } => formatter
                .debug_struct("VellumArtifactMessages")
                .field("room_id_bytes", &room_id.len())
                .field("has_cursor", &cursor.is_some())
                .field("limit", limit)
                .finish(),
            Self::OpenChanges {
                room_id,
                observed_revision,
            } => formatter
                .debug_struct("OpenChanges")
                .field("room_id_bytes", &room_id.len())
                .field("observed_revision", observed_revision)
                .finish(),
            Self::Changes {
                room_id,
                cursor,
                limit,
            } => formatter
                .debug_struct("Changes")
                .field("room_id_bytes", &room_id.len())
                .field("cursor", cursor)
                .field("limit", limit)
                .finish(),
            Self::OpenIdentifiedChanges {
                room_id,
                observed_revision,
            } => formatter
                .debug_struct("OpenIdentifiedChanges")
                .field("room_id_bytes", &room_id.len())
                .field("observed_revision", observed_revision)
                .finish(),
            Self::IdentifiedChanges {
                room_id,
                cursor,
                limit,
            } => formatter
                .debug_struct("IdentifiedChanges")
                .field("room_id_bytes", &room_id.len())
                .field("cursor", cursor)
                .field("limit", limit)
                .finish(),
            Self::OpenAttachmentChanges {
                room_id,
                observed_revision,
            } => formatter
                .debug_struct("OpenAttachmentChanges")
                .field("room_id_bytes", &room_id.len())
                .field("observed_revision", observed_revision)
                .finish(),
            Self::AttachmentChanges {
                room_id,
                cursor,
                limit,
            } => formatter
                .debug_struct("AttachmentChanges")
                .field("room_id_bytes", &room_id.len())
                .field("cursor", cursor)
                .field("limit", limit)
                .finish(),
            Self::ThreadRoots {
                room_id,
                cursor,
                limit,
            } => formatter
                .debug_struct("ThreadRoots")
                .field("room_id_bytes", &room_id.len())
                .field("has_cursor", &cursor.is_some())
                .field("limit", limit)
                .finish(),
            Self::ThreadReplies {
                room_id,
                root_message_id,
                cursor,
                limit,
            } => formatter
                .debug_struct("ThreadReplies")
                .field("room_id_bytes", &room_id.len())
                .field("root_message_id", root_message_id)
                .field("has_cursor", &cursor.is_some())
                .field("limit", limit)
                .finish(),
            Self::OpenThreadedChanges {
                room_id,
                observed_revision,
            } => formatter
                .debug_struct("OpenThreadedChanges")
                .field("room_id_bytes", &room_id.len())
                .field("observed_revision", observed_revision)
                .finish(),
            Self::ThreadedChanges {
                room_id,
                cursor,
                limit,
            } => formatter
                .debug_struct("ThreadedChanges")
                .field("room_id_bytes", &room_id.len())
                .field("cursor", cursor)
                .field("limit", limit)
                .finish(),
            Self::OpenTopicChanges {
                room_id,
                observed_revision,
            } => formatter
                .debug_struct("OpenTopicChanges")
                .field("room_id_bytes", &room_id.len())
                .field("observed_revision", observed_revision)
                .finish(),
            Self::TopicChanges {
                room_id,
                cursor,
                limit,
            } => formatter
                .debug_struct("TopicChanges")
                .field("room_id_bytes", &room_id.len())
                .field("cursor", cursor)
                .field("limit", limit)
                .finish(),
            Self::CreateRoom {
                room_id,
                name,
                admission,
                expected_revision,
            } => formatter
                .debug_struct("CreateRoom")
                .field("room_id_bytes", &room_id.len())
                .field("name_bytes", &name.len())
                .field("admission", admission)
                .field("expected_revision", expected_revision)
                .finish(),
            Self::CreatePeerConversation {
                room_id,
                name,
                profile,
                founding_participant_ids,
                expected_revision,
            } => formatter
                .debug_struct("CreatePeerConversation")
                .field("room_id_bytes", &room_id.len())
                .field("name_bytes", &name.len())
                .field("profile", profile)
                .field(
                    "founding_participant_count",
                    &founding_participant_ids.len(),
                )
                .field("expected_revision", expected_revision)
                .finish(),
            Self::ConversationProfile { room_id } => formatter
                .debug_struct("ConversationProfile")
                .field("room_id_bytes", &room_id.len())
                .finish(),
            Self::Invite {
                room_id,
                participant_id,
                expected_revision,
            } => formatter
                .debug_struct("Invite")
                .field("room_id_bytes", &room_id.len())
                .field("participant_id_bytes", &participant_id.len())
                .field("expected_revision", expected_revision)
                .finish(),
            Self::Join {
                room_id,
                expected_revision,
            } => formatter
                .debug_struct("Join")
                .field("room_id_bytes", &room_id.len())
                .field("expected_revision", expected_revision)
                .finish(),
            Self::Leave {
                room_id,
                expected_revision,
            } => formatter
                .debug_struct("Leave")
                .field("room_id_bytes", &room_id.len())
                .field("expected_revision", expected_revision)
                .finish(),
            Self::SetRole {
                room_id,
                participant_id,
                role,
                expected_revision,
            } => formatter
                .debug_struct("SetRole")
                .field("room_id_bytes", &room_id.len())
                .field("participant_id_bytes", &participant_id.len())
                .field("role", role)
                .field("expected_revision", expected_revision)
                .finish(),
            Self::SetAdmissionPolicy {
                room_id,
                admission,
                expected_revision,
            } => formatter
                .debug_struct("SetAdmissionPolicy")
                .field("room_id_bytes", &room_id.len())
                .field("admission", admission)
                .field("expected_revision", expected_revision)
                .finish(),
            Self::Kick {
                room_id,
                participant_id,
                reason,
                expected_revision,
                ..
            }
            | Self::Ban {
                room_id,
                participant_id,
                reason,
                expected_revision,
                ..
            }
            | Self::Unban {
                room_id,
                participant_id,
                reason,
                expected_revision,
                ..
            } => formatter
                .debug_struct("Moderation")
                .field("room_id_bytes", &room_id.len())
                .field("participant_id_bytes", &participant_id.len())
                .field("reason_bytes", &reason.len())
                .field("causation", &"<opaque>")
                .field("expected_revision", expected_revision)
                .finish(),
            Self::Speak {
                room_id,
                expected_revision,
                logical_time,
                body,
                addressed_to,
            }
            | Self::SpeakIdentified {
                room_id,
                expected_revision,
                logical_time,
                body,
                addressed_to,
            } => formatter
                .debug_struct("Speak")
                .field("room_id_bytes", &room_id.len())
                .field("expected_revision", expected_revision)
                .field("logical_time", logical_time)
                .field("body_bytes", &body.len())
                .field("addressed_count", &addressed_to.len())
                .finish(),
            Self::SpeakReply {
                room_id,
                expected_revision,
                root_message_id,
                logical_time,
                body,
                addressed_to,
            } => formatter
                .debug_struct("SpeakReply")
                .field("room_id_bytes", &room_id.len())
                .field("expected_revision", expected_revision)
                .field("root_message_id", root_message_id)
                .field("logical_time", logical_time)
                .field("body_bytes", &body.len())
                .field("addressed_count", &addressed_to.len())
                .finish(),
            Self::SpeakWithAttachments {
                room_id,
                expected_revision,
                target,
                logical_time,
                body,
                addressed_to,
                attachments,
            } => formatter
                .debug_struct("SpeakWithAttachments")
                .field("room_id_bytes", &room_id.len())
                .field("expected_revision", expected_revision)
                .field("target", target)
                .field("logical_time", logical_time)
                .field("body_bytes", &body.len())
                .field("addressed_count", &addressed_to.len())
                .field("attachment_count", &attachments.len())
                .finish(),
            Self::SpeakWithVellumArtifacts {
                room_id,
                expected_revision,
                target,
                logical_time,
                body,
                addressed_to,
                references,
            } => formatter
                .debug_struct("SpeakWithVellumArtifacts")
                .field("room_id_bytes", &room_id.len())
                .field("expected_revision", expected_revision)
                .field("target", target)
                .field("logical_time", logical_time)
                .field("body_bytes", &body.len())
                .field("addressed_count", &addressed_to.len())
                .field("reference_count", &references.len())
                .finish(),
            Self::WithdrawMessage {
                room_id,
                expected_revision,
                message_id,
            } => formatter
                .debug_struct("WithdrawMessage")
                .field("room_id_bytes", &room_id.len())
                .field("expected_revision", expected_revision)
                .field("message_id", message_id)
                .finish(),
            Self::RedactMessage {
                room_id,
                expected_revision,
                message_id,
                reason,
                ..
            } => formatter
                .debug_struct("RedactMessage")
                .field("room_id_bytes", &room_id.len())
                .field("expected_revision", expected_revision)
                .field("message_id", message_id)
                .field("reason_bytes", &reason.len())
                .field("causation", &"<opaque>")
                .finish(),
            Self::ArchiveThread {
                room_id,
                expected_revision,
                root_message_id,
                reason,
                ..
            }
            | Self::ReopenThread {
                room_id,
                expected_revision,
                root_message_id,
                reason,
                ..
            } => formatter
                .debug_struct("ThreadLifecycle")
                .field("room_id_bytes", &room_id.len())
                .field("expected_revision", expected_revision)
                .field("root_message_id", root_message_id)
                .field("reason_bytes", &reason.len())
                .field("causation", &"<opaque>")
                .finish(),
            Self::ThreadStatus {
                room_id,
                root_message_id,
            } => formatter
                .debug_struct("ThreadStatus")
                .field("room_id_bytes", &room_id.len())
                .field("root_message_id", root_message_id)
                .finish(),
            Self::AddReaction {
                room_id,
                expected_revision,
                message_id,
                key,
            }
            | Self::RemoveReaction {
                room_id,
                expected_revision,
                message_id,
                key,
            } => formatter
                .debug_struct("ReactionChange")
                .field("room_id_bytes", &room_id.len())
                .field("expected_revision", expected_revision)
                .field("message_id", message_id)
                .field("key_bytes", &key.len())
                .finish(),
            Self::ReactionStatus {
                room_id,
                message_id,
            } => formatter
                .debug_struct("ReactionStatus")
                .field("room_id_bytes", &room_id.len())
                .field("message_id", message_id)
                .finish(),
            Self::EditMessage {
                room_id,
                expected_revision,
                message_id,
                logical_time,
                body,
            } => formatter
                .debug_struct("EditMessage")
                .field("room_id_bytes", &room_id.len())
                .field("expected_revision", expected_revision)
                .field("message_id", message_id)
                .field("logical_time", logical_time)
                .field("body_bytes", &body.len())
                .finish(),
            Self::MessageRevisions {
                room_id,
                message_id,
                cursor,
                limit,
            } => formatter
                .debug_struct("MessageRevisions")
                .field("room_id_bytes", &room_id.len())
                .field("message_id", message_id)
                .field("has_cursor", &cursor.is_some())
                .field("limit", limit)
                .finish(),
            Self::Topics {
                room_id,
                cursor,
                limit,
            } => formatter
                .debug_struct("Topics")
                .field("room_id_bytes", &room_id.len())
                .field("has_cursor", &cursor.is_some())
                .field("limit", limit)
                .finish(),
            Self::TopicMessages {
                room_id,
                topic_id,
                cursor,
                limit,
            } => formatter
                .debug_struct("TopicMessages")
                .field("room_id_bytes", &room_id.len())
                .field("topic_id", topic_id)
                .field("has_cursor", &cursor.is_some())
                .field("limit", limit)
                .finish(),
            Self::CreateTopic {
                room_id,
                expected_revision,
                name,
            } => formatter
                .debug_struct("CreateTopic")
                .field("room_id_bytes", &room_id.len())
                .field("expected_revision", expected_revision)
                .field("name_bytes", &name.len())
                .finish(),
            Self::RenameTopic {
                room_id,
                expected_revision,
                topic_id,
                name,
            } => formatter
                .debug_struct("RenameTopic")
                .field("room_id_bytes", &room_id.len())
                .field("expected_revision", expected_revision)
                .field("topic_id", topic_id)
                .field("name_bytes", &name.len())
                .finish(),
            Self::ResolveTopic {
                room_id,
                expected_revision,
                topic_id,
            }
            | Self::ReopenTopic {
                room_id,
                expected_revision,
                topic_id,
            } => formatter
                .debug_struct("TopicLifecycle")
                .field("room_id_bytes", &room_id.len())
                .field("expected_revision", expected_revision)
                .field("topic_id", topic_id)
                .finish(),
            Self::TopicStatus { room_id, topic_id } => formatter
                .debug_struct("TopicStatus")
                .field("room_id_bytes", &room_id.len())
                .field("topic_id", topic_id)
                .finish(),
            Self::SupersedeTopic {
                room_id,
                expected_revision,
                topic_id,
                successor_topic_id,
            } => formatter
                .debug_struct("SupersedeTopic")
                .field("room_id_bytes", &room_id.len())
                .field("expected_revision", expected_revision)
                .field("topic_id", topic_id)
                .field("successor_topic_id", successor_topic_id)
                .finish(),
            Self::ClearTopicSupersession {
                room_id,
                expected_revision,
                topic_id,
            } => formatter
                .debug_struct("ClearTopicSupersession")
                .field("room_id_bytes", &room_id.len())
                .field("expected_revision", expected_revision)
                .field("topic_id", topic_id)
                .finish(),
            Self::TopicSupersessionStatus { room_id, topic_id } => formatter
                .debug_struct("TopicSupersessionStatus")
                .field("room_id_bytes", &room_id.len())
                .field("topic_id", topic_id)
                .finish(),
            Self::AddTopicAlias {
                room_id,
                expected_revision,
                topic_id,
                alias,
            }
            | Self::RemoveTopicAlias {
                room_id,
                expected_revision,
                topic_id,
                alias,
            } => formatter
                .debug_struct("TopicAliasMutation")
                .field("room_id_bytes", &room_id.len())
                .field("expected_revision", expected_revision)
                .field("topic_id", topic_id)
                .field("alias_bytes", &alias.len())
                .finish(),
            Self::TopicAliasStatus { room_id, topic_id } => formatter
                .debug_struct("TopicAliasStatus")
                .field("room_id_bytes", &room_id.len())
                .field("topic_id", topic_id)
                .finish(),
            Self::AddTopicTag {
                room_id,
                expected_revision,
                topic_id,
                tag,
            }
            | Self::RemoveTopicTag {
                room_id,
                expected_revision,
                topic_id,
                tag,
            } => formatter
                .debug_struct("TopicTagMutation")
                .field("room_id_bytes", &room_id.len())
                .field("expected_revision", expected_revision)
                .field("topic_id", topic_id)
                .field("tag_bytes", &tag.len())
                .finish(),
            Self::TopicTagStatus { room_id, topic_id } => formatter
                .debug_struct("TopicTagStatus")
                .field("room_id_bytes", &room_id.len())
                .field("topic_id", topic_id)
                .finish(),
            Self::SpeakInTopic {
                room_id,
                expected_revision,
                topic_id,
                logical_time,
                body,
                addressed_to,
            } => formatter
                .debug_struct("SpeakInTopic")
                .field("room_id_bytes", &room_id.len())
                .field("expected_revision", expected_revision)
                .field("topic_id", topic_id)
                .field("logical_time", logical_time)
                .field("body_bytes", &body.len())
                .field("addressed_count", &addressed_to.len())
                .finish(),
            Self::ReadPosition { room_id, scope } => formatter
                .debug_struct("ReadPosition")
                .field("room_id_bytes", &room_id.len())
                .field("scope", scope)
                .finish(),
            Self::AdvanceReadPosition {
                room_id,
                observed_revision,
                scope,
                target_message_id,
            } => formatter
                .debug_struct("AdvanceReadPosition")
                .field("room_id_bytes", &room_id.len())
                .field("observed_revision", observed_revision)
                .field("scope", scope)
                .field("target_message_id", target_message_id)
                .finish(),
            Self::AttentionPreference { room_id, scope } => formatter
                .debug_struct("AttentionPreference")
                .field("room_id_bytes", &room_id.len())
                .field("scope", scope)
                .finish(),
            Self::SetAttentionPreference {
                room_id,
                observed_revision,
                scope,
                preference: _,
            } => formatter
                .debug_struct("SetAttentionPreference")
                .field("room_id_bytes", &room_id.len())
                .field("observed_revision", observed_revision)
                .field("scope", scope)
                .field("preference", &"<redacted>")
                .finish(),
        }
    }
}

/// Closed feature profiles supported by this exact service implementation.
/// Discovery reports syntax and semantics, never participant authority.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CapabilityV2 {
    RoomLifecycleV2,
    RetainedHistoryV2,
    RoomChangesV1,
    ModerationV1,
    RoomPolicyV1,
    BanObservationV1,
    IdentifiedMessagesV1,
    AuthorWithdrawalV1,
    ModeratorRedactionV1,
    IdentifiedRoomChangesV1,
    ThreadsV1,
    ThreadLifecycleV1,
    ReactionsV1,
    MessageRevisionsV1,
    TopicsV1,
    PrivateReadPositionsV1,
    IndependentChangeRetentionV1,
    TopicRenameV1,
    TopicResolutionV1,
    TopicSupersessionV1,
    TopicAliasesV1,
    TopicTagsV1,
    ImmutableAttachmentReferencesV1,
    VellumArtifactReferencesV1,
    ConversationProfilesV1,
    PrivateAttentionPreferencesV1,
}

#[derive(Clone, Eq, PartialEq)]
pub struct RequestV2 {
    pub request_id: RequestIdV2,
    pub command: CommandV2,
}

impl fmt::Debug for RequestV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RequestV2")
            .field("request_id", &self.request_id)
            .field("command", &self.command)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct RoomSummaryV2 {
    pub room_id: String,
    pub name: String,
    pub admission: AdmissionPolicyV2,
    pub observer_role: RoleV2,
    pub member_count: u32,
    pub retained_message_count: u32,
    pub next_message_sequence: u64,
}

impl fmt::Debug for RoomSummaryV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RoomSummaryV2")
            .field("room_id_bytes", &self.room_id.len())
            .field("name_bytes", &self.name.len())
            .field("admission", &self.admission)
            .field("observer_role", &self.observer_role)
            .field("member_count", &self.member_count)
            .field("retained_message_count", &self.retained_message_count)
            .field("next_message_sequence", &self.next_message_sequence)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct MemberV2 {
    pub participant_id: String,
    pub display_name: String,
    pub kind: ParticipantKindV2,
    pub role: RoleV2,
}

impl fmt::Debug for MemberV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MemberV2")
            .field("participant_id_bytes", &self.participant_id.len())
            .field("display_name_bytes", &self.display_name.len())
            .field("kind", &self.kind)
            .field("role", &self.role)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct BannedParticipantV2 {
    pub participant_id: String,
    pub display_name: String,
    pub kind: ParticipantKindV2,
    pub prior_role: RoleV2,
}

impl fmt::Debug for BannedParticipantV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BannedParticipantV2")
            .field("participant_id_bytes", &self.participant_id.len())
            .field("display_name_bytes", &self.display_name.len())
            .field("kind", &self.kind)
            .field("prior_role", &self.prior_role)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct MessageV2 {
    pub sequence: u64,
    pub author: String,
    pub logical_time: u64,
    pub body: String,
    pub addressed_to: Vec<String>,
}

impl fmt::Debug for MessageV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MessageV2")
            .field("sequence", &self.sequence)
            .field("author_bytes", &self.author.len())
            .field("logical_time", &self.logical_time)
            .field("body_bytes", &self.body.len())
            .field("addressed_count", &self.addressed_to.len())
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct IdentifiedMessageV1 {
    pub message_id: MessageIdV1,
    pub sequence: u64,
    pub author: String,
    pub logical_time: u64,
    pub body: String,
    pub addressed_to: Vec<String>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct AttachmentMessageV1 {
    pub message_id: MessageIdV1,
    pub sequence: u64,
    pub author: String,
    pub logical_time: u64,
    pub body: String,
    pub addressed_to: Vec<String>,
    pub reply_to_root: Option<MessageIdV1>,
    pub topic_id: Option<TopicIdV1>,
    pub attachments: Vec<MessageAttachmentV1>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct VellumArtifactMessageV1 {
    pub message_id: MessageIdV1,
    pub sequence: u64,
    pub author: String,
    pub logical_time: u64,
    pub body: String,
    pub addressed_to: Vec<String>,
    pub reply_to_root: Option<MessageIdV1>,
    pub topic_id: Option<TopicIdV1>,
    pub references: Vec<MessageVellumArtifactReferenceV1>,
}

impl fmt::Debug for VellumArtifactMessageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VellumArtifactMessageV1")
            .field("message_id", &self.message_id)
            .field("sequence", &self.sequence)
            .field("author_bytes", &self.author.len())
            .field("logical_time", &self.logical_time)
            .field("body_bytes", &self.body.len())
            .field("addressed_count", &self.addressed_to.len())
            .field("is_reply", &self.reply_to_root.is_some())
            .field("has_topic", &self.topic_id.is_some())
            .field("reference_count", &self.references.len())
            .finish()
    }
}

impl fmt::Debug for AttachmentMessageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AttachmentMessageV1")
            .field("message_id", &self.message_id)
            .field("sequence", &self.sequence)
            .field("author_bytes", &self.author.len())
            .field("logical_time", &self.logical_time)
            .field("body_bytes", &self.body.len())
            .field("addressed_count", &self.addressed_to.len())
            .field("is_reply", &self.reply_to_root.is_some())
            .field("has_topic", &self.topic_id.is_some())
            .field("attachment_count", &self.attachments.len())
            .finish()
    }
}

impl fmt::Debug for IdentifiedMessageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IdentifiedMessageV1")
            .field("message_id", &self.message_id)
            .field("sequence", &self.sequence)
            .field("author_bytes", &self.author.len())
            .field("logical_time", &self.logical_time)
            .field("body_bytes", &self.body.len())
            .field("addressed_count", &self.addressed_to.len())
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct ThreadedMessageV1 {
    pub message_id: MessageIdV1,
    pub sequence: u64,
    pub author: String,
    pub logical_time: u64,
    pub body: String,
    pub addressed_to: Vec<String>,
    pub reply_to_root: Option<MessageIdV1>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct TopicV1 {
    pub topic_id: TopicIdV1,
    pub name: String,
    pub creator_id: String,
}

impl fmt::Debug for TopicV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TopicV1")
            .field("topic_id", &self.topic_id)
            .field("name_bytes", &self.name.len())
            .field("creator_id_bytes", &self.creator_id.len())
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct TopicStatusV1 {
    pub revision: u64,
    pub room_id: String,
    pub topic_id: TopicIdV1,
    pub resolved: bool,
}

#[derive(Clone, Eq, PartialEq)]
pub struct TopicSupersessionStatusV1 {
    pub revision: u64,
    pub room_id: String,
    pub topic_id: TopicIdV1,
    pub successor_topic_id: Option<TopicIdV1>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct TopicAliasStatusV1 {
    pub revision: u64,
    pub room_id: String,
    pub topic_id: TopicIdV1,
    pub aliases: Vec<String>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct TopicTagStatusV1 {
    pub revision: u64,
    pub room_id: String,
    pub topic_id: TopicIdV1,
    pub tags: Vec<String>,
}

impl fmt::Debug for TopicTagStatusV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TopicTagStatusV1")
            .field("revision", &self.revision)
            .field("room_id_bytes", &self.room_id.len())
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
            .field("room_id_bytes", &self.room_id.len())
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
            .field("room_id_bytes", &self.room_id.len())
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
            .field("room_id_bytes", &self.room_id.len())
            .field("topic_id", &self.topic_id)
            .field("resolved", &self.resolved)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct TopicMessageV1 {
    pub message_id: MessageIdV1,
    pub sequence: u64,
    pub author: String,
    pub logical_time: u64,
    pub body: String,
    pub addressed_to: Vec<String>,
    pub reply_to_root: Option<MessageIdV1>,
    pub topic_id: Option<TopicIdV1>,
}

impl fmt::Debug for TopicMessageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TopicMessageV1")
            .field("message_id", &self.message_id)
            .field("sequence", &self.sequence)
            .field("author_bytes", &self.author.len())
            .field("logical_time", &self.logical_time)
            .field("body_bytes", &self.body.len())
            .field("addressed_count", &self.addressed_to.len())
            .field("is_reply", &self.reply_to_root.is_some())
            .field("has_topic", &self.topic_id.is_some())
            .finish()
    }
}

impl fmt::Debug for ThreadedMessageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ThreadedMessageV1")
            .field("message_id", &self.message_id)
            .field("sequence", &self.sequence)
            .field("author_bytes", &self.author.len())
            .field("logical_time", &self.logical_time)
            .field("body_bytes", &self.body.len())
            .field("addressed_count", &self.addressed_to.len())
            .field("is_reply", &self.reply_to_root.is_some())
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct RoomPageV2 {
    pub revision: u64,
    pub rooms: Vec<RoomSummaryV2>,
    pub next: Option<PageCursorV2>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct MemberPageV2 {
    pub revision: u64,
    pub room_id: String,
    pub members: Vec<MemberV2>,
    pub next: Option<PageCursorV2>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct BanPageV2 {
    pub revision: u64,
    pub room_id: String,
    pub banned: Vec<BannedParticipantV2>,
    pub next: Option<PageCursorV2>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct MessagePageV2 {
    pub revision: u64,
    pub room_id: String,
    pub messages: Vec<MessageV2>,
    pub oldest_retained_sequence: u64,
    pub next_message_sequence: u64,
    pub next: Option<PageCursorV2>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct IdentifiedMessagePageV1 {
    pub revision: u64,
    pub room_id: String,
    pub messages: Vec<IdentifiedMessageV1>,
    pub oldest_retained_sequence: u64,
    pub next_message_sequence: u64,
    pub next: Option<PageCursorV2>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct AttachmentMessagePageV1 {
    pub revision: u64,
    pub room_id: String,
    pub messages: Vec<AttachmentMessageV1>,
    pub oldest_retained_sequence: u64,
    pub next_message_sequence: u64,
    pub next: Option<PageCursorV2>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct VellumArtifactMessagePageV1 {
    pub revision: u64,
    pub room_id: String,
    pub messages: Vec<VellumArtifactMessageV1>,
    pub oldest_retained_sequence: u64,
    pub next_message_sequence: u64,
    pub next: Option<PageCursorV2>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct ThreadMessagePageV1 {
    pub revision: u64,
    pub room_id: String,
    pub root_message_id: Option<MessageIdV1>,
    pub messages: Vec<ThreadedMessageV1>,
    pub oldest_retained_sequence: u64,
    pub next_message_sequence: u64,
    pub next: Option<PageCursorV2>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct TopicPageV1 {
    pub revision: u64,
    pub room_id: String,
    pub topics: Vec<TopicV1>,
    pub next: Option<PageCursorV2>,
}

impl fmt::Debug for TopicPageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TopicPageV1")
            .field("revision", &self.revision)
            .field("room_id_bytes", &self.room_id.len())
            .field("topic_count", &self.topics.len())
            .field("has_next", &self.next.is_some())
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct TopicMessagePageV1 {
    pub revision: u64,
    pub room_id: String,
    pub topic_id: TopicIdV1,
    pub messages: Vec<TopicMessageV1>,
    pub oldest_retained_sequence: u64,
    pub next_message_sequence: u64,
    pub next: Option<PageCursorV2>,
}

impl fmt::Debug for TopicMessagePageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TopicMessagePageV1")
            .field("revision", &self.revision)
            .field("room_id_bytes", &self.room_id.len())
            .field("topic_id", &self.topic_id)
            .field("message_count", &self.messages.len())
            .field("has_next", &self.next.is_some())
            .finish()
    }
}

impl fmt::Debug for ThreadMessagePageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ThreadMessagePageV1")
            .field("revision", &self.revision)
            .field("room_id_bytes", &self.room_id.len())
            .field("is_reply_page", &self.root_message_id.is_some())
            .field("message_count", &self.messages.len())
            .field("has_next", &self.next.is_some())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThreadLifecycleStateV1 {
    Active,
    Archived,
}

#[derive(Clone, Eq, PartialEq)]
pub struct ThreadStatusV1 {
    pub revision: u64,
    pub room_id: String,
    pub root_message_id: MessageIdV1,
    pub state: ThreadLifecycleStateV1,
}

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

#[derive(Clone, Eq, PartialEq)]
pub struct ReactionStatusV1 {
    pub revision: u64,
    pub room_id: String,
    pub message_id: MessageIdV1,
    pub reactions: Vec<ReactionSummaryV1>,
}

impl fmt::Debug for ReactionStatusV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReactionStatusV1")
            .field("revision", &self.revision)
            .field("room_id_bytes", &self.room_id.len())
            .field("message_id", &self.message_id)
            .field("reaction_count", &self.reactions.len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageRevisionAvailabilityV1 {
    Available,
    Tombstoned,
}

#[derive(Clone, Eq, PartialEq)]
pub struct MessageRevisionV1 {
    pub number: u32,
    pub editor_id: String,
    pub logical_time: u64,
    pub availability: MessageRevisionAvailabilityV1,
    pub body: String,
}

impl fmt::Debug for MessageRevisionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MessageRevisionV1")
            .field("number", &self.number)
            .field("editor_id_bytes", &self.editor_id.len())
            .field("logical_time", &self.logical_time)
            .field("availability", &self.availability)
            .field("body_bytes", &self.body.len())
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct MessageRevisionPageV1 {
    pub revision: u64,
    pub room_id: String,
    pub message_id: MessageIdV1,
    pub revisions: Vec<MessageRevisionV1>,
    pub next: Option<PageCursorV2>,
}

impl fmt::Debug for MessageRevisionPageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MessageRevisionPageV1")
            .field("revision", &self.revision)
            .field("room_id_bytes", &self.room_id.len())
            .field("message_id", &self.message_id)
            .field("revision_count", &self.revisions.len())
            .field("has_next", &self.next.is_some())
            .finish()
    }
}

impl fmt::Debug for ThreadStatusV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ThreadStatusV1")
            .field("revision", &self.revision)
            .field("room_id_bytes", &self.room_id.len())
            .field("root_message_id", &self.root_message_id)
            .field("state", &self.state)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct PeerConversationCreatedV1 {
    pub initiator_id: String,
    pub profile: ConversationProfileV1,
    pub founding_participant_ids: Vec<String>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct ConversationProfileStatusV1 {
    pub revision: u64,
    pub room_id: String,
    pub profile: ConversationProfileV1,
    pub founding_participant_ids: Vec<String>,
}

impl fmt::Debug for ConversationProfileStatusV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConversationProfileStatusV1")
            .field("revision", &self.revision)
            .field("room_id_bytes", &self.room_id.len())
            .field("profile", &self.profile)
            .field(
                "founding_participant_count",
                &self.founding_participant_ids.len(),
            )
            .finish()
    }
}

impl fmt::Debug for PeerConversationCreatedV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PeerConversationCreatedV1")
            .field("initiator_id_bytes", &self.initiator_id.len())
            .field("profile", &self.profile)
            .field(
                "founding_participant_count",
                &self.founding_participant_ids.len(),
            )
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum RoomEventV2 {
    RoomCreated {
        owner: String,
        admission: AdmissionPolicyV2,
    },
    PeerConversationCreated(PeerConversationCreatedV1),
    TopicCreated {
        actor_id: String,
        topic_id: TopicIdV1,
        name: String,
    },
    TopicRenamed {
        actor_id: String,
        topic_id: TopicIdV1,
        previous: String,
        name: String,
    },
    TopicLifecycleChanged {
        actor_id: String,
        topic_id: TopicIdV1,
        previous_resolved: bool,
        resolved: bool,
    },
    TopicSupersessionChanged {
        actor_id: String,
        topic_id: TopicIdV1,
        previous_successor_topic_id: Option<TopicIdV1>,
        successor_topic_id: Option<TopicIdV1>,
    },
    TopicAliasChanged {
        actor_id: String,
        topic_id: TopicIdV1,
        alias: String,
        added: bool,
    },
    TopicTagChanged {
        actor_id: String,
        topic_id: TopicIdV1,
        tag: String,
        added: bool,
    },
    InvitationRecorded {
        participant_id: String,
    },
    ParticipantJoined {
        participant_id: String,
    },
    ParticipantLeft {
        participant_id: String,
    },
    RoleChanged {
        participant_id: String,
        role: RoleV2,
    },
    RoomPolicyChanged {
        actor_id: String,
        previous: AdmissionPolicyV2,
        admission: AdmissionPolicyV2,
    },
    ParticipantKicked {
        actor_id: String,
        participant_id: String,
        reason: String,
        causation: [u8; 16],
    },
    ParticipantBanned {
        actor_id: String,
        participant_id: String,
        reason: String,
        causation: [u8; 16],
    },
    ParticipantUnbanned {
        actor_id: String,
        participant_id: String,
        reason: String,
        causation: [u8; 16],
    },
    MessageAccepted(MessageV2),
    MessageWithdrawn {
        actor_id: String,
        message_id: MessageIdV1,
        sequence: u64,
    },
    MessageRedacted {
        actor_id: String,
        author_id: String,
        message_id: MessageIdV1,
        sequence: u64,
        reason: String,
        causation: [u8; 16],
    },
    ThreadLifecycleChanged {
        actor_id: String,
        root_message_id: MessageIdV1,
        archived: bool,
        reason: String,
        causation: [u8; 16],
    },
    ReactionChanged {
        actor_id: String,
        message_id: MessageIdV1,
        key: String,
        reacted: bool,
    },
    MessageEdited {
        actor_id: String,
        message_id: MessageIdV1,
        message_revision: u32,
        logical_time: u64,
        body: String,
    },
}

impl fmt::Debug for RoomEventV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RoomCreated { admission, .. } => formatter
                .debug_struct("RoomCreated")
                .field("admission", admission)
                .finish_non_exhaustive(),
            Self::PeerConversationCreated(event) => event.fmt(formatter),
            Self::TopicCreated { actor_id, name, .. } => formatter
                .debug_struct("TopicCreated")
                .field("actor_id_bytes", &actor_id.len())
                .field("name_bytes", &name.len())
                .finish_non_exhaustive(),
            Self::TopicRenamed {
                actor_id,
                previous,
                name,
                ..
            } => formatter
                .debug_struct("TopicRenamed")
                .field("actor_id_bytes", &actor_id.len())
                .field("previous_bytes", &previous.len())
                .field("name_bytes", &name.len())
                .finish_non_exhaustive(),
            Self::TopicLifecycleChanged {
                actor_id,
                topic_id,
                previous_resolved,
                resolved,
            } => formatter
                .debug_struct("TopicLifecycleChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("topic_id", topic_id)
                .field("previous_resolved", previous_resolved)
                .field("resolved", resolved)
                .finish(),
            Self::TopicSupersessionChanged {
                actor_id,
                topic_id,
                previous_successor_topic_id,
                successor_topic_id,
            } => formatter
                .debug_struct("TopicSupersessionChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("topic_id", topic_id)
                .field("previous_successor_topic_id", previous_successor_topic_id)
                .field("successor_topic_id", successor_topic_id)
                .finish(),
            Self::TopicAliasChanged {
                actor_id,
                topic_id,
                alias,
                added,
            } => formatter
                .debug_struct("TopicAliasChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("topic_id", topic_id)
                .field("alias_bytes", &alias.len())
                .field("added", added)
                .finish(),
            Self::TopicTagChanged {
                actor_id,
                topic_id,
                tag,
                added,
            } => formatter
                .debug_struct("TopicTagChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("topic_id", topic_id)
                .field("tag_bytes", &tag.len())
                .field("added", added)
                .finish(),
            Self::InvitationRecorded { participant_id }
            | Self::ParticipantJoined { participant_id }
            | Self::ParticipantLeft { participant_id } => formatter
                .debug_struct("ParticipantChange")
                .field("participant_id_bytes", &participant_id.len())
                .finish_non_exhaustive(),
            Self::RoleChanged {
                participant_id,
                role,
            } => formatter
                .debug_struct("RoleChanged")
                .field("participant_id_bytes", &participant_id.len())
                .field("role", role)
                .finish(),
            Self::RoomPolicyChanged {
                actor_id,
                previous,
                admission,
            } => formatter
                .debug_struct("RoomPolicyChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("previous", previous)
                .field("admission", admission)
                .finish(),
            Self::ParticipantKicked {
                actor_id,
                participant_id,
                reason,
                ..
            }
            | Self::ParticipantBanned {
                actor_id,
                participant_id,
                reason,
                ..
            }
            | Self::ParticipantUnbanned {
                actor_id,
                participant_id,
                reason,
                ..
            } => formatter
                .debug_struct("ModerationEvent")
                .field("actor_id_bytes", &actor_id.len())
                .field("participant_id_bytes", &participant_id.len())
                .field("reason_bytes", &reason.len())
                .field("causation", &"<opaque>")
                .finish(),
            Self::MessageAccepted(message) => formatter
                .debug_tuple("MessageAccepted")
                .field(message)
                .finish(),
            Self::MessageWithdrawn {
                actor_id,
                message_id,
                sequence,
            } => formatter
                .debug_struct("MessageWithdrawn")
                .field("actor_id_bytes", &actor_id.len())
                .field("message_id", message_id)
                .field("sequence", sequence)
                .finish(),
            Self::MessageRedacted {
                actor_id,
                author_id,
                message_id,
                sequence,
                reason,
                ..
            } => formatter
                .debug_struct("MessageRedacted")
                .field("actor_id_bytes", &actor_id.len())
                .field("author_id_bytes", &author_id.len())
                .field("message_id", message_id)
                .field("sequence", sequence)
                .field("reason_bytes", &reason.len())
                .field("causation", &"<opaque>")
                .finish(),
            Self::ThreadLifecycleChanged {
                actor_id,
                root_message_id,
                archived,
                reason,
                ..
            } => formatter
                .debug_struct("ThreadLifecycleChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("root_message_id", root_message_id)
                .field("archived", archived)
                .field("reason_bytes", &reason.len())
                .field("causation", &"<opaque>")
                .finish(),
            Self::ReactionChanged {
                actor_id,
                message_id,
                key,
                reacted,
            } => formatter
                .debug_struct("ReactionChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("message_id", message_id)
                .field("key_bytes", &key.len())
                .field("reacted", reacted)
                .finish(),
            Self::MessageEdited {
                actor_id,
                message_id,
                message_revision,
                logical_time,
                body,
            } => formatter
                .debug_struct("MessageEdited")
                .field("actor_id_bytes", &actor_id.len())
                .field("message_id", message_id)
                .field("message_revision", message_revision)
                .field("logical_time", logical_time)
                .field("body_bytes", &body.len())
                .finish(),
        }
    }
}

/// Additive room-event profile whose accepted-message variant carries stable
/// identity. It is deliberately distinct from the frozen `RoomEventV2` grammar.
#[derive(Clone, Eq, PartialEq)]
pub enum IdentifiedRoomEventV1 {
    RoomCreated {
        owner: String,
        admission: AdmissionPolicyV2,
    },
    PeerConversationCreated(PeerConversationCreatedV1),
    TopicCreated {
        actor_id: String,
        topic_id: TopicIdV1,
        name: String,
    },
    TopicRenamed {
        actor_id: String,
        topic_id: TopicIdV1,
        previous: String,
        name: String,
    },
    TopicLifecycleChanged {
        actor_id: String,
        topic_id: TopicIdV1,
        previous_resolved: bool,
        resolved: bool,
    },
    TopicSupersessionChanged {
        actor_id: String,
        topic_id: TopicIdV1,
        previous_successor_topic_id: Option<TopicIdV1>,
        successor_topic_id: Option<TopicIdV1>,
    },
    TopicAliasChanged {
        actor_id: String,
        topic_id: TopicIdV1,
        alias: String,
        added: bool,
    },
    TopicTagChanged {
        actor_id: String,
        topic_id: TopicIdV1,
        tag: String,
        added: bool,
    },
    InvitationRecorded {
        participant_id: String,
    },
    ParticipantJoined {
        participant_id: String,
    },
    ParticipantLeft {
        participant_id: String,
    },
    RoleChanged {
        participant_id: String,
        role: RoleV2,
    },
    RoomPolicyChanged {
        actor_id: String,
        previous: AdmissionPolicyV2,
        admission: AdmissionPolicyV2,
    },
    ParticipantKicked {
        actor_id: String,
        participant_id: String,
        reason: String,
        causation: [u8; 16],
    },
    ParticipantBanned {
        actor_id: String,
        participant_id: String,
        reason: String,
        causation: [u8; 16],
    },
    ParticipantUnbanned {
        actor_id: String,
        participant_id: String,
        reason: String,
        causation: [u8; 16],
    },
    MessageAccepted(IdentifiedMessageV1),
    MessageWithdrawn {
        actor_id: String,
        message_id: MessageIdV1,
        sequence: u64,
    },
    MessageRedacted {
        actor_id: String,
        author_id: String,
        message_id: MessageIdV1,
        sequence: u64,
        reason: String,
        causation: [u8; 16],
    },
    ThreadLifecycleChanged {
        actor_id: String,
        root_message_id: MessageIdV1,
        archived: bool,
        reason: String,
        causation: [u8; 16],
    },
    ReactionChanged {
        actor_id: String,
        message_id: MessageIdV1,
        key: String,
        reacted: bool,
    },
    MessageEdited {
        actor_id: String,
        message_id: MessageIdV1,
        message_revision: u32,
        logical_time: u64,
        body: String,
    },
}

impl fmt::Debug for IdentifiedRoomEventV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RoomCreated { admission, .. } => formatter
                .debug_struct("RoomCreated")
                .field("admission", admission)
                .finish_non_exhaustive(),
            Self::PeerConversationCreated(event) => event.fmt(formatter),
            Self::TopicCreated { actor_id, name, .. } => formatter
                .debug_struct("TopicCreated")
                .field("actor_id_bytes", &actor_id.len())
                .field("name_bytes", &name.len())
                .finish_non_exhaustive(),
            Self::TopicRenamed {
                actor_id,
                previous,
                name,
                ..
            } => formatter
                .debug_struct("TopicRenamed")
                .field("actor_id_bytes", &actor_id.len())
                .field("previous_bytes", &previous.len())
                .field("name_bytes", &name.len())
                .finish_non_exhaustive(),
            Self::TopicLifecycleChanged {
                actor_id,
                topic_id,
                previous_resolved,
                resolved,
            } => formatter
                .debug_struct("TopicLifecycleChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("topic_id", topic_id)
                .field("previous_resolved", previous_resolved)
                .field("resolved", resolved)
                .finish(),
            Self::TopicSupersessionChanged {
                actor_id,
                topic_id,
                previous_successor_topic_id,
                successor_topic_id,
            } => formatter
                .debug_struct("TopicSupersessionChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("topic_id", topic_id)
                .field("previous_successor_topic_id", previous_successor_topic_id)
                .field("successor_topic_id", successor_topic_id)
                .finish(),
            Self::TopicAliasChanged {
                actor_id,
                topic_id,
                alias,
                added,
            } => formatter
                .debug_struct("TopicAliasChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("topic_id", topic_id)
                .field("alias_bytes", &alias.len())
                .field("added", added)
                .finish(),
            Self::TopicTagChanged {
                actor_id,
                topic_id,
                tag,
                added,
            } => formatter
                .debug_struct("TopicTagChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("topic_id", topic_id)
                .field("tag_bytes", &tag.len())
                .field("added", added)
                .finish(),
            Self::InvitationRecorded { participant_id }
            | Self::ParticipantJoined { participant_id }
            | Self::ParticipantLeft { participant_id } => formatter
                .debug_struct("ParticipantChange")
                .field("participant_id_bytes", &participant_id.len())
                .finish_non_exhaustive(),
            Self::RoleChanged {
                participant_id,
                role,
            } => formatter
                .debug_struct("RoleChanged")
                .field("participant_id_bytes", &participant_id.len())
                .field("role", role)
                .finish(),
            Self::RoomPolicyChanged {
                actor_id,
                previous,
                admission,
            } => formatter
                .debug_struct("RoomPolicyChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("previous", previous)
                .field("admission", admission)
                .finish(),
            Self::ParticipantKicked {
                actor_id,
                participant_id,
                reason,
                ..
            }
            | Self::ParticipantBanned {
                actor_id,
                participant_id,
                reason,
                ..
            }
            | Self::ParticipantUnbanned {
                actor_id,
                participant_id,
                reason,
                ..
            } => formatter
                .debug_struct("ModerationEvent")
                .field("actor_id_bytes", &actor_id.len())
                .field("participant_id_bytes", &participant_id.len())
                .field("reason_bytes", &reason.len())
                .field("causation", &"<opaque>")
                .finish(),
            Self::MessageAccepted(message) => formatter
                .debug_tuple("MessageAccepted")
                .field(message)
                .finish(),
            Self::MessageWithdrawn {
                actor_id,
                message_id,
                sequence,
            } => formatter
                .debug_struct("MessageWithdrawn")
                .field("actor_id_bytes", &actor_id.len())
                .field("message_id", message_id)
                .field("sequence", sequence)
                .finish(),
            Self::MessageRedacted {
                actor_id,
                author_id,
                message_id,
                sequence,
                reason,
                ..
            } => formatter
                .debug_struct("MessageRedacted")
                .field("actor_id_bytes", &actor_id.len())
                .field("author_id_bytes", &author_id.len())
                .field("message_id", message_id)
                .field("sequence", sequence)
                .field("reason_bytes", &reason.len())
                .field("causation", &"<opaque>")
                .finish(),
            Self::ThreadLifecycleChanged {
                actor_id,
                root_message_id,
                archived,
                reason,
                ..
            } => formatter
                .debug_struct("ThreadLifecycleChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("root_message_id", root_message_id)
                .field("archived", archived)
                .field("reason_bytes", &reason.len())
                .field("causation", &"<opaque>")
                .finish(),
            Self::ReactionChanged {
                actor_id,
                message_id,
                key,
                reacted,
            } => formatter
                .debug_struct("ReactionChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("message_id", message_id)
                .field("key_bytes", &key.len())
                .field("reacted", reacted)
                .finish(),
            Self::MessageEdited {
                actor_id,
                message_id,
                message_revision,
                logical_time,
                body,
            } => formatter
                .debug_struct("MessageEdited")
                .field("actor_id_bytes", &actor_id.len())
                .field("message_id", message_id)
                .field("message_revision", message_revision)
                .field("logical_time", logical_time)
                .field("body_bytes", &body.len())
                .finish(),
        }
    }
}

/// Additive room-event profile whose accepted-message variant carries a stable
/// one-level root relationship.
#[derive(Clone, Eq, PartialEq)]
pub enum ThreadedRoomEventV1 {
    RoomCreated {
        owner: String,
        admission: AdmissionPolicyV2,
    },
    PeerConversationCreated(PeerConversationCreatedV1),
    TopicCreated {
        actor_id: String,
        topic_id: TopicIdV1,
        name: String,
    },
    TopicRenamed {
        actor_id: String,
        topic_id: TopicIdV1,
        previous: String,
        name: String,
    },
    TopicLifecycleChanged {
        actor_id: String,
        topic_id: TopicIdV1,
        previous_resolved: bool,
        resolved: bool,
    },
    TopicSupersessionChanged {
        actor_id: String,
        topic_id: TopicIdV1,
        previous_successor_topic_id: Option<TopicIdV1>,
        successor_topic_id: Option<TopicIdV1>,
    },
    TopicAliasChanged {
        actor_id: String,
        topic_id: TopicIdV1,
        alias: String,
        added: bool,
    },
    TopicTagChanged {
        actor_id: String,
        topic_id: TopicIdV1,
        tag: String,
        added: bool,
    },
    InvitationRecorded {
        participant_id: String,
    },
    ParticipantJoined {
        participant_id: String,
    },
    ParticipantLeft {
        participant_id: String,
    },
    RoleChanged {
        participant_id: String,
        role: RoleV2,
    },
    RoomPolicyChanged {
        actor_id: String,
        previous: AdmissionPolicyV2,
        admission: AdmissionPolicyV2,
    },
    ParticipantKicked {
        actor_id: String,
        participant_id: String,
        reason: String,
        causation: [u8; 16],
    },
    ParticipantBanned {
        actor_id: String,
        participant_id: String,
        reason: String,
        causation: [u8; 16],
    },
    ParticipantUnbanned {
        actor_id: String,
        participant_id: String,
        reason: String,
        causation: [u8; 16],
    },
    MessageAccepted(ThreadedMessageV1),
    MessageWithdrawn {
        actor_id: String,
        message_id: MessageIdV1,
        sequence: u64,
    },
    MessageRedacted {
        actor_id: String,
        author_id: String,
        message_id: MessageIdV1,
        sequence: u64,
        reason: String,
        causation: [u8; 16],
    },
    ThreadLifecycleChanged {
        actor_id: String,
        root_message_id: MessageIdV1,
        archived: bool,
        reason: String,
        causation: [u8; 16],
    },
    ReactionChanged {
        actor_id: String,
        message_id: MessageIdV1,
        key: String,
        reacted: bool,
    },
    MessageEdited {
        actor_id: String,
        message_id: MessageIdV1,
        message_revision: u32,
        logical_time: u64,
        body: String,
    },
}

impl fmt::Debug for ThreadedRoomEventV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RoomCreated { admission, .. } => formatter
                .debug_struct("RoomCreated")
                .field("admission", admission)
                .finish_non_exhaustive(),
            Self::PeerConversationCreated(event) => event.fmt(formatter),
            Self::TopicCreated { actor_id, name, .. } => formatter
                .debug_struct("TopicCreated")
                .field("actor_id_bytes", &actor_id.len())
                .field("name_bytes", &name.len())
                .finish_non_exhaustive(),
            Self::TopicRenamed {
                actor_id,
                previous,
                name,
                ..
            } => formatter
                .debug_struct("TopicRenamed")
                .field("actor_id_bytes", &actor_id.len())
                .field("previous_bytes", &previous.len())
                .field("name_bytes", &name.len())
                .finish_non_exhaustive(),
            Self::TopicLifecycleChanged {
                actor_id,
                topic_id,
                previous_resolved,
                resolved,
            } => formatter
                .debug_struct("TopicLifecycleChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("topic_id", topic_id)
                .field("previous_resolved", previous_resolved)
                .field("resolved", resolved)
                .finish(),
            Self::TopicSupersessionChanged {
                actor_id,
                topic_id,
                previous_successor_topic_id,
                successor_topic_id,
            } => formatter
                .debug_struct("TopicSupersessionChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("topic_id", topic_id)
                .field("previous_successor_topic_id", previous_successor_topic_id)
                .field("successor_topic_id", successor_topic_id)
                .finish(),
            Self::TopicAliasChanged {
                actor_id,
                topic_id,
                alias,
                added,
            } => formatter
                .debug_struct("TopicAliasChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("topic_id", topic_id)
                .field("alias_bytes", &alias.len())
                .field("added", added)
                .finish(),
            Self::TopicTagChanged {
                actor_id,
                topic_id,
                tag,
                added,
            } => formatter
                .debug_struct("TopicTagChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("topic_id", topic_id)
                .field("tag_bytes", &tag.len())
                .field("added", added)
                .finish(),
            Self::InvitationRecorded { participant_id }
            | Self::ParticipantJoined { participant_id }
            | Self::ParticipantLeft { participant_id } => formatter
                .debug_struct("ParticipantChange")
                .field("participant_id_bytes", &participant_id.len())
                .finish_non_exhaustive(),
            Self::RoleChanged {
                participant_id,
                role,
            } => formatter
                .debug_struct("RoleChanged")
                .field("participant_id_bytes", &participant_id.len())
                .field("role", role)
                .finish(),
            Self::RoomPolicyChanged {
                actor_id,
                previous,
                admission,
            } => formatter
                .debug_struct("RoomPolicyChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("previous", previous)
                .field("admission", admission)
                .finish(),
            Self::ParticipantKicked {
                actor_id,
                participant_id,
                reason,
                ..
            }
            | Self::ParticipantBanned {
                actor_id,
                participant_id,
                reason,
                ..
            }
            | Self::ParticipantUnbanned {
                actor_id,
                participant_id,
                reason,
                ..
            } => formatter
                .debug_struct("ModerationEvent")
                .field("actor_id_bytes", &actor_id.len())
                .field("participant_id_bytes", &participant_id.len())
                .field("reason_bytes", &reason.len())
                .field("causation", &"<opaque>")
                .finish(),
            Self::MessageAccepted(message) => formatter
                .debug_tuple("MessageAccepted")
                .field(message)
                .finish(),
            Self::MessageWithdrawn {
                actor_id,
                message_id,
                sequence,
            } => formatter
                .debug_struct("MessageWithdrawn")
                .field("actor_id_bytes", &actor_id.len())
                .field("message_id", message_id)
                .field("sequence", sequence)
                .finish(),
            Self::MessageRedacted {
                actor_id,
                author_id,
                message_id,
                sequence,
                reason,
                ..
            } => formatter
                .debug_struct("MessageRedacted")
                .field("actor_id_bytes", &actor_id.len())
                .field("author_id_bytes", &author_id.len())
                .field("message_id", message_id)
                .field("sequence", sequence)
                .field("reason_bytes", &reason.len())
                .field("causation", &"<opaque>")
                .finish(),
            Self::ThreadLifecycleChanged {
                actor_id,
                root_message_id,
                archived,
                reason,
                ..
            } => formatter
                .debug_struct("ThreadLifecycleChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("root_message_id", root_message_id)
                .field("archived", archived)
                .field("reason_bytes", &reason.len())
                .field("causation", &"<opaque>")
                .finish(),
            Self::ReactionChanged {
                actor_id,
                message_id,
                key,
                reacted,
            } => formatter
                .debug_struct("ReactionChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("message_id", message_id)
                .field("key_bytes", &key.len())
                .field("reacted", reacted)
                .finish(),
            Self::MessageEdited {
                actor_id,
                message_id,
                message_revision,
                logical_time,
                body,
            } => formatter
                .debug_struct("MessageEdited")
                .field("actor_id_bytes", &actor_id.len())
                .field("message_id", message_id)
                .field("message_revision", message_revision)
                .field("logical_time", logical_time)
                .field("body_bytes", &body.len())
                .finish(),
        }
    }
}

/// Additive room-event profile carrying both reply topology and the immutable
/// optional topic relation on accepted messages.
#[derive(Clone, Eq, PartialEq)]
pub enum TopicRoomEventV1 {
    RoomCreated {
        owner: String,
        admission: AdmissionPolicyV2,
    },
    PeerConversationCreated(PeerConversationCreatedV1),
    TopicCreated {
        actor_id: String,
        topic_id: TopicIdV1,
        name: String,
    },
    TopicRenamed {
        actor_id: String,
        topic_id: TopicIdV1,
        previous: String,
        name: String,
    },
    TopicLifecycleChanged {
        actor_id: String,
        topic_id: TopicIdV1,
        previous_resolved: bool,
        resolved: bool,
    },
    TopicSupersessionChanged {
        actor_id: String,
        topic_id: TopicIdV1,
        previous_successor_topic_id: Option<TopicIdV1>,
        successor_topic_id: Option<TopicIdV1>,
    },
    TopicAliasChanged {
        actor_id: String,
        topic_id: TopicIdV1,
        alias: String,
        added: bool,
    },
    TopicTagChanged {
        actor_id: String,
        topic_id: TopicIdV1,
        tag: String,
        added: bool,
    },
    InvitationRecorded {
        participant_id: String,
    },
    ParticipantJoined {
        participant_id: String,
    },
    ParticipantLeft {
        participant_id: String,
    },
    RoleChanged {
        participant_id: String,
        role: RoleV2,
    },
    RoomPolicyChanged {
        actor_id: String,
        previous: AdmissionPolicyV2,
        admission: AdmissionPolicyV2,
    },
    ParticipantKicked {
        actor_id: String,
        participant_id: String,
        reason: String,
        causation: [u8; 16],
    },
    ParticipantBanned {
        actor_id: String,
        participant_id: String,
        reason: String,
        causation: [u8; 16],
    },
    ParticipantUnbanned {
        actor_id: String,
        participant_id: String,
        reason: String,
        causation: [u8; 16],
    },
    MessageAccepted(TopicMessageV1),
    MessageWithdrawn {
        actor_id: String,
        message_id: MessageIdV1,
        sequence: u64,
    },
    MessageRedacted {
        actor_id: String,
        author_id: String,
        message_id: MessageIdV1,
        sequence: u64,
        reason: String,
        causation: [u8; 16],
    },
    ThreadLifecycleChanged {
        actor_id: String,
        root_message_id: MessageIdV1,
        archived: bool,
        reason: String,
        causation: [u8; 16],
    },
    ReactionChanged {
        actor_id: String,
        message_id: MessageIdV1,
        key: String,
        reacted: bool,
    },
    MessageEdited {
        actor_id: String,
        message_id: MessageIdV1,
        message_revision: u32,
        logical_time: u64,
        body: String,
    },
}

impl fmt::Debug for TopicRoomEventV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RoomCreated { admission, .. } => formatter
                .debug_struct("RoomCreated")
                .field("admission", admission)
                .finish_non_exhaustive(),
            Self::PeerConversationCreated(event) => event.fmt(formatter),
            Self::TopicCreated {
                actor_id,
                topic_id,
                name,
            } => formatter
                .debug_struct("TopicCreated")
                .field("actor_id_bytes", &actor_id.len())
                .field("topic_id", topic_id)
                .field("name_bytes", &name.len())
                .finish(),
            Self::TopicRenamed {
                actor_id,
                topic_id,
                previous,
                name,
            } => formatter
                .debug_struct("TopicRenamed")
                .field("actor_id_bytes", &actor_id.len())
                .field("topic_id", topic_id)
                .field("previous_bytes", &previous.len())
                .field("name_bytes", &name.len())
                .finish(),
            Self::TopicLifecycleChanged {
                actor_id,
                topic_id,
                previous_resolved,
                resolved,
            } => formatter
                .debug_struct("TopicLifecycleChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("topic_id", topic_id)
                .field("previous_resolved", previous_resolved)
                .field("resolved", resolved)
                .finish(),
            Self::TopicSupersessionChanged {
                actor_id,
                topic_id,
                previous_successor_topic_id,
                successor_topic_id,
            } => formatter
                .debug_struct("TopicSupersessionChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("topic_id", topic_id)
                .field("previous_successor_topic_id", previous_successor_topic_id)
                .field("successor_topic_id", successor_topic_id)
                .finish(),
            Self::TopicAliasChanged {
                actor_id,
                topic_id,
                alias,
                added,
            } => formatter
                .debug_struct("TopicAliasChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("topic_id", topic_id)
                .field("alias_bytes", &alias.len())
                .field("added", added)
                .finish(),
            Self::TopicTagChanged {
                actor_id,
                topic_id,
                tag,
                added,
            } => formatter
                .debug_struct("TopicTagChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("topic_id", topic_id)
                .field("tag_bytes", &tag.len())
                .field("added", added)
                .finish(),
            Self::InvitationRecorded { participant_id }
            | Self::ParticipantJoined { participant_id }
            | Self::ParticipantLeft { participant_id } => formatter
                .debug_struct("ParticipantChange")
                .field("participant_id_bytes", &participant_id.len())
                .finish_non_exhaustive(),
            Self::RoleChanged {
                participant_id,
                role,
            } => formatter
                .debug_struct("RoleChanged")
                .field("participant_id_bytes", &participant_id.len())
                .field("role", role)
                .finish(),
            Self::RoomPolicyChanged {
                actor_id,
                previous,
                admission,
            } => formatter
                .debug_struct("RoomPolicyChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("previous", previous)
                .field("admission", admission)
                .finish(),
            Self::ParticipantKicked {
                actor_id,
                participant_id,
                reason,
                ..
            }
            | Self::ParticipantBanned {
                actor_id,
                participant_id,
                reason,
                ..
            }
            | Self::ParticipantUnbanned {
                actor_id,
                participant_id,
                reason,
                ..
            } => formatter
                .debug_struct("ModerationEvent")
                .field("actor_id_bytes", &actor_id.len())
                .field("participant_id_bytes", &participant_id.len())
                .field("reason_bytes", &reason.len())
                .field("causation", &"<opaque>")
                .finish(),
            Self::MessageAccepted(message) => formatter
                .debug_tuple("MessageAccepted")
                .field(message)
                .finish(),
            Self::MessageWithdrawn {
                actor_id,
                message_id,
                sequence,
            } => formatter
                .debug_struct("MessageWithdrawn")
                .field("actor_id_bytes", &actor_id.len())
                .field("message_id", message_id)
                .field("sequence", sequence)
                .finish(),
            Self::MessageRedacted {
                actor_id,
                author_id,
                message_id,
                sequence,
                reason,
                ..
            } => formatter
                .debug_struct("MessageRedacted")
                .field("actor_id_bytes", &actor_id.len())
                .field("author_id_bytes", &author_id.len())
                .field("message_id", message_id)
                .field("sequence", sequence)
                .field("reason_bytes", &reason.len())
                .field("causation", &"<opaque>")
                .finish(),
            Self::ThreadLifecycleChanged {
                actor_id,
                root_message_id,
                archived,
                reason,
                ..
            } => formatter
                .debug_struct("ThreadLifecycleChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("root_message_id", root_message_id)
                .field("archived", archived)
                .field("reason_bytes", &reason.len())
                .field("causation", &"<opaque>")
                .finish(),
            Self::ReactionChanged {
                actor_id,
                message_id,
                key,
                reacted,
            } => formatter
                .debug_struct("ReactionChanged")
                .field("actor_id_bytes", &actor_id.len())
                .field("message_id", message_id)
                .field("key_bytes", &key.len())
                .field("reacted", reacted)
                .finish(),
            Self::MessageEdited {
                actor_id,
                message_id,
                message_revision,
                logical_time,
                body,
            } => formatter
                .debug_struct("MessageEdited")
                .field("actor_id_bytes", &actor_id.len())
                .field("message_id", message_id)
                .field("message_revision", message_revision)
                .field("logical_time", logical_time)
                .field("body_bytes", &body.len())
                .finish(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoomChangeV2 {
    pub event_sequence: u64,
    pub revision: u64,
    pub event: RoomEventV2,
}

#[derive(Clone, Eq, PartialEq)]
pub struct RoomChangePageV2 {
    pub revision: u64,
    pub changes: Vec<RoomChangeV2>,
    pub next: PageCursorV2,
    pub has_more: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentifiedRoomChangeV1 {
    pub event_sequence: u64,
    pub revision: u64,
    pub event: IdentifiedRoomEventV1,
}

#[derive(Clone, Eq, PartialEq)]
pub struct IdentifiedRoomChangePageV1 {
    pub revision: u64,
    pub changes: Vec<IdentifiedRoomChangeV1>,
    pub next: PageCursorV2,
    pub has_more: bool,
}

/// Identified change feed plus one full attachment-aware projection for every
/// message-acceptance event, in the same order as those events.
#[derive(Clone, Eq, PartialEq)]
pub struct AttachmentRoomChangePageV1 {
    pub identified: IdentifiedRoomChangePageV1,
    pub accepted_messages: Vec<AttachmentMessageV1>,
}

impl fmt::Debug for AttachmentRoomChangePageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AttachmentRoomChangePageV1")
            .field("revision", &self.identified.revision)
            .field("change_count", &self.identified.changes.len())
            .field("accepted_message_count", &self.accepted_messages.len())
            .field("has_more", &self.identified.has_more)
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThreadedRoomChangeV1 {
    pub event_sequence: u64,
    pub revision: u64,
    pub event: ThreadedRoomEventV1,
}

#[derive(Clone, Eq, PartialEq)]
pub struct ThreadedRoomChangePageV1 {
    pub revision: u64,
    pub changes: Vec<ThreadedRoomChangeV1>,
    pub next: PageCursorV2,
    pub has_more: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TopicRoomChangeV1 {
    pub event_sequence: u64,
    pub revision: u64,
    pub event: TopicRoomEventV1,
}

#[derive(Clone, Eq, PartialEq)]
pub struct TopicRoomChangePageV1 {
    pub revision: u64,
    pub changes: Vec<TopicRoomChangeV1>,
    pub next: PageCursorV2,
    pub has_more: bool,
}

impl fmt::Debug for TopicRoomChangePageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TopicRoomChangePageV1")
            .field("revision", &self.revision)
            .field("change_count", &self.changes.len())
            .field("next", &self.next)
            .field("has_more", &self.has_more)
            .finish()
    }
}

impl fmt::Debug for ThreadedRoomChangePageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ThreadedRoomChangePageV1")
            .field("revision", &self.revision)
            .field("change_count", &self.changes.len())
            .field("next", &self.next)
            .field("has_more", &self.has_more)
            .finish()
    }
}

impl fmt::Debug for IdentifiedRoomChangePageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IdentifiedRoomChangePageV1")
            .field("revision", &self.revision)
            .field("change_count", &self.changes.len())
            .field("next", &self.next)
            .field("has_more", &self.has_more)
            .finish()
    }
}

impl fmt::Debug for RoomChangePageV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RoomChangePageV2")
            .field("revision", &self.revision)
            .field("change_count", &self.changes.len())
            .field("next", &self.next)
            .field("has_more", &self.has_more)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChangeGapV2 {
    Retention { acknowledged_through: u64 },
    MembershipChanged,
    ChangeRetention { discarded_through: u64 },
}

macro_rules! page_debug {
    ($type:ty, $name:literal, $field:ident, $count:literal) => {
        impl fmt::Debug for $type {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_struct($name)
                    .field("revision", &self.revision)
                    .field($count, &self.$field.len())
                    .field("has_next", &self.next.is_some())
                    .finish()
            }
        }
    };
}

page_debug!(RoomPageV2, "RoomPageV2", rooms, "room_count");
page_debug!(MemberPageV2, "MemberPageV2", members, "member_count");
page_debug!(BanPageV2, "BanPageV2", banned, "banned_count");
page_debug!(MessagePageV2, "MessagePageV2", messages, "message_count");
page_debug!(
    IdentifiedMessagePageV1,
    "IdentifiedMessagePageV1",
    messages,
    "message_count"
);
page_debug!(
    VellumArtifactMessagePageV1,
    "VellumArtifactMessagePageV1",
    messages,
    "message_count"
);
page_debug!(
    AttachmentMessagePageV1,
    "AttachmentMessagePageV1",
    messages,
    "message_count"
);

#[derive(Clone, Eq, PartialEq)]
pub enum OutcomeV2 {
    Capabilities(Vec<CapabilityV2>),
    Rooms(RoomPageV2),
    Members(MemberPageV2),
    Bans(BanPageV2),
    Messages(MessagePageV2),
    IdentifiedMessages(IdentifiedMessagePageV1),
    AttachmentMessages(AttachmentMessagePageV1),
    VellumArtifactMessages(VellumArtifactMessagePageV1),
    ThreadRoots(ThreadMessagePageV1),
    ThreadReplies(ThreadMessagePageV1),
    ThreadStatus(ThreadStatusV1),
    ReactionStatus(ReactionStatusV1),
    MessageRevisions(MessageRevisionPageV1),
    Topics(TopicPageV1),
    TopicMessages(TopicMessagePageV1),
    ReadPosition(Option<ReadPositionV1>),
    ReadPositionAdvanced(ReadPositionV1),
    AttentionPreference(AttentionPreferenceV1),
    AttentionPreferenceSet(AttentionPreferenceV1),
    ChangesOpened {
        revision: u64,
        cursor: PageCursorV2,
    },
    Changes(RoomChangePageV2),
    IdentifiedChanges(IdentifiedRoomChangePageV1),
    AttachmentChanges(AttachmentRoomChangePageV1),
    ThreadedChanges(ThreadedRoomChangePageV1),
    TopicChanges(TopicRoomChangePageV1),
    ChangeGap {
        current_revision: u64,
        cause: ChangeGapV2,
    },
    RoomCreated {
        revision: u64,
    },
    PeerConversationCreated {
        revision: u64,
        profile: ConversationProfileV1,
    },
    ConversationProfile(ConversationProfileStatusV1),
    InvitationRecorded {
        revision: u64,
    },
    Joined {
        revision: u64,
    },
    Left {
        revision: u64,
    },
    RoleChanged {
        revision: u64,
    },
    RoomPolicyChanged {
        revision: u64,
    },
    Kicked {
        revision: u64,
    },
    Banned {
        revision: u64,
    },
    Unbanned {
        revision: u64,
    },
    MessageAccepted {
        revision: u64,
        sequence: u64,
    },
    IdentifiedMessageAccepted {
        revision: u64,
        message_id: MessageIdV1,
        sequence: u64,
    },
    AttachmentMessageAccepted {
        revision: u64,
        message_id: MessageIdV1,
        sequence: u64,
        attachment_count: u32,
    },
    VellumArtifactMessageAccepted {
        revision: u64,
        message_id: MessageIdV1,
        sequence: u64,
        reference_count: u32,
    },
    ReplyAccepted {
        revision: u64,
        message_id: MessageIdV1,
        root_message_id: MessageIdV1,
        sequence: u64,
    },
    MessageWithdrawn {
        revision: u64,
        message_id: MessageIdV1,
        sequence: u64,
    },
    MessageRedacted {
        revision: u64,
        message_id: MessageIdV1,
        sequence: u64,
    },
    ThreadArchived {
        revision: u64,
        root_message_id: MessageIdV1,
    },
    ThreadReopened {
        revision: u64,
        root_message_id: MessageIdV1,
    },
    ReactionAdded {
        revision: u64,
        message_id: MessageIdV1,
    },
    ReactionRemoved {
        revision: u64,
        message_id: MessageIdV1,
    },
    MessageEdited {
        revision: u64,
        message_id: MessageIdV1,
        message_revision: u32,
    },
    TopicCreated {
        revision: u64,
        topic_id: TopicIdV1,
    },
    TopicRenamed {
        revision: u64,
        topic_id: TopicIdV1,
    },
    TopicResolved {
        revision: u64,
        topic_id: TopicIdV1,
    },
    TopicReopened {
        revision: u64,
        topic_id: TopicIdV1,
    },
    TopicStatus(TopicStatusV1),
    TopicSuperseded {
        revision: u64,
        topic_id: TopicIdV1,
        successor_topic_id: TopicIdV1,
    },
    TopicSupersessionCleared {
        revision: u64,
        topic_id: TopicIdV1,
    },
    TopicSupersessionStatus(TopicSupersessionStatusV1),
    TopicAliasAdded {
        revision: u64,
        topic_id: TopicIdV1,
        alias: String,
    },
    TopicAliasRemoved {
        revision: u64,
        topic_id: TopicIdV1,
        alias: String,
    },
    TopicAliasStatus(TopicAliasStatusV1),
    TopicTagAdded {
        revision: u64,
        topic_id: TopicIdV1,
        tag: String,
    },
    TopicTagRemoved {
        revision: u64,
        topic_id: TopicIdV1,
        tag: String,
    },
    TopicTagStatus(TopicTagStatusV1),
    TopicMessageAccepted {
        revision: u64,
        message_id: MessageIdV1,
        topic_id: TopicIdV1,
        sequence: u64,
    },
}

impl fmt::Debug for OutcomeV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OutcomeV2(<redacted>)")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefusalV2 {
    UnknownClient,
    UnknownParticipant,
    UnknownRoom,
    NotMember,
    NotAuthorized,
    Stale { actual_revision: u64 },
    RequestConflict,
    ClientLimit,
    RequestLimit,
    CursorLimit,
    InvalidCursor,
    InvalidInput,
    Capacity,
    Exhausted,
    Unavailable,
    Banned,
    ThreadArchived,
    UnknownTopic,
    TopicResolved,
    InvalidTopicSupersession,
    TopicMustBeResolved,
    TopicAliasLimit,
    UnknownTopicAlias,
    InvalidTopicTag,
    DuplicateTopicTag,
    TopicTagLimit,
    UnknownTopicTag,
    InvalidReadScope,
    InvalidReadTarget,
    ReadPositionRegression,
    ImmutableConversationCohort,
    InvalidAttentionScope,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplyV2 {
    Accepted(OutcomeV2),
    Refused(RefusalV2),
}
