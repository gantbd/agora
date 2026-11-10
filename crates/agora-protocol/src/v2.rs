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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionPolicyV2 {
    Open,
    InviteOnly,
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
    Rooms {
        cursor: Option<PageCursorV2>,
        limit: u32,
    },
    Members {
        room_id: String,
        cursor: Option<PageCursorV2>,
        limit: u32,
    },
    Messages {
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
    CreateRoom {
        room_id: String,
        name: String,
        admission: AdmissionPolicyV2,
        expected_revision: u64,
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
    Speak {
        room_id: String,
        expected_revision: u64,
        logical_time: u64,
        body: String,
        addressed_to: Vec<String>,
    },
}

impl fmt::Debug for CommandV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            Self::Speak {
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
        }
    }
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
pub struct MessagePageV2 {
    pub revision: u64,
    pub room_id: String,
    pub messages: Vec<MessageV2>,
    pub oldest_retained_sequence: u64,
    pub next_message_sequence: u64,
    pub next: Option<PageCursorV2>,
}

#[derive(Clone, Eq, PartialEq)]
pub enum RoomEventV2 {
    RoomCreated {
        owner: String,
        admission: AdmissionPolicyV2,
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
    MessageAccepted(MessageV2),
}

impl fmt::Debug for RoomEventV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RoomCreated { admission, .. } => formatter
                .debug_struct("RoomCreated")
                .field("admission", admission)
                .finish_non_exhaustive(),
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
            Self::MessageAccepted(message) => formatter
                .debug_tuple("MessageAccepted")
                .field(message)
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
page_debug!(MessagePageV2, "MessagePageV2", messages, "message_count");

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutcomeV2 {
    Rooms(RoomPageV2),
    Members(MemberPageV2),
    Messages(MessagePageV2),
    ChangesOpened {
        revision: u64,
        cursor: PageCursorV2,
    },
    Changes(RoomChangePageV2),
    ChangeGap {
        current_revision: u64,
        cause: ChangeGapV2,
    },
    RoomCreated {
        revision: u64,
    },
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
    MessageAccepted {
        revision: u64,
        sequence: u64,
    },
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplyV2 {
    Accepted(OutcomeV2),
    Refused(RefusalV2),
}
