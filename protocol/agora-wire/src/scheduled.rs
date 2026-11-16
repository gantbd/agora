//! Canonical bounded wire profile for trusted scheduled publication.
//!
//! This profile is routed only by an explicitly selected composition
//! transport. It is not part of ordinary participant Wire V2.

use super::{
    WireDecodeError, WireEncodeError, WireLimits, decoder, encode, finish_decode, read_text,
    read_u8, read_u64,
};
use agora_delivery_projection_port::{
    DeliveryProjectionRefusalV1, DeliveryProjectionV1, DeliverySourceV1, WakeClassV1,
    WakeProjectionV1,
};
use agora_scheduled_publication_port::{
    ScheduledPublicationAcceptedV1, ScheduledPublicationIdV1, ScheduledPublicationIntentV1,
    ScheduledPublicationObservationRefusalV1, ScheduledPublicationObservationV1,
    ScheduledPublicationRefusalV1, ScheduledPublicationRequestV1,
};

const VERSION_V1: u8 = 1;
const PUBLISH: u8 = 1;
const OBSERVE: u8 = 2;
const PROJECT_DELIVERY: u8 = 3;
const PUBLISHED: u8 = 0x81;
const OBSERVED: u8 = 0x82;
const DELIVERY_PROJECTED: u8 = 0x83;

#[derive(Clone, Eq, PartialEq)]
pub enum ScheduledWireRequestV1 {
    Publish(ScheduledPublicationRequestV1),
    Observe(ScheduledPublicationRequestV1),
    ProjectDelivery(DeliverySourceV1),
}

impl std::fmt::Debug for ScheduledWireRequestV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Publish(request) => formatter.debug_tuple("Publish").field(request).finish(),
            Self::Observe(request) => formatter.debug_tuple("Observe").field(request).finish(),
            Self::ProjectDelivery(source) => formatter
                .debug_tuple("ProjectDelivery")
                .field(source)
                .finish(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScheduledWireReplyV1 {
    Published(Result<ScheduledPublicationAcceptedV1, ScheduledPublicationRefusalV1>),
    Observed(Result<ScheduledPublicationObservationV1, ScheduledPublicationObservationRefusalV1>),
    DeliveryProjected(Result<DeliveryProjectionV1, DeliveryProjectionRefusalV1>),
}

pub fn encode_scheduled_request_v1(
    request: &ScheduledWireRequestV1,
    limits: WireLimits,
) -> Result<Vec<u8>, WireEncodeError> {
    encode(limits, |sink| {
        sink.u8(VERSION_V1)?;
        match request {
            ScheduledWireRequestV1::Publish(request) => {
                sink.u8(PUBLISH)?;
                encode_request(sink, request)
            }
            ScheduledWireRequestV1::Observe(request) => {
                sink.u8(OBSERVE)?;
                encode_request(sink, request)
            }
            ScheduledWireRequestV1::ProjectDelivery(source) => {
                sink.u8(PROJECT_DELIVERY)?;
                sink.u64(source.event_sequence)
            }
        }
    })
}

pub fn decode_scheduled_request_v1(
    input: &[u8],
    limits: WireLimits,
) -> Result<ScheduledWireRequestV1, WireDecodeError> {
    let mut decoder = decoder(input, limits)?;
    read_version(&mut decoder)?;
    let request = match read_u8(&mut decoder)? {
        PUBLISH => ScheduledWireRequestV1::Publish(decode_request(&mut decoder)?),
        OBSERVE => ScheduledWireRequestV1::Observe(decode_request(&mut decoder)?),
        PROJECT_DELIVERY => ScheduledWireRequestV1::ProjectDelivery(DeliverySourceV1 {
            event_sequence: read_u64(&mut decoder)?,
        }),
        _ => return Err(WireDecodeError::UnknownTag),
    };
    finish_decode(&decoder)?;
    Ok(request)
}

pub fn encode_scheduled_reply_v1(
    reply: &ScheduledWireReplyV1,
    limits: WireLimits,
) -> Result<Vec<u8>, WireEncodeError> {
    encode(limits, |sink| {
        sink.u8(VERSION_V1)?;
        match reply {
            ScheduledWireReplyV1::Published(result) => {
                sink.u8(PUBLISHED)?;
                encode_publication_result(sink, *result)
            }
            ScheduledWireReplyV1::Observed(result) => {
                sink.u8(OBSERVED)?;
                encode_observation_result(sink, *result)
            }
            ScheduledWireReplyV1::DeliveryProjected(result) => {
                sink.u8(DELIVERY_PROJECTED)?;
                encode_delivery_projection_result(sink, *result)
            }
        }
    })
}

pub fn decode_scheduled_reply_v1(
    input: &[u8],
    limits: WireLimits,
) -> Result<ScheduledWireReplyV1, WireDecodeError> {
    let mut decoder = decoder(input, limits)?;
    read_version(&mut decoder)?;
    let reply = match read_u8(&mut decoder)? {
        PUBLISHED => ScheduledWireReplyV1::Published(decode_publication_result(&mut decoder)?),
        OBSERVED => ScheduledWireReplyV1::Observed(decode_observation_result(&mut decoder)?),
        DELIVERY_PROJECTED => ScheduledWireReplyV1::DeliveryProjected(
            decode_delivery_projection_result(&mut decoder)?,
        ),
        _ => return Err(WireDecodeError::UnknownTag),
    };
    finish_decode(&decoder)?;
    Ok(reply)
}

fn encode_delivery_projection_result(
    sink: &mut dyn super::Sink,
    result: Result<DeliveryProjectionV1, DeliveryProjectionRefusalV1>,
) -> Result<(), WireEncodeError> {
    match result {
        Ok(DeliveryProjectionV1::Visible(wake)) => {
            sink.u8(1)?;
            sink.u64(wake.source.event_sequence)?;
            match wake.class {
                WakeClassV1::ConversationActivity => sink.u8(1),
            }
        }
        Ok(DeliveryProjectionV1::NotVisible) => sink.u8(2),
        Ok(DeliveryProjectionV1::Gap { retained_from }) => {
            sink.u8(3)?;
            sink.u64(retained_from)
        }
        Ok(DeliveryProjectionV1::Unavailable) => sink.u8(4),
        Err(DeliveryProjectionRefusalV1::NotAuthorized) => sink.u8(5),
        Err(DeliveryProjectionRefusalV1::Unavailable) => sink.u8(6),
    }
}

fn decode_delivery_projection_result(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<Result<DeliveryProjectionV1, DeliveryProjectionRefusalV1>, WireDecodeError> {
    Ok(match read_u8(decoder)? {
        1 => {
            let source = DeliverySourceV1 {
                event_sequence: read_u64(decoder)?,
            };
            let class = match read_u8(decoder)? {
                1 => WakeClassV1::ConversationActivity,
                _ => return Err(WireDecodeError::UnknownTag),
            };
            Ok(DeliveryProjectionV1::Visible(WakeProjectionV1 {
                source,
                class,
            }))
        }
        2 => Ok(DeliveryProjectionV1::NotVisible),
        3 => Ok(DeliveryProjectionV1::Gap {
            retained_from: read_u64(decoder)?,
        }),
        4 => Ok(DeliveryProjectionV1::Unavailable),
        5 => Err(DeliveryProjectionRefusalV1::NotAuthorized),
        6 => Err(DeliveryProjectionRefusalV1::Unavailable),
        _ => return Err(WireDecodeError::UnknownTag),
    })
}

fn encode_request(
    sink: &mut dyn super::Sink,
    request: &ScheduledPublicationRequestV1,
) -> Result<(), WireEncodeError> {
    for byte in request.id.into_bytes() {
        sink.u8(byte)?;
    }
    sink.text(request.intent.room_id())?;
    sink.u64(request.intent.logical_time())?;
    sink.text(request.intent.body())
}

fn decode_request(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<ScheduledPublicationRequestV1, WireDecodeError> {
    let mut id = [0; 16];
    for byte in &mut id {
        *byte = read_u8(decoder)?;
    }
    Ok(ScheduledPublicationRequestV1 {
        id: ScheduledPublicationIdV1::new(id).map_err(|_| WireDecodeError::Malformed)?,
        intent: ScheduledPublicationIntentV1::new(
            read_text(decoder)?,
            read_u64(decoder)?,
            read_text(decoder)?,
        ),
    })
}

fn encode_accepted(
    sink: &mut dyn super::Sink,
    accepted: ScheduledPublicationAcceptedV1,
) -> Result<(), WireEncodeError> {
    sink.u64(accepted.revision)?;
    sink.u64(accepted.message_sequence)
}

fn decode_accepted(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<ScheduledPublicationAcceptedV1, WireDecodeError> {
    Ok(ScheduledPublicationAcceptedV1 {
        revision: read_u64(decoder)?,
        message_sequence: read_u64(decoder)?,
    })
}

fn encode_publication_result(
    sink: &mut dyn super::Sink,
    result: Result<ScheduledPublicationAcceptedV1, ScheduledPublicationRefusalV1>,
) -> Result<(), WireEncodeError> {
    match result {
        Ok(accepted) => {
            sink.u8(1)?;
            encode_accepted(sink, accepted)
        }
        Err(ScheduledPublicationRefusalV1::InvalidInput) => sink.u8(2),
        Err(ScheduledPublicationRefusalV1::NotAuthorized) => sink.u8(3),
        Err(ScheduledPublicationRefusalV1::AdmissionDeferred { retry_after_ms }) => {
            sink.u8(4)?;
            sink.u64(retry_after_ms)
        }
        Err(ScheduledPublicationRefusalV1::Capacity) => sink.u8(5),
        Err(ScheduledPublicationRefusalV1::Conflict) => sink.u8(6),
        Err(ScheduledPublicationRefusalV1::Unavailable) => sink.u8(7),
        Err(ScheduledPublicationRefusalV1::Exhausted) => sink.u8(8),
    }
}

fn decode_publication_result(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<Result<ScheduledPublicationAcceptedV1, ScheduledPublicationRefusalV1>, WireDecodeError>
{
    Ok(match read_u8(decoder)? {
        1 => Ok(decode_accepted(decoder)?),
        2 => Err(ScheduledPublicationRefusalV1::InvalidInput),
        3 => Err(ScheduledPublicationRefusalV1::NotAuthorized),
        4 => Err(ScheduledPublicationRefusalV1::AdmissionDeferred {
            retry_after_ms: read_u64(decoder)?,
        }),
        5 => Err(ScheduledPublicationRefusalV1::Capacity),
        6 => Err(ScheduledPublicationRefusalV1::Conflict),
        7 => Err(ScheduledPublicationRefusalV1::Unavailable),
        8 => Err(ScheduledPublicationRefusalV1::Exhausted),
        _ => return Err(WireDecodeError::UnknownTag),
    })
}

fn encode_observation_result(
    sink: &mut dyn super::Sink,
    result: Result<ScheduledPublicationObservationV1, ScheduledPublicationObservationRefusalV1>,
) -> Result<(), WireEncodeError> {
    match result {
        Ok(ScheduledPublicationObservationV1::Accepted(accepted)) => {
            sink.u8(1)?;
            encode_accepted(sink, accepted)
        }
        Ok(ScheduledPublicationObservationV1::Absent) => sink.u8(2),
        Err(ScheduledPublicationObservationRefusalV1::InvalidInput) => sink.u8(3),
        Err(ScheduledPublicationObservationRefusalV1::NotAuthorized) => sink.u8(4),
        Err(ScheduledPublicationObservationRefusalV1::Conflict) => sink.u8(5),
        Err(ScheduledPublicationObservationRefusalV1::Unavailable) => sink.u8(6),
    }
}

fn decode_observation_result(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<
    Result<ScheduledPublicationObservationV1, ScheduledPublicationObservationRefusalV1>,
    WireDecodeError,
> {
    Ok(match read_u8(decoder)? {
        1 => Ok(ScheduledPublicationObservationV1::Accepted(
            decode_accepted(decoder)?,
        )),
        2 => Ok(ScheduledPublicationObservationV1::Absent),
        3 => Err(ScheduledPublicationObservationRefusalV1::InvalidInput),
        4 => Err(ScheduledPublicationObservationRefusalV1::NotAuthorized),
        5 => Err(ScheduledPublicationObservationRefusalV1::Conflict),
        6 => Err(ScheduledPublicationObservationRefusalV1::Unavailable),
        _ => return Err(WireDecodeError::UnknownTag),
    })
}

fn read_version(decoder: &mut keystone_codec::Decoder<'_>) -> Result<(), WireDecodeError> {
    if read_u8(decoder)? != VERSION_V1 {
        return Err(WireDecodeError::UnsupportedVersion);
    }
    Ok(())
}
