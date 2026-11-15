//! Effect-free participant-bound context and promotion contract for typed
//! interaction adapters.
//!
//! Schema-specific request and response values live in separately owned
//! adapters. This crate contains no plugin payload, callback, credential,
//! harness, model, tool, or task vocabulary.

use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InteractionSourceV1 {
    pub event_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InteractionContextV1 {
    Available { source: InteractionSourceV1 },
    NotVisible,
    Gap { retained_from: u64 },
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InteractionContextRefusalV1 {
    NotAuthorized,
}

pub trait InteractionContextPortV1 {
    fn context(
        &mut self,
        source: InteractionSourceV1,
    ) -> Result<InteractionContextV1, InteractionContextRefusalV1>;
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InteractionInvocationIdV1([u8; 16]);

impl InteractionInvocationIdV1 {
    pub fn new(bytes: [u8; 16]) -> Result<Self, InteractionInputRefusalV1> {
        (bytes != [0; 16])
            .then_some(Self(bytes))
            .ok_or(InteractionInputRefusalV1::ZeroIdentity)
    }

    pub const fn into_bytes(self) -> [u8; 16] {
        self.0
    }
}

impl fmt::Debug for InteractionInvocationIdV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("InteractionInvocationIdV1(<opaque>)")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InteractionInputRefusalV1 {
    ZeroIdentity,
}

#[derive(Clone, Eq, PartialEq)]
pub struct InteractionPromotionIntentV1 {
    source: InteractionSourceV1,
    room_id: String,
    logical_time: u64,
    body: String,
}

impl InteractionPromotionIntentV1 {
    pub fn new(
        source: InteractionSourceV1,
        room_id: impl Into<String>,
        logical_time: u64,
        body: impl Into<String>,
    ) -> Self {
        Self {
            source,
            room_id: room_id.into(),
            logical_time,
            body: body.into(),
        }
    }

    pub const fn source(&self) -> InteractionSourceV1 {
        self.source
    }

    pub const fn logical_time(&self) -> u64 {
        self.logical_time
    }

    pub fn room_id(&self) -> &str {
        &self.room_id
    }

    pub fn body(&self) -> &str {
        &self.body
    }
}

impl fmt::Debug for InteractionPromotionIntentV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InteractionPromotionIntentV1")
            .field("source", &self.source)
            .field("room_id_bytes", &self.room_id.len())
            .field("logical_time", &self.logical_time)
            .field("body_bytes", &self.body.len())
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct InteractionPromotionRequestV1 {
    pub id: InteractionInvocationIdV1,
    pub intent: InteractionPromotionIntentV1,
}

impl fmt::Debug for InteractionPromotionRequestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InteractionPromotionRequestV1")
            .field("id", &self.id)
            .field("intent", &self.intent)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InteractionPromotionAcceptedV1 {
    pub revision: u64,
    pub message_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InteractionPromotionRefusalV1 {
    InvalidInput,
    NotAuthorized,
    SourceUnavailable,
    AdmissionDeferred { retry_after_ms: u64 },
    Capacity,
    Conflict,
    Unavailable,
    Exhausted,
}

pub trait InteractionPromotionPortV1 {
    fn promote(
        &mut self,
        request: InteractionPromotionRequestV1,
    ) -> Result<InteractionPromotionAcceptedV1, InteractionPromotionRefusalV1>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promotion_identity_and_diagnostics_are_content_safe() {
        assert_eq!(
            InteractionInvocationIdV1::new([0; 16]),
            Err(InteractionInputRefusalV1::ZeroIdentity)
        );
        let request = InteractionPromotionRequestV1 {
            id: InteractionInvocationIdV1::new([7; 16]).unwrap(),
            intent: InteractionPromotionIntentV1::new(
                InteractionSourceV1 { event_sequence: 11 },
                "private-room",
                13,
                "private proposed body",
            ),
        };
        let debug = format!("{request:?}");
        assert!(!debug.contains("private proposed body"));
        assert!(!debug.contains("private-room"));
        assert!(debug.contains("body_bytes: 21"));
    }
}
