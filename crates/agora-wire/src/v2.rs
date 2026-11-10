//! Canonical bounded binary encoding for Agora Protocol V2.

use super::{
    Sink, WireDecodeError, WireEncodeError, WireLimits, decoder, encode, finish_decode, read_list,
    read_text, read_u8, read_u64, try_vec,
};
use agora_protocol::v2::{
    AdmissionPolicyV2, BanPageV2, BannedParticipantV2, CapabilityV2, ChangeGapV2, ClientIdV2,
    CommandV2, MemberPageV2, MemberV2, MessagePageV2, MessageV2, OutcomeV2, PageCursorV2,
    ParticipantKindV2, RefusalV2, ReplyV2, RequestIdV2, RequestV2, RoleV2, RoomChangePageV2,
    RoomChangeV2, RoomEventV2, RoomPageV2, RoomSummaryV2,
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
        CommandV2::Capabilities => sink.u8(17),
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
        CommandV2::Bans {
            room_id,
            cursor,
            limit,
        } => {
            sink.u8(16)?;
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
        CommandV2::OpenChanges {
            room_id,
            observed_revision,
        } => {
            sink.u8(10)?;
            sink.text(room_id)?;
            sink.u64(*observed_revision)
        }
        CommandV2::Changes {
            room_id,
            cursor,
            limit,
        } => {
            sink.u8(11)?;
            sink.text(room_id)?;
            sink.u64(cursor.0)?;
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
        CommandV2::SetAdmissionPolicy {
            room_id,
            admission,
            expected_revision,
        } => {
            sink.u8(15)?;
            sink.text(room_id)?;
            sink.u8(encode_admission(*admission))?;
            sink.u64(*expected_revision)
        }
        CommandV2::Kick {
            room_id,
            participant_id,
            reason,
            causation,
            expected_revision,
        } => encode_moderation_command(
            sink,
            12,
            room_id,
            participant_id,
            reason,
            causation,
            *expected_revision,
        ),
        CommandV2::Ban {
            room_id,
            participant_id,
            reason,
            causation,
            expected_revision,
        } => encode_moderation_command(
            sink,
            13,
            room_id,
            participant_id,
            reason,
            causation,
            *expected_revision,
        ),
        CommandV2::Unban {
            room_id,
            participant_id,
            reason,
            causation,
            expected_revision,
        } => encode_moderation_command(
            sink,
            14,
            room_id,
            participant_id,
            reason,
            causation,
            *expected_revision,
        ),
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
        10 => Ok(CommandV2::OpenChanges {
            room_id: read_text(decoder)?,
            observed_revision: read_u64(decoder)?,
        }),
        11 => Ok(CommandV2::Changes {
            room_id: read_text(decoder)?,
            cursor: PageCursorV2(read_u64(decoder)?),
            limit: read_u32(decoder)?,
        }),
        12 => decode_moderation_command(decoder, ModerationCommand::Kick),
        13 => decode_moderation_command(decoder, ModerationCommand::Ban),
        14 => decode_moderation_command(decoder, ModerationCommand::Unban),
        15 => Ok(CommandV2::SetAdmissionPolicy {
            room_id: read_text(decoder)?,
            admission: decode_admission(decoder)?,
            expected_revision: read_u64(decoder)?,
        }),
        16 => Ok(CommandV2::Bans {
            room_id: read_text(decoder)?,
            cursor: decode_optional_cursor(decoder)?,
            limit: read_u32(decoder)?,
        }),
        17 => Ok(CommandV2::Capabilities),
        _ => Err(WireDecodeError::UnknownTag),
    }
}

#[derive(Clone, Copy)]
enum ModerationCommand {
    Kick,
    Ban,
    Unban,
}

fn encode_moderation_command(
    sink: &mut dyn Sink,
    tag: u8,
    room_id: &str,
    participant_id: &str,
    reason: &str,
    causation: &[u8; 16],
    expected_revision: u64,
) -> Result<(), WireEncodeError> {
    sink.u8(tag)?;
    sink.text(room_id)?;
    sink.text(participant_id)?;
    sink.text(reason)?;
    encode_causation(sink, causation)?;
    sink.u64(expected_revision)
}

fn decode_moderation_command(
    decoder: &mut keystone_codec::Decoder<'_>,
    command: ModerationCommand,
) -> Result<CommandV2, WireDecodeError> {
    let room_id = read_text(decoder)?;
    let participant_id = read_text(decoder)?;
    let reason = read_text(decoder)?;
    let causation = decode_causation(decoder)?;
    let expected_revision = read_u64(decoder)?;
    Ok(match command {
        ModerationCommand::Kick => CommandV2::Kick {
            room_id,
            participant_id,
            reason,
            causation,
            expected_revision,
        },
        ModerationCommand::Ban => CommandV2::Ban {
            room_id,
            participant_id,
            reason,
            causation,
            expected_revision,
        },
        ModerationCommand::Unban => CommandV2::Unban {
            room_id,
            participant_id,
            reason,
            causation,
            expected_revision,
        },
    })
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
        OutcomeV2::Capabilities(capabilities) => {
            if capabilities.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(WireEncodeError::Invalid);
            }
            sink.u8(18)?;
            sink.list(capabilities.len())?;
            for capability in capabilities {
                sink.u8(match capability {
                    CapabilityV2::RoomLifecycleV2 => 1,
                    CapabilityV2::RetainedHistoryV2 => 2,
                    CapabilityV2::RoomChangesV1 => 3,
                    CapabilityV2::ModerationV1 => 4,
                    CapabilityV2::RoomPolicyV1 => 5,
                    CapabilityV2::BanObservationV1 => 6,
                })?;
            }
            Ok(())
        }
        OutcomeV2::Rooms(page) => {
            sink.u8(1)?;
            encode_room_page(sink, page)
        }
        OutcomeV2::Members(page) => {
            sink.u8(2)?;
            encode_member_page(sink, page)
        }
        OutcomeV2::Bans(page) => {
            sink.u8(17)?;
            encode_ban_page(sink, page)
        }
        OutcomeV2::Messages(page) => {
            sink.u8(3)?;
            encode_message_page(sink, page)
        }
        OutcomeV2::ChangesOpened { revision, cursor } => {
            sink.u8(10)?;
            sink.u64(*revision)?;
            sink.u64(cursor.0)
        }
        OutcomeV2::Changes(page) => {
            sink.u8(11)?;
            encode_change_page(sink, page)
        }
        OutcomeV2::ChangeGap {
            current_revision,
            cause,
        } => {
            sink.u8(12)?;
            sink.u64(*current_revision)?;
            match cause {
                ChangeGapV2::Retention {
                    acknowledged_through,
                } => {
                    sink.u8(1)?;
                    sink.u64(*acknowledged_through)
                }
                ChangeGapV2::MembershipChanged => sink.u8(2),
            }
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
        OutcomeV2::Kicked { revision } => encode_revision_outcome(sink, 13, *revision),
        OutcomeV2::Banned { revision } => encode_revision_outcome(sink, 14, *revision),
        OutcomeV2::Unbanned { revision } => encode_revision_outcome(sink, 15, *revision),
        OutcomeV2::RoomPolicyChanged { revision } => encode_revision_outcome(sink, 16, *revision),
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
        10 => Ok(OutcomeV2::ChangesOpened {
            revision: read_u64(decoder)?,
            cursor: PageCursorV2(read_u64(decoder)?),
        }),
        11 => Ok(OutcomeV2::Changes(decode_change_page(decoder, limits)?)),
        12 => {
            let current_revision = read_u64(decoder)?;
            let cause = match read_u8(decoder)? {
                1 => ChangeGapV2::Retention {
                    acknowledged_through: read_u64(decoder)?,
                },
                2 => ChangeGapV2::MembershipChanged,
                _ => return Err(WireDecodeError::UnknownTag),
            };
            Ok(OutcomeV2::ChangeGap {
                current_revision,
                cause,
            })
        }
        13 => Ok(OutcomeV2::Kicked {
            revision: read_u64(decoder)?,
        }),
        14 => Ok(OutcomeV2::Banned {
            revision: read_u64(decoder)?,
        }),
        15 => Ok(OutcomeV2::Unbanned {
            revision: read_u64(decoder)?,
        }),
        16 => Ok(OutcomeV2::RoomPolicyChanged {
            revision: read_u64(decoder)?,
        }),
        17 => Ok(OutcomeV2::Bans(decode_ban_page(decoder, limits)?)),
        18 => {
            let length = read_list(decoder, limits)?;
            let mut capabilities = try_vec(length)?;
            for _ in 0..length {
                capabilities.push(match read_u8(decoder)? {
                    1 => CapabilityV2::RoomLifecycleV2,
                    2 => CapabilityV2::RetainedHistoryV2,
                    3 => CapabilityV2::RoomChangesV1,
                    4 => CapabilityV2::ModerationV1,
                    5 => CapabilityV2::RoomPolicyV1,
                    6 => CapabilityV2::BanObservationV1,
                    _ => return Err(WireDecodeError::UnknownTag),
                });
            }
            if capabilities.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(WireDecodeError::Malformed);
            }
            Ok(OutcomeV2::Capabilities(capabilities))
        }
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

fn encode_ban_page(sink: &mut dyn Sink, page: &BanPageV2) -> Result<(), WireEncodeError> {
    sink.u64(page.revision)?;
    sink.text(&page.room_id)?;
    sink.list(page.banned.len())?;
    for entry in &page.banned {
        sink.text(&entry.participant_id)?;
        sink.text(&entry.display_name)?;
        sink.u8(match entry.kind {
            ParticipantKindV2::Human => 1,
            ParticipantKindV2::Agent => 2,
        })?;
        sink.u8(encode_role(entry.prior_role))?;
    }
    encode_optional_cursor(sink, page.next)
}

fn decode_ban_page(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<BanPageV2, WireDecodeError> {
    let revision = read_u64(decoder)?;
    let room_id = read_text(decoder)?;
    let length = read_list(decoder, limits)?;
    let mut banned = try_vec(length)?;
    for _ in 0..length {
        banned.push(BannedParticipantV2 {
            participant_id: read_text(decoder)?,
            display_name: read_text(decoder)?,
            kind: match read_u8(decoder)? {
                1 => ParticipantKindV2::Human,
                2 => ParticipantKindV2::Agent,
                _ => return Err(WireDecodeError::UnknownTag),
            },
            prior_role: decode_role(decoder)?,
        });
    }
    Ok(BanPageV2 {
        revision,
        room_id,
        banned,
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

fn encode_change_page(sink: &mut dyn Sink, page: &RoomChangePageV2) -> Result<(), WireEncodeError> {
    sink.u64(page.revision)?;
    sink.list(page.changes.len())?;
    for change in &page.changes {
        sink.u64(change.event_sequence)?;
        sink.u64(change.revision)?;
        match &change.event {
            RoomEventV2::RoomCreated { owner, admission } => {
                sink.u8(1)?;
                sink.text(owner)?;
                sink.u8(encode_admission(*admission))?;
            }
            RoomEventV2::InvitationRecorded { participant_id } => {
                sink.u8(2)?;
                sink.text(participant_id)?;
            }
            RoomEventV2::ParticipantJoined { participant_id } => {
                sink.u8(3)?;
                sink.text(participant_id)?;
            }
            RoomEventV2::ParticipantLeft { participant_id } => {
                sink.u8(4)?;
                sink.text(participant_id)?;
            }
            RoomEventV2::RoleChanged {
                participant_id,
                role,
            } => {
                sink.u8(5)?;
                sink.text(participant_id)?;
                sink.u8(encode_role(*role))?;
            }
            RoomEventV2::MessageAccepted(message) => {
                sink.u8(6)?;
                encode_message(sink, message)?;
            }
            RoomEventV2::ParticipantKicked {
                actor_id,
                participant_id,
                reason,
                causation,
            } => encode_moderation_event(sink, 7, actor_id, participant_id, reason, causation)?,
            RoomEventV2::ParticipantBanned {
                actor_id,
                participant_id,
                reason,
                causation,
            } => encode_moderation_event(sink, 8, actor_id, participant_id, reason, causation)?,
            RoomEventV2::ParticipantUnbanned {
                actor_id,
                participant_id,
                reason,
                causation,
            } => encode_moderation_event(sink, 9, actor_id, participant_id, reason, causation)?,
            RoomEventV2::RoomPolicyChanged {
                actor_id,
                previous,
                admission,
            } => {
                sink.u8(10)?;
                sink.text(actor_id)?;
                sink.u8(encode_admission(*previous))?;
                sink.u8(encode_admission(*admission))?;
            }
        }
    }
    sink.u64(page.next.0)?;
    sink.u8(u8::from(page.has_more))
}

fn decode_change_page(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<RoomChangePageV2, WireDecodeError> {
    let revision = read_u64(decoder)?;
    let length = read_list(decoder, limits)?;
    let mut changes = try_vec(length)?;
    for _ in 0..length {
        let event_sequence = read_u64(decoder)?;
        let change_revision = read_u64(decoder)?;
        let event = match read_u8(decoder)? {
            1 => RoomEventV2::RoomCreated {
                owner: read_text(decoder)?,
                admission: decode_admission(decoder)?,
            },
            2 => RoomEventV2::InvitationRecorded {
                participant_id: read_text(decoder)?,
            },
            3 => RoomEventV2::ParticipantJoined {
                participant_id: read_text(decoder)?,
            },
            4 => RoomEventV2::ParticipantLeft {
                participant_id: read_text(decoder)?,
            },
            5 => RoomEventV2::RoleChanged {
                participant_id: read_text(decoder)?,
                role: decode_role(decoder)?,
            },
            6 => RoomEventV2::MessageAccepted(decode_message(decoder, limits)?),
            7 => decode_moderation_event(decoder, ModerationEvent::Kicked)?,
            8 => decode_moderation_event(decoder, ModerationEvent::Banned)?,
            9 => decode_moderation_event(decoder, ModerationEvent::Unbanned)?,
            10 => RoomEventV2::RoomPolicyChanged {
                actor_id: read_text(decoder)?,
                previous: decode_admission(decoder)?,
                admission: decode_admission(decoder)?,
            },
            _ => return Err(WireDecodeError::UnknownTag),
        };
        changes.push(RoomChangeV2 {
            event_sequence,
            revision: change_revision,
            event,
        });
    }
    let next = PageCursorV2(read_u64(decoder)?);
    let has_more = match read_u8(decoder)? {
        0 => false,
        1 => true,
        _ => return Err(WireDecodeError::UnknownTag),
    };
    Ok(RoomChangePageV2 {
        revision,
        changes,
        next,
        has_more,
    })
}

#[derive(Clone, Copy)]
enum ModerationEvent {
    Kicked,
    Banned,
    Unbanned,
}

fn encode_moderation_event(
    sink: &mut dyn Sink,
    tag: u8,
    actor_id: &str,
    participant_id: &str,
    reason: &str,
    causation: &[u8; 16],
) -> Result<(), WireEncodeError> {
    sink.u8(tag)?;
    sink.text(actor_id)?;
    sink.text(participant_id)?;
    sink.text(reason)?;
    encode_causation(sink, causation)
}

fn decode_moderation_event(
    decoder: &mut keystone_codec::Decoder<'_>,
    event: ModerationEvent,
) -> Result<RoomEventV2, WireDecodeError> {
    let actor_id = read_text(decoder)?;
    let participant_id = read_text(decoder)?;
    let reason = read_text(decoder)?;
    let causation = decode_causation(decoder)?;
    Ok(match event {
        ModerationEvent::Kicked => RoomEventV2::ParticipantKicked {
            actor_id,
            participant_id,
            reason,
            causation,
        },
        ModerationEvent::Banned => RoomEventV2::ParticipantBanned {
            actor_id,
            participant_id,
            reason,
            causation,
        },
        ModerationEvent::Unbanned => RoomEventV2::ParticipantUnbanned {
            actor_id,
            participant_id,
            reason,
            causation,
        },
    })
}

fn encode_causation(sink: &mut dyn Sink, causation: &[u8; 16]) -> Result<(), WireEncodeError> {
    let mut high = [0_u8; 8];
    let mut low = [0_u8; 8];
    high.copy_from_slice(&causation[..8]);
    low.copy_from_slice(&causation[8..]);
    sink.u64(u64::from_be_bytes(high))?;
    sink.u64(u64::from_be_bytes(low))
}

fn decode_causation(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<[u8; 16], WireDecodeError> {
    let mut causation = [0_u8; 16];
    causation[..8].copy_from_slice(&read_u64(decoder)?.to_be_bytes());
    causation[8..].copy_from_slice(&read_u64(decoder)?.to_be_bytes());
    Ok(causation)
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
        RefusalV2::Banned => 16,
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
        16 => Ok(RefusalV2::Banned),
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
