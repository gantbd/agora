//! Authenticated, non-durable composition for transient collaboration facts.
//!
//! This wrapper deliberately has no wire or checkpoint profile yet. It binds
//! the dark transient owner to Host sessions and qualified Host time while the
//! public Protocol/Wire/SDK surface remains unadvertised.

use crate::v2::{HostSessionIdV2, HostV2, OpenReplyV2};
use crate::{HostRefusal, HostTimeV1, PrincipalVerifier};
use agora_service::transient::{
    PresenceCursorV1, PresencePageV1, PresenceSourceV1, PresenceStateV1, TransientCollaborationV1,
    TransientLimitsV1, TransientRefusalV1, TransientTimeV1,
};
use std::fmt;

#[derive(Debug)]
pub enum TransientHostRefusalV1 {
    Host(HostRefusal),
    Transient(TransientRefusalV1),
}

impl From<HostRefusal> for TransientHostRefusalV1 {
    fn from(value: HostRefusal) -> Self {
        Self::Host(value)
    }
}

impl From<TransientRefusalV1> for TransientHostRefusalV1 {
    fn from(value: TransientRefusalV1) -> Self {
        Self::Transient(value)
    }
}

/// Non-durable Host composition for expiring presence state.
///
/// Reconstructing this wrapper around a recovered Host intentionally starts
/// with an empty transient owner, so every presence projection is `Unknown`
/// until an authenticated live session publishes again.
pub struct TransientHostV1<V> {
    host: HostV2<V>,
    transient: TransientCollaborationV1,
}

impl<V> fmt::Debug for TransientHostV1<V> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TransientHostV1")
            .field("signal_count", &self.transient.signal_count())
            .field("generation", &self.transient.generation())
            .field("fidelity_lost", &self.transient.fidelity_lost())
            .finish_non_exhaustive()
    }
}

impl<V: PrincipalVerifier> TransientHostV1<V> {
    pub fn new(host: HostV2<V>, limits: TransientLimitsV1) -> Result<Self, TransientRefusalV1> {
        Ok(Self {
            host,
            transient: TransientCollaborationV1::new(limits)?,
        })
    }

    pub fn host(&self) -> &HostV2<V> {
        &self.host
    }

    pub fn into_host(self) -> HostV2<V> {
        self.host
    }

    pub fn open(
        &mut self,
        frame: &[u8],
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<OpenReplyV2, HostRefusal> {
        self.host.open(frame, context, now)
    }

    /// Executes an ordinary conversation frame, then retires any transient
    /// state invalidated by session or membership lifecycle before returning.
    pub fn handle(
        &mut self,
        session: HostSessionIdV2,
        frame: &[u8],
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<Vec<u8>, HostRefusal> {
        let result = self.host.handle(session, frame, context, now);
        if !self.host.has_session(session) {
            self.transient.retire_source(source(session));
        }
        self.transient
            .reconcile_membership(self.host.service().engine());
        result
    }

    pub fn close(&mut self, session: HostSessionIdV2) -> bool {
        let closed = self.host.close(session);
        if closed {
            self.transient.retire_source(source(session));
        }
        closed
    }

    pub fn publish_presence(
        &mut self,
        session: HostSessionIdV2,
        room_id: &str,
        state: PresenceStateV1,
        lifetime: u64,
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<TransientTimeV1, TransientHostRefusalV1> {
        let participant = self.authorize(session, context, now)?;
        let result = self.transient.publish_presence_for(
            self.host.service().engine(),
            room_id,
            &participant,
            source(session),
            state,
            TransientTimeV1(now.0),
            lifetime,
        );
        self.finish_transient(session, result)
    }

    pub fn clear_presence(
        &mut self,
        session: HostSessionIdV2,
        room_id: &str,
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<bool, TransientHostRefusalV1> {
        let participant = self.authorize(session, context, now)?;
        let result = self.transient.clear_presence_for(
            self.host.service().engine(),
            room_id,
            &participant,
            source(session),
            TransientTimeV1(now.0),
        );
        self.finish_transient(session, result)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn presence_page(
        &mut self,
        session: HostSessionIdV2,
        room_id: &str,
        cursor: Option<&PresenceCursorV1>,
        limit: usize,
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<PresencePageV1, TransientHostRefusalV1> {
        let participant = self.authorize(session, context, now)?;
        let result = self.transient.presence_page_for(
            self.host.service().engine(),
            &participant,
            room_id,
            cursor,
            limit,
            TransientTimeV1(now.0),
        );
        self.finish_transient(session, result)
    }

    pub fn expire(&mut self, now: HostTimeV1, budget: usize) -> Result<usize, TransientRefusalV1> {
        self.transient.expire(TransientTimeV1(now.0), budget)
    }

    fn authorize(
        &mut self,
        session: HostSessionIdV2,
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<String, TransientHostRefusalV1> {
        match self.host.authorize_transient(session, context, now) {
            Ok(participant) => Ok(participant),
            Err(refusal) => {
                if !self.host.has_session(session) {
                    self.transient.retire_source(source(session));
                }
                Err(refusal.into())
            }
        }
    }

    fn finish_transient<T>(
        &mut self,
        session: HostSessionIdV2,
        result: Result<T, TransientRefusalV1>,
    ) -> Result<T, TransientHostRefusalV1> {
        if matches!(result, Err(TransientRefusalV1::ClockRegression)) {
            let _ = self.host.close(session);
            self.transient.retire_source(source(session));
        }
        result.map_err(Into::into)
    }
}

fn source(session: HostSessionIdV2) -> PresenceSourceV1 {
    PresenceSourceV1::new(session.source_bytes())
}
