//! Authenticated transport-neutral composition for Agora Wire/Service V2.

use super::{
    HostLimits, HostRefusal, HostTimeV1, HostUsage, PrincipalVerifier, VerifiedSubjectV1, count,
    identity_bytes, map_encode,
};
use agora_protocol::v2::{ClientIdV2, RefusalV2};
use agora_service::v2::ServiceV2;
use agora_wire::v2::{WireReplyV2, WireRequestV2, decode_request_v2, encode_reply_v2};
use std::collections::{BTreeMap, VecDeque};
use std::fmt;

pub mod persistence;

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct HostSessionIdV2([u8; 16]);

impl fmt::Debug for HostSessionIdV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HostSessionIdV2(<opaque>)")
    }
}

pub struct OpenReplyV2 {
    session: Option<HostSessionIdV2>,
    frame: Vec<u8>,
}

impl OpenReplyV2 {
    pub fn session(&self) -> Option<HostSessionIdV2> {
        self.session
    }

    pub fn frame(&self) -> &[u8] {
        &self.frame
    }
}

impl fmt::Debug for OpenReplyV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenReplyV2")
            .field("session", &self.session)
            .field("frame_bytes", &self.frame.len())
            .finish()
    }
}

#[derive(Clone)]
struct SessionV2 {
    subject: VerifiedSubjectV1,
    participant: String,
    client: ClientIdV2,
    last_verified_at: HostTimeV1,
}

pub struct HostV2<V> {
    service: ServiceV2,
    verifier: V,
    limits: HostLimits,
    sessions: BTreeMap<HostSessionIdV2, SessionV2>,
    retired: VecDeque<HostSessionIdV2>,
    next_session: u64,
    usage: HostUsage,
}

impl<V> fmt::Debug for HostV2<V> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HostV2")
            .field("limits", &self.limits)
            .field("active_sessions", &self.sessions.len())
            .field("retired_sessions", &self.retired.len())
            .field("usage", &self.usage)
            .finish_non_exhaustive()
    }
}

impl<V: PrincipalVerifier> HostV2<V> {
    pub fn new(service: ServiceV2, verifier: V, limits: HostLimits) -> Self {
        Self {
            service,
            verifier,
            limits,
            sessions: BTreeMap::new(),
            retired: VecDeque::new(),
            next_session: 0,
            usage: HostUsage::default(),
        }
    }

    pub fn service(&self) -> &ServiceV2 {
        &self.service
    }

    pub fn usage(&self) -> HostUsage {
        self.usage
    }

    pub fn open(
        &mut self,
        frame: &[u8],
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<OpenReplyV2, HostRefusal> {
        count(&mut self.usage.open_offers, &mut self.usage.overflowed);
        let request =
            decode_request_v2(frame, self.limits.wire).map_err(HostRefusal::InvalidFrame)?;
        let WireRequestV2::Connect { participant_id } = request else {
            return Err(HostRefusal::ProtocolViolation);
        };
        if self.sessions.len() >= self.limits.max_sessions {
            return self.open_refused(RefusalV2::ClientLimit);
        }
        let Some(next_session) = self.next_session.checked_add(1) else {
            return self.open_refused(RefusalV2::Exhausted);
        };
        let subject = match self.verifier.authenticate(&participant_id, context, now) {
            Ok(subject) => subject,
            Err(_) => {
                count(
                    &mut self.usage.authentication_refused,
                    &mut self.usage.overflowed,
                );
                return self.open_refused(RefusalV2::Unavailable);
            }
        };
        count(&mut self.usage.authenticated, &mut self.usage.overflowed);
        let client = match self.service.connect(&participant_id) {
            Ok(client) => client,
            Err(refusal) => return self.open_refused(refusal),
        };
        let reply = match encode_reply_v2(&WireReplyV2::Connected(client), self.limits.wire) {
            Ok(reply) => reply,
            Err(error) => {
                let _ = self.service.disconnect(client);
                return Err(map_encode(error));
            }
        };
        let session = HostSessionIdV2(identity_bytes(b'2', self.next_session));
        self.next_session = next_session;
        self.sessions.insert(
            session,
            SessionV2 {
                subject,
                participant: participant_id,
                client,
                last_verified_at: now,
            },
        );
        Ok(OpenReplyV2 {
            session: Some(session),
            frame: reply,
        })
    }

    pub fn handle(
        &mut self,
        session_id: HostSessionIdV2,
        frame: &[u8],
        context: &V::Context,
        now: HostTimeV1,
    ) -> Result<Vec<u8>, HostRefusal> {
        count(&mut self.usage.frame_offers, &mut self.usage.overflowed);
        let Some(session) = self.sessions.get(&session_id).cloned() else {
            return self.unavailable();
        };
        if now < session.last_verified_at {
            self.retire(session_id);
            return self.unavailable();
        }
        if self
            .verifier
            .revalidate(session.subject, &session.participant, context, now)
            .is_err()
        {
            self.retire(session_id);
            return self.unavailable();
        }
        if let Some(active) = self.sessions.get_mut(&session_id) {
            active.last_verified_at = now;
        }
        let request =
            decode_request_v2(frame, self.limits.wire).map_err(HostRefusal::InvalidFrame)?;
        match request {
            WireRequestV2::Execute { client, request } => {
                if client != session.client {
                    self.retire(session_id);
                    return self.unavailable();
                }
                let reply = self.service.execute(client, request);
                let frame = encode_reply_v2(&WireReplyV2::Executed(reply), self.limits.wire)
                    .map_err(map_encode)?;
                count(&mut self.usage.executed, &mut self.usage.overflowed);
                Ok(frame)
            }
            WireRequestV2::Disconnect(client) => {
                if client != session.client {
                    self.retire(session_id);
                    return self.unavailable();
                }
                self.retire(session_id);
                let frame = encode_reply_v2(&WireReplyV2::Disconnected, self.limits.wire)
                    .map_err(map_encode)?;
                count(&mut self.usage.disconnected, &mut self.usage.overflowed);
                Ok(frame)
            }
            WireRequestV2::Connect { .. } => Err(HostRefusal::ProtocolViolation),
        }
    }

    pub fn close(&mut self, session: HostSessionIdV2) -> bool {
        if !self.sessions.contains_key(&session) {
            return false;
        }
        self.retire(session);
        count(&mut self.usage.disconnected, &mut self.usage.overflowed);
        true
    }

    fn open_refused(&self, refusal: RefusalV2) -> Result<OpenReplyV2, HostRefusal> {
        let frame = encode_reply_v2(&WireReplyV2::ConnectRefused(refusal), self.limits.wire)
            .map_err(map_encode)?;
        Ok(OpenReplyV2 {
            session: None,
            frame,
        })
    }

    fn unavailable<T>(&mut self) -> Result<T, HostRefusal> {
        count(&mut self.usage.unavailable, &mut self.usage.overflowed);
        Err(HostRefusal::Unavailable)
    }

    fn retire(&mut self, session_id: HostSessionIdV2) {
        let Some(session) = self.sessions.remove(&session_id) else {
            return;
        };
        let _ = self.service.disconnect(session.client);
        if self.limits.max_retired_sessions > 0 {
            if self.retired.len() >= self.limits.max_retired_sessions {
                self.retired.pop_front();
            }
            self.retired.push_back(session_id);
        }
    }
}
