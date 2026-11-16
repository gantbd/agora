//! Effect-free contract for one authenticated actor's durable publication.
//!
//! The caller supplies a stable operation identity and exact optimistic
//! publication intent. The configured host supplies the authenticated actor
//! and current admission time. Neither identity nor clock authority crosses
//! this boundary.

#![forbid(unsafe_code)]

use std::fmt;

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ActorPublicationIdV1([u8; 16]);

impl ActorPublicationIdV1 {
    pub fn new(bytes: [u8; 16]) -> Result<Self, ActorPublicationInputRefusalV1> {
        if bytes == [0; 16] {
            return Err(ActorPublicationInputRefusalV1::ZeroIdentity);
        }
        Ok(Self(bytes))
    }

    pub const fn into_bytes(self) -> [u8; 16] {
        self.0
    }
}

impl fmt::Debug for ActorPublicationIdV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ActorPublicationIdV1(<opaque>)")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActorPublicationInputRefusalV1 {
    ZeroIdentity,
}

#[derive(Clone, Eq, PartialEq)]
pub struct ActorPublicationIntentV1 {
    room_id: String,
    expected_revision: u64,
    logical_time: u64,
    body: String,
    addressed_to: Vec<String>,
}

impl ActorPublicationIntentV1 {
    pub fn new(
        room_id: impl Into<String>,
        expected_revision: u64,
        logical_time: u64,
        body: impl Into<String>,
        addressed_to: Vec<String>,
    ) -> Self {
        Self {
            room_id: room_id.into(),
            expected_revision,
            logical_time,
            body: body.into(),
            addressed_to,
        }
    }

    pub fn room_id(&self) -> &str {
        &self.room_id
    }
    pub const fn expected_revision(&self) -> u64 {
        self.expected_revision
    }
    pub const fn logical_time(&self) -> u64 {
        self.logical_time
    }
    pub fn body(&self) -> &str {
        &self.body
    }
    pub fn addressed_to(&self) -> &[String] {
        &self.addressed_to
    }
}

impl fmt::Debug for ActorPublicationIntentV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActorPublicationIntentV1")
            .field("room_id_bytes", &self.room_id.len())
            .field("expected_revision", &self.expected_revision)
            .field("logical_time", &self.logical_time)
            .field("body_bytes", &self.body.len())
            .field("addressed_count", &self.addressed_to.len())
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct ActorPublicationRequestV1 {
    pub id: ActorPublicationIdV1,
    pub intent: ActorPublicationIntentV1,
}

impl fmt::Debug for ActorPublicationRequestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActorPublicationRequestV1")
            .field("id", &self.id)
            .field("intent", &self.intent)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActorPublicationAcceptedV1 {
    pub revision: u64,
    pub message_id: [u8; 16],
    pub message_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActorPublicationRefusalV1 {
    InvalidInput,
    NotAuthorized,
    AdmissionDeferred { retry_after_ms: u64 },
    Capacity,
    Conflict,
    Unavailable,
    Exhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActorPublicationObservationV1 {
    Accepted(ActorPublicationAcceptedV1),
    Absent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActorPublicationObservationRefusalV1 {
    InvalidInput,
    NotAuthorized,
    Conflict,
    Unavailable,
}

pub trait ActorPublicationPortV1 {
    fn publish_actor(
        &mut self,
        request: ActorPublicationRequestV1,
    ) -> Result<ActorPublicationAcceptedV1, ActorPublicationRefusalV1>;
}

pub trait ActorPublicationObservationPortV1 {
    fn observe_actor(
        &mut self,
        request: ActorPublicationRequestV1,
    ) -> Result<ActorPublicationObservationV1, ActorPublicationObservationRefusalV1>;
}

/// Transport-bearing client seam. The outer error preserves delivery-phase
/// truth; the inner result is Agora's semantic reply.
pub trait ActorPublicationWireClientV1 {
    type Error;

    fn publish_actor_wire(
        &mut self,
        request: ActorPublicationRequestV1,
    ) -> Result<Result<ActorPublicationAcceptedV1, ActorPublicationRefusalV1>, Self::Error>;

    fn observe_actor_wire(
        &mut self,
        request: ActorPublicationRequestV1,
    ) -> Result<
        Result<ActorPublicationObservationV1, ActorPublicationObservationRefusalV1>,
        Self::Error,
    >;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_redact_all_content_and_identity() {
        let request = ActorPublicationRequestV1 {
            id: ActorPublicationIdV1::new([7; 16]).unwrap(),
            intent: ActorPublicationIntentV1::new(
                "secret-room",
                11,
                12,
                "secret-body",
                vec!["secret-recipient".into()],
            ),
        };
        let debug = format!("{request:?}");
        for secret in ["secret-room", "secret-body", "secret-recipient"] {
            assert!(!debug.contains(secret));
        }
        assert!(debug.contains("addressed_count"));
    }
}
