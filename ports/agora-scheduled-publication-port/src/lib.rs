//! Effect-free contract for trusted, actor-bound scheduled publication.
//!
//! This contract carries publication intent and correlation only. It conveys
//! no identity, authentication, clock, scheduling, or retry authority. A
//! trusted Host implementation must bind the current authenticated actor and
//! current admission time when the offer is actually dispatched.

use std::fmt;

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ScheduledPublicationIdV1([u8; 16]);

impl ScheduledPublicationIdV1 {
    pub fn new(bytes: [u8; 16]) -> Result<Self, ScheduledPublicationInputRefusalV1> {
        if bytes == [0; 16] {
            return Err(ScheduledPublicationInputRefusalV1::ZeroIdentity);
        }
        Ok(Self(bytes))
    }

    pub const fn into_bytes(self) -> [u8; 16] {
        self.0
    }
}

impl fmt::Debug for ScheduledPublicationIdV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ScheduledPublicationIdV1(<opaque>)")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScheduledPublicationInputRefusalV1 {
    ZeroIdentity,
}

#[derive(Clone, Eq, PartialEq)]
pub struct ScheduledPublicationIntentV1 {
    room_id: String,
    logical_time: u64,
    body: String,
}

impl ScheduledPublicationIntentV1 {
    pub fn new(room_id: impl Into<String>, logical_time: u64, body: impl Into<String>) -> Self {
        Self {
            room_id: room_id.into(),
            logical_time,
            body: body.into(),
        }
    }

    pub fn room_id(&self) -> &str {
        &self.room_id
    }

    pub const fn logical_time(&self) -> u64 {
        self.logical_time
    }

    pub fn body(&self) -> &str {
        &self.body
    }
}

impl fmt::Debug for ScheduledPublicationIntentV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScheduledPublicationIntentV1")
            .field("room_id_bytes", &self.room_id.len())
            .field("logical_time", &self.logical_time)
            .field("body_bytes", &self.body.len())
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct ScheduledPublicationRequestV1 {
    pub id: ScheduledPublicationIdV1,
    pub intent: ScheduledPublicationIntentV1,
}

impl fmt::Debug for ScheduledPublicationRequestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScheduledPublicationRequestV1")
            .field("id", &self.id)
            .field("intent", &self.intent)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScheduledPublicationAcceptedV1 {
    pub revision: u64,
    pub message_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScheduledPublicationRefusalV1 {
    InvalidInput,
    NotAuthorized,
    AdmissionDeferred { retry_after_ms: u64 },
    Capacity,
    Conflict,
    Unavailable,
    Exhausted,
}

/// Agora-owned retained evidence for one exact actor-bound publication offer.
///
/// `Absent` is authoritative only because the owning Agora service durably
/// retains every accepted scheduled-publication identity and backpressures
/// before mutation when that bounded index is full. It is not a generic
/// cache-miss classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScheduledPublicationObservationV1 {
    Accepted(ScheduledPublicationAcceptedV1),
    Absent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScheduledPublicationObservationRefusalV1 {
    InvalidInput,
    NotAuthorized,
    Conflict,
    Unavailable,
}

/// A configured trusted endpoint. Implementations capture authentication and
/// clock context; neither can be asserted by the scheduling caller.
pub trait ScheduledPublicationPortV1 {
    fn publish_scheduled(
        &mut self,
        request: ScheduledPublicationRequestV1,
    ) -> Result<ScheduledPublicationAcceptedV1, ScheduledPublicationRefusalV1>;
}

/// Read-only companion to [`ScheduledPublicationPortV1`]. Implementations
/// capture the authenticated actor; callers cannot observe another actor's
/// publication identity by supplying an author.
pub trait ScheduledPublicationObservationPortV1 {
    fn observe_scheduled(
        &mut self,
        request: ScheduledPublicationRequestV1,
    ) -> Result<ScheduledPublicationObservationV1, ScheduledPublicationObservationRefusalV1>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_nonzero_and_diagnostics_are_content_free() {
        assert_eq!(
            ScheduledPublicationIdV1::new([0; 16]),
            Err(ScheduledPublicationInputRefusalV1::ZeroIdentity)
        );
        let request = ScheduledPublicationRequestV1 {
            id: ScheduledPublicationIdV1::new([7; 16]).unwrap(),
            intent: ScheduledPublicationIntentV1::new("secret-room", 11, "secret-body"),
        };
        let debug = format!("{request:?}");
        assert!(!debug.contains("secret-room"));
        assert!(!debug.contains("secret-body"));
        assert!(debug.contains("room_id_bytes"));
        assert!(debug.contains("body_bytes"));
    }
}
