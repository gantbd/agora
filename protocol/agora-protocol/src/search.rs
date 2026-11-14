//! Additive transport-neutral participant-bound derived-search values.

use crate::v2::{ClientIdV2, MessageIdV1};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SearchRequestIdV1(pub u64);

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ServiceSearchCursorV1([u8; 16]);

impl ServiceSearchCursorV1 {
    pub const fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn into_bytes(self) -> [u8; 16] {
        self.0
    }
}

impl fmt::Debug for ServiceSearchCursorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ServiceSearchCursorV1(<opaque>)")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchMatchProfileV1 {
    ExactUtf8Substring,
}

#[derive(Clone, Eq, PartialEq)]
pub struct SearchCommandV1 {
    pub room_id: String,
    pub profile: SearchMatchProfileV1,
    pub term: String,
    pub cursor: Option<ServiceSearchCursorV1>,
    pub limit: u32,
}

impl fmt::Debug for SearchCommandV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SearchCommandV1")
            .field("room_id_bytes", &self.room_id.len())
            .field("profile", &self.profile)
            .field("term_bytes", &self.term.len())
            .field("cursor", &self.cursor)
            .field("limit", &self.limit)
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchRequestV1 {
    pub request_id: SearchRequestIdV1,
    pub command: SearchCommandV1,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct SearchHitV1 {
    pub message_id: MessageIdV1,
    pub message_sequence: u64,
}

impl fmt::Debug for SearchHitV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SearchHitV1")
            .field("message_id", &self.message_id)
            .field("message_sequence", &self.message_sequence)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct SearchPageV1 {
    pub source_revision: u64,
    pub index_generation: u64,
    pub hits: Vec<SearchHitV1>,
    pub next: Option<ServiceSearchCursorV1>,
}

impl fmt::Debug for SearchPageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SearchPageV1")
            .field("source_revision", &self.source_revision)
            .field("index_generation", &"<opaque>")
            .field("hit_count", &self.hits.len())
            .field("next", &self.next)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum SearchOutcomeV1 {
    Available(SearchPageV1),
    Incomplete(SearchPageV1),
    Stale {
        source_revision: u64,
        index_generation: u64,
    },
    Unavailable,
}

impl fmt::Debug for SearchOutcomeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Available(page) => formatter.debug_tuple("Available").field(page).finish(),
            Self::Incomplete(page) => formatter.debug_tuple("Incomplete").field(page).finish(),
            Self::Stale {
                source_revision,
                index_generation: _,
            } => formatter
                .debug_struct("Stale")
                .field("source_revision", source_revision)
                .field("index_generation", &"<opaque>")
                .finish(),
            Self::Unavailable => formatter.write_str("Unavailable"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchRefusalV1 {
    UnknownClient,
    RequestConflict,
    RequestLimit,
    UnknownRoom,
    NotMember,
    InvalidInput,
    InvalidCursor,
    CursorLimit,
    IndexCapacity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchReplyV1 {
    Accepted(SearchOutcomeV1),
    Refused(SearchRefusalV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchOfferV1 {
    pub client: ClientIdV2,
    pub request: SearchRequestV1,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_hide_query_and_index_identity() {
        let request = SearchRequestV1 {
            request_id: SearchRequestIdV1(4),
            command: SearchCommandV1 {
                room_id: "secret-room".into(),
                profile: SearchMatchProfileV1::ExactUtf8Substring,
                term: "secret-term".into(),
                cursor: Some(ServiceSearchCursorV1::new([7; 16])),
                limit: 2,
            },
        };
        let stale = SearchOutcomeV1::Stale {
            source_revision: 9,
            index_generation: 44,
        };
        let debug = format!("{request:?} {stale:?}");
        assert!(!debug.contains("secret-room"));
        assert!(!debug.contains("secret-term"));
        assert!(!debug.contains("44"));
    }
}
