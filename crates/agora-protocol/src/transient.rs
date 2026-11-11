//! Closed values for Agora's separate volatile collaboration profile.
//!
//! These values do not enter the durable Protocol V2 command or receipt
//! grammar. The profile remains unadvertised until Host, client, typing, and
//! end-to-end lifecycle evidence close vertically.

use crate::v2::ParticipantKindV2;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TransientRequestIdV1(pub u64);

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct PresencePageCursorV1(pub u64);

impl fmt::Debug for PresencePageCursorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PresencePageCursorV1(<opaque>)")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresenceStateV1 {
    Available,
    Away,
    Busy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresenceViewStateV1 {
    Unknown,
    Available,
    Away,
    Busy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresenceSourceClassV1 {
    ParticipantSession,
}

#[derive(Clone, Eq, PartialEq)]
pub enum TransientCommandV1 {
    PublishPresence {
        room_id: String,
        state: PresenceStateV1,
        lifetime: u64,
    },
    ClearPresence {
        room_id: String,
    },
    PresencePage {
        room_id: String,
        cursor: Option<PresencePageCursorV1>,
        limit: u32,
    },
}

impl fmt::Debug for TransientCommandV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PublishPresence {
                room_id,
                state,
                lifetime,
            } => formatter
                .debug_struct("PublishPresence")
                .field("room_id_bytes", &room_id.len())
                .field("state", state)
                .field("lifetime", lifetime)
                .finish(),
            Self::ClearPresence { room_id } => formatter
                .debug_struct("ClearPresence")
                .field("room_id_bytes", &room_id.len())
                .finish(),
            Self::PresencePage {
                room_id,
                cursor,
                limit,
            } => formatter
                .debug_struct("PresencePage")
                .field("room_id_bytes", &room_id.len())
                .field("has_cursor", &cursor.is_some())
                .field("limit", limit)
                .finish(),
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct TransientRequestV1 {
    pub request_id: TransientRequestIdV1,
    pub command: TransientCommandV1,
}

impl fmt::Debug for TransientRequestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TransientRequestV1")
            .field("request_id", &self.request_id)
            .field("command", &self.command)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct PresenceEntryV1 {
    pub participant_id: String,
    pub participant_kind: ParticipantKindV2,
    pub state: PresenceViewStateV1,
    pub source: Option<PresenceSourceClassV1>,
    pub remaining_lifetime: Option<u64>,
}

impl fmt::Debug for PresenceEntryV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PresenceEntryV1")
            .field("participant_id_bytes", &self.participant_id.len())
            .field("participant_kind", &self.participant_kind)
            .field("state", &self.state)
            .field("source", &self.source)
            .field("remaining_lifetime", &self.remaining_lifetime)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct PresencePageV1 {
    pub entries: Vec<PresenceEntryV1>,
    pub next: Option<PresencePageCursorV1>,
}

impl fmt::Debug for PresencePageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PresencePageV1")
            .field("entry_count", &self.entries.len())
            .field("has_next", &self.next.is_some())
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransientOutcomeV1 {
    PresencePublished { accepted_lifetime: u64 },
    PresenceCleared { was_present: bool },
    PresencePage(PresencePageV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransientRefusalV1 {
    NotMember,
    InvalidLifetime,
    InvalidPage,
    InvalidCursor,
    StaleCursor,
    Capacity,
    FidelityLost,
    Unavailable,
    RequestConflict,
    RequestOutOfSequence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransientReplyV1 {
    Accepted(TransientOutcomeV1),
    Refused(TransientRefusalV1),
}
