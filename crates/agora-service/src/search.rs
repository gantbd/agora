//! Participant-bound volatile composition over a derived search port.

use crate::v2::ServiceV2;
use agora_engine::{MessageId, ParticipantId, RoomId};
use agora_protocol::search::{
    SearchCommandV1, SearchHitV1 as ProtocolSearchHitV1, SearchMatchProfileV1,
    SearchOutcomeV1 as ProtocolSearchOutcomeV1, SearchPageV1 as ProtocolSearchPageV1,
    SearchRefusalV1 as ProtocolSearchRefusalV1, SearchReplyV1, SearchRequestIdV1, SearchRequestV1,
    ServiceSearchCursorV1,
};
use agora_protocol::v2::{ClientIdV2, MessageIdV1, ReplyV2, RequestV2};
pub use agora_search_port::SearchPortV1;
use agora_search_port::{
    SearchCursorV1 as AdapterCursorV1, SearchMatchProfileV1 as AdapterMatchProfileV1,
    SearchObservationV1, SearchPageV1, SearchQueryV1, SearchRefusalV1 as AdapterRefusalV1,
};
use std::collections::BTreeMap;

/// Port vocabulary re-exported for composition adapters that should depend on
/// the service boundary rather than the reference index implementation.
pub mod port {
    pub use agora_search_port::{
        SearchCoverageV1, SearchCursorV1, SearchGenerationV1, SearchHitV1, SearchMatchProfileV1,
        SearchObservationV1, SearchPageV1, SearchQueryV1, SearchRefusalV1,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchServiceLimitsV1 {
    pub max_retained_requests: usize,
    pub max_retained_cursors: usize,
    pub max_room_id_bytes: usize,
    pub max_term_bytes: usize,
    pub max_page_items: u32,
}

impl Default for SearchServiceLimitsV1 {
    fn default() -> Self {
        Self {
            max_retained_requests: 4_096,
            max_retained_cursors: 4_096,
            max_room_id_bytes: 128,
            max_term_bytes: 256,
            max_page_items: 100,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SearchServiceUsageV1 {
    pub offers: u64,
    pub accepted: u64,
    pub refused: u64,
    pub replayed: u64,
    pub conflicts: u64,
    pub cursor_retirements: u64,
    pub overflowed: bool,
}

fn count(value: &mut u64, overflowed: &mut bool) {
    let old = *value;
    *value = value.saturating_add(1);
    *overflowed |= old == u64::MAX;
}

#[derive(Clone)]
struct RetainedSearchRequest {
    request: SearchRequestV1,
    reply: SearchReplyV1,
}

#[derive(Clone)]
struct SearchCursorRecord {
    owner: ClientIdV2,
    room_id: String,
    profile: SearchMatchProfileV1,
    term: String,
    limit: u32,
    source_revision: u64,
    index_generation: u64,
    adapter: AdapterCursorV1,
}

struct SearchContext<'a> {
    service: &'a ServiceV2,
    client: ClientIdV2,
    command: &'a SearchCommandV1,
    room_id: &'a RoomId,
    source_revision: u64,
    expected_generation: Option<u64>,
}

/// Convenience owner used by embedded compositions. Hosts that already own the
/// authoritative service use [`SearchRuntimeV1`] as a sidecar instead.
pub struct SearchServiceV1<P> {
    service: ServiceV2,
    runtime: SearchRuntimeV1<P>,
}

impl<P: SearchPortV1> SearchServiceV1<P> {
    pub fn new(service: ServiceV2, port: P, limits: SearchServiceLimitsV1) -> Self {
        Self {
            service,
            runtime: SearchRuntimeV1::new(port, limits),
        }
    }

    pub fn service(&self) -> &ServiceV2 {
        &self.service
    }

    pub fn port(&self) -> &P {
        self.runtime.port()
    }

    pub fn into_parts(self) -> (ServiceV2, P) {
        (self.service, self.runtime.into_port())
    }

    pub fn usage(&self) -> SearchServiceUsageV1 {
        self.runtime.usage()
    }

    pub fn connect(
        &mut self,
        participant: &str,
    ) -> Result<ClientIdV2, agora_protocol::v2::RefusalV2> {
        self.service.connect(participant)
    }

    pub fn disconnect(
        &mut self,
        client: ClientIdV2,
    ) -> Result<bool, agora_protocol::v2::RefusalV2> {
        self.runtime.retire_client(client);
        self.service.disconnect(client)
    }

    pub fn execute_conversation(&mut self, client: ClientIdV2, request: RequestV2) -> ReplyV2 {
        let before = self.service.engine().revision();
        let reply = self.service.execute(client, request);
        if self.service.engine().revision() != before {
            self.runtime.conversation_mutated();
        }
        reply
    }

    pub fn search(&mut self, client: ClientIdV2, request: SearchRequestV1) -> SearchReplyV1 {
        self.runtime.search(&self.service, client, request)
    }
}

/// Volatile search receipts, cursors, and adapter ownership beside one
/// authoritative [`ServiceV2`]. It never owns or clones conversation state.
pub struct SearchRuntimeV1<P> {
    port: P,
    limits: SearchServiceLimitsV1,
    retained: BTreeMap<(ClientIdV2, SearchRequestIdV1), RetainedSearchRequest>,
    cursors: BTreeMap<ServiceSearchCursorV1, SearchCursorRecord>,
    next_cursor: u64,
    usage: SearchServiceUsageV1,
}

impl<P: SearchPortV1> SearchRuntimeV1<P> {
    pub fn new(port: P, limits: SearchServiceLimitsV1) -> Self {
        Self {
            port,
            limits,
            retained: BTreeMap::new(),
            cursors: BTreeMap::new(),
            next_cursor: 0,
            usage: SearchServiceUsageV1::default(),
        }
    }

    pub fn port(&self) -> &P {
        &self.port
    }

    pub fn into_port(self) -> P {
        self.port
    }

    pub fn usage(&self) -> SearchServiceUsageV1 {
        self.usage
    }

    pub fn retire_client(&mut self, client: ClientIdV2) {
        self.retire_client_search_state(client);
    }

    pub fn conversation_mutated(&mut self) {
        self.retire_all_cursors();
    }

    pub fn search(
        &mut self,
        service: &ServiceV2,
        client: ClientIdV2,
        request: SearchRequestV1,
    ) -> SearchReplyV1 {
        count(&mut self.usage.offers, &mut self.usage.overflowed);
        let Some(participant) = service.participant_for_client(client).cloned() else {
            return self.refused(ProtocolSearchRefusalV1::UnknownClient);
        };
        let key = (client, request.request_id);
        if let Some(retained) = self.retained.get(&key) {
            if retained.request == request {
                count(&mut self.usage.replayed, &mut self.usage.overflowed);
                return retained.reply.clone();
            }
            count(&mut self.usage.conflicts, &mut self.usage.overflowed);
            return SearchReplyV1::Refused(ProtocolSearchRefusalV1::RequestConflict);
        }
        if self.retained.len() >= self.limits.max_retained_requests {
            return self.refused(ProtocolSearchRefusalV1::RequestLimit);
        }
        let reply = match self.search_new(service, client, &participant, &request.command) {
            Ok(outcome) => {
                count(&mut self.usage.accepted, &mut self.usage.overflowed);
                SearchReplyV1::Accepted(outcome)
            }
            Err(refusal) => self.refused(refusal),
        };
        self.retained.insert(
            key,
            RetainedSearchRequest {
                request,
                reply: reply.clone(),
            },
        );
        reply
    }

    fn search_new(
        &mut self,
        service: &ServiceV2,
        client: ClientIdV2,
        participant: &ParticipantId,
        command: &SearchCommandV1,
    ) -> Result<ProtocolSearchOutcomeV1, ProtocolSearchRefusalV1> {
        if command.room_id.is_empty()
            || command.room_id.len() > self.limits.max_room_id_bytes
            || command.term.is_empty()
            || command.term.len() > self.limits.max_term_bytes
            || command.limit == 0
            || command.limit > self.limits.max_page_items
        {
            return Err(ProtocolSearchRefusalV1::InvalidInput);
        }
        let room_id = RoomId::new(&command.room_id);
        if service.engine().room(&room_id).is_none() {
            return Err(ProtocolSearchRefusalV1::UnknownRoom);
        }
        if service
            .engine()
            .member_role(&room_id, participant)
            .is_none()
        {
            return Err(ProtocolSearchRefusalV1::NotMember);
        }
        let source_revision = service.engine().revision().0;
        let cursor_record = if let Some(cursor) = command.cursor {
            let record = self
                .cursors
                .get(&cursor)
                .ok_or(ProtocolSearchRefusalV1::InvalidCursor)?;
            if record.owner != client
                || record.room_id != command.room_id
                || record.profile != command.profile
                || record.term != command.term
                || record.limit != command.limit
                || record.source_revision != source_revision
            {
                return Err(ProtocolSearchRefusalV1::InvalidCursor);
            }
            Some(record.clone())
        } else {
            None
        };
        let adapter_cursor = cursor_record.as_ref().map(|record| record.adapter);
        let adapter_query = SearchQueryV1 {
            room_id: command.room_id.clone(),
            observed_revision: source_revision,
            profile: map_profile(command.profile),
            term: command.term.clone(),
            cursor: adapter_cursor,
            limit: command.limit,
        };
        let result = self.port.search(adapter_query).map_err(map_port_refusal);
        if let Some(cursor) = command.cursor {
            self.cursors.remove(&cursor);
            if let Some(adapter) = adapter_cursor {
                let _ = self.port.retire_cursor(adapter);
            }
            count(
                &mut self.usage.cursor_retirements,
                &mut self.usage.overflowed,
            );
        }
        let observation = result?;
        if service.engine().revision().0 != source_revision
            || service
                .engine()
                .member_role(&room_id, participant)
                .is_none()
        {
            retire_observation_cursor(&mut self.port, &observation);
            return Ok(ProtocolSearchOutcomeV1::Unavailable);
        }
        self.map_observation(
            SearchContext {
                service,
                client,
                command,
                room_id: &room_id,
                source_revision,
                expected_generation: cursor_record.map(|record| record.index_generation),
            },
            observation,
        )
    }

    fn map_observation(
        &mut self,
        context: SearchContext<'_>,
        observation: SearchObservationV1,
    ) -> Result<ProtocolSearchOutcomeV1, ProtocolSearchRefusalV1> {
        match observation {
            SearchObservationV1::Available(page) => Ok(self.map_page(&context, page)?.map_or(
                ProtocolSearchOutcomeV1::Unavailable,
                ProtocolSearchOutcomeV1::Available,
            )),
            SearchObservationV1::Incomplete(page) => Ok(self.map_page(&context, page)?.map_or(
                ProtocolSearchOutcomeV1::Unavailable,
                ProtocolSearchOutcomeV1::Incomplete,
            )),
            SearchObservationV1::Stale {
                source_revision,
                generation,
            } => Ok(ProtocolSearchOutcomeV1::Stale {
                source_revision,
                index_generation: generation.value(),
            }),
            SearchObservationV1::Unavailable => Ok(ProtocolSearchOutcomeV1::Unavailable),
        }
    }

    fn map_page(
        &mut self,
        context: &SearchContext<'_>,
        page: SearchPageV1,
    ) -> Result<Option<ProtocolSearchPageV1>, ProtocolSearchRefusalV1> {
        let canonical_hits = page.hits.len() <= context.command.limit as usize
            && page
                .hits
                .windows(2)
                .all(|pair| pair[0].message_sequence < pair[1].message_sequence)
            && !(page.hits.is_empty() && page.next.is_some());
        if page.source_revision != context.source_revision
            || context
                .expected_generation
                .is_some_and(|expected| expected != page.generation.value())
            || !canonical_hits
            || page.hits.iter().any(|hit| {
                context
                    .service
                    .engine()
                    .message_by_id(context.room_id, MessageId::from_bytes(hit.message_id))
                    .is_none_or(|message| {
                        message.sequence() != hit.message_sequence
                            || !message.body().contains(&context.command.term)
                    })
            })
        {
            if let Some(cursor) = page.next {
                let _ = self.port.retire_cursor(cursor);
            }
            return Ok(None);
        }
        let next = if let Some(adapter) = page.next {
            match self.allocate_cursor(SearchCursorRecord {
                owner: context.client,
                room_id: context.command.room_id.clone(),
                profile: context.command.profile,
                term: context.command.term.clone(),
                limit: context.command.limit,
                source_revision: context.source_revision,
                index_generation: page.generation.value(),
                adapter,
            }) {
                Ok(cursor) => Some(cursor),
                Err(refusal) => {
                    let _ = self.port.retire_cursor(adapter);
                    return Err(refusal);
                }
            }
        } else {
            None
        };
        Ok(Some(ProtocolSearchPageV1 {
            source_revision: context.source_revision,
            index_generation: page.generation.value(),
            hits: page
                .hits
                .into_iter()
                .map(|hit| ProtocolSearchHitV1 {
                    message_id: MessageIdV1::new(hit.message_id),
                    message_sequence: hit.message_sequence,
                })
                .collect(),
            next,
        }))
    }

    fn allocate_cursor(
        &mut self,
        record: SearchCursorRecord,
    ) -> Result<ServiceSearchCursorV1, ProtocolSearchRefusalV1> {
        if self.cursors.len() >= self.limits.max_retained_cursors {
            return Err(ProtocolSearchRefusalV1::CursorLimit);
        }
        let next = self
            .next_cursor
            .checked_add(1)
            .ok_or(ProtocolSearchRefusalV1::CursorLimit)?;
        let mut bytes = [0; 16];
        bytes[..8].copy_from_slice(b"AGSVCUR1");
        bytes[8..].copy_from_slice(&self.next_cursor.to_be_bytes());
        let cursor = ServiceSearchCursorV1::new(bytes);
        self.cursors.insert(cursor, record);
        self.next_cursor = next;
        Ok(cursor)
    }

    fn retire_client_search_state(&mut self, client: ClientIdV2) {
        self.retained.retain(|(owner, _), _| *owner != client);
        let cursors: Vec<_> = self
            .cursors
            .iter()
            .filter_map(|(cursor, record)| (record.owner == client).then_some(*cursor))
            .collect();
        for cursor in cursors {
            if let Some(record) = self.cursors.remove(&cursor) {
                let _ = self.port.retire_cursor(record.adapter);
                count(
                    &mut self.usage.cursor_retirements,
                    &mut self.usage.overflowed,
                );
            }
        }
    }

    fn retire_all_cursors(&mut self) {
        let cursors = std::mem::take(&mut self.cursors);
        for record in cursors.into_values() {
            let _ = self.port.retire_cursor(record.adapter);
            count(
                &mut self.usage.cursor_retirements,
                &mut self.usage.overflowed,
            );
        }
    }

    fn refused(&mut self, refusal: ProtocolSearchRefusalV1) -> SearchReplyV1 {
        count(&mut self.usage.refused, &mut self.usage.overflowed);
        SearchReplyV1::Refused(refusal)
    }
}

fn map_profile(profile: SearchMatchProfileV1) -> AdapterMatchProfileV1 {
    match profile {
        SearchMatchProfileV1::ExactUtf8Substring => AdapterMatchProfileV1::ExactUtf8Substring,
    }
}

fn map_port_refusal(refusal: AdapterRefusalV1) -> ProtocolSearchRefusalV1 {
    match refusal {
        AdapterRefusalV1::InvalidQuery | AdapterRefusalV1::InvalidLimit => {
            ProtocolSearchRefusalV1::InvalidInput
        }
        AdapterRefusalV1::InvalidCursor => ProtocolSearchRefusalV1::InvalidCursor,
        AdapterRefusalV1::Capacity => ProtocolSearchRefusalV1::IndexCapacity,
    }
}

fn retire_observation_cursor<P: SearchPortV1>(port: &mut P, observation: &SearchObservationV1) {
    let cursor = match observation {
        SearchObservationV1::Available(page) | SearchObservationV1::Incomplete(page) => page.next,
        SearchObservationV1::Stale { .. } | SearchObservationV1::Unavailable => None,
    };
    if let Some(cursor) = cursor {
        let _ = port.retire_cursor(cursor);
    }
}
