//! Effect-free contract for current-visibility notification projection.
//!
//! Implementations capture an authenticated observer. A source sequence is an
//! observation, never authority. The initial wake projection contains no room,
//! participant, message, attachment, artifact, attention, or content value.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeliverySourceV1 {
    pub event_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WakeClassV1 {
    ConversationActivity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WakeProjectionV1 {
    pub source: DeliverySourceV1,
    pub class: WakeClassV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryProjectionV1 {
    Visible(WakeProjectionV1),
    NotVisible,
    Gap { retained_from: u64 },
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryProjectionRefusalV1 {
    NotAuthorized,
    /// The configured projection owner could not return a trustworthy result.
    /// This includes transport and protocol failure at a public-client adapter;
    /// it never means that the source was not visible.
    Unavailable,
}

/// Configured observer-bound projection. Calling this port never acknowledges
/// an Agora event and never mutates a read position or conversation fact.
pub trait DeliveryProjectionPortV1 {
    fn project(
        &mut self,
        source: DeliverySourceV1,
    ) -> Result<DeliveryProjectionV1, DeliveryProjectionRefusalV1>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wake_projection_has_only_sequence_and_closed_class() {
        let wake = WakeProjectionV1 {
            source: DeliverySourceV1 { event_sequence: 7 },
            class: WakeClassV1::ConversationActivity,
        };
        assert_eq!(
            format!("{wake:?}"),
            "WakeProjectionV1 { source: DeliverySourceV1 { event_sequence: 7 }, class: ConversationActivity }"
        );
    }
}
