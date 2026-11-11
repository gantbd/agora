//! Session-bound outcome uncertainty for the separate derived-search profile.

use agora_protocol::search::{
    SearchCommandV1, SearchOfferV1, SearchReplyV1, SearchRequestIdV1, SearchRequestV1,
};
use agora_protocol::v2::ClientIdV2;
use std::fmt;

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct SearchReplayEpochV1([u8; 32]);

impl SearchReplayEpochV1 {
    /// The caller must change this value for every new authenticated Host
    /// session and every new server incarnation.
    pub fn new(bytes: [u8; 32]) -> Option<Self> {
        (bytes != [0; 32]).then_some(Self(bytes))
    }
}

impl fmt::Debug for SearchReplayEpochV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SearchReplayEpochV1(<opaque>)")
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct SearchClientStateV1 {
    pub replay_epoch: SearchReplayEpochV1,
    pub client: ClientIdV2,
    pub next_request: u64,
    pub pending: Option<SearchRequestV1>,
}

impl fmt::Debug for SearchClientStateV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SearchClientStateV1")
            .field("replay_epoch", &self.replay_epoch)
            .field("client", &self.client)
            .field("next_request", &self.next_request)
            .field(
                "pending_request",
                &self.pending.as_ref().map(|request| request.request_id),
            )
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchClientRestoreErrorV1 {
    ReplayEpochChanged,
    ClientChanged,
    InvalidPendingRequest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchPortFailureV1<E> {
    NotSent(E),
    OutcomeUnknown(E),
}

pub trait SearchPortV1 {
    type Error;

    fn execute(
        &mut self,
        offer: SearchOfferV1,
    ) -> Result<SearchReplyV1, SearchPortFailureV1<Self::Error>>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchClientErrorV1<E> {
    RequestIdExhausted,
    NotSent(E),
    OutcomeUnknown(E),
    UnresolvedOutcome,
}

#[derive(Debug)]
pub struct SearchClientV1<P> {
    replay_epoch: SearchReplayEpochV1,
    client: ClientIdV2,
    next_request: u64,
    pending: Option<SearchRequestV1>,
    port: P,
}

impl<P: SearchPortV1> SearchClientV1<P> {
    pub fn new(replay_epoch: SearchReplayEpochV1, client: ClientIdV2, port: P) -> Self {
        Self {
            replay_epoch,
            client,
            next_request: 0,
            pending: None,
            port,
        }
    }

    pub fn restore(
        current_replay_epoch: SearchReplayEpochV1,
        current_client: ClientIdV2,
        state: SearchClientStateV1,
        port: P,
    ) -> Result<Self, SearchClientRestoreErrorV1> {
        if current_replay_epoch != state.replay_epoch {
            return Err(SearchClientRestoreErrorV1::ReplayEpochChanged);
        }
        if current_client != state.client {
            return Err(SearchClientRestoreErrorV1::ClientChanged);
        }
        if let Some(pending) = &state.pending
            && state.next_request.checked_sub(1) != Some(pending.request_id.0)
        {
            return Err(SearchClientRestoreErrorV1::InvalidPendingRequest);
        }
        Ok(Self {
            replay_epoch: state.replay_epoch,
            client: state.client,
            next_request: state.next_request,
            pending: state.pending,
            port,
        })
    }

    pub fn execute(
        &mut self,
        command: SearchCommandV1,
    ) -> Result<SearchReplyV1, SearchClientErrorV1<P::Error>> {
        if let Some(pending) = &self.pending {
            if pending.command != command {
                return Err(SearchClientErrorV1::UnresolvedOutcome);
            }
            return self.execute_offer(pending.clone(), true);
        }

        let request_id = SearchRequestIdV1(self.next_request);
        self.next_request = self
            .next_request
            .checked_add(1)
            .ok_or(SearchClientErrorV1::RequestIdExhausted)?;
        self.execute_offer(
            SearchRequestV1 {
                request_id,
                command,
            },
            false,
        )
    }

    fn execute_offer(
        &mut self,
        request: SearchRequestV1,
        was_pending: bool,
    ) -> Result<SearchReplyV1, SearchClientErrorV1<P::Error>> {
        match self.port.execute(SearchOfferV1 {
            client: self.client,
            request: request.clone(),
        }) {
            Ok(reply) => {
                if was_pending {
                    self.pending = None;
                }
                Ok(reply)
            }
            Err(SearchPortFailureV1::NotSent(error)) => Err(SearchClientErrorV1::NotSent(error)),
            Err(SearchPortFailureV1::OutcomeUnknown(error)) => {
                self.pending = Some(request);
                Err(SearchClientErrorV1::OutcomeUnknown(error))
            }
        }
    }

    pub fn pending_request(&self) -> Option<&SearchRequestV1> {
        self.pending.as_ref()
    }

    pub fn state(&self) -> SearchClientStateV1 {
        SearchClientStateV1 {
            replay_epoch: self.replay_epoch,
            client: self.client,
            next_request: self.next_request,
            pending: self.pending.clone(),
        }
    }

    pub fn into_state(self) -> (P, SearchClientStateV1) {
        let state = SearchClientStateV1 {
            replay_epoch: self.replay_epoch,
            client: self.client,
            next_request: self.next_request,
            pending: self.pending,
        };
        (self.port, state)
    }

    /// Starts a fresh namespace and returns uncertainty from the prior session
    /// for explicit application or operator accounting.
    pub fn rebind(
        self,
        replay_epoch: SearchReplayEpochV1,
        client: ClientIdV2,
    ) -> (Self, Option<SearchRequestV1>) {
        let unresolved = self.pending;
        (
            Self {
                replay_epoch,
                client,
                next_request: 0,
                pending: None,
                port: self.port,
            },
            unresolved,
        )
    }
}
