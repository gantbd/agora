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

/// Host-issued proof that one exact retained publication belongs to the
/// authenticated actor observing it. The actor is derived from the Host
/// session; it is never accepted as publication request input.
#[derive(Clone, Eq, PartialEq)]
pub struct ActorPublicationProvenanceV1 {
    pub accepted: ActorPublicationAcceptedV1,
    pub actor: String,
}

impl fmt::Debug for ActorPublicationProvenanceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActorPublicationProvenanceV1")
            .field("accepted", &self.accepted)
            .field("actor_bytes", &self.actor.len())
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActorPublicationProvenanceObservationV1 {
    Accepted(ActorPublicationProvenanceV1),
    Absent,
}

/// Additive Host-issued proof retaining the verified session subject beside
/// the actor and exact publication. The subject is attribution, never a
/// credential or authority token, and is not accepted as request input.
#[derive(Clone, Eq, PartialEq)]
pub struct ActorPublicationSubjectProvenanceV1 {
    pub accepted: ActorPublicationAcceptedV1,
    pub actor: String,
    pub subject: [u8; 16],
}

impl fmt::Debug for ActorPublicationSubjectProvenanceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActorPublicationSubjectProvenanceV1")
            .field("accepted", &self.accepted)
            .field("actor_bytes", &self.actor.len())
            .field("subject", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActorPublicationSubjectProvenanceObservationV1 {
    Accepted(ActorPublicationSubjectProvenanceV1),
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

pub trait ActorPublicationProvenanceObservationPortV1 {
    fn observe_actor_provenance(
        &mut self,
        request: ActorPublicationRequestV1,
    ) -> Result<ActorPublicationProvenanceObservationV1, ActorPublicationObservationRefusalV1>;
}

pub trait ActorPublicationSubjectProvenanceObservationPortV1 {
    fn observe_actor_subject_provenance(
        &mut self,
        request: ActorPublicationRequestV1,
    ) -> Result<ActorPublicationSubjectProvenanceObservationV1, ActorPublicationObservationRefusalV1>;
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

/// Separate additive client seam so existing publication clients do not gain a
/// new mandatory method merely because an integration needs actor provenance.
pub trait ActorPublicationProvenanceWireClientV1 {
    type Error;

    fn observe_actor_provenance_wire(
        &mut self,
        request: ActorPublicationRequestV1,
    ) -> Result<
        Result<ActorPublicationProvenanceObservationV1, ActorPublicationObservationRefusalV1>,
        Self::Error,
    >;
}

/// Separate additive seam preserving the actor-only V1 provenance contract.
pub trait ActorPublicationSubjectProvenanceWireClientV1 {
    type Error;

    fn observe_actor_subject_provenance_wire(
        &mut self,
        request: ActorPublicationRequestV1,
    ) -> Result<
        Result<
            ActorPublicationSubjectProvenanceObservationV1,
            ActorPublicationObservationRefusalV1,
        >,
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

        let provenance = ActorPublicationProvenanceV1 {
            accepted: ActorPublicationAcceptedV1 {
                revision: 13,
                message_id: [8; 16],
                message_sequence: 3,
            },
            actor: "secret-actor".into(),
        };
        let debug = format!("{provenance:?}");
        assert!(!debug.contains("secret-actor"));
        assert!(debug.contains("actor_bytes"));

        let provenance = ActorPublicationSubjectProvenanceV1 {
            accepted: provenance.accepted,
            actor: "secret-actor".into(),
            subject: [9; 16],
        };
        let debug = format!("{provenance:?}");
        assert!(!debug.contains("secret-actor"));
        assert!(!debug.contains("9, 9"));
    }
}
