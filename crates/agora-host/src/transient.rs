//! Authenticated, non-durable composition for transient collaboration facts.
//!
//! This wrapper deliberately has no checkpoint profile. It binds the separate
//! presence and typing owners to Host sessions, qualified Host time, and the
//! unadvertised transient wire profile.

use crate::v2::{HostSessionIdV2, HostV2, OpenReplyV2};
use crate::{HostRefusal, HostTimeV1, PrincipalVerifier};
use agora_protocol::transient::{
    PresenceEntryV1 as ProtocolPresenceEntryV1, PresencePageCursorV1,
    PresencePageV1 as ProtocolPresencePageV1, PresenceSourceClassV1 as ProtocolSourceClassV1,
    PresenceStateV1 as ProtocolPresenceStateV1, PresenceViewStateV1 as ProtocolPresenceViewStateV1,
    TransientCommandV1, TransientOutcomeV1, TransientRefusalV1 as ProtocolTransientRefusalV1,
    TransientReplyV1, TransientRequestV1, TypingEntryV1 as ProtocolTypingEntryV1,
    TypingPageCursorV1, TypingPageV1 as ProtocolTypingPageV1,
    TypingTargetV1 as ProtocolTypingTargetV1, TypingViewStateV1,
};
use agora_protocol::v2::ParticipantKindV2;
use agora_service::transient::{
    PresenceCursorV1, PresencePageV1, PresenceSourceV1, PresenceStateV1 as ServicePresenceStateV1,
    PresenceViewStateV1 as ServicePresenceViewStateV1, TransientCollaborationV1, TransientLimitsV1,
    TransientParticipantKindV1, TransientRefusalV1, TransientTimeV1,
};
use agora_service::typing::{
    TypingCollaborationV1, TypingCursorV1, TypingLimitsV1, TypingPageV1, TypingRefusalV1,
    TypingTargetV1,
};
use agora_wire::transient::{decode_transient_request_v1, encode_transient_reply_v1};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug)]
pub enum TransientHostRefusalV1 {
    Host(HostRefusal),
    Transient(TransientRefusalV1),
    Typing(TypingRefusalV1),
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

impl From<TypingRefusalV1> for TransientHostRefusalV1 {
    fn from(value: TypingRefusalV1) -> Self {
        Self::Typing(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransientHostBuildRefusalV1 {
    InvalidCursorAge,
    Presence(TransientRefusalV1),
    Typing(TypingRefusalV1),
}

/// Non-durable Host composition for expiring presence and typing state.
///
/// Reconstructing this wrapper around a recovered Host intentionally starts
/// with empty transient owners, so presence projects `Unknown` and typing
/// projects inactive until an authenticated live session publishes again.
pub struct TransientHostV1<V> {
    host: HostV2<V>,
    presence: TransientCollaborationV1,
    typing: TypingCollaborationV1,
    sessions: BTreeMap<HostSessionIdV2, TransientSessionV1>,
    max_cursor_age: u64,
    last_time: Option<HostTimeV1>,
}

#[derive(Clone)]
struct PresenceCursorRecordV1 {
    public: PresencePageCursorV1,
    room_id: String,
    inner: PresenceCursorV1,
    as_of: TransientTimeV1,
    expires_at: TransientTimeV1,
}

#[derive(Clone)]
struct TypingCursorRecordV1 {
    public: TypingPageCursorV1,
    room_id: String,
    target: ProtocolTypingTargetV1,
    inner: TypingCursorV1,
    as_of: TransientTimeV1,
    expires_at: TransientTimeV1,
}

#[derive(Clone)]
enum CursorV1 {
    Presence(PresenceCursorRecordV1),
    Typing(TypingCursorRecordV1),
}

#[derive(Default)]
struct TransientSessionV1 {
    next_request: u64,
    last: Option<(TransientRequestV1, TransientReplyV1)>,
    next_cursor: u64,
    cursor: Option<CursorV1>,
}

impl<V> fmt::Debug for TransientHostV1<V> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TransientHostV1")
            .field("presence_signal_count", &self.presence.signal_count())
            .field("presence_generation", &self.presence.generation())
            .field("presence_fidelity_lost", &self.presence.fidelity_lost())
            .field("typing_signal_count", &self.typing.signal_count())
            .field("typing_generation", &self.typing.generation())
            .field("typing_fidelity_lost", &self.typing.fidelity_lost())
            .field("transient_sessions", &self.sessions.len())
            .finish_non_exhaustive()
    }
}

impl<V: PrincipalVerifier> TransientHostV1<V> {
    pub fn new(
        host: HostV2<V>,
        limits: TransientLimitsV1,
        max_cursor_age: u64,
    ) -> Result<Self, TransientRefusalV1> {
        Self::new_with_typing(host, limits, TypingLimitsV1::default(), max_cursor_age).map_err(
            |refusal| match refusal {
                TransientHostBuildRefusalV1::Presence(refusal) => refusal,
                TransientHostBuildRefusalV1::InvalidCursorAge
                | TransientHostBuildRefusalV1::Typing(_) => TransientRefusalV1::InvalidLimits,
            },
        )
    }

    pub fn new_with_typing(
        host: HostV2<V>,
        presence_limits: TransientLimitsV1,
        typing_limits: TypingLimitsV1,
        max_cursor_age: u64,
    ) -> Result<Self, TransientHostBuildRefusalV1> {
        if max_cursor_age == 0
            || max_cursor_age > presence_limits.max_lifetime
            || max_cursor_age > typing_limits.max_lifetime
        {
            return Err(TransientHostBuildRefusalV1::InvalidCursorAge);
        }
        Ok(Self {
            host,
            presence: TransientCollaborationV1::new(presence_limits)
                .map_err(TransientHostBuildRefusalV1::Presence)?,
            typing: TypingCollaborationV1::new(typing_limits)
                .map_err(TransientHostBuildRefusalV1::Typing)?,
            sessions: BTreeMap::new(),
            max_cursor_age,
            last_time: None,
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
            self.retire_source(session);
            self.sessions.remove(&session);
        }
        self.presence
            .reconcile_membership(self.host.service().engine());
        self.typing.reconcile(self.host.service().engine());
        result
    }

    pub fn close(&mut self, session: HostSessionIdV2) -> bool {
        let closed = self.host.close(session);
        if closed {
            self.retire_source(session);
            self.sessions.remove(&session);
        }
        closed
    }

    pub fn publish_presence(
        &mut self,
        session: HostSessionIdV2,
        room_id: &str,
        state: ServicePresenceStateV1,
        lifetime: u64,
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<TransientTimeV1, TransientHostRefusalV1> {
        let participant = self.authorize(session, context, now)?;
        let result = self.presence.publish_presence_for(
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
        let result = self.presence.clear_presence_for(
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
        let result = self.presence.presence_page_for(
            self.host.service().engine(),
            &participant,
            room_id,
            cursor,
            limit,
            TransientTimeV1(now.0),
        );
        self.finish_transient(session, result)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn publish_typing(
        &mut self,
        session: HostSessionIdV2,
        room_id: &str,
        target: TypingTargetV1,
        lifetime: u64,
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<TransientTimeV1, TransientHostRefusalV1> {
        let participant = self.authorize(session, context, now)?;
        let result = self.typing.publish_for(
            self.host.service().engine(),
            room_id,
            &participant,
            target,
            source(session),
            TransientTimeV1(now.0),
            lifetime,
        );
        self.finish_typing(session, result)
    }

    pub fn clear_typing(
        &mut self,
        session: HostSessionIdV2,
        room_id: &str,
        target: TypingTargetV1,
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<bool, TransientHostRefusalV1> {
        let participant = self.authorize(session, context, now)?;
        let result = self.typing.clear_for(
            self.host.service().engine(),
            room_id,
            &participant,
            target,
            source(session),
            TransientTimeV1(now.0),
        );
        self.finish_typing(session, result)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn typing_page(
        &mut self,
        session: HostSessionIdV2,
        room_id: &str,
        target: TypingTargetV1,
        cursor: Option<&TypingCursorV1>,
        limit: usize,
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<TypingPageV1, TransientHostRefusalV1> {
        let participant = self.authorize(session, context, now)?;
        let result = self.typing.page_for(
            self.host.service().engine(),
            &participant,
            room_id,
            target,
            cursor,
            limit,
            TransientTimeV1(now.0),
            TransientTimeV1(now.0),
        );
        self.finish_typing(session, result)
    }

    /// Handles one separately framed volatile request with a session-scoped
    /// exact replay receipt. Durable Service V2 receipts remain untouched.
    pub fn handle_transient(
        &mut self,
        session: HostSessionIdV2,
        frame: &[u8],
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<Vec<u8>, HostRefusal> {
        let participant = match self.authorize(session, context, now) {
            Ok(participant) => participant,
            Err(TransientHostRefusalV1::Host(refusal)) => return Err(refusal),
            Err(TransientHostRefusalV1::Transient(_)) => return Err(HostRefusal::Unavailable),
            Err(TransientHostRefusalV1::Typing(_)) => return Err(HostRefusal::Unavailable),
        };
        let request = decode_transient_request_v1(frame, self.host.wire_limits())
            .map_err(HostRefusal::InvalidFrame)?;

        if let Some(state) = self.sessions.get(&session) {
            if let Some((prior_request, prior_reply)) = &state.last
                && prior_request.request_id == request.request_id
            {
                if prior_request.command == request.command {
                    return self.encode_public_reply(prior_reply);
                }
                return self.encode_public_refusal(ProtocolTransientRefusalV1::RequestConflict);
            }
            if request.request_id.0 != state.next_request || request.request_id.0 == u64::MAX {
                return self
                    .encode_public_refusal(ProtocolTransientRefusalV1::RequestOutOfSequence);
            }
        } else if request.request_id.0 != 0 {
            return self.encode_public_refusal(ProtocolTransientRefusalV1::RequestOutOfSequence);
        }

        let reply = self.execute_public_command(session, &participant, &request.command, now);
        if self.host.has_session(session) {
            let state = self.sessions.entry(session).or_default();
            state.next_request = request.request_id.0 + 1;
            state.last = Some((request, reply.clone()));
        }
        self.encode_public_reply(&reply)
    }

    pub fn expire(&mut self, now: HostTimeV1, budget: usize) -> Result<usize, TransientRefusalV1> {
        self.validate_transient_time(now)?;
        self.presence.expire(TransientTimeV1(now.0), budget)
    }

    pub fn expire_typing(
        &mut self,
        now: HostTimeV1,
        budget: usize,
    ) -> Result<usize, TypingRefusalV1> {
        self.validate_transient_time(now)
            .map_err(|_| TypingRefusalV1::ClockRegression)?;
        self.typing.expire(TransientTimeV1(now.0), budget)
    }

    fn execute_public_command(
        &mut self,
        session: HostSessionIdV2,
        participant: &str,
        command: &TransientCommandV1,
        now: HostTimeV1,
    ) -> TransientReplyV1 {
        match command {
            TransientCommandV1::PublishPresence {
                room_id,
                state,
                lifetime,
            } => {
                let result = self
                    .presence
                    .publish_presence_for(
                        self.host.service().engine(),
                        room_id,
                        participant,
                        source(session),
                        map_presence_state(*state),
                        TransientTimeV1(now.0),
                        *lifetime,
                    )
                    .map(|expires_at| TransientOutcomeV1::PresencePublished {
                        accepted_lifetime: expires_at.0 - now.0,
                    });
                self.public_presence_result(session, result)
            }
            TransientCommandV1::ClearPresence { room_id } => {
                let result = self
                    .presence
                    .clear_presence_for(
                        self.host.service().engine(),
                        room_id,
                        participant,
                        source(session),
                        TransientTimeV1(now.0),
                    )
                    .map(|was_present| TransientOutcomeV1::PresenceCleared { was_present });
                self.public_presence_result(session, result)
            }
            TransientCommandV1::PresencePage {
                room_id,
                cursor,
                limit,
            } => {
                let result = self.execute_public_page(
                    session,
                    participant,
                    room_id,
                    *cursor,
                    *limit,
                    TransientTimeV1(now.0),
                );
                self.public_presence_result(session, result)
            }
            TransientCommandV1::PublishTyping {
                room_id,
                target,
                lifetime,
            } => {
                let result = self
                    .typing
                    .publish_for(
                        self.host.service().engine(),
                        room_id,
                        participant,
                        map_typing_target(*target),
                        source(session),
                        TransientTimeV1(now.0),
                        *lifetime,
                    )
                    .map(|expires_at| TransientOutcomeV1::TypingPublished {
                        accepted_lifetime: expires_at.0 - now.0,
                    });
                self.public_typing_result(session, result)
            }
            TransientCommandV1::ClearTyping { room_id, target } => {
                let result = self
                    .typing
                    .clear_for(
                        self.host.service().engine(),
                        room_id,
                        participant,
                        map_typing_target(*target),
                        source(session),
                        TransientTimeV1(now.0),
                    )
                    .map(|was_present| TransientOutcomeV1::TypingCleared { was_present });
                self.public_typing_result(session, result)
            }
            TransientCommandV1::TypingPage {
                room_id,
                target,
                cursor,
                limit,
            } => {
                let result = self.execute_public_typing_page(
                    session,
                    participant,
                    room_id,
                    *target,
                    *cursor,
                    *limit,
                    TransientTimeV1(now.0),
                );
                self.public_typing_result(session, result)
            }
        }
    }

    fn execute_public_page(
        &mut self,
        session: HostSessionIdV2,
        participant: &str,
        room_id: &str,
        cursor: Option<PresencePageCursorV1>,
        limit: u32,
        now: TransientTimeV1,
    ) -> Result<TransientOutcomeV1, TransientRefusalV1> {
        let (inner, as_of) = if let Some(public) = cursor {
            let Some(CursorV1::Presence(record)) = self
                .sessions
                .get(&session)
                .and_then(|state| state.cursor.as_ref())
                .cloned()
            else {
                return Err(TransientRefusalV1::ForeignCursor);
            };
            if record.public != public || record.room_id != room_id {
                return Err(TransientRefusalV1::ForeignCursor);
            }
            if now > record.expires_at {
                if let Some(state) = self.sessions.get_mut(&session) {
                    state.cursor = None;
                }
                return Err(TransientRefusalV1::StaleCursor);
            }
            if let Some(state) = self.sessions.get_mut(&session) {
                state.cursor = None;
            }
            (Some(record.inner), record.as_of)
        } else {
            if let Some(state) = self.sessions.get_mut(&session) {
                state.cursor = None;
            }
            (None, now)
        };

        let page = self.presence.presence_page_at_for(
            self.host.service().engine(),
            participant,
            room_id,
            inner.as_ref(),
            usize::try_from(limit).map_err(|_| TransientRefusalV1::InvalidPage)?,
            now,
            as_of,
        )?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(page.entries.len())
            .map_err(|_| TransientRefusalV1::Capacity)?;
        for entry in page.entries {
            entries.push(ProtocolPresenceEntryV1 {
                participant_id: entry.participant_id.as_str().to_owned(),
                participant_kind: match entry.participant_kind {
                    TransientParticipantKindV1::Human => ParticipantKindV2::Human,
                    TransientParticipantKindV1::Agent => ParticipantKindV2::Agent,
                },
                state: map_presence_view(entry.state),
                source: entry
                    .source
                    .map(|_| ProtocolSourceClassV1::ParticipantSession),
                remaining_lifetime: entry
                    .expires_at
                    .map(|expires_at| expires_at.0 - page.as_of.0),
            });
        }
        let next = if let Some(inner) = page.next {
            let state = self.sessions.entry(session).or_default();
            let Some(next_cursor) = state.next_cursor.checked_add(1) else {
                return Err(TransientRefusalV1::FidelityLost);
            };
            let expires_at = TransientTimeV1(
                page.as_of
                    .0
                    .checked_add(self.max_cursor_age)
                    .ok_or(TransientRefusalV1::FidelityLost)?,
            );
            let public = PresencePageCursorV1(state.next_cursor);
            state.next_cursor = next_cursor;
            state.cursor = Some(CursorV1::Presence(PresenceCursorRecordV1 {
                public,
                room_id: room_id.to_owned(),
                inner,
                as_of: page.as_of,
                expires_at,
            }));
            Some(public)
        } else {
            None
        };
        Ok(TransientOutcomeV1::PresencePage(ProtocolPresencePageV1 {
            entries,
            next,
        }))
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_public_typing_page(
        &mut self,
        session: HostSessionIdV2,
        participant: &str,
        room_id: &str,
        target: ProtocolTypingTargetV1,
        cursor: Option<TypingPageCursorV1>,
        limit: u32,
        now: TransientTimeV1,
    ) -> Result<TransientOutcomeV1, TypingRefusalV1> {
        let (inner, as_of) = if let Some(public) = cursor {
            let Some(CursorV1::Typing(record)) = self
                .sessions
                .get(&session)
                .and_then(|state| state.cursor.as_ref())
                .cloned()
            else {
                return Err(TypingRefusalV1::ForeignCursor);
            };
            if record.public != public || record.room_id != room_id || record.target != target {
                return Err(TypingRefusalV1::ForeignCursor);
            }
            if now > record.expires_at {
                if let Some(state) = self.sessions.get_mut(&session) {
                    state.cursor = None;
                }
                return Err(TypingRefusalV1::StaleCursor);
            }
            if let Some(state) = self.sessions.get_mut(&session) {
                state.cursor = None;
            }
            (Some(record.inner), record.as_of)
        } else {
            if let Some(state) = self.sessions.get_mut(&session) {
                state.cursor = None;
            }
            (None, now)
        };
        let page = self.typing.page_for(
            self.host.service().engine(),
            participant,
            room_id,
            map_typing_target(target),
            inner.as_ref(),
            usize::try_from(limit).map_err(|_| TypingRefusalV1::InvalidPage)?,
            now,
            as_of,
        )?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(page.entries.len())
            .map_err(|_| TypingRefusalV1::Capacity)?;
        for entry in page.entries {
            entries.push(ProtocolTypingEntryV1 {
                participant_id: entry.participant_id.as_str().to_owned(),
                participant_kind: map_participant_kind(entry.participant_kind),
                state: if entry.active {
                    TypingViewStateV1::Active
                } else {
                    TypingViewStateV1::Inactive
                },
                source: entry
                    .source
                    .map(|_| ProtocolSourceClassV1::ParticipantSession),
                remaining_lifetime: entry
                    .expires_at
                    .map(|expires_at| expires_at.0 - page.as_of.0),
            });
        }
        let next = if let Some(inner) = page.next {
            let state = self.sessions.entry(session).or_default();
            let Some(next_cursor) = state.next_cursor.checked_add(1) else {
                return Err(TypingRefusalV1::FidelityLost);
            };
            let expires_at = TransientTimeV1(
                page.as_of
                    .0
                    .checked_add(self.max_cursor_age)
                    .ok_or(TypingRefusalV1::FidelityLost)?,
            );
            let public = TypingPageCursorV1(state.next_cursor);
            state.next_cursor = next_cursor;
            state.cursor = Some(CursorV1::Typing(TypingCursorRecordV1 {
                public,
                room_id: room_id.to_owned(),
                target,
                inner,
                as_of: page.as_of,
                expires_at,
            }));
            Some(public)
        } else {
            None
        };
        Ok(TransientOutcomeV1::TypingPage(ProtocolTypingPageV1 {
            entries,
            next,
        }))
    }

    fn public_presence_result(
        &mut self,
        session: HostSessionIdV2,
        result: Result<TransientOutcomeV1, TransientRefusalV1>,
    ) -> TransientReplyV1 {
        match result {
            Ok(outcome) => TransientReplyV1::Accepted(outcome),
            Err(refusal) => {
                if refusal == TransientRefusalV1::ClockRegression {
                    let _ = self.host.close(session);
                    self.retire_source(session);
                    self.sessions.remove(&session);
                }
                TransientReplyV1::Refused(map_transient_refusal(refusal))
            }
        }
    }

    fn public_typing_result(
        &mut self,
        session: HostSessionIdV2,
        result: Result<TransientOutcomeV1, TypingRefusalV1>,
    ) -> TransientReplyV1 {
        match result {
            Ok(outcome) => TransientReplyV1::Accepted(outcome),
            Err(refusal) => {
                if refusal == TypingRefusalV1::ClockRegression {
                    let _ = self.host.close(session);
                    self.retire_source(session);
                    self.sessions.remove(&session);
                }
                TransientReplyV1::Refused(map_typing_refusal(refusal))
            }
        }
    }

    fn encode_public_refusal(
        &self,
        refusal: ProtocolTransientRefusalV1,
    ) -> Result<Vec<u8>, HostRefusal> {
        self.encode_public_reply(&TransientReplyV1::Refused(refusal))
    }

    fn encode_public_reply(&self, reply: &TransientReplyV1) -> Result<Vec<u8>, HostRefusal> {
        encode_transient_reply_v1(reply, self.host.wire_limits()).map_err(|_| HostRefusal::Capacity)
    }

    fn authorize(
        &mut self,
        session: HostSessionIdV2,
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<String, TransientHostRefusalV1> {
        match self.host.authorize_transient(session, context, now) {
            Ok(participant) => {
                if let Err(refusal) = self.validate_transient_time(now) {
                    let _ = self.host.close(session);
                    self.retire_source(session);
                    self.sessions.remove(&session);
                    return Err(refusal.into());
                }
                Ok(participant)
            }
            Err(refusal) => {
                if !self.host.has_session(session) {
                    self.retire_source(session);
                    self.sessions.remove(&session);
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
            self.retire_source(session);
            self.sessions.remove(&session);
        }
        result.map_err(Into::into)
    }

    fn finish_typing<T>(
        &mut self,
        session: HostSessionIdV2,
        result: Result<T, TypingRefusalV1>,
    ) -> Result<T, TransientHostRefusalV1> {
        if matches!(result, Err(TypingRefusalV1::ClockRegression)) {
            let _ = self.host.close(session);
            self.retire_source(session);
            self.sessions.remove(&session);
        }
        result.map_err(Into::into)
    }

    fn retire_source(&mut self, session: HostSessionIdV2) {
        self.presence.retire_source(source(session));
        self.typing.retire_source(source(session));
    }

    fn validate_transient_time(&mut self, now: HostTimeV1) -> Result<(), TransientRefusalV1> {
        if self.last_time.is_some_and(|last| now < last) {
            return Err(TransientRefusalV1::ClockRegression);
        }
        self.last_time = Some(now);
        Ok(())
    }
}

fn source(session: HostSessionIdV2) -> PresenceSourceV1 {
    PresenceSourceV1::new(session.source_bytes())
}

fn map_presence_state(state: ProtocolPresenceStateV1) -> ServicePresenceStateV1 {
    match state {
        ProtocolPresenceStateV1::Available => ServicePresenceStateV1::Available,
        ProtocolPresenceStateV1::Away => ServicePresenceStateV1::Away,
        ProtocolPresenceStateV1::Busy => ServicePresenceStateV1::Busy,
    }
}

fn map_presence_view(state: ServicePresenceViewStateV1) -> ProtocolPresenceViewStateV1 {
    match state {
        ServicePresenceViewStateV1::Unknown => ProtocolPresenceViewStateV1::Unknown,
        ServicePresenceViewStateV1::Available => ProtocolPresenceViewStateV1::Available,
        ServicePresenceViewStateV1::Away => ProtocolPresenceViewStateV1::Away,
        ServicePresenceViewStateV1::Busy => ProtocolPresenceViewStateV1::Busy,
    }
}

fn map_participant_kind(kind: TransientParticipantKindV1) -> ParticipantKindV2 {
    match kind {
        TransientParticipantKindV1::Human => ParticipantKindV2::Human,
        TransientParticipantKindV1::Agent => ParticipantKindV2::Agent,
    }
}

fn map_typing_target(target: ProtocolTypingTargetV1) -> TypingTargetV1 {
    match target {
        ProtocolTypingTargetV1::Room => TypingTargetV1::room(),
        ProtocolTypingTargetV1::Thread(root) => TypingTargetV1::thread(root.into_bytes()),
        ProtocolTypingTargetV1::Topic(topic) => TypingTargetV1::topic(topic.into_bytes()),
    }
}

fn map_transient_refusal(refusal: TransientRefusalV1) -> ProtocolTransientRefusalV1 {
    match refusal {
        TransientRefusalV1::InvalidLifetime => ProtocolTransientRefusalV1::InvalidLifetime,
        TransientRefusalV1::NotMember => ProtocolTransientRefusalV1::NotMember,
        TransientRefusalV1::Capacity => ProtocolTransientRefusalV1::Capacity,
        TransientRefusalV1::FidelityLost => ProtocolTransientRefusalV1::FidelityLost,
        TransientRefusalV1::InvalidPage | TransientRefusalV1::InvalidWorkBudget => {
            ProtocolTransientRefusalV1::InvalidPage
        }
        TransientRefusalV1::ForeignCursor => ProtocolTransientRefusalV1::InvalidCursor,
        TransientRefusalV1::StaleCursor | TransientRefusalV1::EngineChanged => {
            ProtocolTransientRefusalV1::StaleCursor
        }
        TransientRefusalV1::ClockRegression | TransientRefusalV1::InvalidLimits => {
            ProtocolTransientRefusalV1::Unavailable
        }
    }
}

fn map_typing_refusal(refusal: TypingRefusalV1) -> ProtocolTransientRefusalV1 {
    match refusal {
        TypingRefusalV1::InvalidLifetime => ProtocolTransientRefusalV1::InvalidLifetime,
        TypingRefusalV1::NotMember => ProtocolTransientRefusalV1::NotMember,
        TypingRefusalV1::InvalidTarget => ProtocolTransientRefusalV1::InvalidTarget,
        TypingRefusalV1::Capacity => ProtocolTransientRefusalV1::Capacity,
        TypingRefusalV1::FidelityLost => ProtocolTransientRefusalV1::FidelityLost,
        TypingRefusalV1::InvalidPage | TypingRefusalV1::InvalidWorkBudget => {
            ProtocolTransientRefusalV1::InvalidPage
        }
        TypingRefusalV1::ForeignCursor => ProtocolTransientRefusalV1::InvalidCursor,
        TypingRefusalV1::StaleCursor | TypingRefusalV1::EngineChanged => {
            ProtocolTransientRefusalV1::StaleCursor
        }
        TypingRefusalV1::ClockRegression | TypingRefusalV1::InvalidLimits => {
            ProtocolTransientRefusalV1::Unavailable
        }
    }
}
