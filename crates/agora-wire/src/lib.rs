//! Canonical bounded binary encoding for Agora protocol V1.
//!
//! The participant in a connect request is an unauthenticated claim. A host
//! must authenticate it before calling `agora_service::Service::connect`.

use agora_protocol::{
    ClientIdV1, CommandV1, MemberV1, MessageV1, OutcomeV1, ParticipantKindV1, RefusalV1, ReplyV1,
    RequestIdV1, RequestV1, RoleV1, RoomSnapshotV1,
};
use keystone_codec::{DecodeError, DecodeLimits, Decoder, encode_bytes};
use std::fmt;

pub mod local_handshake;
pub mod v2;

const VERSION_V1: u8 = 1;
const CONNECT_REQUEST: u8 = 0x01;
const EXECUTE_REQUEST: u8 = 0x02;
const DISCONNECT_REQUEST: u8 = 0x03;
const CONNECTED_REPLY: u8 = 0x81;
const CONNECT_REFUSED_REPLY: u8 = 0x82;
const EXECUTED_REPLY: u8 = 0x83;
const DISCONNECTED_REPLY: u8 = 0x84;
const DISCONNECT_REFUSED_REPLY: u8 = 0x85;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WireLimits {
    pub max_frame_bytes: usize,
    pub max_text_bytes: usize,
    pub max_fields: usize,
    pub max_list_items: usize,
}

impl WireLimits {
    pub const fn reference() -> Self {
        Self {
            max_frame_bytes: 4 * 1_024 * 1_024 + 64 * 1_024,
            max_text_bytes: 16 * 1_024,
            max_fields: 100_000,
            max_list_items: 4_096,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WireEncodeError {
    Limit,
    Capacity,
    LengthOverflow,
    Invalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WireDecodeError {
    Limit,
    Malformed,
    UnsupportedVersion,
    UnknownTag,
    InvalidUtf8,
    TrailingData,
    Capacity,
}

#[derive(Clone, Eq, PartialEq)]
pub enum WireRequestV1 {
    Connect {
        participant_id: String,
    },
    Execute {
        client: ClientIdV1,
        request: RequestV1,
    },
    Disconnect(ClientIdV1),
}

impl fmt::Debug for WireRequestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connect { participant_id } => formatter
                .debug_struct("Connect")
                .field("participant_id_bytes", &participant_id.len())
                .finish(),
            Self::Execute { client, request } => formatter
                .debug_struct("Execute")
                .field("client", client)
                .field("request", request)
                .finish(),
            Self::Disconnect(client) => formatter.debug_tuple("Disconnect").field(client).finish(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WireReplyV1 {
    Connected(ClientIdV1),
    ConnectRefused(RefusalV1),
    Executed(ReplyV1),
    Disconnected,
    DisconnectRefused(RefusalV1),
}

pub fn encode_request_v1(
    request: &WireRequestV1,
    limits: WireLimits,
) -> Result<Vec<u8>, WireEncodeError> {
    encode(limits, |sink| encode_request(sink, request))
}

pub fn decode_request_v1(
    input: &[u8],
    limits: WireLimits,
) -> Result<WireRequestV1, WireDecodeError> {
    let mut decoder = decoder(input, limits)?;
    read_version(&mut decoder)?;
    let request = match read_u8(&mut decoder)? {
        CONNECT_REQUEST => WireRequestV1::Connect {
            participant_id: read_text(&mut decoder)?,
        },
        EXECUTE_REQUEST => WireRequestV1::Execute {
            client: ClientIdV1(read_u64(&mut decoder)?),
            request: RequestV1 {
                request_id: RequestIdV1(read_u64(&mut decoder)?),
                command: decode_command(&mut decoder, limits)?,
            },
        },
        DISCONNECT_REQUEST => WireRequestV1::Disconnect(ClientIdV1(read_u64(&mut decoder)?)),
        _ => return Err(WireDecodeError::UnknownTag),
    };
    finish_decode(&decoder)?;
    Ok(request)
}

pub fn encode_reply_v1(
    reply: &WireReplyV1,
    limits: WireLimits,
) -> Result<Vec<u8>, WireEncodeError> {
    encode(limits, |sink| encode_reply(sink, reply))
}

pub fn decode_reply_v1(input: &[u8], limits: WireLimits) -> Result<WireReplyV1, WireDecodeError> {
    let mut decoder = decoder(input, limits)?;
    read_version(&mut decoder)?;
    let reply = match read_u8(&mut decoder)? {
        CONNECTED_REPLY => WireReplyV1::Connected(ClientIdV1(read_u64(&mut decoder)?)),
        CONNECT_REFUSED_REPLY => WireReplyV1::ConnectRefused(decode_refusal(&mut decoder)?),
        EXECUTED_REPLY => WireReplyV1::Executed(decode_protocol_reply(&mut decoder, limits)?),
        DISCONNECTED_REPLY => WireReplyV1::Disconnected,
        DISCONNECT_REFUSED_REPLY => WireReplyV1::DisconnectRefused(decode_refusal(&mut decoder)?),
        _ => return Err(WireDecodeError::UnknownTag),
    };
    finish_decode(&decoder)?;
    Ok(reply)
}

trait Sink {
    fn u8(&mut self, value: u8) -> Result<(), WireEncodeError>;
    fn u64(&mut self, value: u64) -> Result<(), WireEncodeError>;
    fn text(&mut self, value: &str) -> Result<(), WireEncodeError>;
    fn list(&mut self, length: usize) -> Result<(), WireEncodeError>;
}

fn encode(
    limits: WireLimits,
    write: impl Fn(&mut dyn Sink) -> Result<(), WireEncodeError>,
) -> Result<Vec<u8>, WireEncodeError> {
    let mut measure = Measure::new(limits);
    write(&mut measure)?;
    let length = measure.finish()?;
    let mut encoder = Encoder::new(length)?;
    write(&mut encoder)?;
    encoder.finish(length)
}

fn encode_request(sink: &mut dyn Sink, request: &WireRequestV1) -> Result<(), WireEncodeError> {
    sink.u8(VERSION_V1)?;
    match request {
        WireRequestV1::Connect { participant_id } => {
            sink.u8(CONNECT_REQUEST)?;
            sink.text(participant_id)
        }
        WireRequestV1::Execute { client, request } => {
            sink.u8(EXECUTE_REQUEST)?;
            sink.u64(client.0)?;
            sink.u64(request.request_id.0)?;
            encode_command(sink, &request.command)
        }
        WireRequestV1::Disconnect(client) => {
            sink.u8(DISCONNECT_REQUEST)?;
            sink.u64(client.0)
        }
    }
}

fn encode_reply(sink: &mut dyn Sink, reply: &WireReplyV1) -> Result<(), WireEncodeError> {
    sink.u8(VERSION_V1)?;
    match reply {
        WireReplyV1::Connected(client) => {
            sink.u8(CONNECTED_REPLY)?;
            sink.u64(client.0)
        }
        WireReplyV1::ConnectRefused(refusal) => {
            sink.u8(CONNECT_REFUSED_REPLY)?;
            encode_refusal(sink, *refusal)
        }
        WireReplyV1::Executed(reply) => {
            sink.u8(EXECUTED_REPLY)?;
            encode_protocol_reply(sink, reply)
        }
        WireReplyV1::Disconnected => sink.u8(DISCONNECTED_REPLY),
        WireReplyV1::DisconnectRefused(refusal) => {
            sink.u8(DISCONNECT_REFUSED_REPLY)?;
            encode_refusal(sink, *refusal)
        }
    }
}

fn encode_command(sink: &mut dyn Sink, command: &CommandV1) -> Result<(), WireEncodeError> {
    match command {
        CommandV1::Snapshot { room_id } => {
            sink.u8(1)?;
            sink.text(room_id)
        }
        CommandV1::Speak {
            room_id,
            expected_revision,
            logical_time,
            body,
            addressed_to,
        } => {
            sink.u8(2)?;
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
    decoder: &mut Decoder<'_>,
    limits: WireLimits,
) -> Result<CommandV1, WireDecodeError> {
    match read_u8(decoder)? {
        1 => Ok(CommandV1::Snapshot {
            room_id: read_text(decoder)?,
        }),
        2 => {
            let room_id = read_text(decoder)?;
            let expected_revision = read_u64(decoder)?;
            let logical_time = read_u64(decoder)?;
            let body = read_text(decoder)?;
            let length = read_list(decoder, limits)?;
            let mut addressed_to = try_vec(length)?;
            for _ in 0..length {
                addressed_to.push(read_text(decoder)?);
            }
            Ok(CommandV1::Speak {
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

fn encode_protocol_reply(sink: &mut dyn Sink, reply: &ReplyV1) -> Result<(), WireEncodeError> {
    match reply {
        ReplyV1::Accepted(outcome) => {
            sink.u8(1)?;
            match outcome {
                OutcomeV1::Snapshot(snapshot) => {
                    sink.u8(1)?;
                    encode_snapshot(sink, snapshot)
                }
                OutcomeV1::MessageAccepted { revision, sequence } => {
                    sink.u8(2)?;
                    sink.u64(*revision)?;
                    sink.u64(*sequence)
                }
            }
        }
        ReplyV1::Refused(refusal) => {
            sink.u8(2)?;
            encode_refusal(sink, *refusal)
        }
    }
}

fn decode_protocol_reply(
    decoder: &mut Decoder<'_>,
    limits: WireLimits,
) -> Result<ReplyV1, WireDecodeError> {
    match read_u8(decoder)? {
        1 => match read_u8(decoder)? {
            1 => Ok(ReplyV1::Accepted(OutcomeV1::Snapshot(decode_snapshot(
                decoder, limits,
            )?))),
            2 => Ok(ReplyV1::Accepted(OutcomeV1::MessageAccepted {
                revision: read_u64(decoder)?,
                sequence: read_u64(decoder)?,
            })),
            _ => Err(WireDecodeError::UnknownTag),
        },
        2 => Ok(ReplyV1::Refused(decode_refusal(decoder)?)),
        _ => Err(WireDecodeError::UnknownTag),
    }
}

fn encode_snapshot(sink: &mut dyn Sink, snapshot: &RoomSnapshotV1) -> Result<(), WireEncodeError> {
    sink.u64(snapshot.revision)?;
    sink.text(&snapshot.room_id)?;
    sink.text(&snapshot.name)?;
    sink.list(snapshot.members.len())?;
    for member in &snapshot.members {
        sink.text(&member.participant_id)?;
        sink.u8(match member.kind {
            ParticipantKindV1::Human => 1,
            ParticipantKindV1::Agent => 2,
        })?;
        sink.u8(match member.role {
            RoleV1::Owner => 1,
            RoleV1::Moderator => 2,
            RoleV1::Member => 3,
        })?;
    }
    sink.list(snapshot.messages.len())?;
    for message in &snapshot.messages {
        sink.u64(message.sequence)?;
        sink.text(&message.author)?;
        sink.u64(message.logical_time)?;
        sink.text(&message.body)?;
        sink.list(message.addressed_to.len())?;
        for participant in &message.addressed_to {
            sink.text(participant)?;
        }
    }
    sink.u64(snapshot.next_message_sequence)
}

fn decode_snapshot(
    decoder: &mut Decoder<'_>,
    limits: WireLimits,
) -> Result<RoomSnapshotV1, WireDecodeError> {
    let revision = read_u64(decoder)?;
    let room_id = read_text(decoder)?;
    let name = read_text(decoder)?;
    let member_count = read_list(decoder, limits)?;
    let mut members = try_vec(member_count)?;
    for _ in 0..member_count {
        members.push(MemberV1 {
            participant_id: read_text(decoder)?,
            kind: match read_u8(decoder)? {
                1 => ParticipantKindV1::Human,
                2 => ParticipantKindV1::Agent,
                _ => return Err(WireDecodeError::UnknownTag),
            },
            role: match read_u8(decoder)? {
                1 => RoleV1::Owner,
                2 => RoleV1::Moderator,
                3 => RoleV1::Member,
                _ => return Err(WireDecodeError::UnknownTag),
            },
        });
    }
    let message_count = read_list(decoder, limits)?;
    let mut messages = try_vec(message_count)?;
    for _ in 0..message_count {
        let sequence = read_u64(decoder)?;
        let author = read_text(decoder)?;
        let logical_time = read_u64(decoder)?;
        let body = read_text(decoder)?;
        let addressed_count = read_list(decoder, limits)?;
        let mut addressed_to = try_vec(addressed_count)?;
        for _ in 0..addressed_count {
            addressed_to.push(read_text(decoder)?);
        }
        messages.push(MessageV1 {
            sequence,
            author,
            logical_time,
            body,
            addressed_to,
        });
    }
    Ok(RoomSnapshotV1 {
        revision,
        room_id,
        name,
        members,
        messages,
        next_message_sequence: read_u64(decoder)?,
    })
}

fn encode_refusal(sink: &mut dyn Sink, refusal: RefusalV1) -> Result<(), WireEncodeError> {
    let tag = match refusal {
        RefusalV1::UnknownClient => 1,
        RefusalV1::UnknownParticipant => 2,
        RefusalV1::UnknownRoom => 3,
        RefusalV1::NotMember => 4,
        RefusalV1::Stale { .. } => 5,
        RefusalV1::RequestConflict => 6,
        RefusalV1::ClientLimit => 7,
        RefusalV1::RetiredClientLimit => 8,
        RefusalV1::RequestLimit => 9,
        RefusalV1::SnapshotLimit => 10,
        RefusalV1::InvalidInput => 11,
        RefusalV1::Capacity => 12,
        RefusalV1::Exhausted => 13,
        RefusalV1::Unavailable => 14,
        RefusalV1::NotAuthorized => 15,
    };
    sink.u8(tag)?;
    if let RefusalV1::Stale { actual_revision } = refusal {
        sink.u64(actual_revision)?;
    }
    Ok(())
}

fn decode_refusal(decoder: &mut Decoder<'_>) -> Result<RefusalV1, WireDecodeError> {
    match read_u8(decoder)? {
        1 => Ok(RefusalV1::UnknownClient),
        2 => Ok(RefusalV1::UnknownParticipant),
        3 => Ok(RefusalV1::UnknownRoom),
        4 => Ok(RefusalV1::NotMember),
        5 => Ok(RefusalV1::Stale {
            actual_revision: read_u64(decoder)?,
        }),
        6 => Ok(RefusalV1::RequestConflict),
        7 => Ok(RefusalV1::ClientLimit),
        8 => Ok(RefusalV1::RetiredClientLimit),
        9 => Ok(RefusalV1::RequestLimit),
        10 => Ok(RefusalV1::SnapshotLimit),
        11 => Ok(RefusalV1::InvalidInput),
        12 => Ok(RefusalV1::Capacity),
        13 => Ok(RefusalV1::Exhausted),
        14 => Ok(RefusalV1::Unavailable),
        15 => Ok(RefusalV1::NotAuthorized),
        _ => Err(WireDecodeError::UnknownTag),
    }
}

struct Measure {
    bytes: usize,
    fields: usize,
    limits: WireLimits,
}

impl Measure {
    fn new(limits: WireLimits) -> Self {
        Self {
            bytes: 0,
            fields: 0,
            limits,
        }
    }

    fn field(&mut self, bytes: usize) -> Result<(), WireEncodeError> {
        self.fields = self
            .fields
            .checked_add(1)
            .ok_or(WireEncodeError::LengthOverflow)?;
        if self.fields > self.limits.max_fields {
            return Err(WireEncodeError::Limit);
        }
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or(WireEncodeError::LengthOverflow)?;
        if self.bytes > self.limits.max_frame_bytes {
            return Err(WireEncodeError::Limit);
        }
        Ok(())
    }

    fn finish(self) -> Result<usize, WireEncodeError> {
        if self.bytes > self.limits.max_frame_bytes {
            return Err(WireEncodeError::Limit);
        }
        Ok(self.bytes)
    }
}

impl Sink for Measure {
    fn u8(&mut self, _value: u8) -> Result<(), WireEncodeError> {
        self.field(1)
    }

    fn u64(&mut self, _value: u64) -> Result<(), WireEncodeError> {
        self.field(8)
    }

    fn text(&mut self, value: &str) -> Result<(), WireEncodeError> {
        if value.len() > self.limits.max_text_bytes {
            return Err(WireEncodeError::Limit);
        }
        let bytes = value
            .len()
            .checked_add(4)
            .ok_or(WireEncodeError::LengthOverflow)?;
        u32::try_from(value.len()).map_err(|_| WireEncodeError::LengthOverflow)?;
        self.field(bytes)
    }

    fn list(&mut self, length: usize) -> Result<(), WireEncodeError> {
        if length > self.limits.max_list_items {
            return Err(WireEncodeError::Limit);
        }
        self.u64(u64::try_from(length).map_err(|_| WireEncodeError::LengthOverflow)?)
    }
}

struct Encoder {
    output: Vec<u8>,
}

impl Encoder {
    fn new(length: usize) -> Result<Self, WireEncodeError> {
        let mut output = Vec::new();
        output
            .try_reserve_exact(length)
            .map_err(|_| WireEncodeError::Capacity)?;
        Ok(Self { output })
    }

    fn finish(self, expected: usize) -> Result<Vec<u8>, WireEncodeError> {
        if self.output.len() != expected {
            return Err(WireEncodeError::LengthOverflow);
        }
        Ok(self.output)
    }
}

impl Sink for Encoder {
    fn u8(&mut self, value: u8) -> Result<(), WireEncodeError> {
        self.output.push(value);
        Ok(())
    }

    fn u64(&mut self, value: u64) -> Result<(), WireEncodeError> {
        self.output.extend_from_slice(&value.to_be_bytes());
        Ok(())
    }

    fn text(&mut self, value: &str) -> Result<(), WireEncodeError> {
        encode_bytes(&mut self.output, value.as_bytes())
            .map_err(|_| WireEncodeError::LengthOverflow)
    }

    fn list(&mut self, length: usize) -> Result<(), WireEncodeError> {
        self.u64(u64::try_from(length).map_err(|_| WireEncodeError::LengthOverflow)?)
    }
}

fn decoder(input: &[u8], limits: WireLimits) -> Result<Decoder<'_>, WireDecodeError> {
    Decoder::new(
        input,
        DecodeLimits {
            max_input_bytes: limits.max_frame_bytes,
            max_field_bytes: limits.max_text_bytes,
            max_fields: limits.max_fields,
        },
    )
    .map_err(map_decode)
}

fn read_version(decoder: &mut Decoder<'_>) -> Result<(), WireDecodeError> {
    if read_u8(decoder)? != VERSION_V1 {
        return Err(WireDecodeError::UnsupportedVersion);
    }
    Ok(())
}

fn read_u8(decoder: &mut Decoder<'_>) -> Result<u8, WireDecodeError> {
    decoder.read_u8().map_err(map_decode)
}

fn read_u64(decoder: &mut Decoder<'_>) -> Result<u64, WireDecodeError> {
    decoder.read_u64_be().map_err(map_decode)
}

fn read_text(decoder: &mut Decoder<'_>) -> Result<String, WireDecodeError> {
    let bytes = decoder.read_bytes().map_err(map_decode)?;
    let value = std::str::from_utf8(bytes).map_err(|_| WireDecodeError::InvalidUtf8)?;
    let mut output = String::new();
    output
        .try_reserve_exact(value.len())
        .map_err(|_| WireDecodeError::Capacity)?;
    output.push_str(value);
    Ok(output)
}

fn read_list(decoder: &mut Decoder<'_>, limits: WireLimits) -> Result<usize, WireDecodeError> {
    let length = usize::try_from(read_u64(decoder)?).map_err(|_| WireDecodeError::Limit)?;
    if length > limits.max_list_items {
        return Err(WireDecodeError::Limit);
    }
    Ok(length)
}

fn try_vec<T>(length: usize) -> Result<Vec<T>, WireDecodeError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| WireDecodeError::Capacity)?;
    Ok(output)
}

fn finish_decode(decoder: &Decoder<'_>) -> Result<(), WireDecodeError> {
    if !decoder.is_finished() {
        return Err(WireDecodeError::TrailingData);
    }
    Ok(())
}

fn map_decode(error: DecodeError) -> WireDecodeError {
    match error {
        DecodeError::InputLimit | DecodeError::FieldLimit | DecodeError::FieldCountLimit => {
            WireDecodeError::Limit
        }
        DecodeError::Truncated | DecodeError::LengthOverflow => WireDecodeError::Malformed,
    }
}
