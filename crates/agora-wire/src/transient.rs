//! Canonical bounded encoding for Agora's separate volatile profile.

use super::{
    Sink, WireDecodeError, WireEncodeError, WireLimits, decoder, encode, finish_decode, read_list,
    read_text, read_u8, read_u64, try_vec,
};
use agora_protocol::transient::{
    PresenceEntryV1, PresencePageCursorV1, PresencePageV1, PresenceSourceClassV1, PresenceStateV1,
    PresenceViewStateV1, TransientCommandV1, TransientOutcomeV1, TransientRefusalV1,
    TransientReplyV1, TransientRequestIdV1, TransientRequestV1,
};
use agora_protocol::v2::ParticipantKindV2;

const VERSION_V1: u8 = 1;
const ACCEPTED_REPLY: u8 = 0x81;
const REFUSED_REPLY: u8 = 0x82;

pub fn encode_transient_request_v1(
    request: &TransientRequestV1,
    limits: WireLimits,
) -> Result<Vec<u8>, WireEncodeError> {
    encode(limits, |sink| {
        sink.u8(VERSION_V1)?;
        sink.u64(request.request_id.0)?;
        encode_command(sink, &request.command)
    })
}

pub fn decode_transient_request_v1(
    input: &[u8],
    limits: WireLimits,
) -> Result<TransientRequestV1, WireDecodeError> {
    let mut decoder = decoder(input, limits)?;
    read_version(&mut decoder)?;
    let request = TransientRequestV1 {
        request_id: TransientRequestIdV1(read_u64(&mut decoder)?),
        command: decode_command(&mut decoder, limits)?,
    };
    finish_decode(&decoder)?;
    Ok(request)
}

pub fn encode_transient_reply_v1(
    reply: &TransientReplyV1,
    limits: WireLimits,
) -> Result<Vec<u8>, WireEncodeError> {
    encode(limits, |sink| {
        sink.u8(VERSION_V1)?;
        match reply {
            TransientReplyV1::Accepted(outcome) => {
                sink.u8(ACCEPTED_REPLY)?;
                encode_outcome(sink, outcome)
            }
            TransientReplyV1::Refused(refusal) => {
                sink.u8(REFUSED_REPLY)?;
                encode_refusal(sink, *refusal)
            }
        }
    })
}

pub fn decode_transient_reply_v1(
    input: &[u8],
    limits: WireLimits,
) -> Result<TransientReplyV1, WireDecodeError> {
    let mut decoder = decoder(input, limits)?;
    read_version(&mut decoder)?;
    let reply = match read_u8(&mut decoder)? {
        ACCEPTED_REPLY => TransientReplyV1::Accepted(decode_outcome(&mut decoder, limits)?),
        REFUSED_REPLY => TransientReplyV1::Refused(decode_refusal(&mut decoder)?),
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

fn encode_command(
    sink: &mut dyn Sink,
    command: &TransientCommandV1,
) -> Result<(), WireEncodeError> {
    match command {
        TransientCommandV1::PublishPresence {
            room_id,
            state,
            lifetime,
        } => {
            sink.u8(1)?;
            sink.text(room_id)?;
            sink.u8(encode_presence_state(*state))?;
            sink.u64(*lifetime)
        }
        TransientCommandV1::ClearPresence { room_id } => {
            sink.u8(2)?;
            sink.text(room_id)
        }
        TransientCommandV1::PresencePage {
            room_id,
            cursor,
            limit,
        } => {
            sink.u8(3)?;
            sink.text(room_id)?;
            encode_cursor(sink, *cursor)?;
            sink.u64(u64::from(*limit))
        }
    }
}

fn decode_command(
    decoder: &mut keystone_codec::Decoder<'_>,
    _limits: WireLimits,
) -> Result<TransientCommandV1, WireDecodeError> {
    match read_u8(decoder)? {
        1 => Ok(TransientCommandV1::PublishPresence {
            room_id: read_text(decoder)?,
            state: decode_presence_state(read_u8(decoder)?)?,
            lifetime: read_u64(decoder)?,
        }),
        2 => Ok(TransientCommandV1::ClearPresence {
            room_id: read_text(decoder)?,
        }),
        3 => Ok(TransientCommandV1::PresencePage {
            room_id: read_text(decoder)?,
            cursor: decode_cursor(decoder)?,
            limit: u32::try_from(read_u64(decoder)?).map_err(|_| WireDecodeError::Limit)?,
        }),
        _ => Err(WireDecodeError::UnknownTag),
    }
}

fn encode_outcome(
    sink: &mut dyn Sink,
    outcome: &TransientOutcomeV1,
) -> Result<(), WireEncodeError> {
    match outcome {
        TransientOutcomeV1::PresencePublished { accepted_lifetime } => {
            if *accepted_lifetime == 0 {
                return Err(WireEncodeError::Invalid);
            }
            sink.u8(1)?;
            sink.u64(*accepted_lifetime)
        }
        TransientOutcomeV1::PresenceCleared { was_present } => {
            sink.u8(2)?;
            sink.u8(u8::from(*was_present))
        }
        TransientOutcomeV1::PresencePage(page) => {
            validate_page(page).map_err(|()| WireEncodeError::Invalid)?;
            sink.u8(3)?;
            sink.list(page.entries.len())?;
            for entry in &page.entries {
                encode_entry(sink, entry)?;
            }
            encode_cursor(sink, page.next)
        }
    }
}

fn decode_outcome(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<TransientOutcomeV1, WireDecodeError> {
    match read_u8(decoder)? {
        1 => {
            let accepted_lifetime = read_u64(decoder)?;
            if accepted_lifetime == 0 {
                return Err(WireDecodeError::Malformed);
            }
            Ok(TransientOutcomeV1::PresencePublished { accepted_lifetime })
        }
        2 => Ok(TransientOutcomeV1::PresenceCleared {
            was_present: decode_bool(read_u8(decoder)?)?,
        }),
        3 => {
            let length = read_list(decoder, limits)?;
            let mut entries = try_vec(length)?;
            for _ in 0..length {
                entries.push(decode_entry(decoder)?);
            }
            let page = PresencePageV1 {
                entries,
                next: decode_cursor(decoder)?,
            };
            validate_page(&page).map_err(|()| WireDecodeError::Malformed)?;
            Ok(TransientOutcomeV1::PresencePage(page))
        }
        _ => Err(WireDecodeError::UnknownTag),
    }
}

fn encode_entry(sink: &mut dyn Sink, entry: &PresenceEntryV1) -> Result<(), WireEncodeError> {
    validate_entry(entry).map_err(|()| WireEncodeError::Invalid)?;
    sink.text(&entry.participant_id)?;
    sink.u8(match entry.participant_kind {
        ParticipantKindV2::Human => 1,
        ParticipantKindV2::Agent => 2,
    })?;
    sink.u8(match entry.state {
        PresenceViewStateV1::Unknown => 0,
        PresenceViewStateV1::Available => 1,
        PresenceViewStateV1::Away => 2,
        PresenceViewStateV1::Busy => 3,
    })?;
    match entry.source {
        None => sink.u8(0)?,
        Some(PresenceSourceClassV1::ParticipantSession) => sink.u8(1)?,
    }
    match entry.remaining_lifetime {
        None => sink.u8(0),
        Some(lifetime) => {
            sink.u8(1)?;
            sink.u64(lifetime)
        }
    }
}

fn decode_entry(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<PresenceEntryV1, WireDecodeError> {
    let participant_id = read_text(decoder)?;
    let participant_kind = match read_u8(decoder)? {
        1 => ParticipantKindV2::Human,
        2 => ParticipantKindV2::Agent,
        _ => return Err(WireDecodeError::UnknownTag),
    };
    let state = match read_u8(decoder)? {
        0 => PresenceViewStateV1::Unknown,
        1 => PresenceViewStateV1::Available,
        2 => PresenceViewStateV1::Away,
        3 => PresenceViewStateV1::Busy,
        _ => return Err(WireDecodeError::UnknownTag),
    };
    let source = match read_u8(decoder)? {
        0 => None,
        1 => Some(PresenceSourceClassV1::ParticipantSession),
        _ => return Err(WireDecodeError::UnknownTag),
    };
    let remaining_lifetime = match read_u8(decoder)? {
        0 => None,
        1 => Some(read_u64(decoder)?),
        _ => return Err(WireDecodeError::Malformed),
    };
    let entry = PresenceEntryV1 {
        participant_id,
        participant_kind,
        state,
        source,
        remaining_lifetime,
    };
    validate_entry(&entry).map_err(|()| WireDecodeError::Malformed)?;
    Ok(entry)
}

fn validate_page(page: &PresencePageV1) -> Result<(), ()> {
    for entry in &page.entries {
        validate_entry(entry)?;
    }
    if page
        .entries
        .windows(2)
        .any(|pair| pair[0].participant_id >= pair[1].participant_id)
    {
        return Err(());
    }
    Ok(())
}

fn validate_entry(entry: &PresenceEntryV1) -> Result<(), ()> {
    if entry.participant_id.is_empty() {
        return Err(());
    }
    match entry.state {
        PresenceViewStateV1::Unknown => {
            if entry.source.is_some() || entry.remaining_lifetime.is_some() {
                return Err(());
            }
        }
        PresenceViewStateV1::Available | PresenceViewStateV1::Away | PresenceViewStateV1::Busy => {
            if entry.source != Some(PresenceSourceClassV1::ParticipantSession)
                || entry
                    .remaining_lifetime
                    .is_none_or(|lifetime| lifetime == 0)
            {
                return Err(());
            }
        }
    }
    Ok(())
}

fn encode_cursor(
    sink: &mut dyn Sink,
    cursor: Option<PresencePageCursorV1>,
) -> Result<(), WireEncodeError> {
    match cursor {
        None => sink.u8(0),
        Some(cursor) => {
            sink.u8(1)?;
            sink.u64(cursor.0)
        }
    }
}

fn decode_cursor(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<Option<PresencePageCursorV1>, WireDecodeError> {
    match read_u8(decoder)? {
        0 => Ok(None),
        1 => Ok(Some(PresencePageCursorV1(read_u64(decoder)?))),
        _ => Err(WireDecodeError::Malformed),
    }
}

fn encode_presence_state(state: PresenceStateV1) -> u8 {
    match state {
        PresenceStateV1::Available => 1,
        PresenceStateV1::Away => 2,
        PresenceStateV1::Busy => 3,
    }
}

fn decode_presence_state(tag: u8) -> Result<PresenceStateV1, WireDecodeError> {
    match tag {
        1 => Ok(PresenceStateV1::Available),
        2 => Ok(PresenceStateV1::Away),
        3 => Ok(PresenceStateV1::Busy),
        _ => Err(WireDecodeError::UnknownTag),
    }
}

fn encode_refusal(sink: &mut dyn Sink, refusal: TransientRefusalV1) -> Result<(), WireEncodeError> {
    sink.u8(match refusal {
        TransientRefusalV1::NotMember => 1,
        TransientRefusalV1::InvalidLifetime => 2,
        TransientRefusalV1::InvalidPage => 3,
        TransientRefusalV1::InvalidCursor => 4,
        TransientRefusalV1::StaleCursor => 5,
        TransientRefusalV1::Capacity => 6,
        TransientRefusalV1::FidelityLost => 7,
        TransientRefusalV1::Unavailable => 8,
        TransientRefusalV1::RequestConflict => 9,
        TransientRefusalV1::RequestOutOfSequence => 10,
    })
}

fn decode_refusal(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<TransientRefusalV1, WireDecodeError> {
    match read_u8(decoder)? {
        1 => Ok(TransientRefusalV1::NotMember),
        2 => Ok(TransientRefusalV1::InvalidLifetime),
        3 => Ok(TransientRefusalV1::InvalidPage),
        4 => Ok(TransientRefusalV1::InvalidCursor),
        5 => Ok(TransientRefusalV1::StaleCursor),
        6 => Ok(TransientRefusalV1::Capacity),
        7 => Ok(TransientRefusalV1::FidelityLost),
        8 => Ok(TransientRefusalV1::Unavailable),
        9 => Ok(TransientRefusalV1::RequestConflict),
        10 => Ok(TransientRefusalV1::RequestOutOfSequence),
        _ => Err(WireDecodeError::UnknownTag),
    }
}

fn decode_bool(tag: u8) -> Result<bool, WireDecodeError> {
    match tag {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(WireDecodeError::Malformed),
    }
}
