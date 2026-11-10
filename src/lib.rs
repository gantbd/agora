//! A deterministic, bounded chat-room core for humans and software agents.
//!
//! Agora performs no I/O and does not authenticate callers. Hosts must bind an
//! authenticated principal to the [`ParticipantId`] used in commands.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
pub mod persistence;

/// Resource ceilings for one engine profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_participants: usize,
    pub max_rooms: usize,
    pub max_members_per_room: usize,
    pub max_invitations_per_room: usize,
    pub max_bans_per_room: usize,
    pub max_messages_per_room: usize,
    pub max_pending_events: usize,
    pub max_publication_receipts: usize,
    pub max_addressed_participants_per_message: usize,
    pub max_participant_id_bytes: usize,
    pub max_room_id_bytes: usize,
    pub max_display_name_bytes: usize,
    pub max_room_name_bytes: usize,
    pub max_message_body_bytes: usize,
    pub max_moderation_reason_bytes: usize,
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
            max_pending_events: 4_096,
            max_publication_receipts: 4_096,
            max_addressed_participants_per_message: 64,
            max_participant_id_bytes: 128,
            max_room_id_bytes: 128,
            max_display_name_bytes: 256,
            max_room_name_bytes: 256,
            max_message_body_bytes: 16 * 1_024,
            max_moderation_reason_bytes: 1_024,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    Owner,
    Moderator,
    Member,
}

/// A caller-supplied timestamp in a host-defined logical epoch.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LogicalTime(pub u64);

/// Stable room-local message reference. The byte representation is opaque and
/// carries no ordering contract; use message sequence or event position to order.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MessageId([u8; 16]);

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
            .finish()
    }
}

/// Immutable public room observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoomView {
    pub id: RoomId,
    pub name: String,
    pub admission: AdmissionPolicy,
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
    pub admission: AdmissionPolicy,
    pub observer_role: Role,
    pub member_count: usize,
    pub retained_message_count: usize,
    pub next_message_sequence: u64,
}

impl fmt::Debug for RoomSummary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RoomSummary")
            .field("room_id_bytes", &self.id.as_str().len())
            .field("room_name_bytes", &self.name.len())
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
    Retention { acknowledged_through: EventSequence },
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
struct Room {
    id: RoomId,
    name: String,
    admission: AdmissionPolicy,
    members: BTreeMap<ParticipantId, Role>,
    invitations: BTreeSet<ParticipantId>,
    bans: BTreeMap<ParticipantId, Role>,
    messages: VecDeque<Message>,
    thread_roots: BTreeSet<MessageId>,
    archived_threads: BTreeSet<MessageId>,
    message_identity_incarnation: u64,
    next_message_identity: u64,
    next_message_sequence: u64,
}

impl Room {
    fn view(&self) -> RoomView {
        RoomView {
            id: self.id.clone(),
            name: self.name.clone(),
            admission: self.admission,
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
    Invite,
    Join,
    Leave,
    Speak,
    Reply,
    ManageThread,
    WithdrawOwn,
    RedactAny,
    ManageRole,
    ManageRoomPolicy,
    Kick,
    Ban,
    Unban,
    InspectHistory,
}

impl CommandV2 {
    /// Passive authorization vocabulary for a trusted host or Aegis adapter.
    /// Agora still enforces its room invariants after any external decision.
    pub fn room_action(&self) -> Option<RoomActionV1> {
        match self {
            Self::RegisterParticipant(_) => None,
            Self::CreateRoom { .. } => Some(RoomActionV1::CreateRoom),
            Self::Invite { .. } => Some(RoomActionV1::Invite),
            Self::Join { .. } => Some(RoomActionV1::Join),
            Self::Leave { .. } => Some(RoomActionV1::Leave),
            Self::SetRole { .. } => Some(RoomActionV1::ManageRole),
            Self::SetAdmissionPolicy { .. } => Some(RoomActionV1::ManageRoomPolicy),
            Self::Kick { .. } => Some(RoomActionV1::Kick),
            Self::Ban { .. } => Some(RoomActionV1::Ban),
            Self::Unban { .. } => Some(RoomActionV1::Unban),
            Self::Speak { .. } | Self::SpeakAddressed { .. } => Some(RoomActionV1::Speak),
            Self::SpeakReply { .. } => Some(RoomActionV1::Reply),
            Self::SetThreadArchived { .. } => Some(RoomActionV1::ManageThread),
            Self::WithdrawMessage { .. } => Some(RoomActionV1::WithdrawOwn),
            Self::RedactMessage { .. } => Some(RoomActionV1::RedactAny),
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutcomeV2 {
    ParticipantRegistered(ParticipantId),
    RoomCreated(RoomId),
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
}

/// Typed refusal reasons. Every refusal preserves authoritative state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Refusal {
    InvalidParticipantId,
    InvalidRoomId,
    InvalidDisplayName,
    InvalidRoomName,
    InvalidMessageBody,
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
    InvalidRoleChange,
    InvalidPolicyChange,
    UnknownMessage,
    InvalidReplyTarget,
    ThreadArchived,
    InvalidThreadLifecycleChange,
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
}

impl DomainEventV2 {
    fn room_id(&self) -> Option<&RoomId> {
        match self {
            Self::ParticipantRegistered { .. } => None,
            Self::RoomCreated { room_id, .. }
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
            | Self::ThreadLifecycleChanged { room_id, .. } => Some(room_id),
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
    publications: BTreeMap<PublicationKeyV2, PublicationRecordV2>,
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
            publications: BTreeMap::new(),
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
    /// `page.next` yields no overlap. Global outbox acknowledgement may remove a
    /// position's required prefix; that is an explicit `Gap`, never an empty
    /// page that implies continuity.
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
        if limit == 0 || limit > self.limits.max_pending_events {
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
        if let Some(acknowledged_through) = self.acknowledged_through
            && position
                .after
                .is_none_or(|after| after < acknowledged_through)
        {
            return Ok(RoomChangeResult::Gap {
                cause: RoomChangeGap::Retention {
                    acknowledged_through,
                },
                current_revision: self.revision,
            });
        }

        let mut changes: Vec<EventEnvelopeV2> = Vec::new();
        changes
            .try_reserve_exact(limit)
            .map_err(|_| ObservationRefusal::Capacity)?;
        for event in &self.pending_events {
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
        self.pending_events.push_back(EventEnvelopeV2 {
            sequence: event_sequence,
            revision: next_revision,
            event,
        });
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
            } => self.speak(room_id, author, at, body, Vec::new(), None),
            CommandV2::SpeakAddressed {
                room_id,
                author,
                at,
                body,
                addressed_to,
            } => self.speak(room_id, author, at, body, addressed_to, None),
            CommandV2::SpeakReply {
                room_id,
                author,
                root_message_id,
                at,
                body,
                addressed_to,
            } => self.speak(
                room_id,
                author,
                at,
                body,
                addressed_to,
                Some(root_message_id),
            ),
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
                admission,
                members,
                invitations: BTreeSet::new(),
                bans: BTreeMap::new(),
                messages: VecDeque::new(),
                thread_roots: BTreeSet::new(),
                archived_threads: BTreeSet::new(),
                message_identity_incarnation: self.next_event_sequence.0,
                next_message_identity: 0,
                next_message_sequence: 0,
            },
        );
        Ok(OutcomeV2::RoomCreated(room_id))
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

    fn speak(
        &mut self,
        room_id: RoomId,
        author: ParticipantId,
        at: LogicalTime,
        body: String,
        addressed_to: Vec<ParticipantId>,
        reply_to_root: Option<MessageId>,
    ) -> Result<OutcomeV2, Refusal> {
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
        if let Some(root_message_id) = reply_to_root {
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
        }
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
            if let Some(root) = evicted.reply_to_root
                && !room
                    .messages
                    .iter()
                    .any(|message| message.reply_to_root == Some(root))
            {
                room.thread_roots.remove(&root);
            }
        }
        room.messages.push_back(Message {
            id: message_id,
            sequence,
            author,
            at,
            body,
            addressed_to,
            reply_to_root,
        });
        room.next_message_identity = next_message_identity;
        room.next_message_sequence = next;
        if let Some(root) = reply_to_root {
            room.thread_roots.insert(root);
        }
        Ok(match reply_to_root {
            Some(root_message_id) => OutcomeV2::ReplyAccepted {
                room_id,
                root_message_id,
                sequence,
            },
            None => OutcomeV2::MessageAccepted { room_id, sequence },
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
        room.messages
            .iter_mut()
            .find(|message| message.id == message_id)
            .expect("message established by validation")
            .body
            .clear();
        for envelope in &mut self.pending_events {
            if let DomainEventV2::MessageAccepted {
                room_id: event_room,
                message,
            } = &mut envelope.event
                && event_room == room_id
                && message.id == message_id
            {
                message.body.clear();
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
