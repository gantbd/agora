//! Effect-free contract for trusted, actor-bound federated publication.
//!
//! This contract carries a local publication intent and opaque correlation
//! only. It conveys no peer trust, identity mapping, authentication, clock,
//! transport, retry, or room-home authority. A trusted Host implementation
//! binds the current authenticated actor and admission time at execution.

use std::fmt;

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FederatedPublicationIdV1([u8; 16]);

impl FederatedPublicationIdV1 {
    pub fn new(bytes: [u8; 16]) -> Result<Self, FederatedPublicationInputRefusalV1> {
        if bytes == [0; 16] {
            return Err(FederatedPublicationInputRefusalV1::ZeroIdentity);
        }
        Ok(Self(bytes))
    }

    pub const fn into_bytes(self) -> [u8; 16] {
        self.0
    }
}

impl fmt::Debug for FederatedPublicationIdV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("FederatedPublicationIdV1(<opaque>)")
    }
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FederatedPublicationScopeV1([u8; 64]);

impl FederatedPublicationScopeV1 {
    pub const fn new(bytes: [u8; 64]) -> Self {
        Self(bytes)
    }

    pub const fn into_bytes(self) -> [u8; 64] {
        self.0
    }
}

impl fmt::Debug for FederatedPublicationScopeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("FederatedPublicationScopeV1(<opaque>)")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FederatedPublicationInputRefusalV1 {
    ZeroIdentity,
}

#[derive(Clone, Eq, PartialEq)]
pub struct FederatedPublicationIntentV1 {
    room_id: String,
    body: String,
}

impl FederatedPublicationIntentV1 {
    pub fn new(room_id: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            room_id: room_id.into(),
            body: body.into(),
        }
    }

    pub fn room_id(&self) -> &str {
        &self.room_id
    }

    pub fn body(&self) -> &str {
        &self.body
    }
}

impl fmt::Debug for FederatedPublicationIntentV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FederatedPublicationIntentV1")
            .field("room_id_bytes", &self.room_id.len())
            .field("body_bytes", &self.body.len())
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct FederatedPublicationRequestV1 {
    pub id: FederatedPublicationIdV1,
    pub scope: FederatedPublicationScopeV1,
    pub intent: FederatedPublicationIntentV1,
}

impl fmt::Debug for FederatedPublicationRequestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FederatedPublicationRequestV1")
            .field("id", &self.id)
            .field("scope", &self.scope)
            .field("intent", &self.intent)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FederatedPublicationAcceptedV1 {
    pub revision: u64,
    /// Agora's authoritative outbox position for federation replication.
    pub event_sequence: u64,
    pub message_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FederatedPublicationRefusalV1 {
    InvalidInput,
    NotAuthorized,
    AdmissionDeferred { retry_after_ms: u64 },
    Capacity,
    Conflict,
    Unavailable,
    Exhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FederatedPublicationOutcomeV1 {
    /// Agora returned the exact request's stable semantic result.
    Reply(Result<FederatedPublicationAcceptedV1, FederatedPublicationRefusalV1>),
    /// The configured endpoint establishes that execution did not occur.
    DefinitelyNotExecuted,
    /// The endpoint cannot establish whether the exact request executed.
    OutcomeUnknown,
}

/// A configured trusted endpoint. Implementations capture identity mapping,
/// authentication, and clock context; callers cannot assert those facts.
pub trait FederatedPublicationPortV1 {
    fn publish_federated(
        &mut self,
        request: FederatedPublicationRequestV1,
    ) -> FederatedPublicationOutcomeV1;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_nonzero_and_diagnostics_are_content_free() {
        assert_eq!(
            FederatedPublicationIdV1::new([0; 16]),
            Err(FederatedPublicationInputRefusalV1::ZeroIdentity)
        );
        let request = FederatedPublicationRequestV1 {
            id: FederatedPublicationIdV1::new([7; 16]).unwrap(),
            scope: FederatedPublicationScopeV1::new([8; 64]),
            intent: FederatedPublicationIntentV1::new("secret-room", "secret-body"),
        };
        let debug = format!("{request:?}");
        assert!(!debug.contains("secret-room"));
        assert!(!debug.contains("secret-body"));
        assert!(debug.contains("room_id_bytes"));
        assert!(debug.contains("body_bytes"));
    }
}
