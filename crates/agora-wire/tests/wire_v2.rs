use agora_protocol::v2::{
    AdmissionPolicyV2, ClientIdV2, CommandV2, MemberPageV2, MemberV2, MessagePageV2, MessageV2,
    OutcomeV2, PageCursorV2, ParticipantKindV2, RefusalV2, ReplyV2, RequestIdV2, RequestV2, RoleV2,
    RoomPageV2, RoomSummaryV2,
};
use agora_wire::v2::{
    WireReplyV2, WireRequestV2, decode_reply_v2, decode_request_v2, encode_reply_v2,
    encode_request_v2,
};
use agora_wire::{WireDecodeError, WireEncodeError, WireLimits};

fn limits() -> WireLimits {
    WireLimits {
        max_frame_bytes: 8_192,
        max_text_bytes: 128,
        max_fields: 512,
        max_list_items: 32,
    }
}

fn create_request() -> WireRequestV2 {
    WireRequestV2::Execute {
        client: ClientIdV2(7),
        request: RequestV2 {
            request_id: RequestIdV2(9),
            command: CommandV2::CreateRoom {
                room_id: "private".to_owned(),
                name: "Private".to_owned(),
                admission: AdmissionPolicyV2::InviteOnly,
                expected_revision: 11,
            },
        },
    }
}

fn rooms_reply() -> WireReplyV2 {
    WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::Rooms(RoomPageV2 {
        revision: 12,
        rooms: vec![RoomSummaryV2 {
            room_id: "private".to_owned(),
            name: "Private".to_owned(),
            admission: AdmissionPolicyV2::InviteOnly,
            observer_role: RoleV2::Owner,
            member_count: 3,
            retained_message_count: 1,
            next_message_sequence: 4,
        }],
        next: Some(PageCursorV2(42)),
    })))
}

fn messages_reply() -> WireReplyV2 {
    WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::Messages(MessagePageV2 {
        revision: 12,
        room_id: "private".to_owned(),
        messages: vec![MessageV2 {
            sequence: 3,
            author: "bot".to_owned(),
            logical_time: 8,
            body: "private body".to_owned(),
            addressed_to: vec!["alice".to_owned()],
        }],
        oldest_retained_sequence: 3,
        next_message_sequence: 4,
        next: None,
    })))
}

fn fixture(source: &str) -> Vec<u8> {
    source
        .split_ascii_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).unwrap())
        .collect()
}

#[test]
fn v2_golden_fixtures_are_canonical_and_redacted() {
    let cases = [
        (
            encode_request_v2(&create_request(), limits()).unwrap(),
            fixture(include_str!("fixtures/create-room-request-v2.hex")),
        ),
        (
            encode_reply_v2(&rooms_reply(), limits()).unwrap(),
            fixture(include_str!("fixtures/rooms-reply-v2.hex")),
        ),
        (
            encode_reply_v2(&messages_reply(), limits()).unwrap(),
            fixture(include_str!("fixtures/messages-reply-v2.hex")),
        ),
    ];
    for (actual, expected) in cases {
        assert_eq!(actual, expected);
    }
    let rendered = format!(
        "{:?} {:?} {:?}",
        create_request(),
        rooms_reply(),
        messages_reply()
    );
    for secret in ["private", "Private", "private body", "alice", "bot", "42"] {
        assert!(!rendered.contains(secret));
    }
}

#[test]
fn every_v2_command_round_trips() {
    let commands = [
        CommandV2::Rooms {
            cursor: None,
            limit: 2,
        },
        CommandV2::Rooms {
            cursor: Some(PageCursorV2(1)),
            limit: 2,
        },
        CommandV2::Members {
            room_id: "room".to_owned(),
            cursor: Some(PageCursorV2(2)),
            limit: 3,
        },
        CommandV2::Messages {
            room_id: "room".to_owned(),
            cursor: None,
            limit: 4,
        },
        CommandV2::CreateRoom {
            room_id: "room".to_owned(),
            name: "Room".to_owned(),
            admission: AdmissionPolicyV2::Open,
            expected_revision: 1,
        },
        CommandV2::Invite {
            room_id: "room".to_owned(),
            participant_id: "bot".to_owned(),
            expected_revision: 2,
        },
        CommandV2::Join {
            room_id: "room".to_owned(),
            expected_revision: 3,
        },
        CommandV2::Leave {
            room_id: "room".to_owned(),
            expected_revision: 4,
        },
        CommandV2::SetRole {
            room_id: "room".to_owned(),
            participant_id: "bot".to_owned(),
            role: RoleV2::Moderator,
            expected_revision: 5,
        },
        CommandV2::Speak {
            room_id: "room".to_owned(),
            expected_revision: 6,
            logical_time: 7,
            body: "body".to_owned(),
            addressed_to: vec!["alice".to_owned(), "bot".to_owned()],
        },
    ];
    for (index, command) in commands.into_iter().enumerate() {
        let request = WireRequestV2::Execute {
            client: ClientIdV2(7),
            request: RequestV2 {
                request_id: RequestIdV2(index as u64),
                command,
            },
        };
        let encoded = encode_request_v2(&request, limits()).unwrap();
        assert_eq!(decode_request_v2(&encoded, limits()).unwrap(), request);
    }
}

#[test]
fn every_v2_outcome_and_refusal_round_trips() {
    let member_page = OutcomeV2::Members(MemberPageV2 {
        revision: 1,
        room_id: "room".to_owned(),
        members: vec![MemberV2 {
            participant_id: "bot".to_owned(),
            display_name: "Bot".to_owned(),
            kind: ParticipantKindV2::Agent,
            role: RoleV2::Member,
        }],
        next: None,
    });
    let outcomes = [
        match rooms_reply() {
            WireReplyV2::Executed(ReplyV2::Accepted(outcome)) => outcome,
            _ => unreachable!(),
        },
        member_page,
        match messages_reply() {
            WireReplyV2::Executed(ReplyV2::Accepted(outcome)) => outcome,
            _ => unreachable!(),
        },
        OutcomeV2::RoomCreated { revision: 1 },
        OutcomeV2::InvitationRecorded { revision: 2 },
        OutcomeV2::Joined { revision: 3 },
        OutcomeV2::Left { revision: 4 },
        OutcomeV2::RoleChanged { revision: 5 },
        OutcomeV2::MessageAccepted {
            revision: 6,
            sequence: 7,
        },
    ];
    for outcome in outcomes {
        let reply = WireReplyV2::Executed(ReplyV2::Accepted(outcome));
        let encoded = encode_reply_v2(&reply, limits()).unwrap();
        assert_eq!(decode_reply_v2(&encoded, limits()).unwrap(), reply);
    }

    let refusals = [
        RefusalV2::UnknownClient,
        RefusalV2::UnknownParticipant,
        RefusalV2::UnknownRoom,
        RefusalV2::NotMember,
        RefusalV2::NotAuthorized,
        RefusalV2::Stale { actual_revision: 9 },
        RefusalV2::RequestConflict,
        RefusalV2::ClientLimit,
        RefusalV2::RequestLimit,
        RefusalV2::CursorLimit,
        RefusalV2::InvalidCursor,
        RefusalV2::InvalidInput,
        RefusalV2::Capacity,
        RefusalV2::Exhausted,
        RefusalV2::Unavailable,
    ];
    for refusal in refusals {
        for reply in [
            WireReplyV2::ConnectRefused(refusal),
            WireReplyV2::Executed(ReplyV2::Refused(refusal)),
            WireReplyV2::DisconnectRefused(refusal),
        ] {
            let encoded = encode_reply_v2(&reply, limits()).unwrap();
            assert_eq!(decode_reply_v2(&encoded, limits()).unwrap(), reply);
        }
    }
}

#[test]
fn v2_decoding_is_closed_and_version_isolated() {
    let request = encode_request_v2(&create_request(), limits()).unwrap();
    for end in 0..request.len() {
        assert!(decode_request_v2(&request[..end], limits()).is_err());
    }
    for reply in [rooms_reply(), messages_reply()] {
        let encoded = encode_reply_v2(&reply, limits()).unwrap();
        for end in 0..encoded.len() {
            assert!(decode_reply_v2(&encoded[..end], limits()).is_err());
        }
    }

    let mut trailing = encode_request_v2(&create_request(), limits()).unwrap();
    trailing.push(0);
    assert_eq!(
        decode_request_v2(&trailing, limits()),
        Err(WireDecodeError::TrailingData)
    );
    let mut wrong_version = encode_request_v2(&create_request(), limits()).unwrap();
    wrong_version[0] = 1;
    assert_eq!(
        decode_request_v2(&wrong_version, limits()),
        Err(WireDecodeError::UnsupportedVersion)
    );

    let mut unknown_command = vec![2, 2];
    unknown_command.extend_from_slice(&0_u64.to_be_bytes());
    unknown_command.extend_from_slice(&0_u64.to_be_bytes());
    unknown_command.push(0x7f);
    assert_eq!(
        decode_request_v2(&unknown_command, limits()),
        Err(WireDecodeError::UnknownTag)
    );
    let unknown_cursor_tag = vec![
        2, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0x7f,
    ];
    assert_eq!(
        decode_request_v2(&unknown_cursor_tag, limits()),
        Err(WireDecodeError::UnknownTag)
    );
}

#[test]
fn v2_limits_apply_before_owned_reconstruction() {
    assert_eq!(
        encode_request_v2(
            &create_request(),
            WireLimits {
                max_text_bytes: 3,
                ..limits()
            },
        ),
        Err(WireEncodeError::Limit)
    );
    let encoded = encode_reply_v2(&rooms_reply(), limits()).unwrap();
    assert_eq!(
        decode_reply_v2(
            &encoded,
            WireLimits {
                max_list_items: 0,
                ..limits()
            },
        ),
        Err(WireDecodeError::Limit)
    );
}
