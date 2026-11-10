use agora_protocol::{
    ClientIdV1, CommandV1, MemberV1, MessageV1, OutcomeV1, ParticipantKindV1, RefusalV1, ReplyV1,
    RequestIdV1, RequestV1, RoleV1, RoomSnapshotV1,
};
use agora_wire::{
    WireDecodeError, WireEncodeError, WireLimits, WireReplyV1, WireRequestV1, decode_reply_v1,
    decode_request_v1, encode_reply_v1, encode_request_v1,
};

fn fixture(source: &str) -> Vec<u8> {
    source
        .split_ascii_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).unwrap())
        .collect()
}

fn limits() -> WireLimits {
    WireLimits {
        max_frame_bytes: 2_048,
        max_text_bytes: 64,
        max_fields: 128,
        max_list_items: 8,
    }
}

fn speak_request() -> WireRequestV1 {
    WireRequestV1::Execute {
        client: ClientIdV1(7),
        request: RequestV1 {
            request_id: RequestIdV1(9),
            command: CommandV1::Speak {
                room_id: "general".to_owned(),
                expected_revision: 11,
                logical_time: 12,
                body: "hé".to_owned(),
                addressed_to: vec!["alice".to_owned(), "bot".to_owned()],
            },
        },
    }
}

fn snapshot_reply() -> WireReplyV1 {
    WireReplyV1::Executed(ReplyV1::Accepted(OutcomeV1::Snapshot(RoomSnapshotV1 {
        revision: 13,
        room_id: "general".to_owned(),
        name: "General".to_owned(),
        members: vec![
            MemberV1 {
                participant_id: "alice".to_owned(),
                kind: ParticipantKindV1::Human,
                role: RoleV1::Owner,
            },
            MemberV1 {
                participant_id: "bot".to_owned(),
                kind: ParticipantKindV1::Agent,
                role: RoleV1::Member,
            },
        ],
        messages: vec![MessageV1 {
            sequence: 0,
            author: "alice".to_owned(),
            logical_time: 12,
            body: "private message".to_owned(),
            addressed_to: vec!["bot".to_owned()],
        }],
        next_message_sequence: 1,
    })))
}

#[test]
fn golden_connect_frame_is_canonical() {
    let request = WireRequestV1::Connect {
        participant_id: "alice".to_owned(),
    };
    let expected = fixture(include_str!("fixtures/connect-request-v1.hex"));
    assert_eq!(encode_request_v1(&request, limits()).unwrap(), expected);
    assert_eq!(decode_request_v1(&expected, limits()).unwrap(), request);
}

#[test]
fn golden_speak_frame_is_canonical_and_preserves_utf8() {
    let request = speak_request();
    let expected = fixture(include_str!("fixtures/speak-request-v1.hex"));
    assert_eq!(encode_request_v1(&request, limits()).unwrap(), expected);
    assert_eq!(decode_request_v1(&expected, limits()).unwrap(), request);
}

#[test]
fn golden_snapshot_frame_is_canonical_and_redacted() {
    let reply = snapshot_reply();
    let expected = fixture(include_str!("fixtures/snapshot-reply-v1.hex"));
    assert_eq!(encode_reply_v1(&reply, limits()).unwrap(), expected);
    assert_eq!(decode_reply_v1(&expected, limits()).unwrap(), reply);
    let rendered = format!("{reply:?}");
    for secret in ["general", "General", "alice", "private message", "bot"] {
        assert!(!rendered.contains(secret));
    }
}

#[test]
fn truncation_trailing_versions_tags_utf8_and_limits_are_closed() {
    let encoded_request = encode_request_v1(&speak_request(), limits()).unwrap();
    for end in 0..encoded_request.len() {
        assert!(decode_request_v1(&encoded_request[..end], limits()).is_err());
    }
    let encoded_reply = encode_reply_v1(&snapshot_reply(), limits()).unwrap();
    for end in 0..encoded_reply.len() {
        assert!(decode_reply_v1(&encoded_reply[..end], limits()).is_err());
    }

    let mut request = encoded_request;
    request.push(0);
    assert_eq!(
        decode_request_v1(&request, limits()),
        Err(WireDecodeError::TrailingData)
    );
    request.pop();
    let mut reply = encoded_reply;
    reply.push(0);
    assert_eq!(
        decode_reply_v1(&reply, limits()),
        Err(WireDecodeError::TrailingData)
    );

    let mut unsupported = request.clone();
    unsupported[0] = 2;
    assert_eq!(
        decode_request_v1(&unsupported, limits()),
        Err(WireDecodeError::UnsupportedVersion)
    );
    let mut unknown = request.clone();
    unknown[1] = 0x7f;
    assert_eq!(
        decode_request_v1(&unknown, limits()),
        Err(WireDecodeError::UnknownTag)
    );
    let last = request.len() - 1;
    request[last] = 0xff;
    assert_eq!(
        decode_request_v1(&request, limits()),
        Err(WireDecodeError::InvalidUtf8)
    );

    assert_eq!(
        encode_request_v1(
            &speak_request(),
            WireLimits {
                max_text_bytes: 2,
                ..limits()
            }
        ),
        Err(WireEncodeError::Limit)
    );
    assert_eq!(
        decode_request_v1(
            &encode_request_v1(&speak_request(), limits()).unwrap(),
            WireLimits {
                max_list_items: 1,
                ..limits()
            }
        ),
        Err(WireDecodeError::Limit)
    );
    assert_eq!(
        encode_reply_v1(
            &snapshot_reply(),
            WireLimits {
                max_fields: 3,
                ..limits()
            }
        ),
        Err(WireEncodeError::Limit)
    );
    assert_eq!(
        decode_reply_v1(
            &encode_reply_v1(&snapshot_reply(), limits()).unwrap(),
            WireLimits {
                max_fields: 3,
                ..limits()
            }
        ),
        Err(WireDecodeError::Limit)
    );

    let full_request = encode_request_v1(&speak_request(), limits()).unwrap();
    assert_eq!(
        decode_request_v1(
            &full_request,
            WireLimits {
                max_frame_bytes: full_request.len() - 1,
                ..limits()
            }
        ),
        Err(WireDecodeError::Limit)
    );
}

#[test]
fn every_nested_vocabulary_is_closed_to_unknown_tags() {
    let mut unknown_command = vec![1, 2];
    unknown_command.extend_from_slice(&0_u64.to_be_bytes());
    unknown_command.extend_from_slice(&0_u64.to_be_bytes());
    unknown_command.push(0x7f);
    assert_eq!(
        decode_request_v1(&unknown_command, limits()),
        Err(WireDecodeError::UnknownTag)
    );

    for unknown_reply in [
        vec![1, 0x82, 0x7f],
        vec![1, 0x83, 1, 0x7f],
        vec![1, 0x83, 0x7f],
    ] {
        assert_eq!(
            decode_reply_v1(&unknown_reply, limits()),
            Err(WireDecodeError::UnknownTag)
        );
    }

    let encoded = encode_reply_v1(&snapshot_reply(), limits()).unwrap();
    let member = [0, 0, 0, 5, b'a', b'l', b'i', b'c', b'e', 1, 1];
    let member_start = encoded
        .windows(member.len())
        .position(|window| window == member)
        .unwrap();
    for offset in [member.len() - 2, member.len() - 1] {
        let mut changed = encoded.clone();
        changed[member_start + offset] = 0x7f;
        assert_eq!(
            decode_reply_v1(&changed, limits()),
            Err(WireDecodeError::UnknownTag)
        );
    }
}

#[test]
fn every_reply_refusal_and_scalar_outcome_round_trips() {
    let refusals = [
        RefusalV1::UnknownClient,
        RefusalV1::UnknownParticipant,
        RefusalV1::UnknownRoom,
        RefusalV1::NotMember,
        RefusalV1::NotAuthorized,
        RefusalV1::Stale {
            actual_revision: 17,
        },
        RefusalV1::RequestConflict,
        RefusalV1::ClientLimit,
        RefusalV1::RetiredClientLimit,
        RefusalV1::RequestLimit,
        RefusalV1::SnapshotLimit,
        RefusalV1::InvalidInput,
        RefusalV1::Capacity,
        RefusalV1::Exhausted,
        RefusalV1::Unavailable,
    ];
    for refusal in refusals {
        for reply in [
            WireReplyV1::ConnectRefused(refusal),
            WireReplyV1::Executed(ReplyV1::Refused(refusal)),
            WireReplyV1::DisconnectRefused(refusal),
        ] {
            let encoded = encode_reply_v1(&reply, limits()).unwrap();
            assert_eq!(decode_reply_v1(&encoded, limits()).unwrap(), reply);
        }
    }

    for reply in [
        WireReplyV1::Connected(ClientIdV1(0)),
        WireReplyV1::Executed(ReplyV1::Accepted(OutcomeV1::MessageAccepted {
            revision: 0,
            sequence: u64::MAX,
        })),
        WireReplyV1::Disconnected,
    ] {
        let encoded = encode_reply_v1(&reply, limits()).unwrap();
        assert_eq!(decode_reply_v1(&encoded, limits()).unwrap(), reply);
    }

    for request in [
        WireRequestV1::Disconnect(ClientIdV1(0)),
        WireRequestV1::Execute {
            client: ClientIdV1(u64::MAX),
            request: RequestV1 {
                request_id: RequestIdV1(0),
                command: CommandV1::Snapshot {
                    room_id: "room".to_owned(),
                },
            },
        },
    ] {
        let encoded = encode_request_v1(&request, limits()).unwrap();
        assert_eq!(decode_request_v1(&encoded, limits()).unwrap(), request);
    }
}

#[test]
fn every_member_tag_round_trips() {
    for (kind, role) in [
        (ParticipantKindV1::Human, RoleV1::Owner),
        (ParticipantKindV1::Human, RoleV1::Moderator),
        (ParticipantKindV1::Human, RoleV1::Member),
        (ParticipantKindV1::Agent, RoleV1::Owner),
        (ParticipantKindV1::Agent, RoleV1::Moderator),
        (ParticipantKindV1::Agent, RoleV1::Member),
    ] {
        let reply = WireReplyV1::Executed(ReplyV1::Accepted(OutcomeV1::Snapshot(RoomSnapshotV1 {
            revision: 0,
            room_id: "r".to_owned(),
            name: "R".to_owned(),
            members: vec![MemberV1 {
                participant_id: "p".to_owned(),
                kind,
                role,
            }],
            messages: vec![],
            next_message_sequence: 0,
        })));
        let encoded = encode_reply_v1(&reply, limits()).unwrap();
        assert_eq!(decode_reply_v1(&encoded, limits()).unwrap(), reply);
    }
}
