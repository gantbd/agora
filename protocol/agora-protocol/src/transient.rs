//! Closed values for Agora's separate volatile collaboration profile.
//!
//! These values do not enter the durable Protocol V2 command or receipt
//! grammar. Host, client, typing, and end-to-end lifecycle evidence qualify it
//! as an additive collaboration profile without making it durable state.

use crate::v2::{MessageIdV1, ParticipantKindV2, TopicIdV1};
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

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct TypingPageCursorV1(pub u64);

impl fmt::Debug for TypingPageCursorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TypingPageCursorV1(<opaque>)")
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TypingTargetV1 {
    Room,
    Thread(MessageIdV1),
    Topic(TopicIdV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypingViewStateV1 {
    Inactive,
    Active,
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
    PublishTyping {
        room_id: String,
        target: TypingTargetV1,
        lifetime: u64,
    },
    ClearTyping {
        room_id: String,
        target: TypingTargetV1,
    },
    TypingPage {
        room_id: String,
        target: TypingTargetV1,
        cursor: Option<TypingPageCursorV1>,
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
            Self::PublishTyping {
                room_id,
                target,
                lifetime,
            } => formatter
                .debug_struct("PublishTyping")
                .field("room_id_bytes", &room_id.len())
                .field("target", target)
                .field("lifetime", lifetime)
                .finish(),
            Self::ClearTyping { room_id, target } => formatter
                .debug_struct("ClearTyping")
                .field("room_id_bytes", &room_id.len())
                .field("target", target)
                .finish(),
            Self::TypingPage {
                room_id,
                target,
                cursor,
                limit,
            } => formatter
                .debug_struct("TypingPage")
                .field("room_id_bytes", &room_id.len())
                .field("target", target)
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

#[derive(Clone, Eq, PartialEq)]
pub struct TypingEntryV1 {
    pub participant_id: String,
    pub participant_kind: ParticipantKindV2,
    pub state: TypingViewStateV1,
    pub source: Option<PresenceSourceClassV1>,
    pub remaining_lifetime: Option<u64>,
}

impl fmt::Debug for TypingEntryV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TypingEntryV1")
            .field("participant_id_bytes", &self.participant_id.len())
            .field("participant_kind", &self.participant_kind)
            .field("state", &self.state)
            .field("source", &self.source)
            .field("remaining_lifetime", &self.remaining_lifetime)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct TypingPageV1 {
    pub entries: Vec<TypingEntryV1>,
    pub next: Option<TypingPageCursorV1>,
}

impl fmt::Debug for TypingPageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TypingPageV1")
            .field("entry_count", &self.entries.len())
            .field("has_next", &self.next.is_some())
            .finish()
    }
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
    TypingPublished { accepted_lifetime: u64 },
    TypingCleared { was_present: bool },
    TypingPage(TypingPageV1),
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
    InvalidTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransientReplyV1 {
    Accepted(TransientOutcomeV1),
    Refused(TransientRefusalV1),
}
