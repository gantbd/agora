//! Canonical bounded binary encoding for Agora Protocol V2.

use super::{
    Sink, WireDecodeError, WireEncodeError, WireLimits, decoder, encode, finish_decode, read_list,
    read_text, read_u8, read_u64, try_vec,
};
use agora_protocol::v2::{
    AdmissionPolicyV2, ClientIdV2, CommandV2, MemberPageV2, MemberV2, MessagePageV2, MessageV2,
    OutcomeV2, PageCursorV2, ParticipantKindV2, RefusalV2, ReplyV2, RequestIdV2, RequestV2, RoleV2,
    RoomPageV2, RoomSummaryV2,
};
use std::fmt;

const VERSION_V2: u8 = 2;
const CONNECT_REQUEST: u8 = 0x01;
const EXECUTE_REQUEST: u8 = 0x02;
const DISCONNECT_REQUEST: u8 = 0x03;
const CONNECTED_REPLY: u8 = 0x81;
const CONNECT_REFUSED_REPLY: u8 = 0x82;
const EXECUTED_REPLY: u8 = 0x83;
const DISCONNECTED_REPLY: u8 = 0x84;
const DISCONNECT_REFUSED_REPLY: u8 = 0x85;

#[derive(Clone, Eq, PartialEq)]
pub enum WireRequestV2 {
    Connect {
        participant_id: String,
    },
    Execute {
        client: ClientIdV2,
        request: RequestV2,
    },
    Disconnect(ClientIdV2),
}

impl fmt::Debug for WireRequestV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connect { participant_id } => formatter
                .debug_struct("ConnectV2")
                .field("participant_id_bytes", &participant_id.len())
                .finish(),
            Self::Execute { client, request } => formatter
                .debug_struct("ExecuteV2")
                .field("client", client)
                .field("request", request)
                .finish(),
            Self::Disconnect(client) => {
                formatter.debug_tuple("DisconnectV2").field(client).finish()
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WireReplyV2 {
    Connected(ClientIdV2),
    ConnectRefused(RefusalV2),
    Executed(ReplyV2),
    Disconnected,
    DisconnectRefused(RefusalV2),
}

pub fn encode_request_v2(
    request: &WireRequestV2,
    limits: WireLimits,
) -> Result<Vec<u8>, WireEncodeError> {
    encode(limits, |sink| encode_request(sink, request))
}

pub fn decode_request_v2(
    input: &[u8],
    limits: WireLimits,
) -> Result<WireRequestV2, WireDecodeError> {
    let mut decoder = decoder(input, limits)?;
    if read_u8(&mut decoder)? != VERSION_V2 {
        return Err(WireDecodeError::UnsupportedVersion);
    }
    let request = match read_u8(&mut decoder)? {
        CONNECT_REQUEST => WireRequestV2::Connect {
            participant_id: read_text(&mut decoder)?,
        },
        EXECUTE_REQUEST => WireRequestV2::Execute {
            client: ClientIdV2(read_u64(&mut decoder)?),
            request: RequestV2 {
                request_id: RequestIdV2(read_u64(&mut decoder)?),
                command: decode_command(&mut decoder, limits)?,
            },
        },
        DISCONNECT_REQUEST => WireRequestV2::Disconnect(ClientIdV2(read_u64(&mut decoder)?)),
        _ => return Err(WireDecodeError::UnknownTag),
    };
    finish_decode(&decoder)?;
    Ok(request)
}

pub fn encode_reply_v2(
    reply: &WireReplyV2,
    limits: WireLimits,
) -> Result<Vec<u8>, WireEncodeError> {
    encode(limits, |sink| encode_reply(sink, reply))
}

pub fn decode_reply_v2(input: &[u8], limits: WireLimits) -> Result<WireReplyV2, WireDecodeError> {
    let mut decoder = decoder(input, limits)?;
    if read_u8(&mut decoder)? != VERSION_V2 {
        return Err(WireDecodeError::UnsupportedVersion);
    }
    let reply = match read_u8(&mut decoder)? {
        CONNECTED_REPLY => WireReplyV2::Connected(ClientIdV2(read_u64(&mut decoder)?)),
        CONNECT_REFUSED_REPLY => WireReplyV2::ConnectRefused(decode_refusal(&mut decoder)?),
        EXECUTED_REPLY => WireReplyV2::Executed(decode_protocol_reply(&mut decoder, limits)?),
        DISCONNECTED_REPLY => WireReplyV2::Disconnected,
        DISCONNECT_REFUSED_REPLY => WireReplyV2::DisconnectRefused(decode_refusal(&mut decoder)?),
        _ => return Err(WireDecodeError::UnknownTag),
    };
    finish_decode(&decoder)?;
    Ok(reply)
}

fn encode_request(sink: &mut dyn Sink, request: &WireRequestV2) -> Result<(), WireEncodeError> {
    sink.u8(VERSION_V2)?;
    match request {
        WireRequestV2::Connect { participant_id } => {
            sink.u8(CONNECT_REQUEST)?;
            sink.text(participant_id)
        }
        WireRequestV2::Execute { client, request } => {
            sink.u8(EXECUTE_REQUEST)?;
            sink.u64(client.0)?;
            sink.u64(request.request_id.0)?;
            encode_command(sink, &request.command)
        }
        WireRequestV2::Disconnect(client) => {
            sink.u8(DISCONNECT_REQUEST)?;
            sink.u64(client.0)
        }
    }
}

fn encode_reply(sink: &mut dyn Sink, reply: &WireReplyV2) -> Result<(), WireEncodeError> {
    sink.u8(VERSION_V2)?;
    match reply {
        WireReplyV2::Connected(client) => {
            sink.u8(CONNECTED_REPLY)?;
            sink.u64(client.0)
        }
        WireReplyV2::ConnectRefused(refusal) => {
            sink.u8(CONNECT_REFUSED_REPLY)?;
            encode_refusal(sink, *refusal)
        }
        WireReplyV2::Executed(reply) => {
            sink.u8(EXECUTED_REPLY)?;
            encode_protocol_reply(sink, reply)
        }
        WireReplyV2::Disconnected => sink.u8(DISCONNECTED_REPLY),
        WireReplyV2::DisconnectRefused(refusal) => {
            sink.u8(DISCONNECT_REFUSED_REPLY)?;
            encode_refusal(sink, *refusal)
        }
    }
}

fn encode_optional_cursor(
    sink: &mut dyn Sink,
    cursor: Option<PageCursorV2>,
) -> Result<(), WireEncodeError> {
    match cursor {
        None => sink.u8(0),
        Some(cursor) => {
            sink.u8(1)?;
            sink.u64(cursor.0)
        }
    }
}

fn decode_optional_cursor(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<Option<PageCursorV2>, WireDecodeError> {
    match read_u8(decoder)? {
        0 => Ok(None),
        1 => Ok(Some(PageCursorV2(read_u64(decoder)?))),
        _ => Err(WireDecodeError::UnknownTag),
    }
}

fn encode_command(sink: &mut dyn Sink, command: &CommandV2) -> Result<(), WireEncodeError> {
    match command {
        CommandV2::Rooms { cursor, limit } => {
            sink.u8(1)?;
            encode_optional_cursor(sink, *cursor)?;
            sink.u64(u64::from(*limit))
        }
        CommandV2::Members {
            room_id,
            cursor,
            limit,
        } => {
            sink.u8(2)?;
            sink.text(room_id)?;
            encode_optional_cursor(sink, *cursor)?;
            sink.u64(u64::from(*limit))
        }
        CommandV2::Messages {
            room_id,
            cursor,
            limit,
        } => {
            sink.u8(3)?;
            sink.text(room_id)?;
            encode_optional_cursor(sink, *cursor)?;
            sink.u64(u64::from(*limit))
        }
        CommandV2::CreateRoom {
            room_id,
            name,
            admission,
            expected_revision,
        } => {
            sink.u8(4)?;
            sink.text(room_id)?;
            sink.text(name)?;
            sink.u8(encode_admission(*admission))?;
            sink.u64(*expected_revision)
        }
        CommandV2::Invite {
            room_id,
            participant_id,
            expected_revision,
        } => {
            sink.u8(5)?;
            sink.text(room_id)?;
            sink.text(participant_id)?;
            sink.u64(*expected_revision)
        }
        CommandV2::Join {
            room_id,
            expected_revision,
        } => {
            sink.u8(6)?;
            sink.text(room_id)?;
            sink.u64(*expected_revision)
        }
        CommandV2::Leave {
            room_id,
            expected_revision,
        } => {
            sink.u8(7)?;
            sink.text(room_id)?;
            sink.u64(*expected_revision)
        }
        CommandV2::SetRole {
            room_id,
            participant_id,
            role,
            expected_revision,
        } => {
            sink.u8(8)?;
            sink.text(room_id)?;
            sink.text(participant_id)?;
            sink.u8(encode_role(*role))?;
            sink.u64(*expected_revision)
        }
        CommandV2::Speak {
            room_id,
            expected_revision,
            logical_time,
            body,
            addressed_to,
        } => {
            sink.u8(9)?;
            sink.text(room_id)?;
            sink.u64(*expected_revision)?;
            sink.u64(*logical_time)?;
            sink.text(body)?;
            sink.list(addressed_to.len())?;
            for participant in addressed_to {
                sink.text(participant)?;
            }
            Ok(())
        }
    }
}

fn decode_command(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<CommandV2, WireDecodeError> {
    match read_u8(decoder)? {
        1 => Ok(CommandV2::Rooms {
            cursor: decode_optional_cursor(decoder)?,
            limit: read_u32(decoder)?,
        }),
        2 => Ok(CommandV2::Members {
            room_id: read_text(decoder)?,
            cursor: decode_optional_cursor(decoder)?,
            limit: read_u32(decoder)?,
        }),
        3 => Ok(CommandV2::Messages {
            room_id: read_text(decoder)?,
            cursor: decode_optional_cursor(decoder)?,
            limit: read_u32(decoder)?,
        }),
        4 => Ok(CommandV2::CreateRoom {
            room_id: read_text(decoder)?,
            name: read_text(decoder)?,
            admission: decode_admission(decoder)?,
            expected_revision: read_u64(decoder)?,
        }),
        5 => Ok(CommandV2::Invite {
            room_id: read_text(decoder)?,
            participant_id: read_text(decoder)?,
            expected_revision: read_u64(decoder)?,
        }),
        6 => Ok(CommandV2::Join {
            room_id: read_text(decoder)?,
            expected_revision: read_u64(decoder)?,
        }),
        7 => Ok(CommandV2::Leave {
            room_id: read_text(decoder)?,
            expected_revision: read_u64(decoder)?,
        }),
        8 => Ok(CommandV2::SetRole {
            room_id: read_text(decoder)?,
            participant_id: read_text(decoder)?,
            role: decode_role(decoder)?,
            expected_revision: read_u64(decoder)?,
        }),
        9 => {
            let room_id = read_text(decoder)?;
            let expected_revision = read_u64(decoder)?;
            let logical_time = read_u64(decoder)?;
            let body = read_text(decoder)?;
            let length = read_list(decoder, limits)?;
            let mut addressed_to = try_vec(length)?;
            for _ in 0..length {
                addressed_to.push(read_text(decoder)?);
            }
            Ok(CommandV2::Speak {
                room_id,
                expected_revision,
                logical_time,
                body,
                addressed_to,
            })
        }
        _ => Err(WireDecodeError::UnknownTag),
    }
}

fn encode_protocol_reply(sink: &mut dyn Sink, reply: &ReplyV2) -> Result<(), WireEncodeError> {
    match reply {
        ReplyV2::Accepted(outcome) => {
            sink.u8(1)?;
            encode_outcome(sink, outcome)
        }
        ReplyV2::Refused(refusal) => {
            sink.u8(2)?;
            encode_refusal(sink, *refusal)
        }
    }
}

fn decode_protocol_reply(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<ReplyV2, WireDecodeError> {
    match read_u8(decoder)? {
        1 => Ok(ReplyV2::Accepted(decode_outcome(decoder, limits)?)),
        2 => Ok(ReplyV2::Refused(decode_refusal(decoder)?)),
        _ => Err(WireDecodeError::UnknownTag),
    }
}

fn encode_outcome(sink: &mut dyn Sink, outcome: &OutcomeV2) -> Result<(), WireEncodeError> {
    match outcome {
        OutcomeV2::Rooms(page) => {
            sink.u8(1)?;
            encode_room_page(sink, page)
        }
        OutcomeV2::Members(page) => {
            sink.u8(2)?;
            encode_member_page(sink, page)
        }
        OutcomeV2::Messages(page) => {
            sink.u8(3)?;
            encode_message_page(sink, page)
        }
        OutcomeV2::RoomCreated { revision } => encode_revision_outcome(sink, 4, *revision),
        OutcomeV2::InvitationRecorded { revision } => encode_revision_outcome(sink, 5, *revision),
        OutcomeV2::Joined { revision } => encode_revision_outcome(sink, 6, *revision),
        OutcomeV2::Left { revision } => encode_revision_outcome(sink, 7, *revision),
        OutcomeV2::RoleChanged { revision } => encode_revision_outcome(sink, 8, *revision),
        OutcomeV2::MessageAccepted { revision, sequence } => {
            sink.u8(9)?;
            sink.u64(*revision)?;
            sink.u64(*sequence)
        }
    }
}

fn encode_revision_outcome(
    sink: &mut dyn Sink,
    tag: u8,
    revision: u64,
) -> Result<(), WireEncodeError> {
    sink.u8(tag)?;
    sink.u64(revision)
}

fn decode_outcome(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<OutcomeV2, WireDecodeError> {
    match read_u8(decoder)? {
        1 => Ok(OutcomeV2::Rooms(decode_room_page(decoder, limits)?)),
        2 => Ok(OutcomeV2::Members(decode_member_page(decoder, limits)?)),
        3 => Ok(OutcomeV2::Messages(decode_message_page(decoder, limits)?)),
        4 => Ok(OutcomeV2::RoomCreated {
            revision: read_u64(decoder)?,
        }),
        5 => Ok(OutcomeV2::InvitationRecorded {
            revision: read_u64(decoder)?,
        }),
        6 => Ok(OutcomeV2::Joined {
            revision: read_u64(decoder)?,
        }),
        7 => Ok(OutcomeV2::Left {
            revision: read_u64(decoder)?,
        }),
        8 => Ok(OutcomeV2::RoleChanged {
            revision: read_u64(decoder)?,
        }),
        9 => Ok(OutcomeV2::MessageAccepted {
            revision: read_u64(decoder)?,
            sequence: read_u64(decoder)?,
        }),
        _ => Err(WireDecodeError::UnknownTag),
    }
}

fn encode_room_page(sink: &mut dyn Sink, page: &RoomPageV2) -> Result<(), WireEncodeError> {
    sink.u64(page.revision)?;
    sink.list(page.rooms.len())?;
    for room in &page.rooms {
        sink.text(&room.room_id)?;
        sink.text(&room.name)?;
        sink.u8(encode_admission(room.admission))?;
        sink.u8(encode_role(room.observer_role))?;
        sink.u64(u64::from(room.member_count))?;
        sink.u64(u64::from(room.retained_message_count))?;
        sink.u64(room.next_message_sequence)?;
    }
    encode_optional_cursor(sink, page.next)
}

fn decode_room_page(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<RoomPageV2, WireDecodeError> {
    let revision = read_u64(decoder)?;
    let length = read_list(decoder, limits)?;
    let mut rooms = try_vec(length)?;
    for _ in 0..length {
        rooms.push(RoomSummaryV2 {
            room_id: read_text(decoder)?,
            name: read_text(decoder)?,
            admission: decode_admission(decoder)?,
            observer_role: decode_role(decoder)?,
            member_count: read_u32(decoder)?,
            retained_message_count: read_u32(decoder)?,
            next_message_sequence: read_u64(decoder)?,
        });
    }
    Ok(RoomPageV2 {
        revision,
        rooms,
        next: decode_optional_cursor(decoder)?,
    })
}

fn encode_member_page(sink: &mut dyn Sink, page: &MemberPageV2) -> Result<(), WireEncodeError> {
    sink.u64(page.revision)?;
    sink.text(&page.room_id)?;
    sink.list(page.members.len())?;
    for member in &page.members {
        sink.text(&member.participant_id)?;
        sink.text(&member.display_name)?;
        sink.u8(match member.kind {
            ParticipantKindV2::Human => 1,
            ParticipantKindV2::Agent => 2,
        })?;
        sink.u8(encode_role(member.role))?;
    }
    encode_optional_cursor(sink, page.next)
}

fn decode_member_page(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<MemberPageV2, WireDecodeError> {
    let revision = read_u64(decoder)?;
    let room_id = read_text(decoder)?;
    let length = read_list(decoder, limits)?;
    let mut members = try_vec(length)?;
    for _ in 0..length {
        members.push(MemberV2 {
            participant_id: read_text(decoder)?,
            display_name: read_text(decoder)?,
            kind: match read_u8(decoder)? {
                1 => ParticipantKindV2::Human,
                2 => ParticipantKindV2::Agent,
                _ => return Err(WireDecodeError::UnknownTag),
            },
            role: decode_role(decoder)?,
        });
    }
    Ok(MemberPageV2 {
        revision,
        room_id,
        members,
        next: decode_optional_cursor(decoder)?,
    })
}

fn encode_message_page(sink: &mut dyn Sink, page: &MessagePageV2) -> Result<(), WireEncodeError> {
    sink.u64(page.revision)?;
    sink.text(&page.room_id)?;
    sink.list(page.messages.len())?;
    for message in &page.messages {
        encode_message(sink, message)?;
    }
    sink.u64(page.oldest_retained_sequence)?;
    sink.u64(page.next_message_sequence)?;
    encode_optional_cursor(sink, page.next)
}

fn decode_message_page(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<MessagePageV2, WireDecodeError> {
    let revision = read_u64(decoder)?;
    let room_id = read_text(decoder)?;
    let length = read_list(decoder, limits)?;
    let mut messages = try_vec(length)?;
    for _ in 0..length {
        messages.push(decode_message(decoder, limits)?);
    }
    Ok(MessagePageV2 {
        revision,
        room_id,
        messages,
        oldest_retained_sequence: read_u64(decoder)?,
        next_message_sequence: read_u64(decoder)?,
        next: decode_optional_cursor(decoder)?,
    })
}

fn encode_message(sink: &mut dyn Sink, message: &MessageV2) -> Result<(), WireEncodeError> {
    sink.u64(message.sequence)?;
    sink.text(&message.author)?;
    sink.u64(message.logical_time)?;
    sink.text(&message.body)?;
    sink.list(message.addressed_to.len())?;
    for participant in &message.addressed_to {
        sink.text(participant)?;
    }
    Ok(())
}

fn decode_message(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<MessageV2, WireDecodeError> {
    let sequence = read_u64(decoder)?;
    let author = read_text(decoder)?;
    let logical_time = read_u64(decoder)?;
    let body = read_text(decoder)?;
    let length = read_list(decoder, limits)?;
    let mut addressed_to = try_vec(length)?;
    for _ in 0..length {
        addressed_to.push(read_text(decoder)?);
    }
    Ok(MessageV2 {
        sequence,
        author,
        logical_time,
        body,
        addressed_to,
    })
}

fn encode_refusal(sink: &mut dyn Sink, refusal: RefusalV2) -> Result<(), WireEncodeError> {
    let tag = match refusal {
        RefusalV2::UnknownClient => 1,
        RefusalV2::UnknownParticipant => 2,
        RefusalV2::UnknownRoom => 3,
        RefusalV2::NotMember => 4,
        RefusalV2::NotAuthorized => 5,
        RefusalV2::Stale { .. } => 6,
        RefusalV2::RequestConflict => 7,
        RefusalV2::ClientLimit => 8,
        RefusalV2::RequestLimit => 9,
        RefusalV2::CursorLimit => 10,
        RefusalV2::InvalidCursor => 11,
        RefusalV2::InvalidInput => 12,
        RefusalV2::Capacity => 13,
        RefusalV2::Exhausted => 14,
        RefusalV2::Unavailable => 15,
    };
    sink.u8(tag)?;
    if let RefusalV2::Stale { actual_revision } = refusal {
        sink.u64(actual_revision)?;
    }
    Ok(())
}

fn decode_refusal(decoder: &mut keystone_codec::Decoder<'_>) -> Result<RefusalV2, WireDecodeError> {
    match read_u8(decoder)? {
        1 => Ok(RefusalV2::UnknownClient),
        2 => Ok(RefusalV2::UnknownParticipant),
        3 => Ok(RefusalV2::UnknownRoom),
        4 => Ok(RefusalV2::NotMember),
        5 => Ok(RefusalV2::NotAuthorized),
        6 => Ok(RefusalV2::Stale {
            actual_revision: read_u64(decoder)?,
        }),
        7 => Ok(RefusalV2::RequestConflict),
        8 => Ok(RefusalV2::ClientLimit),
        9 => Ok(RefusalV2::RequestLimit),
        10 => Ok(RefusalV2::CursorLimit),
        11 => Ok(RefusalV2::InvalidCursor),
        12 => Ok(RefusalV2::InvalidInput),
        13 => Ok(RefusalV2::Capacity),
        14 => Ok(RefusalV2::Exhausted),
        15 => Ok(RefusalV2::Unavailable),
        _ => Err(WireDecodeError::UnknownTag),
    }
}

fn encode_admission(value: AdmissionPolicyV2) -> u8 {
    match value {
        AdmissionPolicyV2::Open => 1,
        AdmissionPolicyV2::InviteOnly => 2,
    }
}

fn decode_admission(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<AdmissionPolicyV2, WireDecodeError> {
    match read_u8(decoder)? {
        1 => Ok(AdmissionPolicyV2::Open),
        2 => Ok(AdmissionPolicyV2::InviteOnly),
        _ => Err(WireDecodeError::UnknownTag),
    }
}

fn encode_role(value: RoleV2) -> u8 {
    match value {
        RoleV2::Owner => 1,
        RoleV2::Moderator => 2,
        RoleV2::Member => 3,
    }
}

fn decode_role(decoder: &mut keystone_codec::Decoder<'_>) -> Result<RoleV2, WireDecodeError> {
    match read_u8(decoder)? {
        1 => Ok(RoleV2::Owner),
        2 => Ok(RoleV2::Moderator),
        3 => Ok(RoleV2::Member),
        _ => Err(WireDecodeError::UnknownTag),
    }
}

fn read_u32(decoder: &mut keystone_codec::Decoder<'_>) -> Result<u32, WireDecodeError> {
    read_u64(decoder)?
        .try_into()
        .map_err(|_| WireDecodeError::Limit)
}
