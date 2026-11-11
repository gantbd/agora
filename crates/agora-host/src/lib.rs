//! Authenticated, transport-neutral composition boundary for Agora.
//!
//! The host trusts its injected [`PrincipalVerifier`]. It does not inspect
//! credentials, define authority scopes, or perform network I/O.

use agora_protocol::{ClientIdV1, RefusalV1};
use agora_service::Service;
use agora_wire::{
    WireDecodeError, WireEncodeError, WireLimits, WireReplyV1, WireRequestV1, decode_request_v1,
    encode_reply_v1,
};
use std::collections::{BTreeMap, VecDeque};
use std::fmt;

pub mod search;
pub mod transient;
pub mod v2;

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct HostSessionIdV1([u8; 16]);

impl fmt::Debug for HostSessionIdV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HostSessionIdV1(<opaque>)")
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct VerifiedSubjectV1([u8; 16]);

impl VerifiedSubjectV1 {
    /// Constructs evidence only for implementations of the trusted verifier
    /// port. This value is attribution, not a transferable credential.
    pub const fn from_verifier(value: [u8; 16]) -> Self {
        Self(value)
    }
}

impl fmt::Debug for VerifiedSubjectV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("VerifiedSubjectV1(<opaque>)")
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct HostTimeV1(pub u64);

/// Trusted adapter port. Implementations may consume credential-system or Aegis
/// evidence, but those vocabularies do not cross into Agora.
pub trait PrincipalVerifier {
    type Context: ?Sized;
    type Error;

    fn authenticate(
        &mut self,
        claimed_participant: &str,
        context: &Self::Context,
        now: HostTimeV1,
    ) -> Result<VerifiedSubjectV1, Self::Error>;

    fn revalidate(
        &mut self,
        subject: VerifiedSubjectV1,
        participant: &str,
        context: &Self::Context,
        now: HostTimeV1,
    ) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostLimits {
    pub max_sessions: usize,
    pub max_retired_sessions: usize,
    pub wire: WireLimits,
}

impl Default for HostLimits {
    fn default() -> Self {
        Self {
            max_sessions: 64,
            max_retired_sessions: 256,
            wire: WireLimits::reference(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HostUsage {
    pub open_offers: u64,
    pub authenticated: u64,
    pub authentication_refused: u64,
    pub frame_offers: u64,
    pub executed: u64,
    pub disconnected: u64,
    pub unavailable: u64,
    pub overflowed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostRefusal {
    InvalidFrame(WireDecodeError),
    ProtocolViolation,
    Unavailable,
    Capacity,
}

pub struct OpenReply {
    session: Option<HostSessionIdV1>,
    frame: Vec<u8>,
}

impl OpenReply {
    pub fn session(&self) -> Option<HostSessionIdV1> {
        self.session
    }

    pub fn frame(&self) -> &[u8] {
        &self.frame
    }
}

impl fmt::Debug for OpenReply {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenReply")
            .field("session", &self.session)
            .field("frame_bytes", &self.frame.len())
            .finish()
    }
}

#[derive(Clone)]
struct Session {
    subject: VerifiedSubjectV1,
    participant: String,
    client: ClientIdV1,
    last_verified_at: HostTimeV1,
}

pub struct Host<V> {
    service: Service,
    verifier: V,
    limits: HostLimits,
    sessions: BTreeMap<HostSessionIdV1, Session>,
    retired: VecDeque<HostSessionIdV1>,
    next_session: u64,
    usage: HostUsage,
}

impl<V> fmt::Debug for Host<V> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Host")
            .field("limits", &self.limits)
            .field("active_sessions", &self.sessions.len())
            .field("retired_sessions", &self.retired.len())
            .field("usage", &self.usage)
            .finish_non_exhaustive()
    }
}

impl<V: PrincipalVerifier> Host<V> {
    pub fn new(service: Service, verifier: V, limits: HostLimits) -> Self {
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

    pub fn service(&self) -> &Service {
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
    ) -> Result<OpenReply, HostRefusal> {
        count(&mut self.usage.open_offers, &mut self.usage.overflowed);
        let request =
            decode_request_v1(frame, self.limits.wire).map_err(HostRefusal::InvalidFrame)?;
        let WireRequestV1::Connect { participant_id } = request else {
            return Err(HostRefusal::ProtocolViolation);
        };
        if self.sessions.len() >= self.limits.max_sessions {
            return self.open_refused(RefusalV1::ClientLimit);
        }
        let Some(next_session) = self.next_session.checked_add(1) else {
            return self.open_refused(RefusalV1::Exhausted);
        };
        let subject = match self.verifier.authenticate(&participant_id, context, now) {
            Ok(subject) => subject,
            Err(_) => {
                count(
                    &mut self.usage.authentication_refused,
                    &mut self.usage.overflowed,
                );
                return self.open_refused(RefusalV1::Unavailable);
            }
        };
        count(&mut self.usage.authenticated, &mut self.usage.overflowed);
        let client = match self.service.connect(&participant_id) {
            Ok(client) => client,
            Err(refusal) => return self.open_refused(refusal),
        };
        let reply = match encode_reply_v1(&WireReplyV1::Connected(client), self.limits.wire) {
            Ok(reply) => reply,
            Err(error) => {
                let _ = self.service.disconnect(client);
                return Err(map_encode(error));
            }
        };
        let session = HostSessionIdV1(identity_bytes(b's', self.next_session));
        self.next_session = next_session;
        self.sessions.insert(
            session,
            Session {
                subject,
                participant: participant_id,
                client,
                last_verified_at: now,
            },
        );
        Ok(OpenReply {
            session: Some(session),
            frame: reply,
        })
    }

    pub fn handle(
        &mut self,
        session_id: HostSessionIdV1,
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
            decode_request_v1(frame, self.limits.wire).map_err(HostRefusal::InvalidFrame)?;
        match request {
            WireRequestV1::Execute { client, request } => {
                if client != session.client {
                    self.retire(session_id);
                    return self.unavailable();
                }
                let reply = self.service.execute(client, request);
                let frame = encode_reply_v1(&WireReplyV1::Executed(reply), self.limits.wire)
                    .map_err(map_encode)?;
                count(&mut self.usage.executed, &mut self.usage.overflowed);
                Ok(frame)
            }
            WireRequestV1::Disconnect(client) => {
                if client != session.client {
                    self.retire(session_id);
                    return self.unavailable();
                }
                self.retire(session_id);
                let frame = encode_reply_v1(&WireReplyV1::Disconnected, self.limits.wire)
                    .map_err(map_encode)?;
                count(&mut self.usage.disconnected, &mut self.usage.overflowed);
                Ok(frame)
            }
            WireRequestV1::Connect { .. } => Err(HostRefusal::ProtocolViolation),
        }
    }

    /// Releases a transport-bound session without requiring authentication.
    /// This is the cleanup path for transport loss and must remain available.
    pub fn close(&mut self, session: HostSessionIdV1) -> bool {
        if !self.sessions.contains_key(&session) {
            return false;
        }
        self.retire(session);
        count(&mut self.usage.disconnected, &mut self.usage.overflowed);
        true
    }

    fn open_refused(&self, refusal: RefusalV1) -> Result<OpenReply, HostRefusal> {
        let frame = encode_reply_v1(&WireReplyV1::ConnectRefused(refusal), self.limits.wire)
            .map_err(map_encode)?;
        Ok(OpenReply {
            session: None,
            frame,
        })
    }

    fn unavailable<T>(&mut self) -> Result<T, HostRefusal> {
        count(&mut self.usage.unavailable, &mut self.usage.overflowed);
        Err(HostRefusal::Unavailable)
    }

    fn retire(&mut self, session_id: HostSessionIdV1) {
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

fn count(value: &mut u64, overflowed: &mut bool) {
    let old = *value;
    *value = value.saturating_add(1);
    *overflowed |= old == u64::MAX;
}

fn identity_bytes(tag: u8, value: u64) -> [u8; 16] {
    let mut bytes = [0; 16];
    bytes[0] = tag;
    bytes[8..].copy_from_slice(&value.to_be_bytes());
    bytes
}

fn map_encode(error: WireEncodeError) -> HostRefusal {
    match error {
        WireEncodeError::Limit
        | WireEncodeError::Capacity
        | WireEncodeError::LengthOverflow
        | WireEncodeError::Invalid => HostRefusal::Capacity,
    }
}
