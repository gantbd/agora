//! Transport-neutral client contract for the trusted scheduled-publication
//! composition profile.

use agora_scheduled_publication_port::{
    ScheduledPublicationAcceptedV1, ScheduledPublicationObservationRefusalV1,
    ScheduledPublicationObservationV1, ScheduledPublicationRefusalV1,
    ScheduledPublicationRequestV1,
};

pub trait ScheduledPublicationWireClientV1 {
    type Error;

    fn publish_scheduled(
        &mut self,
        request: ScheduledPublicationRequestV1,
    ) -> Result<Result<ScheduledPublicationAcceptedV1, ScheduledPublicationRefusalV1>, Self::Error>;

    fn observe_scheduled(
        &mut self,
        request: ScheduledPublicationRequestV1,
    ) -> Result<
        Result<ScheduledPublicationObservationV1, ScheduledPublicationObservationRefusalV1>,
        Self::Error,
    >;
}

/// Transport certainty needed by mutation coordinators. `false` means only
/// that the outcome is not provably unsent; it never authorizes a retry.
pub trait ScheduledTransportErrorV1 {
    fn definitely_not_sent(&self) -> bool;
}
