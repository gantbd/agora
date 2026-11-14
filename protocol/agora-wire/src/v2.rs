//! Canonical bounded binary encoding for Agora Protocol V2.

use super::{
    Sink, WireDecodeError, WireEncodeError, WireLimits, decoder, encode, finish_decode, read_list,
    read_text, read_u8, read_u64, try_vec,
};
use agora_protocol::v2::{
    AdmissionPolicyV2, AttachmentDescriptorV1, AttachmentIdV1, AttachmentIntegrityV1,
    AttachmentMessagePageV1, AttachmentMessageTargetV1, AttachmentMessageV1, AttachmentObjectKeyV1,
    AttachmentRoomChangePageV1, AttachmentStoreIdV1, BanPageV2, BannedParticipantV2, CapabilityV2,
    ChangeGapV2, ClientIdV2, CommandV2, ConversationProfileStatusV1, ConversationProfileV1,
    IdentifiedMessagePageV1, IdentifiedMessageV1, IdentifiedRoomChangePageV1,
    IdentifiedRoomChangeV1, IdentifiedRoomEventV1, MemberPageV2, MemberV2, MessageAttachmentV1,
    MessageIdV1, MessagePageV2, MessageRevisionAvailabilityV1, MessageRevisionPageV1,
    MessageRevisionV1, MessageV2, MessageVellumArtifactReferenceV1, OutcomeV2, PageCursorV2,
    ParticipantKindV2, PeerConversationCreatedV1, ReactionStatusV1, ReactionSummaryV1,
    ReadPositionV1, ReadScopeV1, RefusalV2, ReplyV2, RequestIdV2, RequestV2, RoleV2,
    RoomChangePageV2, RoomChangeV2, RoomEventV2, RoomPageV2, RoomSummaryV2, Sha256DigestV1,
    ThreadLifecycleStateV1, ThreadMessagePageV1, ThreadStatusV1, ThreadedMessageV1,
    ThreadedRoomChangePageV1, ThreadedRoomChangeV1, ThreadedRoomEventV1, TopicAliasStatusV1,
    TopicIdV1, TopicMessagePageV1, TopicMessageV1, TopicPageV1, TopicRoomChangePageV1,
    TopicRoomChangeV1, TopicRoomEventV1, TopicStatusV1, TopicSupersessionStatusV1,
    TopicTagStatusV1, TopicV1, VellumArtifactMessagePageV1, VellumArtifactMessageTargetV1,
    VellumArtifactMessageV1, VellumArtifactReferenceIdV1, VellumArtifactReferenceV1,
    VellumDocumentTargetV1,
};
use std::{collections::BTreeSet, fmt};

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
        CommandV2::IdentifiedMessages {
            room_id,
            cursor,
            limit,
        } => {
            sink.u8(18)?;
            sink.text(room_id)?;
            encode_optional_cursor(sink, *cursor)?;
            sink.u64(u64::from(*limit))
        }
        CommandV2::AttachmentMessages {
            room_id,
            cursor,
            limit,
        } => {
            sink.u8(58)?;
            sink.text(room_id)?;
            encode_optional_cursor(sink, *cursor)?;
            sink.u64(u64::from(*limit))
        }
        CommandV2::VellumArtifactMessages {
            room_id,
            cursor,
            limit,
        } => {
            sink.u8(62)?;
            sink.text(room_id)?;
            encode_optional_cursor(sink, *cursor)?;
            sink.u64(u64::from(*limit))
        }
        CommandV2::OpenAttachmentChanges {
            room_id,
            observed_revision,
        } => {
            sink.u8(60)?;
            sink.text(room_id)?;
            sink.u64(*observed_revision)
        }
        CommandV2::AttachmentChanges {
            room_id,
            cursor,
            limit,
        } => {
            sink.u8(61)?;
            sink.text(room_id)?;
            sink.u64(cursor.0)?;
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
        CommandV2::OpenIdentifiedChanges {
            room_id,
            observed_revision,
        } => {
            sink.u8(22)?;
            sink.text(room_id)?;
            sink.u64(*observed_revision)
        }
        CommandV2::IdentifiedChanges {
            room_id,
            cursor,
            limit,
        } => {
            sink.u8(23)?;
            sink.text(room_id)?;
            sink.u64(cursor.0)?;
            sink.u64(u64::from(*limit))
        }
        CommandV2::ThreadRoots {
            room_id,
            cursor,
            limit,
        } => {
            sink.u8(24)?;
            sink.text(room_id)?;
            encode_optional_cursor(sink, *cursor)?;
            sink.u64(u64::from(*limit))
        }
        CommandV2::ThreadReplies {
            room_id,
            root_message_id,
            cursor,
            limit,
        } => {
            sink.u8(25)?;
            sink.text(room_id)?;
            encode_message_id(sink, *root_message_id)?;
            encode_optional_cursor(sink, *cursor)?;
            sink.u64(u64::from(*limit))
        }
        CommandV2::OpenThreadedChanges {
            room_id,
            observed_revision,
        } => {
            sink.u8(27)?;
            sink.text(room_id)?;
            sink.u64(*observed_revision)
        }
        CommandV2::ThreadedChanges {
            room_id,
            cursor,
            limit,
        } => {
            sink.u8(28)?;
            sink.text(room_id)?;
            sink.u64(cursor.0)?;
            sink.u64(u64::from(*limit))
        }
        CommandV2::OpenTopicChanges {
            room_id,
            observed_revision,
        } => {
            sink.u8(41)?;
            sink.text(room_id)?;
            sink.u64(*observed_revision)
        }
        CommandV2::TopicChanges {
            room_id,
            cursor,
            limit,
        } => {
            sink.u8(42)?;
            sink.text(room_id)?;
            sink.u64(cursor.0)?;
            sink.u64(u64::from(*limit))
        }
        CommandV2::ReadPosition { room_id, scope } => {
            sink.u8(43)?;
            sink.text(room_id)?;
            encode_read_scope(sink, *scope)
        }
        CommandV2::AdvanceReadPosition {
            room_id,
            observed_revision,
            scope,
            target_message_id,
        } => {
            sink.u8(44)?;
            sink.text(room_id)?;
            sink.u64(*observed_revision)?;
            encode_read_scope(sink, *scope)?;
            encode_message_id(sink, *target_message_id)
        }
        CommandV2::ArchiveThread {
            room_id,
            expected_revision,
            root_message_id,
            reason,
            causation,
        }
        | CommandV2::ReopenThread {
            room_id,
            expected_revision,
            root_message_id,
            reason,
            causation,
        } => {
            sink.u8(if matches!(command, CommandV2::ArchiveThread { .. }) {
                29
            } else {
                30
            })?;
            sink.text(room_id)?;
            sink.u64(*expected_revision)?;
            encode_message_id(sink, *root_message_id)?;
            sink.text(reason)?;
            encode_causation(sink, causation)
        }
        CommandV2::ThreadStatus {
            room_id,
            root_message_id,
        } => {
            sink.u8(31)?;
            sink.text(room_id)?;
            encode_message_id(sink, *root_message_id)
        }
        CommandV2::AddReaction {
            room_id,
            expected_revision,
            message_id,
            key,
        }
        | CommandV2::RemoveReaction {
            room_id,
            expected_revision,
            message_id,
            key,
        } => {
            sink.u8(if matches!(command, CommandV2::AddReaction { .. }) {
                32
            } else {
                33
            })?;
            sink.text(room_id)?;
            sink.u64(*expected_revision)?;
            encode_message_id(sink, *message_id)?;
            sink.text(key)
        }
        CommandV2::ReactionStatus {
            room_id,
            message_id,
        } => {
            sink.u8(34)?;
            sink.text(room_id)?;
            encode_message_id(sink, *message_id)
        }
        CommandV2::EditMessage {
            room_id,
            expected_revision,
            message_id,
            logical_time,
            body,
        } => {
            sink.u8(35)?;
            sink.text(room_id)?;
            sink.u64(*expected_revision)?;
            encode_message_id(sink, *message_id)?;
            sink.u64(*logical_time)?;
            sink.text(body)
        }
        CommandV2::MessageRevisions {
            room_id,
            message_id,
            cursor,
            limit,
        } => {
            sink.u8(36)?;
            sink.text(room_id)?;
            encode_message_id(sink, *message_id)?;
            encode_optional_cursor(sink, *cursor)?;
            sink.u64(u64::from(*limit))
        }
        CommandV2::Topics {
            room_id,
            cursor,
            limit,
        } => {
            sink.u8(37)?;
            sink.text(room_id)?;
            encode_optional_cursor(sink, *cursor)?;
            sink.u64(u64::from(*limit))
        }
        CommandV2::TopicMessages {
            room_id,
            topic_id,
            cursor,
            limit,
        } => {
            sink.u8(38)?;
            sink.text(room_id)?;
            encode_topic_id(sink, *topic_id)?;
            encode_optional_cursor(sink, *cursor)?;
            sink.u64(u64::from(*limit))
        }
        CommandV2::CreateTopic {
            room_id,
            expected_revision,
            name,
        } => {
            sink.u8(39)?;
            sink.text(room_id)?;
            sink.u64(*expected_revision)?;
            sink.text(name)
        }
        CommandV2::RenameTopic {
            room_id,
            expected_revision,
            topic_id,
            name,
        } => {
            sink.u8(45)?;
            sink.text(room_id)?;
            sink.u64(*expected_revision)?;
            encode_topic_id(sink, *topic_id)?;
            sink.text(name)
        }
        CommandV2::ResolveTopic {
            room_id,
            expected_revision,
            topic_id,
        }
        | CommandV2::ReopenTopic {
            room_id,
            expected_revision,
            topic_id,
        } => {
            sink.u8(if matches!(command, CommandV2::ResolveTopic { .. }) {
                46
            } else {
                47
            })?;
            sink.text(room_id)?;
            sink.u64(*expected_revision)?;
            encode_topic_id(sink, *topic_id)
        }
        CommandV2::TopicStatus { room_id, topic_id } => {
            sink.u8(48)?;
            sink.text(room_id)?;
            encode_topic_id(sink, *topic_id)
        }
        CommandV2::SupersedeTopic {
            room_id,
            expected_revision,
            topic_id,
            successor_topic_id,
        } => {
            sink.u8(49)?;
            sink.text(room_id)?;
            sink.u64(*expected_revision)?;
            encode_topic_id(sink, *topic_id)?;
            encode_topic_id(sink, *successor_topic_id)
        }
        CommandV2::ClearTopicSupersession {
            room_id,
            expected_revision,
            topic_id,
        } => {
            sink.u8(50)?;
            sink.text(room_id)?;
            sink.u64(*expected_revision)?;
            encode_topic_id(sink, *topic_id)
        }
        CommandV2::TopicSupersessionStatus { room_id, topic_id } => {
            sink.u8(51)?;
            sink.text(room_id)?;
            encode_topic_id(sink, *topic_id)
        }
        CommandV2::AddTopicAlias {
            room_id,
            expected_revision,
            topic_id,
            alias,
        }
        | CommandV2::RemoveTopicAlias {
            room_id,
            expected_revision,
            topic_id,
            alias,
        } => {
            sink.u8(if matches!(command, CommandV2::AddTopicAlias { .. }) {
                52
            } else {
                53
            })?;
            sink.text(room_id)?;
            sink.u64(*expected_revision)?;
            encode_topic_id(sink, *topic_id)?;
            sink.text(alias)
        }
        CommandV2::TopicAliasStatus { room_id, topic_id } => {
            sink.u8(54)?;
            sink.text(room_id)?;
            encode_topic_id(sink, *topic_id)
        }
        CommandV2::AddTopicTag {
            room_id,
            expected_revision,
            topic_id,
            tag,
        }
        | CommandV2::RemoveTopicTag {
            room_id,
            expected_revision,
            topic_id,
            tag,
        } => {
            sink.u8(if matches!(command, CommandV2::AddTopicTag { .. }) {
                55
            } else {
                56
            })?;
            sink.text(room_id)?;
            sink.u64(*expected_revision)?;
            encode_topic_id(sink, *topic_id)?;
            sink.text(tag)
        }
        CommandV2::TopicTagStatus { room_id, topic_id } => {
            sink.u8(57)?;
            sink.text(room_id)?;
            encode_topic_id(sink, *topic_id)
        }
        CommandV2::SpeakInTopic {
            room_id,
            expected_revision,
            topic_id,
            logical_time,
            body,
            addressed_to,
        } => {
            sink.u8(40)?;
            sink.text(room_id)?;
            sink.u64(*expected_revision)?;
            encode_topic_id(sink, *topic_id)?;
            sink.u64(*logical_time)?;
            sink.text(body)?;
            sink.list(addressed_to.len())?;
            for participant in addressed_to {
                sink.text(participant)?;
            }
            Ok(())
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
        CommandV2::CreatePeerConversation {
            room_id,
            name,
            profile,
            founding_participant_ids,
            expected_revision,
        } => {
            if *profile == ConversationProfileV1::Ordinary
                || founding_participant_ids.is_empty()
                || founding_participant_ids
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
            {
                return Err(WireEncodeError::Invalid);
            }
            sink.u8(64)?;
            sink.text(room_id)?;
            sink.text(name)?;
            sink.u8(encode_conversation_profile(*profile))?;
            sink.list(founding_participant_ids.len())?;
            for participant_id in founding_participant_ids {
                sink.text(participant_id)?;
            }
            sink.u64(*expected_revision)
        }
        CommandV2::ConversationProfile { room_id } => {
            sink.u8(65)?;
            sink.text(room_id)
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
        CommandV2::SpeakIdentified {
            room_id,
            expected_revision,
            logical_time,
            body,
            addressed_to,
        } => {
            sink.u8(19)?;
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
        CommandV2::SpeakReply {
            room_id,
            expected_revision,
            root_message_id,
            logical_time,
            body,
            addressed_to,
        } => {
            sink.u8(26)?;
            sink.text(room_id)?;
            sink.u64(*expected_revision)?;
            encode_message_id(sink, *root_message_id)?;
            sink.u64(*logical_time)?;
            sink.text(body)?;
            sink.list(addressed_to.len())?;
            for participant in addressed_to {
                sink.text(participant)?;
            }
            Ok(())
        }
        CommandV2::SpeakWithAttachments {
            room_id,
            expected_revision,
            target,
            logical_time,
            body,
            addressed_to,
            attachments,
        } => {
            if !valid_attachment_descriptors(attachments) {
                return Err(WireEncodeError::Invalid);
            }
            sink.u8(59)?;
            sink.text(room_id)?;
            sink.u64(*expected_revision)?;
            encode_attachment_target(sink, *target)?;
            sink.u64(*logical_time)?;
            sink.text(body)?;
            sink.list(addressed_to.len())?;
            for participant in addressed_to {
                sink.text(participant)?;
            }
            sink.list(attachments.len())?;
            for descriptor in attachments {
                encode_attachment_descriptor(sink, descriptor)?;
            }
            Ok(())
        }
        CommandV2::SpeakWithVellumArtifacts {
            room_id,
            expected_revision,
            target,
            logical_time,
            body,
            addressed_to,
            references,
        } => {
            if !valid_vellum_artifact_references(references) {
                return Err(WireEncodeError::Invalid);
            }
            sink.u8(63)?;
            sink.text(room_id)?;
            sink.u64(*expected_revision)?;
            encode_vellum_artifact_target(sink, *target)?;
            sink.u64(*logical_time)?;
            sink.text(body)?;
            sink.list(addressed_to.len())?;
            for participant in addressed_to {
                sink.text(participant)?;
            }
            sink.list(references.len())?;
            for reference in references {
                encode_vellum_artifact_reference(sink, reference)?;
            }
            Ok(())
        }
        CommandV2::WithdrawMessage {
            room_id,
            expected_revision,
            message_id,
        } => {
            sink.u8(20)?;
            sink.text(room_id)?;
            sink.u64(*expected_revision)?;
            encode_message_id(sink, *message_id)
        }
        CommandV2::RedactMessage {
            room_id,
            expected_revision,
            message_id,
            reason,
            causation,
        } => {
            sink.u8(21)?;
            sink.text(room_id)?;
            sink.u64(*expected_revision)?;
            encode_message_id(sink, *message_id)?;
            sink.text(reason)?;
            for byte in causation {
                sink.u8(*byte)?;
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
        18 => Ok(CommandV2::IdentifiedMessages {
            room_id: read_text(decoder)?,
            cursor: decode_optional_cursor(decoder)?,
            limit: read_u32(decoder)?,
        }),
        19 => {
            let room_id = read_text(decoder)?;
            let expected_revision = read_u64(decoder)?;
            let logical_time = read_u64(decoder)?;
            let body = read_text(decoder)?;
            let length = read_list(decoder, limits)?;
            let mut addressed_to = try_vec(length)?;
            for _ in 0..length {
                addressed_to.push(read_text(decoder)?);
            }
            Ok(CommandV2::SpeakIdentified {
                room_id,
                expected_revision,
                logical_time,
                body,
                addressed_to,
            })
        }
        20 => Ok(CommandV2::WithdrawMessage {
            room_id: read_text(decoder)?,
            expected_revision: read_u64(decoder)?,
            message_id: decode_message_id(decoder)?,
        }),
        21 => {
            let room_id = read_text(decoder)?;
            let expected_revision = read_u64(decoder)?;
            let message_id = decode_message_id(decoder)?;
            let reason = read_text(decoder)?;
            let mut causation = [0; 16];
            for byte in &mut causation {
                *byte = read_u8(decoder)?;
            }
            Ok(CommandV2::RedactMessage {
                room_id,
                expected_revision,
                message_id,
                reason,
                causation,
            })
        }
        22 => Ok(CommandV2::OpenIdentifiedChanges {
            room_id: read_text(decoder)?,
            observed_revision: read_u64(decoder)?,
        }),
        23 => Ok(CommandV2::IdentifiedChanges {
            room_id: read_text(decoder)?,
            cursor: PageCursorV2(read_u64(decoder)?),
            limit: read_u32(decoder)?,
        }),
        24 => Ok(CommandV2::ThreadRoots {
            room_id: read_text(decoder)?,
            cursor: decode_optional_cursor(decoder)?,
            limit: read_u32(decoder)?,
        }),
        25 => Ok(CommandV2::ThreadReplies {
            room_id: read_text(decoder)?,
            root_message_id: decode_message_id(decoder)?,
            cursor: decode_optional_cursor(decoder)?,
            limit: read_u32(decoder)?,
        }),
        26 => {
            let room_id = read_text(decoder)?;
            let expected_revision = read_u64(decoder)?;
            let root_message_id = decode_message_id(decoder)?;
            let logical_time = read_u64(decoder)?;
            let body = read_text(decoder)?;
            let length = read_list(decoder, limits)?;
            let mut addressed_to = try_vec(length)?;
            for _ in 0..length {
                addressed_to.push(read_text(decoder)?);
            }
            Ok(CommandV2::SpeakReply {
                room_id,
                expected_revision,
                root_message_id,
                logical_time,
                body,
                addressed_to,
            })
        }
        27 => Ok(CommandV2::OpenThreadedChanges {
            room_id: read_text(decoder)?,
            observed_revision: read_u64(decoder)?,
        }),
        28 => Ok(CommandV2::ThreadedChanges {
            room_id: read_text(decoder)?,
            cursor: PageCursorV2(read_u64(decoder)?),
            limit: read_u32(decoder)?,
        }),
        tag @ 29..=30 => {
            let room_id = read_text(decoder)?;
            let expected_revision = read_u64(decoder)?;
            let root_message_id = decode_message_id(decoder)?;
            let reason = read_text(decoder)?;
            let causation = decode_causation(decoder)?;
            Ok(if tag == 29 {
                CommandV2::ArchiveThread {
                    room_id,
                    expected_revision,
                    root_message_id,
                    reason,
                    causation,
                }
            } else {
                CommandV2::ReopenThread {
                    room_id,
                    expected_revision,
                    root_message_id,
                    reason,
                    causation,
                }
            })
        }
        31 => Ok(CommandV2::ThreadStatus {
            room_id: read_text(decoder)?,
            root_message_id: decode_message_id(decoder)?,
        }),
        tag @ 32..=33 => {
            let room_id = read_text(decoder)?;
            let expected_revision = read_u64(decoder)?;
            let message_id = decode_message_id(decoder)?;
            let key = read_text(decoder)?;
            Ok(if tag == 32 {
                CommandV2::AddReaction {
                    room_id,
                    expected_revision,
                    message_id,
                    key,
                }
            } else {
                CommandV2::RemoveReaction {
                    room_id,
                    expected_revision,
                    message_id,
                    key,
                }
            })
        }
        34 => Ok(CommandV2::ReactionStatus {
            room_id: read_text(decoder)?,
            message_id: decode_message_id(decoder)?,
        }),
        35 => Ok(CommandV2::EditMessage {
            room_id: read_text(decoder)?,
            expected_revision: read_u64(decoder)?,
            message_id: decode_message_id(decoder)?,
            logical_time: read_u64(decoder)?,
            body: read_text(decoder)?,
        }),
        36 => Ok(CommandV2::MessageRevisions {
            room_id: read_text(decoder)?,
            message_id: decode_message_id(decoder)?,
            cursor: decode_optional_cursor(decoder)?,
            limit: read_u32(decoder)?,
        }),
        37 => Ok(CommandV2::Topics {
            room_id: read_text(decoder)?,
            cursor: decode_optional_cursor(decoder)?,
            limit: read_u32(decoder)?,
        }),
        38 => Ok(CommandV2::TopicMessages {
            room_id: read_text(decoder)?,
            topic_id: decode_topic_id(decoder)?,
            cursor: decode_optional_cursor(decoder)?,
            limit: read_u32(decoder)?,
        }),
        39 => Ok(CommandV2::CreateTopic {
            room_id: read_text(decoder)?,
            expected_revision: read_u64(decoder)?,
            name: read_text(decoder)?,
        }),
        40 => {
            let room_id = read_text(decoder)?;
            let expected_revision = read_u64(decoder)?;
            let topic_id = decode_topic_id(decoder)?;
            let logical_time = read_u64(decoder)?;
            let body = read_text(decoder)?;
            let length = read_list(decoder, limits)?;
            let mut addressed_to = try_vec(length)?;
            for _ in 0..length {
                addressed_to.push(read_text(decoder)?);
            }
            Ok(CommandV2::SpeakInTopic {
                room_id,
                expected_revision,
                topic_id,
                logical_time,
                body,
                addressed_to,
            })
        }
        41 => Ok(CommandV2::OpenTopicChanges {
            room_id: read_text(decoder)?,
            observed_revision: read_u64(decoder)?,
        }),
        42 => Ok(CommandV2::TopicChanges {
            room_id: read_text(decoder)?,
            cursor: PageCursorV2(read_u64(decoder)?),
            limit: read_u32(decoder)?,
        }),
        43 => Ok(CommandV2::ReadPosition {
            room_id: read_text(decoder)?,
            scope: decode_read_scope(decoder)?,
        }),
        44 => Ok(CommandV2::AdvanceReadPosition {
            room_id: read_text(decoder)?,
            observed_revision: read_u64(decoder)?,
            scope: decode_read_scope(decoder)?,
            target_message_id: decode_message_id(decoder)?,
        }),
        45 => Ok(CommandV2::RenameTopic {
            room_id: read_text(decoder)?,
            expected_revision: read_u64(decoder)?,
            topic_id: decode_topic_id(decoder)?,
            name: read_text(decoder)?,
        }),
        tag @ 46..=47 => Ok(if tag == 46 {
            CommandV2::ResolveTopic {
                room_id: read_text(decoder)?,
                expected_revision: read_u64(decoder)?,
                topic_id: decode_topic_id(decoder)?,
            }
        } else {
            CommandV2::ReopenTopic {
                room_id: read_text(decoder)?,
                expected_revision: read_u64(decoder)?,
                topic_id: decode_topic_id(decoder)?,
            }
        }),
        48 => Ok(CommandV2::TopicStatus {
            room_id: read_text(decoder)?,
            topic_id: decode_topic_id(decoder)?,
        }),
        49 => Ok(CommandV2::SupersedeTopic {
            room_id: read_text(decoder)?,
            expected_revision: read_u64(decoder)?,
            topic_id: decode_topic_id(decoder)?,
            successor_topic_id: decode_topic_id(decoder)?,
        }),
        50 => Ok(CommandV2::ClearTopicSupersession {
            room_id: read_text(decoder)?,
            expected_revision: read_u64(decoder)?,
            topic_id: decode_topic_id(decoder)?,
        }),
        51 => Ok(CommandV2::TopicSupersessionStatus {
            room_id: read_text(decoder)?,
            topic_id: decode_topic_id(decoder)?,
        }),
        tag @ (52 | 53) => Ok(if tag == 52 {
            CommandV2::AddTopicAlias {
                room_id: read_text(decoder)?,
                expected_revision: read_u64(decoder)?,
                topic_id: decode_topic_id(decoder)?,
                alias: read_text(decoder)?,
            }
        } else {
            CommandV2::RemoveTopicAlias {
                room_id: read_text(decoder)?,
                expected_revision: read_u64(decoder)?,
                topic_id: decode_topic_id(decoder)?,
                alias: read_text(decoder)?,
            }
        }),
        54 => Ok(CommandV2::TopicAliasStatus {
            room_id: read_text(decoder)?,
            topic_id: decode_topic_id(decoder)?,
        }),
        tag @ (55 | 56) => Ok(if tag == 55 {
            CommandV2::AddTopicTag {
                room_id: read_text(decoder)?,
                expected_revision: read_u64(decoder)?,
                topic_id: decode_topic_id(decoder)?,
                tag: read_text(decoder)?,
            }
        } else {
            CommandV2::RemoveTopicTag {
                room_id: read_text(decoder)?,
                expected_revision: read_u64(decoder)?,
                topic_id: decode_topic_id(decoder)?,
                tag: read_text(decoder)?,
            }
        }),
        57 => Ok(CommandV2::TopicTagStatus {
            room_id: read_text(decoder)?,
            topic_id: decode_topic_id(decoder)?,
        }),
        58 => Ok(CommandV2::AttachmentMessages {
            room_id: read_text(decoder)?,
            cursor: decode_optional_cursor(decoder)?,
            limit: read_u32(decoder)?,
        }),
        59 => {
            let room_id = read_text(decoder)?;
            let expected_revision = read_u64(decoder)?;
            let target = decode_attachment_target(decoder)?;
            let logical_time = read_u64(decoder)?;
            let body = read_text(decoder)?;
            let addressed_count = read_list(decoder, limits)?;
            let mut addressed_to = try_vec(addressed_count)?;
            for _ in 0..addressed_count {
                addressed_to.push(read_text(decoder)?);
            }
            let attachment_count = read_list(decoder, limits)?;
            if !(1..=8).contains(&attachment_count) {
                return Err(WireDecodeError::Limit);
            }
            let mut attachments = try_vec(attachment_count)?;
            for _ in 0..attachment_count {
                attachments.push(decode_attachment_descriptor(decoder)?);
            }
            if !valid_attachment_descriptors(&attachments) {
                return Err(WireDecodeError::Malformed);
            }
            Ok(CommandV2::SpeakWithAttachments {
                room_id,
                expected_revision,
                target,
                logical_time,
                body,
                addressed_to,
                attachments,
            })
        }
        60 => Ok(CommandV2::OpenAttachmentChanges {
            room_id: read_text(decoder)?,
            observed_revision: read_u64(decoder)?,
        }),
        61 => Ok(CommandV2::AttachmentChanges {
            room_id: read_text(decoder)?,
            cursor: PageCursorV2(read_u64(decoder)?),
            limit: read_u32(decoder)?,
        }),
        62 => Ok(CommandV2::VellumArtifactMessages {
            room_id: read_text(decoder)?,
            cursor: decode_optional_cursor(decoder)?,
            limit: read_u32(decoder)?,
        }),
        63 => {
            let room_id = read_text(decoder)?;
            let expected_revision = read_u64(decoder)?;
            let target = decode_vellum_artifact_target(decoder)?;
            let logical_time = read_u64(decoder)?;
            let body = read_text(decoder)?;
            let addressed_count = read_list(decoder, limits)?;
            let mut addressed_to = try_vec(addressed_count)?;
            for _ in 0..addressed_count {
                addressed_to.push(read_text(decoder)?);
            }
            let reference_count = read_list(decoder, limits)?;
            if !(1..=8).contains(&reference_count) {
                return Err(WireDecodeError::Limit);
            }
            let mut references = try_vec(reference_count)?;
            for _ in 0..reference_count {
                references.push(decode_vellum_artifact_reference(decoder)?);
            }
            if !valid_vellum_artifact_references(&references) {
                return Err(WireDecodeError::Malformed);
            }
            Ok(CommandV2::SpeakWithVellumArtifacts {
                room_id,
                expected_revision,
                target,
                logical_time,
                body,
                addressed_to,
                references,
            })
        }
        64 => {
            let room_id = read_text(decoder)?;
            let name = read_text(decoder)?;
            let profile = decode_conversation_profile(decoder)?;
            if profile == ConversationProfileV1::Ordinary {
                return Err(WireDecodeError::Malformed);
            }
            let count = read_list(decoder, limits)?;
            let mut founding_participant_ids = try_vec(count)?;
            for _ in 0..count {
                let participant_id = read_text(decoder)?;
                if founding_participant_ids
                    .last()
                    .is_some_and(|previous| previous >= &participant_id)
                {
                    return Err(WireDecodeError::Malformed);
                }
                founding_participant_ids.push(participant_id);
            }
            Ok(CommandV2::CreatePeerConversation {
                room_id,
                name,
                profile,
                founding_participant_ids,
                expected_revision: read_u64(decoder)?,
            })
        }
        65 => Ok(CommandV2::ConversationProfile {
            room_id: read_text(decoder)?,
        }),
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
                    CapabilityV2::IdentifiedMessagesV1 => 7,
                    CapabilityV2::AuthorWithdrawalV1 => 8,
                    CapabilityV2::ModeratorRedactionV1 => 9,
                    CapabilityV2::IdentifiedRoomChangesV1 => 10,
                    CapabilityV2::ThreadsV1 => 11,
                    CapabilityV2::ThreadLifecycleV1 => 12,
                    CapabilityV2::ReactionsV1 => 13,
                    CapabilityV2::MessageRevisionsV1 => 14,
                    CapabilityV2::TopicsV1 => 15,
                    CapabilityV2::PrivateReadPositionsV1 => 16,
                    CapabilityV2::IndependentChangeRetentionV1 => 17,
                    CapabilityV2::TopicRenameV1 => 18,
                    CapabilityV2::TopicResolutionV1 => 19,
                    CapabilityV2::TopicSupersessionV1 => 20,
                    CapabilityV2::TopicAliasesV1 => 21,
                    CapabilityV2::TopicTagsV1 => 22,
                    CapabilityV2::ImmutableAttachmentReferencesV1 => 23,
                    CapabilityV2::VellumArtifactReferencesV1 => 24,
                    CapabilityV2::ConversationProfilesV1 => 25,
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
        OutcomeV2::IdentifiedMessages(page) => {
            sink.u8(19)?;
            encode_identified_message_page(sink, page)
        }
        OutcomeV2::AttachmentMessages(page) => {
            if !valid_attachment_message_page(page) {
                return Err(WireEncodeError::Invalid);
            }
            sink.u8(56)?;
            encode_attachment_message_page(sink, page)
        }
        OutcomeV2::VellumArtifactMessages(page) => {
            if !valid_vellum_artifact_message_page(page) {
                return Err(WireEncodeError::Invalid);
            }
            sink.u8(59)?;
            encode_vellum_artifact_message_page(sink, page)
        }
        OutcomeV2::AttachmentChanges(page) => {
            if !valid_attachment_change_page(page) {
                return Err(WireEncodeError::Invalid);
            }
            sink.u8(58)?;
            encode_attachment_change_page(sink, page)
        }
        OutcomeV2::ThreadRoots(page) => {
            if !valid_thread_message_page(page, None) {
                return Err(WireEncodeError::Invalid);
            }
            sink.u8(24)?;
            encode_thread_message_page(sink, page)
        }
        OutcomeV2::ThreadReplies(page) => {
            let Some(root_message_id) = page.root_message_id else {
                return Err(WireEncodeError::Invalid);
            };
            if !valid_thread_message_page(page, Some(root_message_id)) {
                return Err(WireEncodeError::Invalid);
            }
            sink.u8(25)?;
            encode_thread_message_page(sink, page)
        }
        OutcomeV2::ThreadStatus(status) => {
            sink.u8(30)?;
            sink.u64(status.revision)?;
            sink.text(&status.room_id)?;
            encode_message_id(sink, status.root_message_id)?;
            sink.u8(match status.state {
                ThreadLifecycleStateV1::Active => 1,
                ThreadLifecycleStateV1::Archived => 2,
            })
        }
        OutcomeV2::ReactionStatus(status) => {
            if status
                .reactions
                .windows(2)
                .any(|pair| pair[0].key >= pair[1].key)
                || status
                    .reactions
                    .iter()
                    .any(|reaction| reaction.key.is_empty() || reaction.count == 0)
            {
                return Err(WireEncodeError::Invalid);
            }
            sink.u8(31)?;
            sink.u64(status.revision)?;
            sink.text(&status.room_id)?;
            encode_message_id(sink, status.message_id)?;
            sink.list(status.reactions.len())?;
            for reaction in &status.reactions {
                sink.text(&reaction.key)?;
                sink.u64(u64::from(reaction.count))?;
                sink.u8(u8::from(reaction.caller_reacted))?;
            }
            Ok(())
        }
        OutcomeV2::MessageRevisions(page) => {
            if page
                .revisions
                .windows(2)
                .any(|pair| pair[0].number.checked_add(1) != Some(pair[1].number))
                || page.revisions.iter().any(|revision| {
                    revision.editor_id.is_empty()
                        || (matches!(
                            revision.availability,
                            MessageRevisionAvailabilityV1::Available
                        ) == revision.body.is_empty())
                })
            {
                return Err(WireEncodeError::Invalid);
            }
            sink.u8(34)?;
            sink.u64(page.revision)?;
            sink.text(&page.room_id)?;
            encode_message_id(sink, page.message_id)?;
            sink.list(page.revisions.len())?;
            for revision in &page.revisions {
                sink.u64(u64::from(revision.number))?;
                sink.text(&revision.editor_id)?;
                sink.u64(revision.logical_time)?;
                sink.u8(match revision.availability {
                    MessageRevisionAvailabilityV1::Available => 1,
                    MessageRevisionAvailabilityV1::Tombstoned => 2,
                })?;
                sink.text(&revision.body)?;
            }
            encode_optional_cursor(sink, page.next)
        }
        OutcomeV2::Topics(page) => {
            if page
                .topics
                .windows(2)
                .any(|pair| pair[0].topic_id >= pair[1].topic_id)
                || page
                    .topics
                    .iter()
                    .any(|topic| topic.name.is_empty() || topic.creator_id.is_empty())
            {
                return Err(WireEncodeError::Invalid);
            }
            sink.u8(36)?;
            encode_topic_page(sink, page)
        }
        OutcomeV2::TopicMessages(page) => {
            if page
                .messages
                .iter()
                .any(|message| message.topic_id != Some(page.topic_id))
            {
                return Err(WireEncodeError::Invalid);
            }
            sink.u8(37)?;
            encode_topic_message_page(sink, page)
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
        OutcomeV2::IdentifiedChanges(page) => {
            sink.u8(23)?;
            encode_identified_change_page(sink, page)
        }
        OutcomeV2::ThreadedChanges(page) => {
            sink.u8(27)?;
            encode_threaded_change_page(sink, page)
        }
        OutcomeV2::TopicChanges(page) => {
            sink.u8(40)?;
            encode_topic_change_page(sink, page)
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
                ChangeGapV2::ChangeRetention { discarded_through } => {
                    sink.u8(3)?;
                    sink.u64(*discarded_through)
                }
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
        OutcomeV2::IdentifiedMessageAccepted {
            revision,
            message_id,
            sequence,
        } => {
            sink.u8(20)?;
            sink.u64(*revision)?;
            encode_message_id(sink, *message_id)?;
            sink.u64(*sequence)
        }
        OutcomeV2::AttachmentMessageAccepted {
            revision,
            message_id,
            sequence,
            attachment_count,
        } => {
            if !(1..=8).contains(attachment_count) {
                return Err(WireEncodeError::Invalid);
            }
            sink.u8(57)?;
            sink.u64(*revision)?;
            encode_message_id(sink, *message_id)?;
            sink.u64(*sequence)?;
            sink.u64(u64::from(*attachment_count))
        }
        OutcomeV2::VellumArtifactMessageAccepted {
            revision,
            message_id,
            sequence,
            reference_count,
        } => {
            if !(1..=8).contains(reference_count) {
                return Err(WireEncodeError::Invalid);
            }
            sink.u8(60)?;
            sink.u64(*revision)?;
            encode_message_id(sink, *message_id)?;
            sink.u64(*sequence)?;
            sink.u64(u64::from(*reference_count))
        }
        OutcomeV2::ReplyAccepted {
            revision,
            message_id,
            root_message_id,
            sequence,
        } => {
            sink.u8(26)?;
            sink.u64(*revision)?;
            encode_message_id(sink, *message_id)?;
            encode_message_id(sink, *root_message_id)?;
            sink.u64(*sequence)
        }
        OutcomeV2::MessageWithdrawn {
            revision,
            message_id,
            sequence,
        } => {
            sink.u8(21)?;
            sink.u64(*revision)?;
            encode_message_id(sink, *message_id)?;
            sink.u64(*sequence)
        }
        OutcomeV2::MessageRedacted {
            revision,
            message_id,
            sequence,
        } => {
            sink.u8(22)?;
            sink.u64(*revision)?;
            encode_message_id(sink, *message_id)?;
            sink.u64(*sequence)
        }
        OutcomeV2::ThreadArchived {
            revision,
            root_message_id,
        }
        | OutcomeV2::ThreadReopened {
            revision,
            root_message_id,
        } => {
            sink.u8(if matches!(outcome, OutcomeV2::ThreadArchived { .. }) {
                28
            } else {
                29
            })?;
            sink.u64(*revision)?;
            encode_message_id(sink, *root_message_id)
        }
        OutcomeV2::ReactionAdded {
            revision,
            message_id,
        }
        | OutcomeV2::ReactionRemoved {
            revision,
            message_id,
        } => {
            sink.u8(if matches!(outcome, OutcomeV2::ReactionAdded { .. }) {
                32
            } else {
                33
            })?;
            sink.u64(*revision)?;
            encode_message_id(sink, *message_id)
        }
        OutcomeV2::MessageEdited {
            revision,
            message_id,
            message_revision,
        } => {
            sink.u8(35)?;
            sink.u64(*revision)?;
            encode_message_id(sink, *message_id)?;
            sink.u64(u64::from(*message_revision))
        }
        OutcomeV2::TopicCreated { revision, topic_id } => {
            sink.u8(38)?;
            sink.u64(*revision)?;
            encode_topic_id(sink, *topic_id)
        }
        OutcomeV2::TopicRenamed { revision, topic_id } => {
            sink.u8(43)?;
            sink.u64(*revision)?;
            encode_topic_id(sink, *topic_id)
        }
        OutcomeV2::TopicResolved { revision, topic_id }
        | OutcomeV2::TopicReopened { revision, topic_id } => {
            sink.u8(if matches!(outcome, OutcomeV2::TopicResolved { .. }) {
                44
            } else {
                45
            })?;
            sink.u64(*revision)?;
            encode_topic_id(sink, *topic_id)
        }
        OutcomeV2::TopicStatus(status) => {
            sink.u8(46)?;
            sink.u64(status.revision)?;
            sink.text(&status.room_id)?;
            encode_topic_id(sink, status.topic_id)?;
            sink.u8(u8::from(status.resolved))
        }
        OutcomeV2::TopicSuperseded {
            revision,
            topic_id,
            successor_topic_id,
        } => {
            sink.u8(47)?;
            sink.u64(*revision)?;
            encode_topic_id(sink, *topic_id)?;
            encode_topic_id(sink, *successor_topic_id)
        }
        OutcomeV2::TopicSupersessionCleared { revision, topic_id } => {
            sink.u8(48)?;
            sink.u64(*revision)?;
            encode_topic_id(sink, *topic_id)
        }
        OutcomeV2::TopicSupersessionStatus(status) => {
            sink.u8(49)?;
            sink.u64(status.revision)?;
            sink.text(&status.room_id)?;
            encode_topic_id(sink, status.topic_id)?;
            match status.successor_topic_id {
                None => sink.u8(0),
                Some(successor_topic_id) => {
                    sink.u8(1)?;
                    encode_topic_id(sink, successor_topic_id)
                }
            }
        }
        OutcomeV2::TopicAliasAdded {
            revision,
            topic_id,
            alias,
        }
        | OutcomeV2::TopicAliasRemoved {
            revision,
            topic_id,
            alias,
        } => {
            sink.u8(if matches!(outcome, OutcomeV2::TopicAliasAdded { .. }) {
                50
            } else {
                51
            })?;
            sink.u64(*revision)?;
            encode_topic_id(sink, *topic_id)?;
            sink.text(alias)
        }
        OutcomeV2::TopicAliasStatus(status) => {
            sink.u8(52)?;
            sink.u64(status.revision)?;
            sink.text(&status.room_id)?;
            encode_topic_id(sink, status.topic_id)?;
            sink.list(status.aliases.len())?;
            for alias in &status.aliases {
                sink.text(alias)?;
            }
            Ok(())
        }
        OutcomeV2::TopicTagAdded {
            revision,
            topic_id,
            tag,
        }
        | OutcomeV2::TopicTagRemoved {
            revision,
            topic_id,
            tag,
        } => {
            sink.u8(if matches!(outcome, OutcomeV2::TopicTagAdded { .. }) {
                53
            } else {
                54
            })?;
            sink.u64(*revision)?;
            encode_topic_id(sink, *topic_id)?;
            sink.text(tag)
        }
        OutcomeV2::TopicTagStatus(status) => {
            sink.u8(55)?;
            sink.u64(status.revision)?;
            sink.text(&status.room_id)?;
            encode_topic_id(sink, status.topic_id)?;
            sink.list(status.tags.len())?;
            for tag in &status.tags {
                sink.text(tag)?;
            }
            Ok(())
        }
        OutcomeV2::TopicMessageAccepted {
            revision,
            message_id,
            topic_id,
            sequence,
        } => {
            sink.u8(39)?;
            sink.u64(*revision)?;
            encode_message_id(sink, *message_id)?;
            encode_topic_id(sink, *topic_id)?;
            sink.u64(*sequence)
        }
        OutcomeV2::ReadPosition(position) => {
            sink.u8(41)?;
            match position {
                None => sink.u8(0),
                Some(position) => {
                    sink.u8(1)?;
                    encode_read_position(sink, position)
                }
            }
        }
        OutcomeV2::ReadPositionAdvanced(position) => {
            sink.u8(42)?;
            encode_read_position(sink, position)
        }
        OutcomeV2::Kicked { revision } => encode_revision_outcome(sink, 13, *revision),
        OutcomeV2::Banned { revision } => encode_revision_outcome(sink, 14, *revision),
        OutcomeV2::Unbanned { revision } => encode_revision_outcome(sink, 15, *revision),
        OutcomeV2::RoomPolicyChanged { revision } => encode_revision_outcome(sink, 16, *revision),
        OutcomeV2::PeerConversationCreated { revision, profile } => {
            sink.u8(61)?;
            sink.u64(*revision)?;
            sink.u8(encode_conversation_profile(*profile))
        }
        OutcomeV2::ConversationProfile(status) => {
            if status.founding_participant_ids.is_empty()
                || status
                    .founding_participant_ids
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
            {
                return Err(WireEncodeError::Invalid);
            }
            sink.u8(62)?;
            sink.u64(status.revision)?;
            sink.text(&status.room_id)?;
            sink.u8(encode_conversation_profile(status.profile))?;
            sink.list(status.founding_participant_ids.len())?;
            for participant_id in &status.founding_participant_ids {
                sink.text(participant_id)?;
            }
            Ok(())
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
                3 => ChangeGapV2::ChangeRetention {
                    discarded_through: read_u64(decoder)?,
                },
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
                    7 => CapabilityV2::IdentifiedMessagesV1,
                    8 => CapabilityV2::AuthorWithdrawalV1,
                    9 => CapabilityV2::ModeratorRedactionV1,
                    10 => CapabilityV2::IdentifiedRoomChangesV1,
                    11 => CapabilityV2::ThreadsV1,
                    12 => CapabilityV2::ThreadLifecycleV1,
                    13 => CapabilityV2::ReactionsV1,
                    14 => CapabilityV2::MessageRevisionsV1,
                    15 => CapabilityV2::TopicsV1,
                    16 => CapabilityV2::PrivateReadPositionsV1,
                    17 => CapabilityV2::IndependentChangeRetentionV1,
                    18 => CapabilityV2::TopicRenameV1,
                    19 => CapabilityV2::TopicResolutionV1,
                    20 => CapabilityV2::TopicSupersessionV1,
                    21 => CapabilityV2::TopicAliasesV1,
                    22 => CapabilityV2::TopicTagsV1,
                    23 => CapabilityV2::ImmutableAttachmentReferencesV1,
                    24 => CapabilityV2::VellumArtifactReferencesV1,
                    25 => CapabilityV2::ConversationProfilesV1,
                    _ => return Err(WireDecodeError::UnknownTag),
                });
            }
            if capabilities.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(WireDecodeError::Malformed);
            }
            Ok(OutcomeV2::Capabilities(capabilities))
        }
        19 => Ok(OutcomeV2::IdentifiedMessages(
            decode_identified_message_page(decoder, limits)?,
        )),
        20 => Ok(OutcomeV2::IdentifiedMessageAccepted {
            revision: read_u64(decoder)?,
            message_id: decode_message_id(decoder)?,
            sequence: read_u64(decoder)?,
        }),
        21 => Ok(OutcomeV2::MessageWithdrawn {
            revision: read_u64(decoder)?,
            message_id: decode_message_id(decoder)?,
            sequence: read_u64(decoder)?,
        }),
        22 => Ok(OutcomeV2::MessageRedacted {
            revision: read_u64(decoder)?,
            message_id: decode_message_id(decoder)?,
            sequence: read_u64(decoder)?,
        }),
        23 => Ok(OutcomeV2::IdentifiedChanges(decode_identified_change_page(
            decoder, limits,
        )?)),
        24 => {
            let page = decode_thread_message_page(decoder, limits)?;
            if !valid_thread_message_page(&page, None) {
                return Err(WireDecodeError::Malformed);
            }
            Ok(OutcomeV2::ThreadRoots(page))
        }
        25 => {
            let page = decode_thread_message_page(decoder, limits)?;
            let Some(root_message_id) = page.root_message_id else {
                return Err(WireDecodeError::Malformed);
            };
            if !valid_thread_message_page(&page, Some(root_message_id)) {
                return Err(WireDecodeError::Malformed);
            }
            Ok(OutcomeV2::ThreadReplies(page))
        }
        26 => Ok(OutcomeV2::ReplyAccepted {
            revision: read_u64(decoder)?,
            message_id: decode_message_id(decoder)?,
            root_message_id: decode_message_id(decoder)?,
            sequence: read_u64(decoder)?,
        }),
        27 => Ok(OutcomeV2::ThreadedChanges(decode_threaded_change_page(
            decoder, limits,
        )?)),
        28 => Ok(OutcomeV2::ThreadArchived {
            revision: read_u64(decoder)?,
            root_message_id: decode_message_id(decoder)?,
        }),
        29 => Ok(OutcomeV2::ThreadReopened {
            revision: read_u64(decoder)?,
            root_message_id: decode_message_id(decoder)?,
        }),
        30 => Ok(OutcomeV2::ThreadStatus(ThreadStatusV1 {
            revision: read_u64(decoder)?,
            room_id: read_text(decoder)?,
            root_message_id: decode_message_id(decoder)?,
            state: match read_u8(decoder)? {
                1 => ThreadLifecycleStateV1::Active,
                2 => ThreadLifecycleStateV1::Archived,
                _ => return Err(WireDecodeError::UnknownTag),
            },
        })),
        31 => {
            let revision = read_u64(decoder)?;
            let room_id = read_text(decoder)?;
            let message_id = decode_message_id(decoder)?;
            let length = read_list(decoder, limits)?;
            let mut reactions = try_vec(length)?;
            for _ in 0..length {
                reactions.push(ReactionSummaryV1 {
                    key: read_text(decoder)?,
                    count: read_u32(decoder)?,
                    caller_reacted: match read_u8(decoder)? {
                        0 => false,
                        1 => true,
                        _ => return Err(WireDecodeError::UnknownTag),
                    },
                });
            }
            if reactions.windows(2).any(|pair| pair[0].key >= pair[1].key)
                || reactions
                    .iter()
                    .any(|reaction| reaction.key.is_empty() || reaction.count == 0)
            {
                return Err(WireDecodeError::Malformed);
            }
            Ok(OutcomeV2::ReactionStatus(ReactionStatusV1 {
                revision,
                room_id,
                message_id,
                reactions,
            }))
        }
        32 => Ok(OutcomeV2::ReactionAdded {
            revision: read_u64(decoder)?,
            message_id: decode_message_id(decoder)?,
        }),
        33 => Ok(OutcomeV2::ReactionRemoved {
            revision: read_u64(decoder)?,
            message_id: decode_message_id(decoder)?,
        }),
        34 => {
            let revision = read_u64(decoder)?;
            let room_id = read_text(decoder)?;
            let message_id = decode_message_id(decoder)?;
            let length = read_list(decoder, limits)?;
            let mut revisions = try_vec(length)?;
            for _ in 0..length {
                let number = read_u32(decoder)?;
                let editor_id = read_text(decoder)?;
                let logical_time = read_u64(decoder)?;
                let availability = match read_u8(decoder)? {
                    1 => MessageRevisionAvailabilityV1::Available,
                    2 => MessageRevisionAvailabilityV1::Tombstoned,
                    _ => return Err(WireDecodeError::UnknownTag),
                };
                let body = read_text(decoder)?;
                if editor_id.is_empty()
                    || matches!(availability, MessageRevisionAvailabilityV1::Available)
                        == body.is_empty()
                {
                    return Err(WireDecodeError::Malformed);
                }
                revisions.push(MessageRevisionV1 {
                    number,
                    editor_id,
                    logical_time,
                    availability,
                    body,
                });
            }
            if revisions
                .windows(2)
                .any(|pair| pair[0].number.checked_add(1) != Some(pair[1].number))
            {
                return Err(WireDecodeError::Malformed);
            }
            Ok(OutcomeV2::MessageRevisions(MessageRevisionPageV1 {
                revision,
                room_id,
                message_id,
                revisions,
                next: decode_optional_cursor(decoder)?,
            }))
        }
        35 => Ok(OutcomeV2::MessageEdited {
            revision: read_u64(decoder)?,
            message_id: decode_message_id(decoder)?,
            message_revision: read_u32(decoder)?,
        }),
        36 => {
            let page = decode_topic_page(decoder, limits)?;
            if page
                .topics
                .windows(2)
                .any(|pair| pair[0].topic_id >= pair[1].topic_id)
                || page
                    .topics
                    .iter()
                    .any(|topic| topic.name.is_empty() || topic.creator_id.is_empty())
            {
                return Err(WireDecodeError::Malformed);
            }
            Ok(OutcomeV2::Topics(page))
        }
        37 => {
            let page = decode_topic_message_page(decoder, limits)?;
            if page
                .messages
                .iter()
                .any(|message| message.topic_id != Some(page.topic_id))
            {
                return Err(WireDecodeError::Malformed);
            }
            Ok(OutcomeV2::TopicMessages(page))
        }
        38 => Ok(OutcomeV2::TopicCreated {
            revision: read_u64(decoder)?,
            topic_id: decode_topic_id(decoder)?,
        }),
        39 => Ok(OutcomeV2::TopicMessageAccepted {
            revision: read_u64(decoder)?,
            message_id: decode_message_id(decoder)?,
            topic_id: decode_topic_id(decoder)?,
            sequence: read_u64(decoder)?,
        }),
        40 => Ok(OutcomeV2::TopicChanges(decode_topic_change_page(
            decoder, limits,
        )?)),
        41 => Ok(OutcomeV2::ReadPosition(match read_u8(decoder)? {
            0 => None,
            1 => Some(decode_read_position(decoder)?),
            _ => return Err(WireDecodeError::UnknownTag),
        })),
        42 => Ok(OutcomeV2::ReadPositionAdvanced(decode_read_position(
            decoder,
        )?)),
        43 => Ok(OutcomeV2::TopicRenamed {
            revision: read_u64(decoder)?,
            topic_id: decode_topic_id(decoder)?,
        }),
        tag @ 44..=45 => Ok(if tag == 44 {
            OutcomeV2::TopicResolved {
                revision: read_u64(decoder)?,
                topic_id: decode_topic_id(decoder)?,
            }
        } else {
            OutcomeV2::TopicReopened {
                revision: read_u64(decoder)?,
                topic_id: decode_topic_id(decoder)?,
            }
        }),
        46 => Ok(OutcomeV2::TopicStatus(TopicStatusV1 {
            revision: read_u64(decoder)?,
            room_id: read_text(decoder)?,
            topic_id: decode_topic_id(decoder)?,
            resolved: match read_u8(decoder)? {
                0 => false,
                1 => true,
                _ => return Err(WireDecodeError::UnknownTag),
            },
        })),
        47 => Ok(OutcomeV2::TopicSuperseded {
            revision: read_u64(decoder)?,
            topic_id: decode_topic_id(decoder)?,
            successor_topic_id: decode_topic_id(decoder)?,
        }),
        48 => Ok(OutcomeV2::TopicSupersessionCleared {
            revision: read_u64(decoder)?,
            topic_id: decode_topic_id(decoder)?,
        }),
        49 => Ok(OutcomeV2::TopicSupersessionStatus(
            TopicSupersessionStatusV1 {
                revision: read_u64(decoder)?,
                room_id: read_text(decoder)?,
                topic_id: decode_topic_id(decoder)?,
                successor_topic_id: match read_u8(decoder)? {
                    0 => None,
                    1 => Some(decode_topic_id(decoder)?),
                    _ => return Err(WireDecodeError::UnknownTag),
                },
            },
        )),
        tag @ (50 | 51) => Ok(if tag == 50 {
            OutcomeV2::TopicAliasAdded {
                revision: read_u64(decoder)?,
                topic_id: decode_topic_id(decoder)?,
                alias: read_text(decoder)?,
            }
        } else {
            OutcomeV2::TopicAliasRemoved {
                revision: read_u64(decoder)?,
                topic_id: decode_topic_id(decoder)?,
                alias: read_text(decoder)?,
            }
        }),
        52 => {
            let revision = read_u64(decoder)?;
            let room_id = read_text(decoder)?;
            let topic_id = decode_topic_id(decoder)?;
            let length = read_list(decoder, limits)?;
            if length > 16 {
                return Err(WireDecodeError::Limit);
            }
            let mut aliases = try_vec(length)?;
            for _ in 0..length {
                aliases.push(read_text(decoder)?);
            }
            Ok(OutcomeV2::TopicAliasStatus(TopicAliasStatusV1 {
                revision,
                room_id,
                topic_id,
                aliases,
            }))
        }
        tag @ (53 | 54) => Ok(if tag == 53 {
            OutcomeV2::TopicTagAdded {
                revision: read_u64(decoder)?,
                topic_id: decode_topic_id(decoder)?,
                tag: read_text(decoder)?,
            }
        } else {
            OutcomeV2::TopicTagRemoved {
                revision: read_u64(decoder)?,
                topic_id: decode_topic_id(decoder)?,
                tag: read_text(decoder)?,
            }
        }),
        55 => {
            let revision = read_u64(decoder)?;
            let room_id = read_text(decoder)?;
            let topic_id = decode_topic_id(decoder)?;
            let length = read_list(decoder, limits)?;
            if length > 8 {
                return Err(WireDecodeError::Limit);
            }
            let mut tags = try_vec(length)?;
            for _ in 0..length {
                tags.push(read_text(decoder)?);
            }
            Ok(OutcomeV2::TopicTagStatus(TopicTagStatusV1 {
                revision,
                room_id,
                topic_id,
                tags,
            }))
        }
        56 => {
            let page = decode_attachment_message_page(decoder, limits)?;
            if !valid_attachment_message_page(&page) {
                return Err(WireDecodeError::Malformed);
            }
            Ok(OutcomeV2::AttachmentMessages(page))
        }
        57 => {
            let revision = read_u64(decoder)?;
            let message_id = decode_message_id(decoder)?;
            let sequence = read_u64(decoder)?;
            let attachment_count = read_u32(decoder)?;
            if !(1..=8).contains(&attachment_count) {
                return Err(WireDecodeError::Malformed);
            }
            Ok(OutcomeV2::AttachmentMessageAccepted {
                revision,
                message_id,
                sequence,
                attachment_count,
            })
        }
        58 => {
            let page = decode_attachment_change_page(decoder, limits)?;
            if !valid_attachment_change_page(&page) {
                return Err(WireDecodeError::Malformed);
            }
            Ok(OutcomeV2::AttachmentChanges(page))
        }
        59 => {
            let page = decode_vellum_artifact_message_page(decoder, limits)?;
            if !valid_vellum_artifact_message_page(&page) {
                return Err(WireDecodeError::Malformed);
            }
            Ok(OutcomeV2::VellumArtifactMessages(page))
        }
        60 => {
            let revision = read_u64(decoder)?;
            let message_id = decode_message_id(decoder)?;
            let sequence = read_u64(decoder)?;
            let reference_count = read_u32(decoder)?;
            if !(1..=8).contains(&reference_count) {
                return Err(WireDecodeError::Malformed);
            }
            Ok(OutcomeV2::VellumArtifactMessageAccepted {
                revision,
                message_id,
                sequence,
                reference_count,
            })
        }
        61 => Ok(OutcomeV2::PeerConversationCreated {
            revision: read_u64(decoder)?,
            profile: decode_conversation_profile(decoder)?,
        }),
        62 => {
            let revision = read_u64(decoder)?;
            let room_id = read_text(decoder)?;
            let profile = decode_conversation_profile(decoder)?;
            let count = read_list(decoder, limits)?;
            let mut founding_participant_ids = try_vec(count)?;
            for _ in 0..count {
                let participant_id = read_text(decoder)?;
                if founding_participant_ids
                    .last()
                    .is_some_and(|previous| previous >= &participant_id)
                {
                    return Err(WireDecodeError::Malformed);
                }
                founding_participant_ids.push(participant_id);
            }
            if founding_participant_ids.is_empty() {
                return Err(WireDecodeError::Malformed);
            }
            Ok(OutcomeV2::ConversationProfile(
                ConversationProfileStatusV1 {
                    revision,
                    room_id,
                    profile,
                    founding_participant_ids,
                },
            ))
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

fn encode_message_id(sink: &mut dyn Sink, message_id: MessageIdV1) -> Result<(), WireEncodeError> {
    for byte in message_id.into_bytes() {
        sink.u8(byte)?;
    }
    Ok(())
}

fn decode_message_id(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<MessageIdV1, WireDecodeError> {
    let mut bytes = [0; 16];
    for byte in &mut bytes {
        *byte = read_u8(decoder)?;
    }
    Ok(MessageIdV1::new(bytes))
}

fn encode_topic_id(sink: &mut dyn Sink, topic_id: TopicIdV1) -> Result<(), WireEncodeError> {
    for byte in topic_id.into_bytes() {
        sink.u8(byte)?;
    }
    Ok(())
}

fn decode_topic_id(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<TopicIdV1, WireDecodeError> {
    let mut bytes = [0; 16];
    for byte in &mut bytes {
        *byte = read_u8(decoder)?;
    }
    Ok(TopicIdV1::new(bytes))
}

fn encode_attachment_target(
    sink: &mut dyn Sink,
    target: AttachmentMessageTargetV1,
) -> Result<(), WireEncodeError> {
    match target {
        AttachmentMessageTargetV1::Room => sink.u8(1),
        AttachmentMessageTargetV1::Reply(message_id) => {
            sink.u8(2)?;
            encode_message_id(sink, message_id)
        }
        AttachmentMessageTargetV1::Topic(topic_id) => {
            sink.u8(3)?;
            encode_topic_id(sink, topic_id)
        }
    }
}

fn decode_attachment_target(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<AttachmentMessageTargetV1, WireDecodeError> {
    match read_u8(decoder)? {
        1 => Ok(AttachmentMessageTargetV1::Room),
        2 => Ok(AttachmentMessageTargetV1::Reply(decode_message_id(
            decoder,
        )?)),
        3 => Ok(AttachmentMessageTargetV1::Topic(decode_topic_id(decoder)?)),
        _ => Err(WireDecodeError::UnknownTag),
    }
}

fn encode_attachment_descriptor(
    sink: &mut dyn Sink,
    descriptor: &AttachmentDescriptorV1,
) -> Result<(), WireEncodeError> {
    for byte in descriptor.attachment_id.into_bytes() {
        sink.u8(byte)?;
    }
    for byte in descriptor.store_id.into_bytes() {
        sink.u8(byte)?;
    }
    for byte in descriptor.object_key.into_bytes() {
        sink.u8(byte)?;
    }
    sink.u64(descriptor.declared_bytes)?;
    sink.text(&descriptor.media_type)?;
    sink.text(&descriptor.display_label)?;
    match descriptor.integrity {
        AttachmentIntegrityV1::Sha256(digest) => {
            sink.u8(1)?;
            for byte in digest.into_bytes() {
                sink.u8(byte)?;
            }
        }
    }
    Ok(())
}

fn decode_attachment_descriptor(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<AttachmentDescriptorV1, WireDecodeError> {
    let mut attachment_id = [0; 16];
    for byte in &mut attachment_id {
        *byte = read_u8(decoder)?;
    }
    let mut store_id = [0; 16];
    for byte in &mut store_id {
        *byte = read_u8(decoder)?;
    }
    let mut object_key = [0; 32];
    for byte in &mut object_key {
        *byte = read_u8(decoder)?;
    }
    let declared_bytes = read_u64(decoder)?;
    let media_type = read_text(decoder)?;
    let display_label = read_text(decoder)?;
    let integrity = match read_u8(decoder)? {
        1 => {
            let mut digest = [0; 32];
            for byte in &mut digest {
                *byte = read_u8(decoder)?;
            }
            AttachmentIntegrityV1::Sha256(Sha256DigestV1::new(digest))
        }
        _ => return Err(WireDecodeError::UnknownTag),
    };
    Ok(AttachmentDescriptorV1 {
        attachment_id: AttachmentIdV1::new(attachment_id),
        store_id: AttachmentStoreIdV1::new(store_id),
        object_key: AttachmentObjectKeyV1::new(object_key),
        declared_bytes,
        media_type,
        display_label,
        integrity,
    })
}

fn valid_attachment_descriptors(descriptors: &[AttachmentDescriptorV1]) -> bool {
    let mut attachment_ids = BTreeSet::new();
    let mut object_refs = BTreeSet::new();
    (1..=8).contains(&descriptors.len())
        && descriptors.iter().all(|descriptor| {
            descriptor.declared_bytes > 0
                && canonical_media_type(&descriptor.media_type)
                && !descriptor.display_label.is_empty()
                && !descriptor.display_label.chars().any(char::is_control)
                && attachment_ids.insert(descriptor.attachment_id)
                && object_refs.insert((descriptor.store_id, descriptor.object_key))
        })
}

fn canonical_media_type(value: &str) -> bool {
    let Some((top, subtype)) = value.split_once('/') else {
        return false;
    };
    let token = |byte: u8| {
        byte.is_ascii_lowercase()
            || byte.is_ascii_digit()
            || matches!(
                byte,
                b'!' | b'#'
                    | b'$'
                    | b'%'
                    | b'&'
                    | b'\''
                    | b'*'
                    | b'+'
                    | b'-'
                    | b'.'
                    | b'^'
                    | b'_'
                    | b'`'
                    | b'|'
                    | b'~'
            )
    };
    !top.is_empty()
        && !subtype.is_empty()
        && !subtype.contains('/')
        && top.bytes().all(token)
        && subtype.bytes().all(token)
}

fn encode_read_scope(sink: &mut dyn Sink, scope: ReadScopeV1) -> Result<(), WireEncodeError> {
    match scope {
        ReadScopeV1::Room => sink.u8(1),
        ReadScopeV1::Thread(root_message_id) => {
            sink.u8(2)?;
            encode_message_id(sink, root_message_id)
        }
        ReadScopeV1::Topic(topic_id) => {
            sink.u8(3)?;
            encode_topic_id(sink, topic_id)
        }
    }
}

fn decode_read_scope(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<ReadScopeV1, WireDecodeError> {
    match read_u8(decoder)? {
        1 => Ok(ReadScopeV1::Room),
        2 => Ok(ReadScopeV1::Thread(decode_message_id(decoder)?)),
        3 => Ok(ReadScopeV1::Topic(decode_topic_id(decoder)?)),
        _ => Err(WireDecodeError::UnknownTag),
    }
}

fn encode_read_position(
    sink: &mut dyn Sink,
    position: &ReadPositionV1,
) -> Result<(), WireEncodeError> {
    if position.room_id.is_empty() {
        return Err(WireEncodeError::Invalid);
    }
    sink.text(&position.room_id)?;
    encode_read_scope(sink, position.scope)?;
    encode_message_id(sink, position.target_message_id)?;
    sink.u64(position.target_sequence)?;
    sink.u64(position.observed_revision)
}

fn decode_read_position(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<ReadPositionV1, WireDecodeError> {
    let room_id = read_text(decoder)?;
    if room_id.is_empty() {
        return Err(WireDecodeError::Malformed);
    }
    Ok(ReadPositionV1 {
        room_id,
        scope: decode_read_scope(decoder)?,
        target_message_id: decode_message_id(decoder)?,
        target_sequence: read_u64(decoder)?,
        observed_revision: read_u64(decoder)?,
    })
}

fn encode_identified_message(
    sink: &mut dyn Sink,
    message: &IdentifiedMessageV1,
) -> Result<(), WireEncodeError> {
    encode_message_id(sink, message.message_id)?;
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

fn decode_identified_message(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<IdentifiedMessageV1, WireDecodeError> {
    let message_id = decode_message_id(decoder)?;
    let sequence = read_u64(decoder)?;
    let author = read_text(decoder)?;
    let logical_time = read_u64(decoder)?;
    let body = read_text(decoder)?;
    let length = read_list(decoder, limits)?;
    let mut addressed_to = try_vec(length)?;
    for _ in 0..length {
        addressed_to.push(read_text(decoder)?);
    }
    Ok(IdentifiedMessageV1 {
        message_id,
        sequence,
        author,
        logical_time,
        body,
        addressed_to,
    })
}

fn encode_identified_message_page(
    sink: &mut dyn Sink,
    page: &IdentifiedMessagePageV1,
) -> Result<(), WireEncodeError> {
    sink.u64(page.revision)?;
    sink.text(&page.room_id)?;
    sink.list(page.messages.len())?;
    for message in &page.messages {
        encode_identified_message(sink, message)?;
    }
    sink.u64(page.oldest_retained_sequence)?;
    sink.u64(page.next_message_sequence)?;
    encode_optional_cursor(sink, page.next)
}

fn decode_identified_message_page(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<IdentifiedMessagePageV1, WireDecodeError> {
    let revision = read_u64(decoder)?;
    let room_id = read_text(decoder)?;
    let length = read_list(decoder, limits)?;
    let mut messages = try_vec(length)?;
    for _ in 0..length {
        messages.push(decode_identified_message(decoder, limits)?);
    }
    Ok(IdentifiedMessagePageV1 {
        revision,
        room_id,
        messages,
        oldest_retained_sequence: read_u64(decoder)?,
        next_message_sequence: read_u64(decoder)?,
        next: decode_optional_cursor(decoder)?,
    })
}

fn encode_attachment_message(
    sink: &mut dyn Sink,
    message: &AttachmentMessageV1,
) -> Result<(), WireEncodeError> {
    encode_message_id(sink, message.message_id)?;
    sink.u64(message.sequence)?;
    sink.text(&message.author)?;
    sink.u64(message.logical_time)?;
    sink.text(&message.body)?;
    sink.list(message.addressed_to.len())?;
    for participant in &message.addressed_to {
        sink.text(participant)?;
    }
    encode_optional_message_id(sink, message.reply_to_root)?;
    encode_optional_topic_id(sink, message.topic_id)?;
    sink.list(message.attachments.len())?;
    for attachment in &message.attachments {
        for byte in attachment.attachment_id.into_bytes() {
            sink.u8(byte)?;
        }
        match &attachment.descriptor {
            None => sink.u8(0)?,
            Some(descriptor) => {
                sink.u8(1)?;
                encode_attachment_descriptor(sink, descriptor)?;
            }
        }
    }
    Ok(())
}

fn decode_attachment_message(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<AttachmentMessageV1, WireDecodeError> {
    let message_id = decode_message_id(decoder)?;
    let sequence = read_u64(decoder)?;
    let author = read_text(decoder)?;
    let logical_time = read_u64(decoder)?;
    let body = read_text(decoder)?;
    let addressed_count = read_list(decoder, limits)?;
    let mut addressed_to = try_vec(addressed_count)?;
    for _ in 0..addressed_count {
        addressed_to.push(read_text(decoder)?);
    }
    let reply_to_root = decode_optional_message_id(decoder)?;
    let topic_id = decode_optional_topic_id(decoder)?;
    let attachment_count = read_list(decoder, limits)?;
    if attachment_count > 8 {
        return Err(WireDecodeError::Limit);
    }
    let mut attachments = try_vec(attachment_count)?;
    for _ in 0..attachment_count {
        let mut attachment_id = [0; 16];
        for byte in &mut attachment_id {
            *byte = read_u8(decoder)?;
        }
        let attachment_id = AttachmentIdV1::new(attachment_id);
        let descriptor = match read_u8(decoder)? {
            0 => None,
            1 => Some(decode_attachment_descriptor(decoder)?),
            _ => return Err(WireDecodeError::UnknownTag),
        };
        attachments.push(MessageAttachmentV1 {
            attachment_id,
            descriptor,
        });
    }
    Ok(AttachmentMessageV1 {
        message_id,
        sequence,
        author,
        logical_time,
        body,
        addressed_to,
        reply_to_root,
        topic_id,
        attachments,
    })
}

fn encode_attachment_message_page(
    sink: &mut dyn Sink,
    page: &AttachmentMessagePageV1,
) -> Result<(), WireEncodeError> {
    sink.u64(page.revision)?;
    sink.text(&page.room_id)?;
    sink.list(page.messages.len())?;
    for message in &page.messages {
        encode_attachment_message(sink, message)?;
    }
    sink.u64(page.oldest_retained_sequence)?;
    sink.u64(page.next_message_sequence)?;
    encode_optional_cursor(sink, page.next)
}

fn decode_attachment_message_page(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<AttachmentMessagePageV1, WireDecodeError> {
    let revision = read_u64(decoder)?;
    let room_id = read_text(decoder)?;
    let length = read_list(decoder, limits)?;
    let mut messages = try_vec(length)?;
    for _ in 0..length {
        messages.push(decode_attachment_message(decoder, limits)?);
    }
    Ok(AttachmentMessagePageV1 {
        revision,
        room_id,
        messages,
        oldest_retained_sequence: read_u64(decoder)?,
        next_message_sequence: read_u64(decoder)?,
        next: decode_optional_cursor(decoder)?,
    })
}

fn valid_attachment_message_page(page: &AttachmentMessagePageV1) -> bool {
    !page.room_id.is_empty()
        && page
            .messages
            .iter()
            .all(|message| valid_attachment_message(message, true))
}

fn valid_attachment_message(message: &AttachmentMessageV1, require_attachments: bool) -> bool {
    let available = !message.body.is_empty();
    let descriptors = message
        .attachments
        .iter()
        .filter_map(|attachment| attachment.descriptor.clone())
        .collect::<Vec<_>>();
    (!require_attachments || !message.attachments.is_empty())
        && message.attachments.len() <= 8
        && message.attachments.iter().all(|attachment| {
            attachment.descriptor.is_some() == available
                && attachment
                    .descriptor
                    .as_ref()
                    .is_none_or(|descriptor| descriptor.attachment_id == attachment.attachment_id)
        })
        && (!available || descriptors.is_empty() || valid_attachment_descriptors(&descriptors))
        && (available || descriptors.is_empty())
}

fn encode_vellum_artifact_target(
    sink: &mut dyn Sink,
    target: VellumArtifactMessageTargetV1,
) -> Result<(), WireEncodeError> {
    match target {
        VellumArtifactMessageTargetV1::Room => sink.u8(1),
        VellumArtifactMessageTargetV1::Reply(message_id) => {
            sink.u8(2)?;
            encode_message_id(sink, message_id)
        }
        VellumArtifactMessageTargetV1::Topic(topic_id) => {
            sink.u8(3)?;
            encode_topic_id(sink, topic_id)
        }
    }
}

fn decode_vellum_artifact_target(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<VellumArtifactMessageTargetV1, WireDecodeError> {
    match read_u8(decoder)? {
        1 => Ok(VellumArtifactMessageTargetV1::Room),
        2 => Ok(VellumArtifactMessageTargetV1::Reply(decode_message_id(
            decoder,
        )?)),
        3 => Ok(VellumArtifactMessageTargetV1::Topic(decode_topic_id(
            decoder,
        )?)),
        _ => Err(WireDecodeError::UnknownTag),
    }
}

fn encode_vellum_artifact_reference(
    sink: &mut dyn Sink,
    reference: &VellumArtifactReferenceV1,
) -> Result<(), WireEncodeError> {
    for byte in reference.reference_id.into_bytes() {
        sink.u8(byte)?;
    }
    if reference.target.workspace_id == 0 || reference.target.document_id == 0 {
        return Err(WireEncodeError::Invalid);
    }
    sink.u64(reference.target.workspace_id)?;
    sink.u64(reference.target.document_id)?;
    sink.u64(reference.target.source_revision)
}

fn decode_vellum_artifact_reference(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<VellumArtifactReferenceV1, WireDecodeError> {
    let mut reference_id = [0; 16];
    for byte in &mut reference_id {
        *byte = read_u8(decoder)?;
    }
    let target = VellumDocumentTargetV1 {
        workspace_id: read_u64(decoder)?,
        document_id: read_u64(decoder)?,
        source_revision: read_u64(decoder)?,
    };
    if target.workspace_id == 0 || target.document_id == 0 {
        return Err(WireDecodeError::Malformed);
    }
    Ok(VellumArtifactReferenceV1 {
        reference_id: VellumArtifactReferenceIdV1::new(reference_id),
        target,
    })
}

fn valid_vellum_artifact_references(references: &[VellumArtifactReferenceV1]) -> bool {
    let mut ids = BTreeSet::new();
    let mut targets = BTreeSet::new();
    (1..=8).contains(&references.len())
        && references.iter().all(|reference| {
            reference.target.workspace_id != 0
                && reference.target.document_id != 0
                && ids.insert(reference.reference_id)
                && targets.insert(reference.target)
        })
}

fn encode_vellum_artifact_message(
    sink: &mut dyn Sink,
    message: &VellumArtifactMessageV1,
) -> Result<(), WireEncodeError> {
    encode_message_id(sink, message.message_id)?;
    sink.u64(message.sequence)?;
    sink.text(&message.author)?;
    sink.u64(message.logical_time)?;
    sink.text(&message.body)?;
    sink.list(message.addressed_to.len())?;
    for participant in &message.addressed_to {
        sink.text(participant)?;
    }
    encode_optional_message_id(sink, message.reply_to_root)?;
    encode_optional_topic_id(sink, message.topic_id)?;
    sink.list(message.references.len())?;
    for reference in &message.references {
        for byte in reference.reference_id.into_bytes() {
            sink.u8(byte)?;
        }
        match reference.target {
            None => sink.u8(0)?,
            Some(target) => {
                sink.u8(1)?;
                sink.u64(target.workspace_id)?;
                sink.u64(target.document_id)?;
                sink.u64(target.source_revision)?;
            }
        }
    }
    Ok(())
}

fn decode_vellum_artifact_message(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<VellumArtifactMessageV1, WireDecodeError> {
    let message_id = decode_message_id(decoder)?;
    let sequence = read_u64(decoder)?;
    let author = read_text(decoder)?;
    let logical_time = read_u64(decoder)?;
    let body = read_text(decoder)?;
    let addressed_count = read_list(decoder, limits)?;
    let mut addressed_to = try_vec(addressed_count)?;
    for _ in 0..addressed_count {
        addressed_to.push(read_text(decoder)?);
    }
    let reply_to_root = decode_optional_message_id(decoder)?;
    let topic_id = decode_optional_topic_id(decoder)?;
    let reference_count = read_list(decoder, limits)?;
    if reference_count > 8 {
        return Err(WireDecodeError::Limit);
    }
    let mut references = try_vec(reference_count)?;
    for _ in 0..reference_count {
        let mut reference_id = [0; 16];
        for byte in &mut reference_id {
            *byte = read_u8(decoder)?;
        }
        let target = match read_u8(decoder)? {
            0 => None,
            1 => {
                let target = VellumDocumentTargetV1 {
                    workspace_id: read_u64(decoder)?,
                    document_id: read_u64(decoder)?,
                    source_revision: read_u64(decoder)?,
                };
                if target.workspace_id == 0 || target.document_id == 0 {
                    return Err(WireDecodeError::Malformed);
                }
                Some(target)
            }
            _ => return Err(WireDecodeError::UnknownTag),
        };
        references.push(MessageVellumArtifactReferenceV1 {
            reference_id: VellumArtifactReferenceIdV1::new(reference_id),
            target,
        });
    }
    Ok(VellumArtifactMessageV1 {
        message_id,
        sequence,
        author,
        logical_time,
        body,
        addressed_to,
        reply_to_root,
        topic_id,
        references,
    })
}

fn encode_vellum_artifact_message_page(
    sink: &mut dyn Sink,
    page: &VellumArtifactMessagePageV1,
) -> Result<(), WireEncodeError> {
    sink.u64(page.revision)?;
    sink.text(&page.room_id)?;
    sink.list(page.messages.len())?;
    for message in &page.messages {
        encode_vellum_artifact_message(sink, message)?;
    }
    sink.u64(page.oldest_retained_sequence)?;
    sink.u64(page.next_message_sequence)?;
    encode_optional_cursor(sink, page.next)
}

fn decode_vellum_artifact_message_page(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<VellumArtifactMessagePageV1, WireDecodeError> {
    let revision = read_u64(decoder)?;
    let room_id = read_text(decoder)?;
    let message_count = read_list(decoder, limits)?;
    let mut messages = try_vec(message_count)?;
    for _ in 0..message_count {
        messages.push(decode_vellum_artifact_message(decoder, limits)?);
    }
    Ok(VellumArtifactMessagePageV1 {
        revision,
        room_id,
        messages,
        oldest_retained_sequence: read_u64(decoder)?,
        next_message_sequence: read_u64(decoder)?,
        next: decode_optional_cursor(decoder)?,
    })
}

fn valid_vellum_artifact_message_page(page: &VellumArtifactMessagePageV1) -> bool {
    !page.room_id.is_empty()
        && page
            .messages
            .windows(2)
            .all(|pair| pair[0].sequence < pair[1].sequence)
        && page.messages.iter().all(valid_vellum_artifact_message)
}

fn valid_vellum_artifact_message(message: &VellumArtifactMessageV1) -> bool {
    let available = !message.body.is_empty();
    let mut ids = BTreeSet::new();
    let mut targets = BTreeSet::new();
    !message.author.is_empty()
        && (1..=8).contains(&message.references.len())
        && message.references.iter().all(|reference| {
            ids.insert(reference.reference_id)
                && reference.target.is_some() == available
                && reference.target.is_none_or(|target| {
                    target.workspace_id != 0 && target.document_id != 0 && targets.insert(target)
                })
        })
}

fn encode_attachment_change_page(
    sink: &mut dyn Sink,
    page: &AttachmentRoomChangePageV1,
) -> Result<(), WireEncodeError> {
    encode_identified_change_page(sink, &page.identified)?;
    sink.list(page.accepted_messages.len())?;
    for message in &page.accepted_messages {
        encode_attachment_message(sink, message)?;
    }
    Ok(())
}

fn decode_attachment_change_page(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<AttachmentRoomChangePageV1, WireDecodeError> {
    let identified = decode_identified_change_page(decoder, limits)?;
    let length = read_list(decoder, limits)?;
    let mut accepted_messages = try_vec(length)?;
    for _ in 0..length {
        accepted_messages.push(decode_attachment_message(decoder, limits)?);
    }
    Ok(AttachmentRoomChangePageV1 {
        identified,
        accepted_messages,
    })
}

fn valid_attachment_change_page(page: &AttachmentRoomChangePageV1) -> bool {
    let accepted = page
        .identified
        .changes
        .iter()
        .filter_map(|change| match &change.event {
            IdentifiedRoomEventV1::MessageAccepted(message) => Some(message),
            _ => None,
        })
        .collect::<Vec<_>>();
    accepted.len() == page.accepted_messages.len()
        && accepted
            .iter()
            .zip(&page.accepted_messages)
            .all(|(identified, attachment)| {
                identified.message_id == attachment.message_id
                    && identified.sequence == attachment.sequence
                    && identified.author == attachment.author
                    && identified.logical_time == attachment.logical_time
                    && identified.body == attachment.body
                    && identified.addressed_to == attachment.addressed_to
                    && valid_attachment_message(attachment, false)
            })
}

fn encode_optional_message_id(
    sink: &mut dyn Sink,
    value: Option<MessageIdV1>,
) -> Result<(), WireEncodeError> {
    match value {
        None => sink.u8(0),
        Some(value) => {
            sink.u8(1)?;
            encode_message_id(sink, value)
        }
    }
}

fn decode_optional_message_id(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<Option<MessageIdV1>, WireDecodeError> {
    match read_u8(decoder)? {
        0 => Ok(None),
        1 => Ok(Some(decode_message_id(decoder)?)),
        _ => Err(WireDecodeError::UnknownTag),
    }
}

fn encode_threaded_message(
    sink: &mut dyn Sink,
    message: &ThreadedMessageV1,
) -> Result<(), WireEncodeError> {
    encode_message_id(sink, message.message_id)?;
    sink.u64(message.sequence)?;
    sink.text(&message.author)?;
    sink.u64(message.logical_time)?;
    sink.text(&message.body)?;
    sink.list(message.addressed_to.len())?;
    for participant in &message.addressed_to {
        sink.text(participant)?;
    }
    encode_optional_message_id(sink, message.reply_to_root)
}

fn decode_threaded_message(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<ThreadedMessageV1, WireDecodeError> {
    let message_id = decode_message_id(decoder)?;
    let sequence = read_u64(decoder)?;
    let author = read_text(decoder)?;
    let logical_time = read_u64(decoder)?;
    let body = read_text(decoder)?;
    let length = read_list(decoder, limits)?;
    let mut addressed_to = try_vec(length)?;
    for _ in 0..length {
        addressed_to.push(read_text(decoder)?);
    }
    Ok(ThreadedMessageV1 {
        message_id,
        sequence,
        author,
        logical_time,
        body,
        addressed_to,
        reply_to_root: decode_optional_message_id(decoder)?,
    })
}

fn encode_thread_message_page(
    sink: &mut dyn Sink,
    page: &ThreadMessagePageV1,
) -> Result<(), WireEncodeError> {
    sink.u64(page.revision)?;
    sink.text(&page.room_id)?;
    encode_optional_message_id(sink, page.root_message_id)?;
    sink.list(page.messages.len())?;
    for message in &page.messages {
        encode_threaded_message(sink, message)?;
    }
    sink.u64(page.oldest_retained_sequence)?;
    sink.u64(page.next_message_sequence)?;
    encode_optional_cursor(sink, page.next)
}

fn valid_thread_message_page(
    page: &ThreadMessagePageV1,
    root_message_id: Option<MessageIdV1>,
) -> bool {
    page.root_message_id == root_message_id
        && page
            .messages
            .iter()
            .all(|message| message.reply_to_root == root_message_id)
}

fn decode_thread_message_page(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<ThreadMessagePageV1, WireDecodeError> {
    let revision = read_u64(decoder)?;
    let room_id = read_text(decoder)?;
    let root_message_id = decode_optional_message_id(decoder)?;
    let length = read_list(decoder, limits)?;
    let mut messages = try_vec(length)?;
    for _ in 0..length {
        messages.push(decode_threaded_message(decoder, limits)?);
    }
    Ok(ThreadMessagePageV1 {
        revision,
        room_id,
        root_message_id,
        messages,
        oldest_retained_sequence: read_u64(decoder)?,
        next_message_sequence: read_u64(decoder)?,
        next: decode_optional_cursor(decoder)?,
    })
}

fn encode_optional_topic_id(
    sink: &mut dyn Sink,
    value: Option<TopicIdV1>,
) -> Result<(), WireEncodeError> {
    match value {
        None => sink.u8(0),
        Some(value) => {
            sink.u8(1)?;
            encode_topic_id(sink, value)
        }
    }
}

fn decode_optional_topic_id(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<Option<TopicIdV1>, WireDecodeError> {
    match read_u8(decoder)? {
        0 => Ok(None),
        1 => Ok(Some(decode_topic_id(decoder)?)),
        _ => Err(WireDecodeError::UnknownTag),
    }
}

fn encode_topic_page(sink: &mut dyn Sink, page: &TopicPageV1) -> Result<(), WireEncodeError> {
    sink.u64(page.revision)?;
    sink.text(&page.room_id)?;
    sink.list(page.topics.len())?;
    for topic in &page.topics {
        encode_topic_id(sink, topic.topic_id)?;
        sink.text(&topic.name)?;
        sink.text(&topic.creator_id)?;
    }
    encode_optional_cursor(sink, page.next)
}

fn decode_topic_page(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<TopicPageV1, WireDecodeError> {
    let revision = read_u64(decoder)?;
    let room_id = read_text(decoder)?;
    let length = read_list(decoder, limits)?;
    let mut topics = try_vec(length)?;
    for _ in 0..length {
        topics.push(TopicV1 {
            topic_id: decode_topic_id(decoder)?,
            name: read_text(decoder)?,
            creator_id: read_text(decoder)?,
        });
    }
    Ok(TopicPageV1 {
        revision,
        room_id,
        topics,
        next: decode_optional_cursor(decoder)?,
    })
}

fn encode_topic_message(
    sink: &mut dyn Sink,
    message: &TopicMessageV1,
) -> Result<(), WireEncodeError> {
    encode_message_id(sink, message.message_id)?;
    sink.u64(message.sequence)?;
    sink.text(&message.author)?;
    sink.u64(message.logical_time)?;
    sink.text(&message.body)?;
    sink.list(message.addressed_to.len())?;
    for participant in &message.addressed_to {
        sink.text(participant)?;
    }
    encode_optional_message_id(sink, message.reply_to_root)?;
    encode_optional_topic_id(sink, message.topic_id)
}

fn decode_topic_message(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<TopicMessageV1, WireDecodeError> {
    let message_id = decode_message_id(decoder)?;
    let sequence = read_u64(decoder)?;
    let author = read_text(decoder)?;
    let logical_time = read_u64(decoder)?;
    let body = read_text(decoder)?;
    let length = read_list(decoder, limits)?;
    let mut addressed_to = try_vec(length)?;
    for _ in 0..length {
        addressed_to.push(read_text(decoder)?);
    }
    Ok(TopicMessageV1 {
        message_id,
        sequence,
        author,
        logical_time,
        body,
        addressed_to,
        reply_to_root: decode_optional_message_id(decoder)?,
        topic_id: decode_optional_topic_id(decoder)?,
    })
}

fn encode_topic_message_page(
    sink: &mut dyn Sink,
    page: &TopicMessagePageV1,
) -> Result<(), WireEncodeError> {
    sink.u64(page.revision)?;
    sink.text(&page.room_id)?;
    encode_topic_id(sink, page.topic_id)?;
    sink.list(page.messages.len())?;
    for message in &page.messages {
        encode_topic_message(sink, message)?;
    }
    sink.u64(page.oldest_retained_sequence)?;
    sink.u64(page.next_message_sequence)?;
    encode_optional_cursor(sink, page.next)
}

fn decode_topic_message_page(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<TopicMessagePageV1, WireDecodeError> {
    let revision = read_u64(decoder)?;
    let room_id = read_text(decoder)?;
    let topic_id = decode_topic_id(decoder)?;
    let length = read_list(decoder, limits)?;
    let mut messages = try_vec(length)?;
    for _ in 0..length {
        messages.push(decode_topic_message(decoder, limits)?);
    }
    Ok(TopicMessagePageV1 {
        revision,
        room_id,
        topic_id,
        messages,
        oldest_retained_sequence: read_u64(decoder)?,
        next_message_sequence: read_u64(decoder)?,
        next: decode_optional_cursor(decoder)?,
    })
}

fn encode_peer_conversation_event(
    sink: &mut dyn Sink,
    event: &PeerConversationCreatedV1,
) -> Result<(), WireEncodeError> {
    if event.profile == ConversationProfileV1::Ordinary
        || event.founding_participant_ids.is_empty()
        || event
            .founding_participant_ids
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || event
            .founding_participant_ids
            .binary_search(&event.initiator_id)
            .is_err()
    {
        return Err(WireEncodeError::Invalid);
    }
    sink.u8(23)?;
    sink.text(&event.initiator_id)?;
    sink.u8(encode_conversation_profile(event.profile))?;
    sink.list(event.founding_participant_ids.len())?;
    for participant_id in &event.founding_participant_ids {
        sink.text(participant_id)?;
    }
    Ok(())
}

fn decode_peer_conversation_event(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<PeerConversationCreatedV1, WireDecodeError> {
    let initiator_id = read_text(decoder)?;
    let profile = decode_conversation_profile(decoder)?;
    if profile == ConversationProfileV1::Ordinary {
        return Err(WireDecodeError::Malformed);
    }
    let count = read_list(decoder, limits)?;
    let mut founding_participant_ids = try_vec(count)?;
    for _ in 0..count {
        let participant_id = read_text(decoder)?;
        if founding_participant_ids
            .last()
            .is_some_and(|previous| previous >= &participant_id)
        {
            return Err(WireDecodeError::Malformed);
        }
        founding_participant_ids.push(participant_id);
    }
    if founding_participant_ids
        .binary_search(&initiator_id)
        .is_err()
    {
        return Err(WireDecodeError::Malformed);
    }
    Ok(PeerConversationCreatedV1 {
        initiator_id,
        profile,
        founding_participant_ids,
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
            RoomEventV2::PeerConversationCreated(event) => {
                encode_peer_conversation_event(sink, event)?
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
            RoomEventV2::MessageWithdrawn {
                actor_id,
                message_id,
                sequence,
            } => {
                sink.u8(11)?;
                sink.text(actor_id)?;
                encode_message_id(sink, *message_id)?;
                sink.u64(*sequence)?;
            }
            RoomEventV2::MessageRedacted {
                actor_id,
                author_id,
                message_id,
                sequence,
                reason,
                causation,
            } => {
                sink.u8(12)?;
                sink.text(actor_id)?;
                sink.text(author_id)?;
                encode_message_id(sink, *message_id)?;
                sink.u64(*sequence)?;
                sink.text(reason)?;
                for byte in causation {
                    sink.u8(*byte)?;
                }
            }
            RoomEventV2::ThreadLifecycleChanged {
                actor_id,
                root_message_id,
                archived,
                reason,
                causation,
            } => encode_thread_lifecycle_event(
                sink,
                *archived,
                actor_id,
                *root_message_id,
                reason,
                causation,
            )?,
            RoomEventV2::ReactionChanged {
                actor_id,
                message_id,
                key,
                reacted,
            } => encode_reaction_event(sink, actor_id, *message_id, key, *reacted)?,
            RoomEventV2::MessageEdited {
                actor_id,
                message_id,
                message_revision,
                logical_time,
                body,
            } => encode_message_edited_event(
                sink,
                actor_id,
                *message_id,
                *message_revision,
                *logical_time,
                body,
            )?,
            RoomEventV2::TopicCreated {
                actor_id,
                topic_id,
                name,
            } => encode_topic_created_event(sink, actor_id, *topic_id, name)?,
            RoomEventV2::TopicRenamed {
                actor_id,
                topic_id,
                previous,
                name,
            } => encode_topic_renamed_event(sink, actor_id, *topic_id, previous, name)?,
            RoomEventV2::TopicLifecycleChanged {
                actor_id,
                topic_id,
                previous_resolved,
                resolved,
            } => encode_topic_lifecycle_event(
                sink,
                actor_id,
                *topic_id,
                *previous_resolved,
                *resolved,
            )?,
            RoomEventV2::TopicSupersessionChanged {
                actor_id,
                topic_id,
                previous_successor_topic_id,
                successor_topic_id,
            } => encode_topic_supersession_event(
                sink,
                actor_id,
                *topic_id,
                *previous_successor_topic_id,
                *successor_topic_id,
            )?,
            RoomEventV2::TopicAliasChanged {
                actor_id,
                topic_id,
                alias,
                added,
            } => encode_topic_alias_event(sink, actor_id, *topic_id, alias, *added)?,
            RoomEventV2::TopicTagChanged {
                actor_id,
                topic_id,
                tag,
                added,
            } => encode_topic_tag_event(sink, actor_id, *topic_id, tag, *added)?,
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
            11 => RoomEventV2::MessageWithdrawn {
                actor_id: read_text(decoder)?,
                message_id: decode_message_id(decoder)?,
                sequence: read_u64(decoder)?,
            },
            12 => {
                let actor_id = read_text(decoder)?;
                let author_id = read_text(decoder)?;
                let message_id = decode_message_id(decoder)?;
                let sequence = read_u64(decoder)?;
                let reason = read_text(decoder)?;
                let mut causation = [0; 16];
                for byte in &mut causation {
                    *byte = read_u8(decoder)?;
                }
                RoomEventV2::MessageRedacted {
                    actor_id,
                    author_id,
                    message_id,
                    sequence,
                    reason,
                    causation,
                }
            }
            tag @ 13..=14 => {
                let (actor_id, root_message_id, reason, causation) =
                    decode_thread_lifecycle_event(decoder)?;
                RoomEventV2::ThreadLifecycleChanged {
                    actor_id,
                    root_message_id,
                    archived: tag == 13,
                    reason,
                    causation,
                }
            }
            15 => {
                let (actor_id, message_id, key, reacted) = decode_reaction_event(decoder)?;
                RoomEventV2::ReactionChanged {
                    actor_id,
                    message_id,
                    key,
                    reacted,
                }
            }
            16 => decode_message_edited_event(decoder)?,
            17 => {
                let (actor_id, topic_id, name) = decode_topic_created_event(decoder)?;
                RoomEventV2::TopicCreated {
                    actor_id,
                    topic_id,
                    name,
                }
            }
            18 => {
                let (actor_id, topic_id, previous, name) = decode_topic_renamed_event(decoder)?;
                RoomEventV2::TopicRenamed {
                    actor_id,
                    topic_id,
                    previous,
                    name,
                }
            }
            19 => {
                let (actor_id, topic_id, previous_resolved, resolved) =
                    decode_topic_lifecycle_event(decoder)?;
                RoomEventV2::TopicLifecycleChanged {
                    actor_id,
                    topic_id,
                    previous_resolved,
                    resolved,
                }
            }
            20 => {
                let (actor_id, topic_id, previous_successor_topic_id, successor_topic_id) =
                    decode_topic_supersession_event(decoder)?;
                RoomEventV2::TopicSupersessionChanged {
                    actor_id,
                    topic_id,
                    previous_successor_topic_id,
                    successor_topic_id,
                }
            }
            21 => {
                let (actor_id, topic_id, alias, added) = decode_topic_alias_event(decoder)?;
                RoomEventV2::TopicAliasChanged {
                    actor_id,
                    topic_id,
                    alias,
                    added,
                }
            }
            22 => {
                let (actor_id, topic_id, tag, added) = decode_topic_tag_event(decoder)?;
                RoomEventV2::TopicTagChanged {
                    actor_id,
                    topic_id,
                    tag,
                    added,
                }
            }
            23 => RoomEventV2::PeerConversationCreated(decode_peer_conversation_event(
                decoder, limits,
            )?),
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

fn encode_identified_change_page(
    sink: &mut dyn Sink,
    page: &IdentifiedRoomChangePageV1,
) -> Result<(), WireEncodeError> {
    sink.u64(page.revision)?;
    sink.list(page.changes.len())?;
    for change in &page.changes {
        sink.u64(change.event_sequence)?;
        sink.u64(change.revision)?;
        match &change.event {
            IdentifiedRoomEventV1::RoomCreated { owner, admission } => {
                sink.u8(1)?;
                sink.text(owner)?;
                sink.u8(encode_admission(*admission))?;
            }
            IdentifiedRoomEventV1::PeerConversationCreated(event) => {
                encode_peer_conversation_event(sink, event)?
            }
            IdentifiedRoomEventV1::InvitationRecorded { participant_id } => {
                sink.u8(2)?;
                sink.text(participant_id)?;
            }
            IdentifiedRoomEventV1::ParticipantJoined { participant_id } => {
                sink.u8(3)?;
                sink.text(participant_id)?;
            }
            IdentifiedRoomEventV1::ParticipantLeft { participant_id } => {
                sink.u8(4)?;
                sink.text(participant_id)?;
            }
            IdentifiedRoomEventV1::RoleChanged {
                participant_id,
                role,
            } => {
                sink.u8(5)?;
                sink.text(participant_id)?;
                sink.u8(encode_role(*role))?;
            }
            IdentifiedRoomEventV1::MessageAccepted(message) => {
                sink.u8(6)?;
                encode_identified_message(sink, message)?;
            }
            IdentifiedRoomEventV1::ParticipantKicked {
                actor_id,
                participant_id,
                reason,
                causation,
            } => encode_moderation_event(sink, 7, actor_id, participant_id, reason, causation)?,
            IdentifiedRoomEventV1::ParticipantBanned {
                actor_id,
                participant_id,
                reason,
                causation,
            } => encode_moderation_event(sink, 8, actor_id, participant_id, reason, causation)?,
            IdentifiedRoomEventV1::ParticipantUnbanned {
                actor_id,
                participant_id,
                reason,
                causation,
            } => encode_moderation_event(sink, 9, actor_id, participant_id, reason, causation)?,
            IdentifiedRoomEventV1::RoomPolicyChanged {
                actor_id,
                previous,
                admission,
            } => {
                sink.u8(10)?;
                sink.text(actor_id)?;
                sink.u8(encode_admission(*previous))?;
                sink.u8(encode_admission(*admission))?;
            }
            IdentifiedRoomEventV1::MessageWithdrawn {
                actor_id,
                message_id,
                sequence,
            } => {
                sink.u8(11)?;
                sink.text(actor_id)?;
                encode_message_id(sink, *message_id)?;
                sink.u64(*sequence)?;
            }
            IdentifiedRoomEventV1::MessageRedacted {
                actor_id,
                author_id,
                message_id,
                sequence,
                reason,
                causation,
            } => {
                sink.u8(12)?;
                sink.text(actor_id)?;
                sink.text(author_id)?;
                encode_message_id(sink, *message_id)?;
                sink.u64(*sequence)?;
                sink.text(reason)?;
                encode_causation(sink, causation)?;
            }
            IdentifiedRoomEventV1::ThreadLifecycleChanged {
                actor_id,
                root_message_id,
                archived,
                reason,
                causation,
            } => encode_thread_lifecycle_event(
                sink,
                *archived,
                actor_id,
                *root_message_id,
                reason,
                causation,
            )?,
            IdentifiedRoomEventV1::ReactionChanged {
                actor_id,
                message_id,
                key,
                reacted,
            } => encode_reaction_event(sink, actor_id, *message_id, key, *reacted)?,
            IdentifiedRoomEventV1::MessageEdited {
                actor_id,
                message_id,
                message_revision,
                logical_time,
                body,
            } => encode_message_edited_event(
                sink,
                actor_id,
                *message_id,
                *message_revision,
                *logical_time,
                body,
            )?,
            IdentifiedRoomEventV1::TopicCreated {
                actor_id,
                topic_id,
                name,
            } => encode_topic_created_event(sink, actor_id, *topic_id, name)?,
            IdentifiedRoomEventV1::TopicRenamed {
                actor_id,
                topic_id,
                previous,
                name,
            } => encode_topic_renamed_event(sink, actor_id, *topic_id, previous, name)?,
            IdentifiedRoomEventV1::TopicLifecycleChanged {
                actor_id,
                topic_id,
                previous_resolved,
                resolved,
            } => encode_topic_lifecycle_event(
                sink,
                actor_id,
                *topic_id,
                *previous_resolved,
                *resolved,
            )?,
            IdentifiedRoomEventV1::TopicSupersessionChanged {
                actor_id,
                topic_id,
                previous_successor_topic_id,
                successor_topic_id,
            } => encode_topic_supersession_event(
                sink,
                actor_id,
                *topic_id,
                *previous_successor_topic_id,
                *successor_topic_id,
            )?,
            IdentifiedRoomEventV1::TopicAliasChanged {
                actor_id,
                topic_id,
                alias,
                added,
            } => encode_topic_alias_event(sink, actor_id, *topic_id, alias, *added)?,
            IdentifiedRoomEventV1::TopicTagChanged {
                actor_id,
                topic_id,
                tag,
                added,
            } => encode_topic_tag_event(sink, actor_id, *topic_id, tag, *added)?,
        }
    }
    sink.u64(page.next.0)?;
    sink.u8(u8::from(page.has_more))
}

fn decode_identified_change_page(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<IdentifiedRoomChangePageV1, WireDecodeError> {
    let revision = read_u64(decoder)?;
    let length = read_list(decoder, limits)?;
    let mut changes = try_vec(length)?;
    for _ in 0..length {
        let event_sequence = read_u64(decoder)?;
        let revision = read_u64(decoder)?;
        let event = match read_u8(decoder)? {
            1 => IdentifiedRoomEventV1::RoomCreated {
                owner: read_text(decoder)?,
                admission: decode_admission(decoder)?,
            },
            2 => IdentifiedRoomEventV1::InvitationRecorded {
                participant_id: read_text(decoder)?,
            },
            3 => IdentifiedRoomEventV1::ParticipantJoined {
                participant_id: read_text(decoder)?,
            },
            4 => IdentifiedRoomEventV1::ParticipantLeft {
                participant_id: read_text(decoder)?,
            },
            5 => IdentifiedRoomEventV1::RoleChanged {
                participant_id: read_text(decoder)?,
                role: decode_role(decoder)?,
            },
            6 => {
                IdentifiedRoomEventV1::MessageAccepted(decode_identified_message(decoder, limits)?)
            }
            7 => decode_identified_moderation_event(decoder, ModerationEvent::Kicked)?,
            8 => decode_identified_moderation_event(decoder, ModerationEvent::Banned)?,
            9 => decode_identified_moderation_event(decoder, ModerationEvent::Unbanned)?,
            10 => IdentifiedRoomEventV1::RoomPolicyChanged {
                actor_id: read_text(decoder)?,
                previous: decode_admission(decoder)?,
                admission: decode_admission(decoder)?,
            },
            11 => IdentifiedRoomEventV1::MessageWithdrawn {
                actor_id: read_text(decoder)?,
                message_id: decode_message_id(decoder)?,
                sequence: read_u64(decoder)?,
            },
            12 => IdentifiedRoomEventV1::MessageRedacted {
                actor_id: read_text(decoder)?,
                author_id: read_text(decoder)?,
                message_id: decode_message_id(decoder)?,
                sequence: read_u64(decoder)?,
                reason: read_text(decoder)?,
                causation: decode_causation(decoder)?,
            },
            tag @ 13..=14 => {
                let (actor_id, root_message_id, reason, causation) =
                    decode_thread_lifecycle_event(decoder)?;
                IdentifiedRoomEventV1::ThreadLifecycleChanged {
                    actor_id,
                    root_message_id,
                    archived: tag == 13,
                    reason,
                    causation,
                }
            }
            15 => {
                let (actor_id, message_id, key, reacted) = decode_reaction_event(decoder)?;
                IdentifiedRoomEventV1::ReactionChanged {
                    actor_id,
                    message_id,
                    key,
                    reacted,
                }
            }
            16 => {
                let RoomEventV2::MessageEdited {
                    actor_id,
                    message_id,
                    message_revision,
                    logical_time,
                    body,
                } = decode_message_edited_event(decoder)?
                else {
                    unreachable!()
                };
                IdentifiedRoomEventV1::MessageEdited {
                    actor_id,
                    message_id,
                    message_revision,
                    logical_time,
                    body,
                }
            }
            17 => {
                let (actor_id, topic_id, name) = decode_topic_created_event(decoder)?;
                IdentifiedRoomEventV1::TopicCreated {
                    actor_id,
                    topic_id,
                    name,
                }
            }
            18 => {
                let (actor_id, topic_id, previous, name) = decode_topic_renamed_event(decoder)?;
                IdentifiedRoomEventV1::TopicRenamed {
                    actor_id,
                    topic_id,
                    previous,
                    name,
                }
            }
            19 => {
                let (actor_id, topic_id, previous_resolved, resolved) =
                    decode_topic_lifecycle_event(decoder)?;
                IdentifiedRoomEventV1::TopicLifecycleChanged {
                    actor_id,
                    topic_id,
                    previous_resolved,
                    resolved,
                }
            }
            20 => {
                let (actor_id, topic_id, previous_successor_topic_id, successor_topic_id) =
                    decode_topic_supersession_event(decoder)?;
                IdentifiedRoomEventV1::TopicSupersessionChanged {
                    actor_id,
                    topic_id,
                    previous_successor_topic_id,
                    successor_topic_id,
                }
            }
            21 => {
                let (actor_id, topic_id, alias, added) = decode_topic_alias_event(decoder)?;
                IdentifiedRoomEventV1::TopicAliasChanged {
                    actor_id,
                    topic_id,
                    alias,
                    added,
                }
            }
            22 => {
                let (actor_id, topic_id, tag, added) = decode_topic_tag_event(decoder)?;
                IdentifiedRoomEventV1::TopicTagChanged {
                    actor_id,
                    topic_id,
                    tag,
                    added,
                }
            }
            23 => IdentifiedRoomEventV1::PeerConversationCreated(decode_peer_conversation_event(
                decoder, limits,
            )?),
            _ => return Err(WireDecodeError::UnknownTag),
        };
        changes.push(IdentifiedRoomChangeV1 {
            event_sequence,
            revision,
            event,
        });
    }
    let next = PageCursorV2(read_u64(decoder)?);
    let has_more = match read_u8(decoder)? {
        0 => false,
        1 => true,
        _ => return Err(WireDecodeError::UnknownTag),
    };
    Ok(IdentifiedRoomChangePageV1 {
        revision,
        changes,
        next,
        has_more,
    })
}

fn encode_threaded_change_page(
    sink: &mut dyn Sink,
    page: &ThreadedRoomChangePageV1,
) -> Result<(), WireEncodeError> {
    sink.u64(page.revision)?;
    sink.list(page.changes.len())?;
    for change in &page.changes {
        sink.u64(change.event_sequence)?;
        sink.u64(change.revision)?;
        match &change.event {
            ThreadedRoomEventV1::RoomCreated { owner, admission } => {
                sink.u8(1)?;
                sink.text(owner)?;
                sink.u8(encode_admission(*admission))?;
            }
            ThreadedRoomEventV1::PeerConversationCreated(event) => {
                encode_peer_conversation_event(sink, event)?
            }
            ThreadedRoomEventV1::InvitationRecorded { participant_id } => {
                sink.u8(2)?;
                sink.text(participant_id)?;
            }
            ThreadedRoomEventV1::ParticipantJoined { participant_id } => {
                sink.u8(3)?;
                sink.text(participant_id)?;
            }
            ThreadedRoomEventV1::ParticipantLeft { participant_id } => {
                sink.u8(4)?;
                sink.text(participant_id)?;
            }
            ThreadedRoomEventV1::RoleChanged {
                participant_id,
                role,
            } => {
                sink.u8(5)?;
                sink.text(participant_id)?;
                sink.u8(encode_role(*role))?;
            }
            ThreadedRoomEventV1::MessageAccepted(message) => {
                sink.u8(6)?;
                encode_threaded_message(sink, message)?;
            }
            ThreadedRoomEventV1::ParticipantKicked {
                actor_id,
                participant_id,
                reason,
                causation,
            } => encode_moderation_event(sink, 7, actor_id, participant_id, reason, causation)?,
            ThreadedRoomEventV1::ParticipantBanned {
                actor_id,
                participant_id,
                reason,
                causation,
            } => encode_moderation_event(sink, 8, actor_id, participant_id, reason, causation)?,
            ThreadedRoomEventV1::ParticipantUnbanned {
                actor_id,
                participant_id,
                reason,
                causation,
            } => encode_moderation_event(sink, 9, actor_id, participant_id, reason, causation)?,
            ThreadedRoomEventV1::RoomPolicyChanged {
                actor_id,
                previous,
                admission,
            } => {
                sink.u8(10)?;
                sink.text(actor_id)?;
                sink.u8(encode_admission(*previous))?;
                sink.u8(encode_admission(*admission))?;
            }
            ThreadedRoomEventV1::MessageWithdrawn {
                actor_id,
                message_id,
                sequence,
            } => {
                sink.u8(11)?;
                sink.text(actor_id)?;
                encode_message_id(sink, *message_id)?;
                sink.u64(*sequence)?;
            }
            ThreadedRoomEventV1::MessageRedacted {
                actor_id,
                author_id,
                message_id,
                sequence,
                reason,
                causation,
            } => {
                sink.u8(12)?;
                sink.text(actor_id)?;
                sink.text(author_id)?;
                encode_message_id(sink, *message_id)?;
                sink.u64(*sequence)?;
                sink.text(reason)?;
                encode_causation(sink, causation)?;
            }
            ThreadedRoomEventV1::ThreadLifecycleChanged {
                actor_id,
                root_message_id,
                archived,
                reason,
                causation,
            } => encode_thread_lifecycle_event(
                sink,
                *archived,
                actor_id,
                *root_message_id,
                reason,
                causation,
            )?,
            ThreadedRoomEventV1::ReactionChanged {
                actor_id,
                message_id,
                key,
                reacted,
            } => encode_reaction_event(sink, actor_id, *message_id, key, *reacted)?,
            ThreadedRoomEventV1::MessageEdited {
                actor_id,
                message_id,
                message_revision,
                logical_time,
                body,
            } => encode_message_edited_event(
                sink,
                actor_id,
                *message_id,
                *message_revision,
                *logical_time,
                body,
            )?,
            ThreadedRoomEventV1::TopicCreated {
                actor_id,
                topic_id,
                name,
            } => encode_topic_created_event(sink, actor_id, *topic_id, name)?,
            ThreadedRoomEventV1::TopicRenamed {
                actor_id,
                topic_id,
                previous,
                name,
            } => encode_topic_renamed_event(sink, actor_id, *topic_id, previous, name)?,
            ThreadedRoomEventV1::TopicLifecycleChanged {
                actor_id,
                topic_id,
                previous_resolved,
                resolved,
            } => encode_topic_lifecycle_event(
                sink,
                actor_id,
                *topic_id,
                *previous_resolved,
                *resolved,
            )?,
            ThreadedRoomEventV1::TopicSupersessionChanged {
                actor_id,
                topic_id,
                previous_successor_topic_id,
                successor_topic_id,
            } => encode_topic_supersession_event(
                sink,
                actor_id,
                *topic_id,
                *previous_successor_topic_id,
                *successor_topic_id,
            )?,
            ThreadedRoomEventV1::TopicAliasChanged {
                actor_id,
                topic_id,
                alias,
                added,
            } => encode_topic_alias_event(sink, actor_id, *topic_id, alias, *added)?,
            ThreadedRoomEventV1::TopicTagChanged {
                actor_id,
                topic_id,
                tag,
                added,
            } => encode_topic_tag_event(sink, actor_id, *topic_id, tag, *added)?,
        }
    }
    sink.u64(page.next.0)?;
    sink.u8(u8::from(page.has_more))
}

fn decode_threaded_change_page(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<ThreadedRoomChangePageV1, WireDecodeError> {
    let revision = read_u64(decoder)?;
    let length = read_list(decoder, limits)?;
    let mut changes = try_vec(length)?;
    for _ in 0..length {
        let event_sequence = read_u64(decoder)?;
        let change_revision = read_u64(decoder)?;
        let event = match read_u8(decoder)? {
            1 => ThreadedRoomEventV1::RoomCreated {
                owner: read_text(decoder)?,
                admission: decode_admission(decoder)?,
            },
            2 => ThreadedRoomEventV1::InvitationRecorded {
                participant_id: read_text(decoder)?,
            },
            3 => ThreadedRoomEventV1::ParticipantJoined {
                participant_id: read_text(decoder)?,
            },
            4 => ThreadedRoomEventV1::ParticipantLeft {
                participant_id: read_text(decoder)?,
            },
            5 => ThreadedRoomEventV1::RoleChanged {
                participant_id: read_text(decoder)?,
                role: decode_role(decoder)?,
            },
            6 => ThreadedRoomEventV1::MessageAccepted(decode_threaded_message(decoder, limits)?),
            7 => decode_threaded_moderation_event(decoder, ModerationEvent::Kicked)?,
            8 => decode_threaded_moderation_event(decoder, ModerationEvent::Banned)?,
            9 => decode_threaded_moderation_event(decoder, ModerationEvent::Unbanned)?,
            10 => ThreadedRoomEventV1::RoomPolicyChanged {
                actor_id: read_text(decoder)?,
                previous: decode_admission(decoder)?,
                admission: decode_admission(decoder)?,
            },
            11 => ThreadedRoomEventV1::MessageWithdrawn {
                actor_id: read_text(decoder)?,
                message_id: decode_message_id(decoder)?,
                sequence: read_u64(decoder)?,
            },
            12 => ThreadedRoomEventV1::MessageRedacted {
                actor_id: read_text(decoder)?,
                author_id: read_text(decoder)?,
                message_id: decode_message_id(decoder)?,
                sequence: read_u64(decoder)?,
                reason: read_text(decoder)?,
                causation: decode_causation(decoder)?,
            },
            tag @ 13..=14 => {
                let (actor_id, root_message_id, reason, causation) =
                    decode_thread_lifecycle_event(decoder)?;
                ThreadedRoomEventV1::ThreadLifecycleChanged {
                    actor_id,
                    root_message_id,
                    archived: tag == 13,
                    reason,
                    causation,
                }
            }
            15 => {
                let (actor_id, message_id, key, reacted) = decode_reaction_event(decoder)?;
                ThreadedRoomEventV1::ReactionChanged {
                    actor_id,
                    message_id,
                    key,
                    reacted,
                }
            }
            16 => {
                let RoomEventV2::MessageEdited {
                    actor_id,
                    message_id,
                    message_revision,
                    logical_time,
                    body,
                } = decode_message_edited_event(decoder)?
                else {
                    unreachable!()
                };
                ThreadedRoomEventV1::MessageEdited {
                    actor_id,
                    message_id,
                    message_revision,
                    logical_time,
                    body,
                }
            }
            17 => {
                let (actor_id, topic_id, name) = decode_topic_created_event(decoder)?;
                ThreadedRoomEventV1::TopicCreated {
                    actor_id,
                    topic_id,
                    name,
                }
            }
            18 => {
                let (actor_id, topic_id, previous, name) = decode_topic_renamed_event(decoder)?;
                ThreadedRoomEventV1::TopicRenamed {
                    actor_id,
                    topic_id,
                    previous,
                    name,
                }
            }
            19 => {
                let (actor_id, topic_id, previous_resolved, resolved) =
                    decode_topic_lifecycle_event(decoder)?;
                ThreadedRoomEventV1::TopicLifecycleChanged {
                    actor_id,
                    topic_id,
                    previous_resolved,
                    resolved,
                }
            }
            20 => {
                let (actor_id, topic_id, previous_successor_topic_id, successor_topic_id) =
                    decode_topic_supersession_event(decoder)?;
                ThreadedRoomEventV1::TopicSupersessionChanged {
                    actor_id,
                    topic_id,
                    previous_successor_topic_id,
                    successor_topic_id,
                }
            }
            21 => {
                let (actor_id, topic_id, alias, added) = decode_topic_alias_event(decoder)?;
                ThreadedRoomEventV1::TopicAliasChanged {
                    actor_id,
                    topic_id,
                    alias,
                    added,
                }
            }
            22 => {
                let (actor_id, topic_id, tag, added) = decode_topic_tag_event(decoder)?;
                ThreadedRoomEventV1::TopicTagChanged {
                    actor_id,
                    topic_id,
                    tag,
                    added,
                }
            }
            23 => ThreadedRoomEventV1::PeerConversationCreated(decode_peer_conversation_event(
                decoder, limits,
            )?),
            _ => return Err(WireDecodeError::UnknownTag),
        };
        changes.push(ThreadedRoomChangeV1 {
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
    Ok(ThreadedRoomChangePageV1 {
        revision,
        changes,
        next,
        has_more,
    })
}

fn encode_topic_change_page(
    sink: &mut dyn Sink,
    page: &TopicRoomChangePageV1,
) -> Result<(), WireEncodeError> {
    sink.u64(page.revision)?;
    sink.list(page.changes.len())?;
    for change in &page.changes {
        sink.u64(change.event_sequence)?;
        sink.u64(change.revision)?;
        match &change.event {
            TopicRoomEventV1::RoomCreated { owner, admission } => {
                sink.u8(1)?;
                sink.text(owner)?;
                sink.u8(encode_admission(*admission))?;
            }
            TopicRoomEventV1::PeerConversationCreated(event) => {
                encode_peer_conversation_event(sink, event)?
            }
            TopicRoomEventV1::InvitationRecorded { participant_id } => {
                sink.u8(2)?;
                sink.text(participant_id)?;
            }
            TopicRoomEventV1::ParticipantJoined { participant_id } => {
                sink.u8(3)?;
                sink.text(participant_id)?;
            }
            TopicRoomEventV1::ParticipantLeft { participant_id } => {
                sink.u8(4)?;
                sink.text(participant_id)?;
            }
            TopicRoomEventV1::RoleChanged {
                participant_id,
                role,
            } => {
                sink.u8(5)?;
                sink.text(participant_id)?;
                sink.u8(encode_role(*role))?;
            }
            TopicRoomEventV1::MessageAccepted(message) => {
                sink.u8(6)?;
                encode_topic_message(sink, message)?;
            }
            TopicRoomEventV1::ParticipantKicked {
                actor_id,
                participant_id,
                reason,
                causation,
            } => encode_moderation_event(sink, 7, actor_id, participant_id, reason, causation)?,
            TopicRoomEventV1::ParticipantBanned {
                actor_id,
                participant_id,
                reason,
                causation,
            } => encode_moderation_event(sink, 8, actor_id, participant_id, reason, causation)?,
            TopicRoomEventV1::ParticipantUnbanned {
                actor_id,
                participant_id,
                reason,
                causation,
            } => encode_moderation_event(sink, 9, actor_id, participant_id, reason, causation)?,
            TopicRoomEventV1::RoomPolicyChanged {
                actor_id,
                previous,
                admission,
            } => {
                sink.u8(10)?;
                sink.text(actor_id)?;
                sink.u8(encode_admission(*previous))?;
                sink.u8(encode_admission(*admission))?;
            }
            TopicRoomEventV1::MessageWithdrawn {
                actor_id,
                message_id,
                sequence,
            } => {
                sink.u8(11)?;
                sink.text(actor_id)?;
                encode_message_id(sink, *message_id)?;
                sink.u64(*sequence)?;
            }
            TopicRoomEventV1::MessageRedacted {
                actor_id,
                author_id,
                message_id,
                sequence,
                reason,
                causation,
            } => {
                sink.u8(12)?;
                sink.text(actor_id)?;
                sink.text(author_id)?;
                encode_message_id(sink, *message_id)?;
                sink.u64(*sequence)?;
                sink.text(reason)?;
                encode_causation(sink, causation)?;
            }
            TopicRoomEventV1::ThreadLifecycleChanged {
                actor_id,
                root_message_id,
                archived,
                reason,
                causation,
            } => encode_thread_lifecycle_event(
                sink,
                *archived,
                actor_id,
                *root_message_id,
                reason,
                causation,
            )?,
            TopicRoomEventV1::ReactionChanged {
                actor_id,
                message_id,
                key,
                reacted,
            } => encode_reaction_event(sink, actor_id, *message_id, key, *reacted)?,
            TopicRoomEventV1::MessageEdited {
                actor_id,
                message_id,
                message_revision,
                logical_time,
                body,
            } => encode_message_edited_event(
                sink,
                actor_id,
                *message_id,
                *message_revision,
                *logical_time,
                body,
            )?,
            TopicRoomEventV1::TopicCreated {
                actor_id,
                topic_id,
                name,
            } => encode_topic_created_event(sink, actor_id, *topic_id, name)?,
            TopicRoomEventV1::TopicRenamed {
                actor_id,
                topic_id,
                previous,
                name,
            } => encode_topic_renamed_event(sink, actor_id, *topic_id, previous, name)?,
            TopicRoomEventV1::TopicLifecycleChanged {
                actor_id,
                topic_id,
                previous_resolved,
                resolved,
            } => encode_topic_lifecycle_event(
                sink,
                actor_id,
                *topic_id,
                *previous_resolved,
                *resolved,
            )?,
            TopicRoomEventV1::TopicSupersessionChanged {
                actor_id,
                topic_id,
                previous_successor_topic_id,
                successor_topic_id,
            } => encode_topic_supersession_event(
                sink,
                actor_id,
                *topic_id,
                *previous_successor_topic_id,
                *successor_topic_id,
            )?,
            TopicRoomEventV1::TopicAliasChanged {
                actor_id,
                topic_id,
                alias,
                added,
            } => encode_topic_alias_event(sink, actor_id, *topic_id, alias, *added)?,
            TopicRoomEventV1::TopicTagChanged {
                actor_id,
                topic_id,
                tag,
                added,
            } => encode_topic_tag_event(sink, actor_id, *topic_id, tag, *added)?,
        }
    }
    sink.u64(page.next.0)?;
    sink.u8(u8::from(page.has_more))
}

fn decode_topic_change_page(
    decoder: &mut keystone_codec::Decoder<'_>,
    limits: WireLimits,
) -> Result<TopicRoomChangePageV1, WireDecodeError> {
    let revision = read_u64(decoder)?;
    let length = read_list(decoder, limits)?;
    let mut changes = try_vec(length)?;
    for _ in 0..length {
        let event_sequence = read_u64(decoder)?;
        let change_revision = read_u64(decoder)?;
        let event = match read_u8(decoder)? {
            1 => TopicRoomEventV1::RoomCreated {
                owner: read_text(decoder)?,
                admission: decode_admission(decoder)?,
            },
            2 => TopicRoomEventV1::InvitationRecorded {
                participant_id: read_text(decoder)?,
            },
            3 => TopicRoomEventV1::ParticipantJoined {
                participant_id: read_text(decoder)?,
            },
            4 => TopicRoomEventV1::ParticipantLeft {
                participant_id: read_text(decoder)?,
            },
            5 => TopicRoomEventV1::RoleChanged {
                participant_id: read_text(decoder)?,
                role: decode_role(decoder)?,
            },
            6 => TopicRoomEventV1::MessageAccepted(decode_topic_message(decoder, limits)?),
            tag @ 7..=9 => {
                match decode_moderation_event(
                    decoder,
                    match tag {
                        7 => ModerationEvent::Kicked,
                        8 => ModerationEvent::Banned,
                        _ => ModerationEvent::Unbanned,
                    },
                )? {
                    RoomEventV2::ParticipantKicked {
                        actor_id,
                        participant_id,
                        reason,
                        causation,
                    } => TopicRoomEventV1::ParticipantKicked {
                        actor_id,
                        participant_id,
                        reason,
                        causation,
                    },
                    RoomEventV2::ParticipantBanned {
                        actor_id,
                        participant_id,
                        reason,
                        causation,
                    } => TopicRoomEventV1::ParticipantBanned {
                        actor_id,
                        participant_id,
                        reason,
                        causation,
                    },
                    RoomEventV2::ParticipantUnbanned {
                        actor_id,
                        participant_id,
                        reason,
                        causation,
                    } => TopicRoomEventV1::ParticipantUnbanned {
                        actor_id,
                        participant_id,
                        reason,
                        causation,
                    },
                    _ => unreachable!(),
                }
            }
            10 => TopicRoomEventV1::RoomPolicyChanged {
                actor_id: read_text(decoder)?,
                previous: decode_admission(decoder)?,
                admission: decode_admission(decoder)?,
            },
            11 => TopicRoomEventV1::MessageWithdrawn {
                actor_id: read_text(decoder)?,
                message_id: decode_message_id(decoder)?,
                sequence: read_u64(decoder)?,
            },
            12 => TopicRoomEventV1::MessageRedacted {
                actor_id: read_text(decoder)?,
                author_id: read_text(decoder)?,
                message_id: decode_message_id(decoder)?,
                sequence: read_u64(decoder)?,
                reason: read_text(decoder)?,
                causation: decode_causation(decoder)?,
            },
            tag @ 13..=14 => {
                let (actor_id, root_message_id, reason, causation) =
                    decode_thread_lifecycle_event(decoder)?;
                TopicRoomEventV1::ThreadLifecycleChanged {
                    actor_id,
                    root_message_id,
                    archived: tag == 13,
                    reason,
                    causation,
                }
            }
            15 => {
                let (actor_id, message_id, key, reacted) = decode_reaction_event(decoder)?;
                TopicRoomEventV1::ReactionChanged {
                    actor_id,
                    message_id,
                    key,
                    reacted,
                }
            }
            16 => {
                let RoomEventV2::MessageEdited {
                    actor_id,
                    message_id,
                    message_revision,
                    logical_time,
                    body,
                } = decode_message_edited_event(decoder)?
                else {
                    unreachable!()
                };
                TopicRoomEventV1::MessageEdited {
                    actor_id,
                    message_id,
                    message_revision,
                    logical_time,
                    body,
                }
            }
            17 => {
                let (actor_id, topic_id, name) = decode_topic_created_event(decoder)?;
                TopicRoomEventV1::TopicCreated {
                    actor_id,
                    topic_id,
                    name,
                }
            }
            18 => {
                let (actor_id, topic_id, previous, name) = decode_topic_renamed_event(decoder)?;
                TopicRoomEventV1::TopicRenamed {
                    actor_id,
                    topic_id,
                    previous,
                    name,
                }
            }
            19 => {
                let (actor_id, topic_id, previous_resolved, resolved) =
                    decode_topic_lifecycle_event(decoder)?;
                TopicRoomEventV1::TopicLifecycleChanged {
                    actor_id,
                    topic_id,
                    previous_resolved,
                    resolved,
                }
            }
            20 => {
                let (actor_id, topic_id, previous_successor_topic_id, successor_topic_id) =
                    decode_topic_supersession_event(decoder)?;
                TopicRoomEventV1::TopicSupersessionChanged {
                    actor_id,
                    topic_id,
                    previous_successor_topic_id,
                    successor_topic_id,
                }
            }
            21 => {
                let (actor_id, topic_id, alias, added) = decode_topic_alias_event(decoder)?;
                TopicRoomEventV1::TopicAliasChanged {
                    actor_id,
                    topic_id,
                    alias,
                    added,
                }
            }
            22 => {
                let (actor_id, topic_id, tag, added) = decode_topic_tag_event(decoder)?;
                TopicRoomEventV1::TopicTagChanged {
                    actor_id,
                    topic_id,
                    tag,
                    added,
                }
            }
            23 => TopicRoomEventV1::PeerConversationCreated(decode_peer_conversation_event(
                decoder, limits,
            )?),
            _ => return Err(WireDecodeError::UnknownTag),
        };
        changes.push(TopicRoomChangeV1 {
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
    Ok(TopicRoomChangePageV1 {
        revision,
        changes,
        next,
        has_more,
    })
}

fn encode_thread_lifecycle_event(
    sink: &mut dyn Sink,
    archived: bool,
    actor_id: &str,
    root_message_id: MessageIdV1,
    reason: &str,
    causation: &[u8; 16],
) -> Result<(), WireEncodeError> {
    sink.u8(if archived { 13 } else { 14 })?;
    sink.text(actor_id)?;
    encode_message_id(sink, root_message_id)?;
    sink.text(reason)?;
    encode_causation(sink, causation)
}

fn decode_thread_lifecycle_event(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<(String, MessageIdV1, String, [u8; 16]), WireDecodeError> {
    Ok((
        read_text(decoder)?,
        decode_message_id(decoder)?,
        read_text(decoder)?,
        decode_causation(decoder)?,
    ))
}

fn encode_reaction_event(
    sink: &mut dyn Sink,
    actor_id: &str,
    message_id: MessageIdV1,
    key: &str,
    reacted: bool,
) -> Result<(), WireEncodeError> {
    sink.u8(15)?;
    sink.text(actor_id)?;
    encode_message_id(sink, message_id)?;
    sink.text(key)?;
    sink.u8(u8::from(reacted))
}

fn decode_reaction_event(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<(String, MessageIdV1, String, bool), WireDecodeError> {
    Ok((
        read_text(decoder)?,
        decode_message_id(decoder)?,
        read_text(decoder)?,
        match read_u8(decoder)? {
            0 => false,
            1 => true,
            _ => return Err(WireDecodeError::UnknownTag),
        },
    ))
}

fn encode_message_edited_event(
    sink: &mut dyn Sink,
    actor_id: &str,
    message_id: MessageIdV1,
    message_revision: u32,
    logical_time: u64,
    body: &str,
) -> Result<(), WireEncodeError> {
    sink.u8(16)?;
    sink.text(actor_id)?;
    encode_message_id(sink, message_id)?;
    sink.u64(u64::from(message_revision))?;
    sink.u64(logical_time)?;
    sink.text(body)
}

fn decode_message_edited_event(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<RoomEventV2, WireDecodeError> {
    Ok(RoomEventV2::MessageEdited {
        actor_id: read_text(decoder)?,
        message_id: decode_message_id(decoder)?,
        message_revision: read_u32(decoder)?,
        logical_time: read_u64(decoder)?,
        body: read_text(decoder)?,
    })
}

fn encode_topic_created_event(
    sink: &mut dyn Sink,
    actor_id: &str,
    topic_id: TopicIdV1,
    name: &str,
) -> Result<(), WireEncodeError> {
    sink.u8(17)?;
    sink.text(actor_id)?;
    for byte in topic_id.into_bytes() {
        sink.u8(byte)?;
    }
    sink.text(name)
}

fn decode_topic_created_event(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<(String, TopicIdV1, String), WireDecodeError> {
    let actor_id = read_text(decoder)?;
    let mut bytes = [0; 16];
    for byte in &mut bytes {
        *byte = read_u8(decoder)?;
    }
    Ok((actor_id, TopicIdV1::new(bytes), read_text(decoder)?))
}

fn encode_topic_renamed_event(
    sink: &mut dyn Sink,
    actor_id: &str,
    topic_id: TopicIdV1,
    previous: &str,
    name: &str,
) -> Result<(), WireEncodeError> {
    sink.u8(18)?;
    sink.text(actor_id)?;
    encode_topic_id(sink, topic_id)?;
    sink.text(previous)?;
    sink.text(name)
}

fn decode_topic_renamed_event(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<(String, TopicIdV1, String, String), WireDecodeError> {
    Ok((
        read_text(decoder)?,
        decode_topic_id(decoder)?,
        read_text(decoder)?,
        read_text(decoder)?,
    ))
}

fn encode_topic_lifecycle_event(
    sink: &mut dyn Sink,
    actor_id: &str,
    topic_id: TopicIdV1,
    previous_resolved: bool,
    resolved: bool,
) -> Result<(), WireEncodeError> {
    if previous_resolved == resolved {
        return Err(WireEncodeError::Invalid);
    }
    sink.u8(19)?;
    sink.text(actor_id)?;
    encode_topic_id(sink, topic_id)?;
    sink.u8(u8::from(previous_resolved))?;
    sink.u8(u8::from(resolved))
}

fn decode_topic_lifecycle_event(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<(String, TopicIdV1, bool, bool), WireDecodeError> {
    let actor_id = read_text(decoder)?;
    let topic_id = decode_topic_id(decoder)?;
    let previous_resolved = match read_u8(decoder)? {
        0 => false,
        1 => true,
        _ => return Err(WireDecodeError::UnknownTag),
    };
    let resolved = match read_u8(decoder)? {
        0 => false,
        1 => true,
        _ => return Err(WireDecodeError::UnknownTag),
    };
    if previous_resolved == resolved {
        return Err(WireDecodeError::Malformed);
    }
    Ok((actor_id, topic_id, previous_resolved, resolved))
}

fn encode_topic_supersession_event(
    sink: &mut dyn Sink,
    actor_id: &str,
    topic_id: TopicIdV1,
    previous_successor_topic_id: Option<TopicIdV1>,
    successor_topic_id: Option<TopicIdV1>,
) -> Result<(), WireEncodeError> {
    if previous_successor_topic_id == successor_topic_id {
        return Err(WireEncodeError::Invalid);
    }
    sink.u8(20)?;
    sink.text(actor_id)?;
    encode_topic_id(sink, topic_id)?;
    encode_optional_topic_id(sink, previous_successor_topic_id)?;
    encode_optional_topic_id(sink, successor_topic_id)
}

fn decode_topic_supersession_event(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<(String, TopicIdV1, Option<TopicIdV1>, Option<TopicIdV1>), WireDecodeError> {
    let actor_id = read_text(decoder)?;
    let topic_id = decode_topic_id(decoder)?;
    let previous_successor_topic_id = decode_optional_topic_id(decoder)?;
    let successor_topic_id = decode_optional_topic_id(decoder)?;
    if previous_successor_topic_id == successor_topic_id {
        return Err(WireDecodeError::Malformed);
    }
    Ok((
        actor_id,
        topic_id,
        previous_successor_topic_id,
        successor_topic_id,
    ))
}

fn encode_topic_alias_event(
    sink: &mut dyn Sink,
    actor_id: &str,
    topic_id: TopicIdV1,
    alias: &str,
    added: bool,
) -> Result<(), WireEncodeError> {
    sink.u8(21)?;
    sink.text(actor_id)?;
    encode_topic_id(sink, topic_id)?;
    sink.text(alias)?;
    sink.u8(u8::from(added))
}

fn decode_topic_alias_event(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<(String, TopicIdV1, String, bool), WireDecodeError> {
    let actor_id = read_text(decoder)?;
    let topic_id = decode_topic_id(decoder)?;
    let alias = read_text(decoder)?;
    let added = match read_u8(decoder)? {
        0 => false,
        1 => true,
        _ => return Err(WireDecodeError::UnknownTag),
    };
    Ok((actor_id, topic_id, alias, added))
}

fn encode_topic_tag_event(
    sink: &mut dyn Sink,
    actor_id: &str,
    topic_id: TopicIdV1,
    tag: &str,
    added: bool,
) -> Result<(), WireEncodeError> {
    sink.u8(22)?;
    sink.text(actor_id)?;
    encode_topic_id(sink, topic_id)?;
    sink.text(tag)?;
    sink.u8(u8::from(added))
}

fn decode_topic_tag_event(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<(String, TopicIdV1, String, bool), WireDecodeError> {
    let actor_id = read_text(decoder)?;
    let topic_id = decode_topic_id(decoder)?;
    let tag = read_text(decoder)?;
    let added = match read_u8(decoder)? {
        0 => false,
        1 => true,
        _ => return Err(WireDecodeError::UnknownTag),
    };
    Ok((actor_id, topic_id, tag, added))
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

fn decode_identified_moderation_event(
    decoder: &mut keystone_codec::Decoder<'_>,
    event: ModerationEvent,
) -> Result<IdentifiedRoomEventV1, WireDecodeError> {
    let actor_id = read_text(decoder)?;
    let participant_id = read_text(decoder)?;
    let reason = read_text(decoder)?;
    let causation = decode_causation(decoder)?;
    Ok(match event {
        ModerationEvent::Kicked => IdentifiedRoomEventV1::ParticipantKicked {
            actor_id,
            participant_id,
            reason,
            causation,
        },
        ModerationEvent::Banned => IdentifiedRoomEventV1::ParticipantBanned {
            actor_id,
            participant_id,
            reason,
            causation,
        },
        ModerationEvent::Unbanned => IdentifiedRoomEventV1::ParticipantUnbanned {
            actor_id,
            participant_id,
            reason,
            causation,
        },
    })
}

fn decode_threaded_moderation_event(
    decoder: &mut keystone_codec::Decoder<'_>,
    event: ModerationEvent,
) -> Result<ThreadedRoomEventV1, WireDecodeError> {
    let actor_id = read_text(decoder)?;
    let participant_id = read_text(decoder)?;
    let reason = read_text(decoder)?;
    let causation = decode_causation(decoder)?;
    Ok(match event {
        ModerationEvent::Kicked => ThreadedRoomEventV1::ParticipantKicked {
            actor_id,
            participant_id,
            reason,
            causation,
        },
        ModerationEvent::Banned => ThreadedRoomEventV1::ParticipantBanned {
            actor_id,
            participant_id,
            reason,
            causation,
        },
        ModerationEvent::Unbanned => ThreadedRoomEventV1::ParticipantUnbanned {
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
        RefusalV2::ThreadArchived => 17,
        RefusalV2::UnknownTopic => 18,
        RefusalV2::InvalidReadScope => 19,
        RefusalV2::InvalidReadTarget => 20,
        RefusalV2::ReadPositionRegression => 21,
        RefusalV2::TopicResolved => 22,
        RefusalV2::InvalidTopicSupersession => 23,
        RefusalV2::TopicMustBeResolved => 24,
        RefusalV2::TopicAliasLimit => 25,
        RefusalV2::UnknownTopicAlias => 26,
        RefusalV2::InvalidTopicTag => 27,
        RefusalV2::DuplicateTopicTag => 28,
        RefusalV2::TopicTagLimit => 29,
        RefusalV2::UnknownTopicTag => 30,
        RefusalV2::ImmutableConversationCohort => 31,
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
        17 => Ok(RefusalV2::ThreadArchived),
        18 => Ok(RefusalV2::UnknownTopic),
        19 => Ok(RefusalV2::InvalidReadScope),
        20 => Ok(RefusalV2::InvalidReadTarget),
        21 => Ok(RefusalV2::ReadPositionRegression),
        22 => Ok(RefusalV2::TopicResolved),
        23 => Ok(RefusalV2::InvalidTopicSupersession),
        24 => Ok(RefusalV2::TopicMustBeResolved),
        25 => Ok(RefusalV2::TopicAliasLimit),
        26 => Ok(RefusalV2::UnknownTopicAlias),
        27 => Ok(RefusalV2::InvalidTopicTag),
        28 => Ok(RefusalV2::DuplicateTopicTag),
        29 => Ok(RefusalV2::TopicTagLimit),
        30 => Ok(RefusalV2::UnknownTopicTag),
        31 => Ok(RefusalV2::ImmutableConversationCohort),
        _ => Err(WireDecodeError::UnknownTag),
    }
}

fn encode_admission(value: AdmissionPolicyV2) -> u8 {
    match value {
        AdmissionPolicyV2::Open => 1,
        AdmissionPolicyV2::InviteOnly => 2,
    }
}

fn encode_conversation_profile(value: ConversationProfileV1) -> u8 {
    match value {
        ConversationProfileV1::Ordinary => 1,
        ConversationProfileV1::Direct => 2,
        ConversationProfileV1::GroupDirect => 3,
    }
}

fn decode_conversation_profile(
    decoder: &mut keystone_codec::Decoder<'_>,
) -> Result<ConversationProfileV1, WireDecodeError> {
    match read_u8(decoder)? {
        1 => Ok(ConversationProfileV1::Ordinary),
        2 => Ok(ConversationProfileV1::Direct),
        3 => Ok(ConversationProfileV1::GroupDirect),
        _ => Err(WireDecodeError::UnknownTag),
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
