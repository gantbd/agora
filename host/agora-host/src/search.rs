//! Authenticated composition for the separate participant-bound search profile.

use crate::v2::{HostSessionIdV2, HostV2, OpenReplyV2};
use crate::{HostRefusal, HostTimeV1, PrincipalVerifier};
pub use agora_service::search::SearchPortV1;
use agora_service::search::{SearchRuntimeV1, SearchServiceLimitsV1, SearchServiceUsageV1};
use agora_wire::search::{decode_search_offer_v1, encode_search_reply_v1};

/// Owns one conversation Host and a volatile search sidecar over the same
/// authoritative Service V2. Search receipts and cursors are never checkpointed.
pub struct SearchHostV1<V, P> {
    host: HostV2<V>,
    search: SearchRuntimeV1<P>,
}

impl<V: PrincipalVerifier, P: SearchPortV1> SearchHostV1<V, P> {
    pub fn new(host: HostV2<V>, port: P, limits: SearchServiceLimitsV1) -> Self {
        Self {
            host,
            search: SearchRuntimeV1::new(port, limits),
        }
    }

    pub fn host(&self) -> &HostV2<V> {
        &self.host
    }

    pub fn search_usage(&self) -> SearchServiceUsageV1 {
        self.search.usage()
    }

    pub fn into_parts(self) -> (HostV2<V>, P) {
        (self.host, self.search.into_port())
    }

    pub fn open(
        &mut self,
        frame: &[u8],
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<OpenReplyV2, HostRefusal> {
        self.host.open(frame, context, now)
    }

    /// Executes an ordinary conversation frame and retires search state after
    /// any authoritative mutation or session retirement.
    pub fn handle(
        &mut self,
        session: HostSessionIdV2,
        frame: &[u8],
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<Vec<u8>, HostRefusal> {
        let client = self.host.client_for_session(session);
        let before = self.host.service().engine().revision();
        let result = self.host.handle(session, frame, context, now);
        if self.host.service().engine().revision() != before {
            self.search.conversation_mutated();
        }
        if !self.host.has_session(session)
            && let Some(client) = client
        {
            self.search.retire_client(client);
        }
        result
    }

    pub fn handle_search(
        &mut self,
        session: HostSessionIdV2,
        frame: &[u8],
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<Vec<u8>, HostRefusal> {
        let prior_client = self.host.client_for_session(session);
        let client = match self.host.authorize_search(session, context, now) {
            Ok(client) => client,
            Err(refusal) => {
                if !self.host.has_session(session)
                    && let Some(client) = prior_client
                {
                    self.search.retire_client(client);
                }
                return Err(refusal);
            }
        };
        let offer = decode_search_offer_v1(frame, self.host.wire_limits())
            .map_err(HostRefusal::InvalidFrame)?;
        if offer.client != client {
            let refusal = self.host.retire_search_protocol_violation(session);
            self.search.retire_client(client);
            return Err(refusal);
        }
        let reply = self
            .search
            .search(self.host.service(), client, offer.request);
        encode_search_reply_v1(&reply, self.host.wire_limits()).map_err(|_| HostRefusal::Capacity)
    }

    pub fn close(&mut self, session: HostSessionIdV2) -> bool {
        let client = self.host.client_for_session(session);
        let closed = self.host.close(session);
        if closed && let Some(client) = client {
            self.search.retire_client(client);
        }
        closed
    }
}
