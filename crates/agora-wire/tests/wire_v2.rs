use agora_protocol::v2::{
    AdmissionPolicyV2, BanPageV2, BannedParticipantV2, CapabilityV2, ChangeGapV2, ClientIdV2,
    CommandV2, IdentifiedMessagePageV1, IdentifiedMessageV1, MemberPageV2, MemberV2, MessageIdV1,
    MessagePageV2, MessageV2, OutcomeV2, PageCursorV2, ParticipantKindV2, RefusalV2, ReplyV2,
    RequestIdV2, RequestV2, RoleV2, RoomChangePageV2, RoomChangeV2, RoomEventV2, RoomPageV2,
    RoomSummaryV2,
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

fn changes_reply() -> WireReplyV2 {
    WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::Changes(RoomChangePageV2 {
        revision: 13,
        changes: vec![RoomChangeV2 {
            event_sequence: 12,
            revision: 13,
            event: RoomEventV2::MessageAccepted(MessageV2 {
                sequence: 4,
                author: "bot".to_owned(),
                logical_time: 9,
                body: "change body".to_owned(),
                addressed_to: vec!["alice".to_owned()],
            }),
        }],
        next: PageCursorV2(43),
        has_more: false,
    })))
}

fn moderation_reply() -> WireReplyV2 {
    WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::Changes(RoomChangePageV2 {
        revision: 16,
        changes: vec![RoomChangeV2 {
            event_sequence: 15,
            revision: 16,
            event: RoomEventV2::ParticipantBanned {
                actor_id: "alice".to_owned(),
                participant_id: "bot".to_owned(),
                reason: "policy".to_owned(),
                causation: [0x5a; 16],
            },
        }],
        next: PageCursorV2(16),
        has_more: false,
    })))
}

fn policy_reply() -> WireReplyV2 {
    WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::Changes(RoomChangePageV2 {
        revision: 17,
        changes: vec![RoomChangeV2 {
            event_sequence: 16,
            revision: 17,
            event: RoomEventV2::RoomPolicyChanged {
                actor_id: "alice".to_owned(),
                previous: AdmissionPolicyV2::InviteOnly,
                admission: AdmissionPolicyV2::Open,
            },
        }],
        next: PageCursorV2(17),
        has_more: false,
    })))
}

fn bans_reply() -> WireReplyV2 {
    WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::Bans(BanPageV2 {
        revision: 18,
        room_id: "private".to_owned(),
        banned: vec![BannedParticipantV2 {
            participant_id: "bot".to_owned(),
            display_name: "Bot".to_owned(),
            kind: ParticipantKindV2::Agent,
            prior_role: RoleV2::Moderator,
        }],
        next: Some(PageCursorV2(18)),
    })))
}

fn capabilities_reply() -> WireReplyV2 {
    WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::Capabilities(vec![
        CapabilityV2::RoomLifecycleV2,
        CapabilityV2::RetainedHistoryV2,
        CapabilityV2::RoomChangesV1,
        CapabilityV2::ModerationV1,
        CapabilityV2::RoomPolicyV1,
        CapabilityV2::BanObservationV1,
        CapabilityV2::IdentifiedMessagesV1,
    ])))
}

fn identified_messages_reply() -> WireReplyV2 {
    WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::IdentifiedMessages(
        IdentifiedMessagePageV1 {
            revision: 19,
            room_id: "private".to_owned(),
            messages: vec![IdentifiedMessageV1 {
                message_id: MessageIdV1::new([0x5a; 16]),
                sequence: 4,
                author: "bot".to_owned(),
                logical_time: 9,
                body: "identified body".to_owned(),
                addressed_to: vec!["alice".to_owned()],
            }],
            oldest_retained_sequence: 4,
            next_message_sequence: 5,
            next: None,
        },
    )))
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
        (
            encode_reply_v2(&changes_reply(), limits()).unwrap(),
            fixture(include_str!("fixtures/changes-reply-v2.hex")),
        ),
        (
            encode_reply_v2(&moderation_reply(), limits()).unwrap(),
            fixture(include_str!("fixtures/moderation-change-reply-v2.hex")),
        ),
        (
            encode_reply_v2(&policy_reply(), limits()).unwrap(),
            fixture(include_str!("fixtures/policy-change-reply-v2.hex")),
        ),
        (
            encode_reply_v2(&bans_reply(), limits()).unwrap(),
            fixture(include_str!("fixtures/bans-reply-v2.hex")),
        ),
        (
            encode_reply_v2(&capabilities_reply(), limits()).unwrap(),
            fixture(include_str!("fixtures/capabilities-reply-v2.hex")),
        ),
        (
            encode_reply_v2(&identified_messages_reply(), limits()).unwrap(),
            fixture(include_str!("fixtures/identified-messages-reply-v2.hex")),
        ),
    ];
    for (actual, expected) in cases {
        assert_eq!(actual, expected);
    }
    let rendered = format!(
        "{:?} {:?} {:?} {:?}",
        create_request(),
        rooms_reply(),
        messages_reply(),
        changes_reply()
    );
    for secret in [
        "private",
        "Private",
        "private body",
        "change body",
        "alice",
        "bot",
        "42",
        "43",
    ] {
        assert!(!rendered.contains(secret));
    }
}

#[test]
fn every_v2_command_round_trips() {
    let commands = [
        CommandV2::Capabilities,
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
        CommandV2::Bans {
            room_id: "room".to_owned(),
            cursor: Some(PageCursorV2(2)),
            limit: 3,
        },
        CommandV2::Messages {
            room_id: "room".to_owned(),
            cursor: None,
            limit: 4,
        },
        CommandV2::IdentifiedMessages {
            room_id: "room".to_owned(),
            cursor: Some(PageCursorV2(2)),
            limit: 3,
        },
        CommandV2::OpenChanges {
            room_id: "room".to_owned(),
            observed_revision: 4,
        },
        CommandV2::Changes {
            room_id: "room".to_owned(),
            cursor: PageCursorV2(3),
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
        CommandV2::SetAdmissionPolicy {
            room_id: "room".to_owned(),
            admission: AdmissionPolicyV2::InviteOnly,
            expected_revision: 6,
        },
        CommandV2::Kick {
            room_id: "room".to_owned(),
            participant_id: "bot".to_owned(),
            reason: "reason".to_owned(),
            causation: [1; 16],
            expected_revision: 6,
        },
        CommandV2::Ban {
            room_id: "room".to_owned(),
            participant_id: "bot".to_owned(),
            reason: "reason".to_owned(),
            causation: [2; 16],
            expected_revision: 7,
        },
        CommandV2::Unban {
            room_id: "room".to_owned(),
            participant_id: "bot".to_owned(),
            reason: "reason".to_owned(),
            causation: [3; 16],
            expected_revision: 8,
        },
        CommandV2::Speak {
            room_id: "room".to_owned(),
            expected_revision: 6,
            logical_time: 7,
            body: "body".to_owned(),
            addressed_to: vec!["alice".to_owned(), "bot".to_owned()],
        },
        CommandV2::SpeakIdentified {
            room_id: "room".to_owned(),
            expected_revision: 7,
            logical_time: 8,
            body: "identified".to_owned(),
            addressed_to: vec!["alice".to_owned()],
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
        OutcomeV2::Capabilities(vec![
            CapabilityV2::RoomLifecycleV2,
            CapabilityV2::RetainedHistoryV2,
        ]),
        match rooms_reply() {
            WireReplyV2::Executed(ReplyV2::Accepted(outcome)) => outcome,
            _ => unreachable!(),
        },
        member_page,
        match bans_reply() {
            WireReplyV2::Executed(ReplyV2::Accepted(outcome)) => outcome,
            _ => unreachable!(),
        },
        match messages_reply() {
            WireReplyV2::Executed(ReplyV2::Accepted(outcome)) => outcome,
            _ => unreachable!(),
        },
        match identified_messages_reply() {
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
        OutcomeV2::IdentifiedMessageAccepted {
            revision: 6,
            message_id: MessageIdV1::new([0x33; 16]),
            sequence: 7,
        },
        OutcomeV2::ChangesOpened {
            revision: 7,
            cursor: PageCursorV2(8),
        },
        OutcomeV2::Changes(RoomChangePageV2 {
            revision: 9,
            changes: vec![
                RoomChangeV2 {
                    event_sequence: 3,
                    revision: 4,
                    event: RoomEventV2::RoomCreated {
                        owner: "alice".to_owned(),
                        admission: AdmissionPolicyV2::Open,
                    },
                },
                RoomChangeV2 {
                    event_sequence: 4,
                    revision: 5,
                    event: RoomEventV2::InvitationRecorded {
                        participant_id: "bot".to_owned(),
                    },
                },
                RoomChangeV2 {
                    event_sequence: 5,
                    revision: 6,
                    event: RoomEventV2::ParticipantJoined {
                        participant_id: "bot".to_owned(),
                    },
                },
                RoomChangeV2 {
                    event_sequence: 6,
                    revision: 7,
                    event: RoomEventV2::RoleChanged {
                        participant_id: "bot".to_owned(),
                        role: RoleV2::Moderator,
                    },
                },
                RoomChangeV2 {
                    event_sequence: 7,
                    revision: 8,
                    event: RoomEventV2::ParticipantLeft {
                        participant_id: "bot".to_owned(),
                    },
                },
                RoomChangeV2 {
                    event_sequence: 8,
                    revision: 9,
                    event: RoomEventV2::MessageAccepted(MessageV2 {
                        sequence: 1,
                        author: "bot".to_owned(),
                        logical_time: 2,
                        body: "change".to_owned(),
                        addressed_to: vec!["alice".to_owned()],
                    }),
                },
                RoomChangeV2 {
                    event_sequence: 9,
                    revision: 10,
                    event: RoomEventV2::ParticipantKicked {
                        actor_id: "alice".to_owned(),
                        participant_id: "bot".to_owned(),
                        reason: "kick".to_owned(),
                        causation: [1; 16],
                    },
                },
                RoomChangeV2 {
                    event_sequence: 10,
                    revision: 11,
                    event: RoomEventV2::ParticipantBanned {
                        actor_id: "alice".to_owned(),
                        participant_id: "bot".to_owned(),
                        reason: "ban".to_owned(),
                        causation: [2; 16],
                    },
                },
                RoomChangeV2 {
                    event_sequence: 11,
                    revision: 12,
                    event: RoomEventV2::ParticipantUnbanned {
                        actor_id: "alice".to_owned(),
                        participant_id: "bot".to_owned(),
                        reason: "unban".to_owned(),
                        causation: [3; 16],
                    },
                },
                RoomChangeV2 {
                    event_sequence: 12,
                    revision: 13,
                    event: RoomEventV2::RoomPolicyChanged {
                        actor_id: "alice".to_owned(),
                        previous: AdmissionPolicyV2::Open,
                        admission: AdmissionPolicyV2::InviteOnly,
                    },
                },
            ],
            next: PageCursorV2(9),
            has_more: false,
        }),
        OutcomeV2::ChangeGap {
            current_revision: 10,
            cause: ChangeGapV2::Retention {
                acknowledged_through: 8,
            },
        },
        OutcomeV2::ChangeGap {
            current_revision: 11,
            cause: ChangeGapV2::MembershipChanged,
        },
        OutcomeV2::Kicked { revision: 12 },
        OutcomeV2::Banned { revision: 13 },
        OutcomeV2::Unbanned { revision: 14 },
        OutcomeV2::RoomPolicyChanged { revision: 15 },
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
        RefusalV2::Banned,
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

    let mut unknown_change_event = encode_reply_v2(&changes_reply(), limits()).unwrap();
    unknown_change_event[36] = 0x7f;
    assert_eq!(
        decode_reply_v2(&unknown_change_event, limits()),
        Err(WireDecodeError::UnknownTag)
    );
    let mut invalid_has_more = encode_reply_v2(&changes_reply(), limits()).unwrap();
    *invalid_has_more.last_mut().unwrap() = 2;
    assert_eq!(
        decode_reply_v2(&invalid_has_more, limits()),
        Err(WireDecodeError::UnknownTag)
    );
    let gap = WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::ChangeGap {
        current_revision: 9,
        cause: ChangeGapV2::MembershipChanged,
    }));
    let mut unknown_gap = encode_reply_v2(&gap, limits()).unwrap();
    unknown_gap[12] = 0x7f;
    assert_eq!(
        decode_reply_v2(&unknown_gap, limits()),
        Err(WireDecodeError::UnknownTag)
    );

    let noncanonical = WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::Capabilities(vec![
        CapabilityV2::ModerationV1,
        CapabilityV2::RoomLifecycleV2,
    ])));
    assert_eq!(
        encode_reply_v2(&noncanonical, limits()),
        Err(WireEncodeError::Invalid)
    );
    let mut duplicate = fixture(include_str!("fixtures/capabilities-reply-v2.hex"));
    *duplicate.last_mut().unwrap() = 5;
    assert_eq!(
        decode_reply_v2(&duplicate, limits()),
        Err(WireDecodeError::Malformed)
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
