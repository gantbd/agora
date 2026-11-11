//! Effect-free contract for derived Agora message search.
//!
//! This crate owns no index, conversation state, identity, or authority. A
//! caller must establish current room membership before querying and resolve
//! every returned identity against current authoritative history before
//! disclosure.

use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchMatchProfileV1 {
    /// Case-sensitive, byte-exact substring matching over valid UTF-8 content.
    ExactUtf8Substring,
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SearchGenerationV1(u64);

impl SearchGenerationV1 {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Debug for SearchGenerationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SearchGenerationV1(<opaque>)")
    }
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SearchCursorV1([u8; 16]);

impl SearchCursorV1 {
    pub const fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn into_bytes(self) -> [u8; 16] {
        self.0
    }
}

impl fmt::Debug for SearchCursorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SearchCursorV1(<opaque>)")
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct SearchQueryV1 {
    pub room_id: String,
    pub observed_revision: u64,
    pub profile: SearchMatchProfileV1,
    pub term: String,
    pub cursor: Option<SearchCursorV1>,
    pub limit: u32,
}

impl fmt::Debug for SearchQueryV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SearchQueryV1")
            .field("room_id_bytes", &self.room_id.len())
            .field("observed_revision", &self.observed_revision)
            .field("profile", &self.profile)
            .field("term_bytes", &self.term.len())
            .field("cursor", &self.cursor)
            .field("limit", &self.limit)
            .finish()
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct SearchHitV1 {
    pub message_id: [u8; 16],
    pub message_sequence: u64,
}

impl fmt::Debug for SearchHitV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SearchHitV1")
            .field("message_id", &"<opaque>")
            .field("message_sequence", &self.message_sequence)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchCoverageV1 {
    Complete,
    /// The supplied source could not prove complete retained-message coverage.
    Incomplete,
}

#[derive(Clone, Eq, PartialEq)]
pub struct SearchPageV1 {
    pub source_revision: u64,
    pub generation: SearchGenerationV1,
    pub hits: Vec<SearchHitV1>,
    pub next: Option<SearchCursorV1>,
}

impl fmt::Debug for SearchPageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SearchPageV1")
            .field("source_revision", &self.source_revision)
            .field("generation", &self.generation)
            .field("hit_count", &self.hits.len())
            .field("next", &self.next)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum SearchObservationV1 {
    Available(SearchPageV1),
    Incomplete(SearchPageV1),
    Stale {
        source_revision: u64,
        generation: SearchGenerationV1,
    },
    Unavailable,
}

impl fmt::Debug for SearchObservationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Available(page) => formatter.debug_tuple("Available").field(page).finish(),
            Self::Incomplete(page) => formatter.debug_tuple("Incomplete").field(page).finish(),
            Self::Stale {
                source_revision,
                generation,
            } => formatter
                .debug_struct("Stale")
                .field("source_revision", source_revision)
                .field("generation", generation)
                .finish(),
            Self::Unavailable => formatter.write_str("Unavailable"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchRefusalV1 {
    InvalidQuery,
    InvalidLimit,
    InvalidCursor,
    Capacity,
}

/// Derived search boundary. Implementations must perform bounded work for every
/// request and must never infer participant authority from query values.
pub trait SearchPortV1 {
    fn search(&mut self, query: SearchQueryV1) -> Result<SearchObservationV1, SearchRefusalV1>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_is_content_free() {
        let query = SearchQueryV1 {
            room_id: "secret-room".into(),
            observed_revision: 7,
            profile: SearchMatchProfileV1::ExactUtf8Substring,
            term: "secret-term".into(),
            cursor: Some(SearchCursorV1::new([9; 16])),
            limit: 3,
        };
        let hit = SearchHitV1 {
            message_id: [8; 16],
            message_sequence: 4,
        };
        let debug = format!("{query:?} {hit:?}");
        assert!(!debug.contains("secret-room"));
        assert!(!debug.contains("secret-term"));
        assert!(!debug.contains("080808"));
        assert!(debug.contains("room_id_bytes"));
        assert!(debug.contains("message_sequence"));
    }
}
