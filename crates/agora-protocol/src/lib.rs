//! Transport-neutral values for the initial Agora service profile.
//!
//! IDs in this crate are correlation handles, not authentication or authority.

use std::fmt;

pub mod v2;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ClientIdV1(pub u64);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RequestIdV1(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParticipantKindV1 {
    Human,
    Agent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoleV1 {
    Owner,
    Moderator,
    Member,
}

#[derive(Clone, Eq, PartialEq)]
pub enum CommandV1 {
    Snapshot {
        room_id: String,
    },
    Speak {
        room_id: String,
        expected_revision: u64,
        logical_time: u64,
        body: String,
        addressed_to: Vec<String>,
    },
}

impl fmt::Debug for CommandV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Snapshot { room_id } => formatter
                .debug_struct("Snapshot")
                .field("room_id_bytes", &room_id.len())
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
pub struct RequestV1 {
    pub request_id: RequestIdV1,
    pub command: CommandV1,
}

impl fmt::Debug for RequestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RequestV1")
            .field("request_id", &self.request_id)
            .field("command", &self.command)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct MessageV1 {
    pub sequence: u64,
    pub author: String,
    pub logical_time: u64,
    pub body: String,
    pub addressed_to: Vec<String>,
}

impl fmt::Debug for MessageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MessageV1")
            .field("sequence", &self.sequence)
            .field("author_bytes", &self.author.len())
            .field("logical_time", &self.logical_time)
            .field("body_bytes", &self.body.len())
            .field("addressed_count", &self.addressed_to.len())
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct MemberV1 {
    pub participant_id: String,
    pub kind: ParticipantKindV1,
    pub role: RoleV1,
}

impl fmt::Debug for MemberV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MemberV1")
            .field("participant_id_bytes", &self.participant_id.len())
            .field("kind", &self.kind)
            .field("role", &self.role)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct RoomSnapshotV1 {
    pub revision: u64,
    pub room_id: String,
    pub name: String,
    pub members: Vec<MemberV1>,
    pub messages: Vec<MessageV1>,
    pub next_message_sequence: u64,
}

impl fmt::Debug for RoomSnapshotV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RoomSnapshotV1")
            .field("revision", &self.revision)
            .field("room_id_bytes", &self.room_id.len())
            .field("name_bytes", &self.name.len())
            .field("member_count", &self.members.len())
            .field("message_count", &self.messages.len())
            .field("next_message_sequence", &self.next_message_sequence)
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutcomeV1 {
    Snapshot(RoomSnapshotV1),
    MessageAccepted { revision: u64, sequence: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefusalV1 {
    UnknownClient,
    UnknownParticipant,
    UnknownRoom,
    NotMember,
    NotAuthorized,
    Stale { actual_revision: u64 },
    RequestConflict,
    ClientLimit,
    RetiredClientLimit,
    RequestLimit,
    SnapshotLimit,
    InvalidInput,
    Capacity,
    Exhausted,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplyV1 {
    Accepted(OutcomeV1),
    Refused(RefusalV1),
}
