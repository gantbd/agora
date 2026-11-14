//! Canonical bounded encoding for the additive derived-search profile.

use std::collections::BTreeSet;

use super::{
    Sink, WireDecodeError, WireEncodeError, WireLimits, decoder, encode, finish_decode, read_list,
    read_text, read_u8, read_u64, try_vec,
};
use agora_protocol::search::{
    SearchCommandV1, SearchHitV1, SearchMatchProfileV1, SearchOfferV1, SearchOutcomeV1,
    SearchPageV1, SearchRefusalV1, SearchReplyV1, SearchRequestIdV1, SearchRequestV1,
    ServiceSearchCursorV1,
};
use agora_protocol::v2::{ClientIdV2, MessageIdV1};

const VERSION_V1: u8 = 1;
const ACCEPTED_REPLY: u8 = 0x81;
const REFUSED_REPLY: u8 = 0x82;

pub fn encode_search_offer_v1(
    offer: &SearchOfferV1,
    limits: WireLimits,
) -> Result<Vec<u8>, WireEncodeError> {
    encode(limits, |sink| {
        sink.u8(VERSION_V1)?;
        sink.u64(offer.client.0)?;
        sink.u64(offer.request.request_id.0)?;
        sink.text(&offer.request.command.room_id)?;
        sink.u8(match offer.request.command.profile {
            SearchMatchProfileV1::ExactUtf8Substring => 1,
        })?;
        sink.text(&offer.request.command.term)?;
        encode_cursor(sink, offer.request.command.cursor)?;
        sink.u64(u64::from(offer.request.command.limit))
    })
}

pub fn decode_search_offer_v1(
    input: &[u8],
    limits: WireLimits,
) -> Result<SearchOfferV1, WireDecodeError> {
    let mut decoder = decoder(input, limits)?;
    read_version(&mut decoder)?;
    let client = ClientIdV2(read_u64(&mut decoder)?);
    let request_id = SearchRequestIdV1(read_u64(&mut decoder)?);
    let room_id = read_text(&mut decoder)?;
    let profile = match read_u8(&mut decoder)? {
        1 => SearchMatchProfileV1::ExactUtf8Substring,
        _ => return Err(WireDecodeError::UnknownTag),
    };
    let term = read_text(&mut decoder)?;
    let cursor = decode_cursor(&mut decoder)?;
    let limit = u32::try_from(read_u64(&mut decoder)?).map_err(|_| WireDecodeError::Limit)?;
    finish_decode(&decoder)?;
    Ok(SearchOfferV1 {
        client,
        request: SearchRequestV1 {
            request_id,
            command: SearchCommandV1 {
                room_id,
                profile,
                term,
                cursor,
                limit,
            },
        },
    })
}

pub fn encode_search_reply_v1(
    reply: &SearchReplyV1,
    limits: WireLimits,
) -> Result<Vec<u8>, WireEncodeError> {
    encode(limits, |sink| {
        sink.u8(VERSION_V1)?;
        match reply {
            SearchReplyV1::Accepted(outcome) => {
                sink.u8(ACCEPTED_REPLY)?;
                encode_outcome(sink, outcome)
            }
            SearchReplyV1::Refused(refusal) => {
                sink.u8(REFUSED_REPLY)?;
                sink.u8(encode_refusal(*refusal))
            }
        }
    })
}

pub fn decode_search_reply_v1(
    input: &[u8],
    limits: WireLimits,
) -> Result<SearchReplyV1, WireDecodeError> {
    let mut decoder = decoder(input, limits)?;
    read_version(&mut decoder)?;
    let reply = match read_u8(&mut decoder)? {
        ACCEPTED_REPLY => SearchReplyV1::Accepted(decode_outcome(&mut decoder, limits)?),
        REFUSED_REPLY => SearchReplyV1::Refused(decode_refusal(read_u8(&mut decoder)?)?),
        _ => return Err(WireDecodeError::UnknownTag),
    };
    finish_decode(&decoder)?;
    Ok(reply)
}

fn read_version(decoder: &mut keystone_codec::Decoder<'_>) -> Result<(), WireDecodeError> {
    if read_u8(decoder)? != VERSION_V1 {
        return Err(WireDecodeError::UnsupportedVersion);
    }
    Ok(())
}

fn encode_outcome(sink: &mut dyn Sink, outcome: &SearchOutcomeV1) -> Result<(), WireEncodeError> {
    match outcome {
        SearchOutcomeV1::Available(page) => {
            sink.u8(1)?;
            encode_page(sink, page)
        }
        SearchOutcomeV1::Incomplete(page) => {
            sink.u8(2)?;
            encode_page(sink, page)
        }
        SearchOutcomeV1::Stale {
            source_revision,
            index_generation,
        } => {
            sink.u8(3)?;
            sink.u64(*source_revision)?;
            sink.u64(*index_generation)
        }
        SearchOutcomeV1::Unavailable => sink.u8(4),
    }
}

fn decode_outcome(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<SearchOutcomeV1, WireDecodeError> {
    match read_u8(decoder)? {
        1 => Ok(SearchOutcomeV1::Available(decode_page(decoder, limits)?)),
        2 => Ok(SearchOutcomeV1::Incomplete(decode_page(decoder, limits)?)),
        3 => Ok(SearchOutcomeV1::Stale {
            source_revision: read_u64(decoder)?,
            index_generation: read_u64(decoder)?,
        }),
        4 => Ok(SearchOutcomeV1::Unavailable),
        _ => Err(WireDecodeError::UnknownTag),
    }
}

fn encode_page(sink: &mut dyn Sink, page: &SearchPageV1) -> Result<(), WireEncodeError> {
    if !valid_hits(&page.hits) || (page.hits.is_empty() && page.next.is_some()) {
        return Err(WireEncodeError::Invalid);
    }
    sink.u64(page.source_revision)?;
    sink.u64(page.index_generation)?;
    sink.list(page.hits.len())?;
    for hit in &page.hits {
        encode_message_id(sink, hit.message_id)?;
        sink.u64(hit.message_sequence)?;
    }
    encode_cursor(sink, page.next)
}

fn decode_page(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<SearchPageV1, WireDecodeError> {
    let source_revision = read_u64(decoder)?;
    let index_generation = read_u64(decoder)?;
    let length = read_list(decoder, limits)?;
    let mut hits = try_vec(length)?;
    for _ in 0..length {
        hits.push(SearchHitV1 {
            message_id: decode_message_id(decoder)?,
            message_sequence: read_u64(decoder)?,
        });
    }
    let next = decode_cursor(decoder)?;
    if !valid_hits(&hits) || (hits.is_empty() && next.is_some()) {
        return Err(WireDecodeError::Malformed);
    }
    Ok(SearchPageV1 {
        source_revision,
        index_generation,
        hits,
        next,
    })
}

fn valid_hits(hits: &[SearchHitV1]) -> bool {
    let mut ids = BTreeSet::new();
    hits.iter().all(|hit| ids.insert(hit.message_id))
        && hits
            .windows(2)
            .all(|pair| pair[0].message_sequence < pair[1].message_sequence)
}

fn encode_cursor(
    sink: &mut dyn Sink,
    cursor: Option<ServiceSearchCursorV1>,
) -> Result<(), WireEncodeError> {
    match cursor {
        None => sink.u8(0),
        Some(cursor) => {
            sink.u8(1)?;
            encode_bytes_16(sink, cursor.into_bytes())
        }
    }
}

fn decode_cursor(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<Option<ServiceSearchCursorV1>, WireDecodeError> {
    match read_u8(decoder)? {
        0 => Ok(None),
        1 => Ok(Some(ServiceSearchCursorV1::new(decode_bytes_16(decoder)?))),
        _ => Err(WireDecodeError::UnknownTag),
    }
}

fn encode_message_id(sink: &mut dyn Sink, id: MessageIdV1) -> Result<(), WireEncodeError> {
    encode_bytes_16(sink, id.into_bytes())
}

fn decode_message_id(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<MessageIdV1, WireDecodeError> {
    Ok(MessageIdV1::new(decode_bytes_16(decoder)?))
}

fn encode_bytes_16(sink: &mut dyn Sink, bytes: [u8; 16]) -> Result<(), WireEncodeError> {
    sink.u64(u64::from_be_bytes(
        bytes[..8].try_into().expect("fixed slice"),
    ))?;
    sink.u64(u64::from_be_bytes(
        bytes[8..].try_into().expect("fixed slice"),
    ))
}

fn decode_bytes_16(decoder: &mut keystone_codec::Decoder<'_>) -> Result<[u8; 16], WireDecodeError> {
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&read_u64(decoder)?.to_be_bytes());
    bytes[8..].copy_from_slice(&read_u64(decoder)?.to_be_bytes());
    Ok(bytes)
}

fn encode_refusal(refusal: SearchRefusalV1) -> u8 {
    match refusal {
        SearchRefusalV1::UnknownClient => 1,
        SearchRefusalV1::RequestConflict => 2,
        SearchRefusalV1::RequestLimit => 3,
        SearchRefusalV1::UnknownRoom => 4,
        SearchRefusalV1::NotMember => 5,
        SearchRefusalV1::InvalidInput => 6,
        SearchRefusalV1::InvalidCursor => 7,
        SearchRefusalV1::CursorLimit => 8,
        SearchRefusalV1::IndexCapacity => 9,
    }
}

fn decode_refusal(tag: u8) -> Result<SearchRefusalV1, WireDecodeError> {
    match tag {
        1 => Ok(SearchRefusalV1::UnknownClient),
        2 => Ok(SearchRefusalV1::RequestConflict),
        3 => Ok(SearchRefusalV1::RequestLimit),
        4 => Ok(SearchRefusalV1::UnknownRoom),
        5 => Ok(SearchRefusalV1::NotMember),
        6 => Ok(SearchRefusalV1::InvalidInput),
        7 => Ok(SearchRefusalV1::InvalidCursor),
        8 => Ok(SearchRefusalV1::CursorLimit),
        9 => Ok(SearchRefusalV1::IndexCapacity),
        _ => Err(WireDecodeError::UnknownTag),
    }
}
