//! Bounded in-memory semantic reference for Agora's derived search port.
//!
//! This adapter is volatile and accepts explicit projection replacement. It is
//! not an authoritative history store, authority provider, or durability claim.

use agora_search_port::{
    SearchCoverageV1, SearchCursorV1, SearchGenerationV1, SearchHitV1, SearchMatchProfileV1,
    SearchObservationV1, SearchPageV1, SearchPortV1, SearchQueryV1, SearchRefusalV1,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchMemoryLimitsV1 {
    pub max_rooms: usize,
    pub max_messages_per_room: usize,
    pub max_body_bytes: usize,
    pub max_total_content_bytes: usize,
    pub max_room_id_bytes: usize,
    pub max_term_bytes: usize,
    pub max_page_size: u32,
    pub max_cursors: usize,
}

impl Default for SearchMemoryLimitsV1 {
    fn default() -> Self {
        Self {
            max_rooms: 64,
            max_messages_per_room: 1_024,
            max_body_bytes: 16 * 1_024,
            max_total_content_bytes: 4 * 1_024 * 1_024,
            max_room_id_bytes: 128,
            max_term_bytes: 256,
            max_page_size: 100,
            max_cursors: 1_024,
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct IndexedMessageV1 {
    pub message_id: [u8; 16],
    pub message_sequence: u64,
    pub body: String,
}

impl fmt::Debug for IndexedMessageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexedMessageV1")
            .field("message_id", &"<opaque>")
            .field("message_sequence", &self.message_sequence)
            .field("body_bytes", &self.body.len())
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct RoomProjectionV1 {
    pub room_id: String,
    pub source_revision: u64,
    pub coverage: SearchCoverageV1,
    pub messages: Vec<IndexedMessageV1>,
}

impl fmt::Debug for RoomProjectionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RoomProjectionV1")
            .field("room_id_bytes", &self.room_id.len())
            .field("source_revision", &self.source_revision)
            .field("coverage", &self.coverage)
            .field("message_count", &self.messages.len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionRefusalV1 {
    InvalidProjection,
    RevisionRegression,
    Capacity,
    Exhausted,
    Fenced,
}

#[derive(Clone)]
struct Projection {
    source_revision: u64,
    generation: SearchGenerationV1,
    coverage: SearchCoverageV1,
    messages: Vec<IndexedMessageV1>,
    content_bytes: usize,
}

#[derive(Clone, Eq, PartialEq)]
struct CursorState {
    room_id: String,
    source_revision: u64,
    generation: SearchGenerationV1,
    profile: SearchMatchProfileV1,
    term: String,
    limit: u32,
    offset: usize,
}

pub struct SearchMemoryV1 {
    limits: SearchMemoryLimitsV1,
    rooms: BTreeMap<String, Projection>,
    cursors: BTreeMap<SearchCursorV1, CursorState>,
    total_content_bytes: usize,
    next_generation: u64,
    next_cursor: u64,
    fenced: bool,
}

impl fmt::Debug for SearchMemoryV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SearchMemoryV1")
            .field("limits", &self.limits)
            .field("room_count", &self.rooms.len())
            .field("cursor_count", &self.cursors.len())
            .field("total_content_bytes", &self.total_content_bytes)
            .field("fenced", &self.fenced)
            .finish()
    }
}

impl SearchMemoryV1 {
    pub fn new(limits: SearchMemoryLimitsV1) -> Self {
        Self {
            limits,
            rooms: BTreeMap::new(),
            cursors: BTreeMap::new(),
            total_content_bytes: 0,
            next_generation: 1,
            next_cursor: 0,
            fenced: false,
        }
    }

    pub fn replace_room(
        &mut self,
        projection: RoomProjectionV1,
    ) -> Result<SearchGenerationV1, ProjectionRefusalV1> {
        if self.fenced {
            return Err(ProjectionRefusalV1::Fenced);
        }
        let content_bytes = self.validate_projection(&projection)?;
        if self
            .rooms
            .get(&projection.room_id)
            .is_some_and(|prior| projection.source_revision < prior.source_revision)
        {
            return Err(ProjectionRefusalV1::RevisionRegression);
        }
        let prior_bytes = self
            .rooms
            .get(&projection.room_id)
            .map_or(0, |prior| prior.content_bytes);
        let prospective_total = self
            .total_content_bytes
            .checked_sub(prior_bytes)
            .and_then(|value| value.checked_add(content_bytes))
            .ok_or(ProjectionRefusalV1::Capacity)?;
        if prospective_total > self.limits.max_total_content_bytes {
            return Err(ProjectionRefusalV1::Capacity);
        }
        if !self.rooms.contains_key(&projection.room_id)
            && self.rooms.len() >= self.limits.max_rooms
        {
            return Err(ProjectionRefusalV1::Capacity);
        }
        let generation = SearchGenerationV1::new(self.next_generation);
        let next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or(ProjectionRefusalV1::Exhausted)?;
        let room_id = projection.room_id;
        self.rooms.insert(
            room_id.clone(),
            Projection {
                source_revision: projection.source_revision,
                generation,
                coverage: projection.coverage,
                messages: projection.messages,
                content_bytes,
            },
        );
        self.cursors
            .retain(|_, cursor| cursor.room_id.as_str() != room_id.as_str());
        self.total_content_bytes = prospective_total;
        self.next_generation = next_generation;
        Ok(generation)
    }

    /// Irreversibly withdraws this instance. A replacement owner must rebuild
    /// projections explicitly and receives a fresh cursor namespace.
    pub fn fence(&mut self) {
        self.fenced = true;
        self.cursors.clear();
    }

    pub fn room_count(&self) -> usize {
        self.rooms.len()
    }

    pub fn total_content_bytes(&self) -> usize {
        self.total_content_bytes
    }

    fn validate_projection(
        &self,
        projection: &RoomProjectionV1,
    ) -> Result<usize, ProjectionRefusalV1> {
        if projection.room_id.is_empty()
            || projection.room_id.len() > self.limits.max_room_id_bytes
            || projection.messages.len() > self.limits.max_messages_per_room
        {
            return Err(ProjectionRefusalV1::InvalidProjection);
        }
        let mut ids = BTreeSet::new();
        let mut prior_sequence = None;
        let mut content_bytes = 0usize;
        for message in &projection.messages {
            if message.body.len() > self.limits.max_body_bytes
                || !ids.insert(message.message_id)
                || prior_sequence.is_some_and(|prior| message.message_sequence <= prior)
            {
                return Err(ProjectionRefusalV1::InvalidProjection);
            }
            prior_sequence = Some(message.message_sequence);
            content_bytes = content_bytes
                .checked_add(message.body.len())
                .ok_or(ProjectionRefusalV1::Capacity)?;
        }
        Ok(content_bytes)
    }

    fn validate_query(&self, query: &SearchQueryV1) -> Result<(), SearchRefusalV1> {
        if query.room_id.is_empty()
            || query.room_id.len() > self.limits.max_room_id_bytes
            || query.term.is_empty()
            || query.term.len() > self.limits.max_term_bytes
        {
            return Err(SearchRefusalV1::InvalidQuery);
        }
        if query.limit == 0 || query.limit > self.limits.max_page_size {
            return Err(SearchRefusalV1::InvalidLimit);
        }
        Ok(())
    }

    fn allocate_cursor(&mut self, state: CursorState) -> Result<SearchCursorV1, SearchRefusalV1> {
        if self.cursors.len() >= self.limits.max_cursors {
            return Err(SearchRefusalV1::Capacity);
        }
        let next = self
            .next_cursor
            .checked_add(1)
            .ok_or(SearchRefusalV1::Capacity)?;
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(b"AGSRCH01");
        bytes[8..].copy_from_slice(&self.next_cursor.to_be_bytes());
        let cursor = SearchCursorV1::new(bytes);
        self.cursors.insert(cursor, state);
        self.next_cursor = next;
        Ok(cursor)
    }
}

impl SearchPortV1 for SearchMemoryV1 {
    fn search(&mut self, query: SearchQueryV1) -> Result<SearchObservationV1, SearchRefusalV1> {
        self.validate_query(&query)?;
        if self.fenced {
            return Ok(SearchObservationV1::Unavailable);
        }
        let Some(projection) = self.rooms.get(&query.room_id) else {
            return Ok(SearchObservationV1::Unavailable);
        };
        if projection.source_revision != query.observed_revision {
            return Ok(SearchObservationV1::Stale {
                source_revision: projection.source_revision,
                generation: projection.generation,
            });
        }
        let generation = projection.generation;
        let coverage = projection.coverage;
        let offset = if let Some(cursor) = query.cursor {
            let state = self
                .cursors
                .get(&cursor)
                .ok_or(SearchRefusalV1::InvalidCursor)?;
            if state.room_id != query.room_id
                || state.source_revision != query.observed_revision
                || state.generation != generation
                || state.profile != query.profile
                || state.term != query.term
                || state.limit != query.limit
            {
                return Err(SearchRefusalV1::InvalidCursor);
            }
            state.offset
        } else {
            0
        };
        let matches: Vec<SearchHitV1> = projection
            .messages
            .iter()
            .filter(|message| match query.profile {
                SearchMatchProfileV1::ExactUtf8Substring => message.body.contains(&query.term),
            })
            .map(|message| SearchHitV1 {
                message_id: message.message_id,
                message_sequence: message.message_sequence,
            })
            .collect();
        if offset > matches.len() {
            return Err(SearchRefusalV1::InvalidCursor);
        }
        let end = offset
            .saturating_add(query.limit as usize)
            .min(matches.len());
        let next_state = (end < matches.len()).then(|| CursorState {
            room_id: query.room_id.clone(),
            source_revision: query.observed_revision,
            generation,
            profile: query.profile,
            term: query.term.clone(),
            limit: query.limit,
            offset: end,
        });
        if next_state.is_some()
            && query.cursor.is_none()
            && self.cursors.len() >= self.limits.max_cursors
        {
            return Err(SearchRefusalV1::Capacity);
        }
        if let Some(cursor) = query.cursor {
            self.cursors.remove(&cursor);
        }
        let next = next_state
            .map(|state| self.allocate_cursor(state))
            .transpose()?;
        let page = SearchPageV1 {
            source_revision: query.observed_revision,
            generation,
            hits: matches[offset..end].to_vec(),
            next,
        };
        Ok(match coverage {
            SearchCoverageV1::Complete => SearchObservationV1::Available(page),
            SearchCoverageV1::Incomplete => SearchObservationV1::Incomplete(page),
        })
    }
}
