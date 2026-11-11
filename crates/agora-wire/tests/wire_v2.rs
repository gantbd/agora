use agora_protocol::v2::{
    AdmissionPolicyV2, BanPageV2, BannedParticipantV2, CapabilityV2, ChangeGapV2, ClientIdV2,
    CommandV2, IdentifiedMessagePageV1, IdentifiedMessageV1, IdentifiedRoomChangePageV1,
    IdentifiedRoomChangeV1, IdentifiedRoomEventV1, MemberPageV2, MemberV2, MessageIdV1,
    MessagePageV2, MessageRevisionAvailabilityV1, MessageRevisionPageV1, MessageRevisionV1,
    MessageV2, OutcomeV2, PageCursorV2, ParticipantKindV2, ReactionStatusV1, ReactionSummaryV1,
    ReadPositionV1, ReadScopeV1, RefusalV2, ReplyV2, RequestIdV2, RequestV2, RoleV2,
    RoomChangePageV2, RoomChangeV2, RoomEventV2, RoomPageV2, RoomSummaryV2, ThreadLifecycleStateV1,
    ThreadMessagePageV1, ThreadStatusV1, ThreadedMessageV1, ThreadedRoomChangePageV1,
    ThreadedRoomChangeV1, ThreadedRoomEventV1, TopicIdV1, TopicMessagePageV1, TopicMessageV1,
    TopicPageV1, TopicRoomChangePageV1, TopicRoomChangeV1, TopicRoomEventV1, TopicV1,
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
        CapabilityV2::AuthorWithdrawalV1,
        CapabilityV2::ModeratorRedactionV1,
        CapabilityV2::IdentifiedRoomChangesV1,
        CapabilityV2::ThreadsV1,
        CapabilityV2::ThreadLifecycleV1,
        CapabilityV2::ReactionsV1,
        CapabilityV2::MessageRevisionsV1,
        CapabilityV2::TopicsV1,
        CapabilityV2::PrivateReadPositionsV1,
        CapabilityV2::IndependentChangeRetentionV1,
    ])))
}

fn change_retention_gap_reply() -> WireReplyV2 {
    WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::ChangeGap {
        current_revision: 42,
        cause: ChangeGapV2::ChangeRetention {
            discarded_through: 38,
        },
    }))
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

fn identified_changes_reply() -> WireReplyV2 {
    WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::IdentifiedChanges(
        IdentifiedRoomChangePageV1 {
            revision: 22,
            changes: vec![IdentifiedRoomChangeV1 {
                event_sequence: 21,
                revision: 22,
                event: IdentifiedRoomEventV1::MessageAccepted(IdentifiedMessageV1 {
                    message_id: MessageIdV1::new([0x8d; 16]),
                    sequence: 5,
                    author: "bot".to_owned(),
                    logical_time: 10,
                    body: "identified change".to_owned(),
                    addressed_to: vec!["alice".to_owned()],
                }),
            }],
            next: PageCursorV2(22),
            has_more: false,
        },
    )))
}

fn thread_replies_reply() -> WireReplyV2 {
    WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::ThreadReplies(
        ThreadMessagePageV1 {
            revision: 23,
            room_id: "private".to_owned(),
            root_message_id: Some(MessageIdV1::new([0x90; 16])),
            messages: vec![ThreadedMessageV1 {
                message_id: MessageIdV1::new([0x91; 16]),
                sequence: 6,
                author: "bot".to_owned(),
                logical_time: 11,
                body: "thread reply".to_owned(),
                addressed_to: vec!["alice".to_owned()],
                reply_to_root: Some(MessageIdV1::new([0x90; 16])),
            }],
            oldest_retained_sequence: 4,
            next_message_sequence: 7,
            next: None,
        },
    )))
}

fn thread_roots_reply() -> WireReplyV2 {
    WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::ThreadRoots(
        ThreadMessagePageV1 {
            revision: 23,
            room_id: "private".to_owned(),
            root_message_id: None,
            messages: vec![ThreadedMessageV1 {
                message_id: MessageIdV1::new([0x90; 16]),
                sequence: 5,
                author: "alice".to_owned(),
                logical_time: 10,
                body: "thread root".to_owned(),
                addressed_to: Vec::new(),
                reply_to_root: None,
            }],
            oldest_retained_sequence: 4,
            next_message_sequence: 7,
            next: None,
        },
    )))
}

fn threaded_changes_reply() -> WireReplyV2 {
    WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::ThreadedChanges(
        ThreadedRoomChangePageV1 {
            revision: 23,
            changes: vec![ThreadedRoomChangeV1 {
                event_sequence: 22,
                revision: 23,
                event: ThreadedRoomEventV1::MessageAccepted(ThreadedMessageV1 {
                    message_id: MessageIdV1::new([0x91; 16]),
                    sequence: 6,
                    author: "bot".to_owned(),
                    logical_time: 11,
                    body: "thread reply".to_owned(),
                    addressed_to: vec!["alice".to_owned()],
                    reply_to_root: Some(MessageIdV1::new([0x90; 16])),
                }),
            }],
            next: PageCursorV2(23),
            has_more: false,
        },
    )))
}

fn thread_lifecycle_change_reply() -> WireReplyV2 {
    WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::ThreadedChanges(
        ThreadedRoomChangePageV1 {
            revision: 24,
            changes: vec![ThreadedRoomChangeV1 {
                event_sequence: 23,
                revision: 24,
                event: ThreadedRoomEventV1::ThreadLifecycleChanged {
                    actor_id: "bot".to_owned(),
                    root_message_id: MessageIdV1::new([0x90; 16]),
                    archived: true,
                    reason: "pause".to_owned(),
                    causation: [0x92; 16],
                },
            }],
            next: PageCursorV2(24),
            has_more: false,
        },
    )))
}

fn thread_status_reply() -> WireReplyV2 {
    WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::ThreadStatus(ThreadStatusV1 {
        revision: 24,
        room_id: "private".to_owned(),
        root_message_id: MessageIdV1::new([0x90; 16]),
        state: ThreadLifecycleStateV1::Archived,
    })))
}

fn reaction_change_reply() -> WireReplyV2 {
    WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::IdentifiedChanges(
        IdentifiedRoomChangePageV1 {
            revision: 25,
            changes: vec![IdentifiedRoomChangeV1 {
                event_sequence: 24,
                revision: 25,
                event: IdentifiedRoomEventV1::ReactionChanged {
                    actor_id: "bot".to_owned(),
                    message_id: MessageIdV1::new([0x93; 16]),
                    key: "spark".to_owned(),
                    reacted: true,
                },
            }],
            next: PageCursorV2(25),
            has_more: false,
        },
    )))
}

fn reaction_status_reply() -> WireReplyV2 {
    WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::ReactionStatus(
        ReactionStatusV1 {
            revision: 25,
            room_id: "private".to_owned(),
            message_id: MessageIdV1::new([0x93; 16]),
            reactions: vec![ReactionSummaryV1 {
                key: "spark".to_owned(),
                count: 2,
                caller_reacted: true,
            }],
        },
    )))
}

fn message_revision_page_reply() -> WireReplyV2 {
    WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::MessageRevisions(
        MessageRevisionPageV1 {
            revision: 26,
            room_id: "private".to_owned(),
            message_id: MessageIdV1::new([0x94; 16]),
            revisions: vec![
                MessageRevisionV1 {
                    number: 0,
                    editor_id: "bot".to_owned(),
                    logical_time: 10,
                    availability: MessageRevisionAvailabilityV1::Available,
                    body: "original".to_owned(),
                },
                MessageRevisionV1 {
                    number: 1,
                    editor_id: "bot".to_owned(),
                    logical_time: 20,
                    availability: MessageRevisionAvailabilityV1::Available,
                    body: "revised".to_owned(),
                },
            ],
            next: Some(PageCursorV2(26)),
        },
    )))
}

fn message_edited_change_reply() -> WireReplyV2 {
    WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::IdentifiedChanges(
        IdentifiedRoomChangePageV1 {
            revision: 26,
            changes: vec![IdentifiedRoomChangeV1 {
                event_sequence: 25,
                revision: 26,
                event: IdentifiedRoomEventV1::MessageEdited {
                    actor_id: "bot".to_owned(),
                    message_id: MessageIdV1::new([0x94; 16]),
                    message_revision: 1,
                    logical_time: 20,
                    body: "revised".to_owned(),
                },
            }],
            next: PageCursorV2(26),
            has_more: false,
        },
    )))
}

fn read_position_reply() -> WireReplyV2 {
    WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::ReadPositionAdvanced(
        ReadPositionV1 {
            room_id: "private".to_owned(),
            scope: ReadScopeV1::Topic(TopicIdV1::new([0xa1; 16])),
            target_message_id: MessageIdV1::new([0xa2; 16]),
            target_sequence: 8,
            observed_revision: 9,
        },
    )))
}

fn withdrawal_change_reply() -> WireReplyV2 {
    WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::Changes(RoomChangePageV2 {
        revision: 20,
        changes: vec![RoomChangeV2 {
            event_sequence: 19,
            revision: 20,
            event: RoomEventV2::MessageWithdrawn {
                actor_id: "alice".to_owned(),
                message_id: MessageIdV1::new([0x6b; 16]),
                sequence: 4,
            },
        }],
        next: PageCursorV2(20),
        has_more: false,
    })))
}

fn redaction_change_reply() -> WireReplyV2 {
    WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::Changes(RoomChangePageV2 {
        revision: 21,
        changes: vec![RoomChangeV2 {
            event_sequence: 20,
            revision: 21,
            event: RoomEventV2::MessageRedacted {
                actor_id: "alice".to_owned(),
                author_id: "bot".to_owned(),
                message_id: MessageIdV1::new([0x7c; 16]),
                sequence: 4,
                reason: "policy".to_owned(),
                causation: [0x5d; 16],
            },
        }],
        next: PageCursorV2(21),
        has_more: false,
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
            encode_reply_v2(&change_retention_gap_reply(), limits()).unwrap(),
            fixture(include_str!("fixtures/change-retention-gap-reply-v2.hex")),
        ),
        (
            encode_reply_v2(&identified_messages_reply(), limits()).unwrap(),
            fixture(include_str!("fixtures/identified-messages-reply-v2.hex")),
        ),
        (
            encode_reply_v2(&withdrawal_change_reply(), limits()).unwrap(),
            fixture(include_str!("fixtures/withdrawal-change-reply-v2.hex")),
        ),
        (
            encode_reply_v2(&redaction_change_reply(), limits()).unwrap(),
            fixture(include_str!("fixtures/redaction-change-reply-v2.hex")),
        ),
        (
            encode_reply_v2(&identified_changes_reply(), limits()).unwrap(),
            fixture(include_str!("fixtures/identified-changes-reply-v2.hex")),
        ),
        (
            encode_reply_v2(&thread_replies_reply(), limits()).unwrap(),
            fixture(include_str!("fixtures/thread-replies-reply-v2.hex")),
        ),
        (
            encode_reply_v2(&threaded_changes_reply(), limits()).unwrap(),
            fixture(include_str!("fixtures/threaded-changes-reply-v2.hex")),
        ),
        (
            encode_reply_v2(&thread_lifecycle_change_reply(), limits()).unwrap(),
            fixture(include_str!(
                "fixtures/thread-lifecycle-change-reply-v2.hex"
            )),
        ),
        (
            encode_reply_v2(&thread_status_reply(), limits()).unwrap(),
            fixture(include_str!("fixtures/thread-status-reply-v2.hex")),
        ),
        (
            encode_reply_v2(&reaction_change_reply(), limits()).unwrap(),
            fixture(include_str!("fixtures/reaction-change-reply-v2.hex")),
        ),
        (
            encode_reply_v2(&reaction_status_reply(), limits()).unwrap(),
            fixture(include_str!("fixtures/reaction-status-reply-v2.hex")),
        ),
        (
            encode_reply_v2(&message_revision_page_reply(), limits()).unwrap(),
            fixture(include_str!("fixtures/message-revision-page-reply-v2.hex")),
        ),
        (
            encode_reply_v2(&message_edited_change_reply(), limits()).unwrap(),
            fixture(include_str!("fixtures/message-edited-change-reply-v2.hex")),
        ),
        (
            encode_reply_v2(&read_position_reply(), limits()).unwrap(),
            fixture(include_str!("fixtures/read-position-reply-v2.hex")),
        ),
    ];
    for (actual, expected) in cases {
        assert_eq!(actual, expected);
    }
    let rendered = format!(
        "{:?} {:?} {:?} {:?} {:?}",
        create_request(),
        rooms_reply(),
        messages_reply(),
        changes_reply(),
        read_position_reply()
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
        CommandV2::OpenIdentifiedChanges {
            room_id: "room".to_owned(),
            observed_revision: 4,
        },
        CommandV2::IdentifiedChanges {
            room_id: "room".to_owned(),
            cursor: PageCursorV2(3),
            limit: 4,
        },
        CommandV2::ThreadRoots {
            room_id: "room".to_owned(),
            cursor: Some(PageCursorV2(4)),
            limit: 2,
        },
        CommandV2::ThreadReplies {
            room_id: "room".to_owned(),
            root_message_id: MessageIdV1::new([0x90; 16]),
            cursor: None,
            limit: 2,
        },
        CommandV2::OpenThreadedChanges {
            room_id: "room".to_owned(),
            observed_revision: 4,
        },
        CommandV2::ThreadedChanges {
            room_id: "room".to_owned(),
            cursor: PageCursorV2(3),
            limit: 4,
        },
        CommandV2::ArchiveThread {
            room_id: "room".to_owned(),
            expected_revision: 4,
            root_message_id: MessageIdV1::new([0x90; 16]),
            reason: "pause".to_owned(),
            causation: [0x91; 16],
        },
        CommandV2::ReopenThread {
            room_id: "room".to_owned(),
            expected_revision: 5,
            root_message_id: MessageIdV1::new([0x90; 16]),
            reason: "resume".to_owned(),
            causation: [0x92; 16],
        },
        CommandV2::ThreadStatus {
            room_id: "room".to_owned(),
            root_message_id: MessageIdV1::new([0x90; 16]),
        },
        CommandV2::AddReaction {
            room_id: "room".to_owned(),
            expected_revision: 6,
            message_id: MessageIdV1::new([0x93; 16]),
            key: "spark".to_owned(),
        },
        CommandV2::RemoveReaction {
            room_id: "room".to_owned(),
            expected_revision: 7,
            message_id: MessageIdV1::new([0x93; 16]),
            key: "spark".to_owned(),
        },
        CommandV2::ReactionStatus {
            room_id: "room".to_owned(),
            message_id: MessageIdV1::new([0x93; 16]),
        },
        CommandV2::EditMessage {
            room_id: "room".to_owned(),
            expected_revision: 8,
            message_id: MessageIdV1::new([0x94; 16]),
            logical_time: 20,
            body: "revised".to_owned(),
        },
        CommandV2::MessageRevisions {
            room_id: "room".to_owned(),
            message_id: MessageIdV1::new([0x94; 16]),
            cursor: Some(PageCursorV2(9)),
            limit: 2,
        },
        CommandV2::ReadPosition {
            room_id: "room".to_owned(),
            scope: ReadScopeV1::Room,
        },
        CommandV2::ReadPosition {
            room_id: "room".to_owned(),
            scope: ReadScopeV1::Thread(MessageIdV1::new([0xa0; 16])),
        },
        CommandV2::ReadPosition {
            room_id: "room".to_owned(),
            scope: ReadScopeV1::Topic(TopicIdV1::new([0xa1; 16])),
        },
        CommandV2::AdvanceReadPosition {
            room_id: "room".to_owned(),
            observed_revision: 9,
            scope: ReadScopeV1::Topic(TopicIdV1::new([0xa1; 16])),
            target_message_id: MessageIdV1::new([0xa2; 16]),
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
        CommandV2::SpeakReply {
            room_id: "room".to_owned(),
            expected_revision: 8,
            root_message_id: MessageIdV1::new([0x90; 16]),
            logical_time: 9,
            body: "reply".to_owned(),
            addressed_to: vec!["alice".to_owned()],
        },
        CommandV2::WithdrawMessage {
            room_id: "room".to_owned(),
            expected_revision: 8,
            message_id: MessageIdV1::new([0x44; 16]),
        },
        CommandV2::RedactMessage {
            room_id: "room".to_owned(),
            expected_revision: 9,
            message_id: MessageIdV1::new([0x45; 16]),
            reason: "policy".to_owned(),
            causation: [0x46; 16],
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
        match thread_roots_reply() {
            WireReplyV2::Executed(ReplyV2::Accepted(outcome)) => outcome,
            _ => unreachable!(),
        },
        match thread_replies_reply() {
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
        OutcomeV2::MessageWithdrawn {
            revision: 7,
            message_id: MessageIdV1::new([0x44; 16]),
            sequence: 8,
        },
        OutcomeV2::MessageRedacted {
            revision: 8,
            message_id: MessageIdV1::new([0x45; 16]),
            sequence: 9,
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
                    event_sequence: 17,
                    revision: 18,
                    event: RoomEventV2::ReactionChanged {
                        actor_id: "bot".to_owned(),
                        message_id: MessageIdV1::new([0x50; 16]),
                        key: "spark".to_owned(),
                        reacted: true,
                    },
                },
                RoomChangeV2 {
                    event_sequence: 18,
                    revision: 19,
                    event: RoomEventV2::MessageEdited {
                        actor_id: "bot".to_owned(),
                        message_id: MessageIdV1::new([0x51; 16]),
                        message_revision: 1,
                        logical_time: 20,
                        body: "revised".to_owned(),
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
                RoomChangeV2 {
                    event_sequence: 13,
                    revision: 14,
                    event: RoomEventV2::MessageWithdrawn {
                        actor_id: "bot".to_owned(),
                        message_id: MessageIdV1::new([0x44; 16]),
                        sequence: 1,
                    },
                },
                RoomChangeV2 {
                    event_sequence: 14,
                    revision: 15,
                    event: RoomEventV2::MessageRedacted {
                        actor_id: "alice".to_owned(),
                        author_id: "bot".to_owned(),
                        message_id: MessageIdV1::new([0x45; 16]),
                        sequence: 1,
                        reason: "policy".to_owned(),
                        causation: [0x46; 16],
                    },
                },
                RoomChangeV2 {
                    event_sequence: 15,
                    revision: 16,
                    event: RoomEventV2::ThreadLifecycleChanged {
                        actor_id: "bot".to_owned(),
                        root_message_id: MessageIdV1::new([0x47; 16]),
                        archived: true,
                        reason: "pause".to_owned(),
                        causation: [0x48; 16],
                    },
                },
                RoomChangeV2 {
                    event_sequence: 16,
                    revision: 17,
                    event: RoomEventV2::ThreadLifecycleChanged {
                        actor_id: "bot".to_owned(),
                        root_message_id: MessageIdV1::new([0x47; 16]),
                        archived: false,
                        reason: "resume".to_owned(),
                        causation: [0x49; 16],
                    },
                },
            ],
            next: PageCursorV2(9),
            has_more: false,
        }),
        match identified_changes_reply() {
            WireReplyV2::Executed(ReplyV2::Accepted(outcome)) => outcome,
            _ => unreachable!(),
        },
        match threaded_changes_reply() {
            WireReplyV2::Executed(ReplyV2::Accepted(outcome)) => outcome,
            _ => unreachable!(),
        },
        OutcomeV2::ReplyAccepted {
            revision: 9,
            message_id: MessageIdV1::new([0x91; 16]),
            root_message_id: MessageIdV1::new([0x90; 16]),
            sequence: 6,
        },
        OutcomeV2::ThreadArchived {
            revision: 10,
            root_message_id: MessageIdV1::new([0x90; 16]),
        },
        OutcomeV2::ThreadReopened {
            revision: 11,
            root_message_id: MessageIdV1::new([0x90; 16]),
        },
        match thread_status_reply() {
            WireReplyV2::Executed(ReplyV2::Accepted(outcome)) => outcome,
            _ => unreachable!(),
        },
        OutcomeV2::ReactionStatus(ReactionStatusV1 {
            revision: 12,
            room_id: "room".to_owned(),
            message_id: MessageIdV1::new([0x93; 16]),
            reactions: vec![ReactionSummaryV1 {
                key: "spark".to_owned(),
                count: 2,
                caller_reacted: true,
            }],
        }),
        OutcomeV2::ReactionAdded {
            revision: 13,
            message_id: MessageIdV1::new([0x93; 16]),
        },
        OutcomeV2::ReactionRemoved {
            revision: 14,
            message_id: MessageIdV1::new([0x93; 16]),
        },
        match message_revision_page_reply() {
            WireReplyV2::Executed(ReplyV2::Accepted(outcome)) => outcome,
            _ => unreachable!(),
        },
        OutcomeV2::MessageEdited {
            revision: 15,
            message_id: MessageIdV1::new([0x94; 16]),
            message_revision: 1,
        },
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
        OutcomeV2::ChangeGap {
            current_revision: 12,
            cause: ChangeGapV2::ChangeRetention {
                discarded_through: 9,
            },
        },
        OutcomeV2::Kicked { revision: 12 },
        OutcomeV2::Banned { revision: 13 },
        OutcomeV2::Unbanned { revision: 14 },
        OutcomeV2::RoomPolicyChanged { revision: 15 },
        OutcomeV2::ReadPosition(None),
        OutcomeV2::ReadPosition(Some(ReadPositionV1 {
            room_id: "room".to_owned(),
            scope: ReadScopeV1::Thread(MessageIdV1::new([0xa0; 16])),
            target_message_id: MessageIdV1::new([0xa2; 16]),
            target_sequence: 8,
            observed_revision: 9,
        })),
        OutcomeV2::ReadPositionAdvanced(ReadPositionV1 {
            room_id: "room".to_owned(),
            scope: ReadScopeV1::Topic(TopicIdV1::new([0xa1; 16])),
            target_message_id: MessageIdV1::new([0xa2; 16]),
            target_sequence: 8,
            observed_revision: 9,
        }),
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
        RefusalV2::ThreadArchived,
        RefusalV2::UnknownTopic,
        RefusalV2::InvalidReadScope,
        RefusalV2::InvalidReadTarget,
        RefusalV2::ReadPositionRegression,
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
fn every_identified_change_event_round_trips() {
    let events = [
        IdentifiedRoomEventV1::RoomCreated {
            owner: "alice".to_owned(),
            admission: AdmissionPolicyV2::Open,
        },
        IdentifiedRoomEventV1::InvitationRecorded {
            participant_id: "bot".to_owned(),
        },
        IdentifiedRoomEventV1::ParticipantJoined {
            participant_id: "bot".to_owned(),
        },
        IdentifiedRoomEventV1::ParticipantLeft {
            participant_id: "bot".to_owned(),
        },
        IdentifiedRoomEventV1::RoleChanged {
            participant_id: "bot".to_owned(),
            role: RoleV2::Moderator,
        },
        IdentifiedRoomEventV1::MessageAccepted(IdentifiedMessageV1 {
            message_id: MessageIdV1::new([6; 16]),
            sequence: 6,
            author: "bot".to_owned(),
            logical_time: 7,
            body: "body".to_owned(),
            addressed_to: vec!["alice".to_owned()],
        }),
        IdentifiedRoomEventV1::ParticipantKicked {
            actor_id: "alice".to_owned(),
            participant_id: "bot".to_owned(),
            reason: "kick".to_owned(),
            causation: [7; 16],
        },
        IdentifiedRoomEventV1::ParticipantBanned {
            actor_id: "alice".to_owned(),
            participant_id: "bot".to_owned(),
            reason: "ban".to_owned(),
            causation: [8; 16],
        },
        IdentifiedRoomEventV1::ParticipantUnbanned {
            actor_id: "alice".to_owned(),
            participant_id: "bot".to_owned(),
            reason: "unban".to_owned(),
            causation: [9; 16],
        },
        IdentifiedRoomEventV1::RoomPolicyChanged {
            actor_id: "alice".to_owned(),
            previous: AdmissionPolicyV2::Open,
            admission: AdmissionPolicyV2::InviteOnly,
        },
        IdentifiedRoomEventV1::MessageWithdrawn {
            actor_id: "bot".to_owned(),
            message_id: MessageIdV1::new([11; 16]),
            sequence: 6,
        },
        IdentifiedRoomEventV1::MessageRedacted {
            actor_id: "alice".to_owned(),
            author_id: "bot".to_owned(),
            message_id: MessageIdV1::new([12; 16]),
            sequence: 6,
            reason: "policy".to_owned(),
            causation: [12; 16],
        },
        IdentifiedRoomEventV1::ThreadLifecycleChanged {
            actor_id: "bot".to_owned(),
            root_message_id: MessageIdV1::new([13; 16]),
            archived: true,
            reason: "pause".to_owned(),
            causation: [13; 16],
        },
        IdentifiedRoomEventV1::ThreadLifecycleChanged {
            actor_id: "bot".to_owned(),
            root_message_id: MessageIdV1::new([13; 16]),
            archived: false,
            reason: "resume".to_owned(),
            causation: [14; 16],
        },
        IdentifiedRoomEventV1::ReactionChanged {
            actor_id: "bot".to_owned(),
            message_id: MessageIdV1::new([15; 16]),
            key: "spark".to_owned(),
            reacted: true,
        },
        IdentifiedRoomEventV1::MessageEdited {
            actor_id: "bot".to_owned(),
            message_id: MessageIdV1::new([16; 16]),
            message_revision: 1,
            logical_time: 20,
            body: "revised".to_owned(),
        },
    ];
    for (index, event) in events.into_iter().enumerate() {
        let reply = WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::IdentifiedChanges(
            IdentifiedRoomChangePageV1 {
                revision: 20,
                changes: vec![IdentifiedRoomChangeV1 {
                    event_sequence: index as u64,
                    revision: 20,
                    event,
                }],
                next: PageCursorV2(21),
                has_more: index % 2 == 0,
            },
        )));
        let encoded = encode_reply_v2(&reply, limits()).unwrap();
        assert_eq!(decode_reply_v2(&encoded, limits()).unwrap(), reply);
    }
}

#[test]
fn every_threaded_change_event_round_trips() {
    let events = [
        ThreadedRoomEventV1::RoomCreated {
            owner: "alice".to_owned(),
            admission: AdmissionPolicyV2::Open,
        },
        ThreadedRoomEventV1::InvitationRecorded {
            participant_id: "bot".to_owned(),
        },
        ThreadedRoomEventV1::ParticipantJoined {
            participant_id: "bot".to_owned(),
        },
        ThreadedRoomEventV1::ParticipantLeft {
            participant_id: "bot".to_owned(),
        },
        ThreadedRoomEventV1::RoleChanged {
            participant_id: "bot".to_owned(),
            role: RoleV2::Moderator,
        },
        ThreadedRoomEventV1::MessageAccepted(ThreadedMessageV1 {
            message_id: MessageIdV1::new([6; 16]),
            sequence: 6,
            author: "bot".to_owned(),
            logical_time: 7,
            body: "body".to_owned(),
            addressed_to: vec!["alice".to_owned()],
            reply_to_root: Some(MessageIdV1::new([5; 16])),
        }),
        ThreadedRoomEventV1::ParticipantKicked {
            actor_id: "alice".to_owned(),
            participant_id: "bot".to_owned(),
            reason: "kick".to_owned(),
            causation: [7; 16],
        },
        ThreadedRoomEventV1::ParticipantBanned {
            actor_id: "alice".to_owned(),
            participant_id: "bot".to_owned(),
            reason: "ban".to_owned(),
            causation: [8; 16],
        },
        ThreadedRoomEventV1::ParticipantUnbanned {
            actor_id: "alice".to_owned(),
            participant_id: "bot".to_owned(),
            reason: "unban".to_owned(),
            causation: [9; 16],
        },
        ThreadedRoomEventV1::RoomPolicyChanged {
            actor_id: "alice".to_owned(),
            previous: AdmissionPolicyV2::Open,
            admission: AdmissionPolicyV2::InviteOnly,
        },
        ThreadedRoomEventV1::MessageWithdrawn {
            actor_id: "bot".to_owned(),
            message_id: MessageIdV1::new([11; 16]),
            sequence: 6,
        },
        ThreadedRoomEventV1::MessageRedacted {
            actor_id: "alice".to_owned(),
            author_id: "bot".to_owned(),
            message_id: MessageIdV1::new([12; 16]),
            sequence: 6,
            reason: "policy".to_owned(),
            causation: [12; 16],
        },
        ThreadedRoomEventV1::ThreadLifecycleChanged {
            actor_id: "bot".to_owned(),
            root_message_id: MessageIdV1::new([13; 16]),
            archived: true,
            reason: "pause".to_owned(),
            causation: [13; 16],
        },
        ThreadedRoomEventV1::ThreadLifecycleChanged {
            actor_id: "bot".to_owned(),
            root_message_id: MessageIdV1::new([13; 16]),
            archived: false,
            reason: "resume".to_owned(),
            causation: [14; 16],
        },
        ThreadedRoomEventV1::ReactionChanged {
            actor_id: "bot".to_owned(),
            message_id: MessageIdV1::new([15; 16]),
            key: "spark".to_owned(),
            reacted: false,
        },
        ThreadedRoomEventV1::MessageEdited {
            actor_id: "bot".to_owned(),
            message_id: MessageIdV1::new([16; 16]),
            message_revision: 1,
            logical_time: 20,
            body: "revised".to_owned(),
        },
    ];
    for (index, event) in events.into_iter().enumerate() {
        let reply = WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::ThreadedChanges(
            ThreadedRoomChangePageV1 {
                revision: 20,
                changes: vec![ThreadedRoomChangeV1 {
                    event_sequence: index as u64,
                    revision: 20,
                    event,
                }],
                next: PageCursorV2(21),
                has_more: index % 2 == 0,
            },
        )));
        let encoded = encode_reply_v2(&reply, limits()).unwrap();
        assert_eq!(decode_reply_v2(&encoded, limits()).unwrap(), reply);
    }
}

#[test]
fn v2_decoding_is_closed_and_version_isolated() {
    let request = encode_request_v2(&create_request(), limits()).unwrap();
    for end in 0..request.len() {
        assert!(decode_request_v2(&request[..end], limits()).is_err());
    }
    for reply in [
        rooms_reply(),
        messages_reply(),
        identified_changes_reply(),
        thread_roots_reply(),
        thread_replies_reply(),
        threaded_changes_reply(),
        thread_lifecycle_change_reply(),
        thread_status_reply(),
        message_revision_page_reply(),
        message_edited_change_reply(),
    ] {
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
    let mut unknown_identified_change_event =
        encode_reply_v2(&identified_changes_reply(), limits()).unwrap();
    unknown_identified_change_event[36] = 0x7f;
    assert_eq!(
        decode_reply_v2(&unknown_identified_change_event, limits()),
        Err(WireDecodeError::UnknownTag)
    );
    let mut unknown_threaded_change_event =
        encode_reply_v2(&threaded_changes_reply(), limits()).unwrap();
    unknown_threaded_change_event[36] = 0x7f;
    assert_eq!(
        decode_reply_v2(&unknown_threaded_change_event, limits()),
        Err(WireDecodeError::UnknownTag)
    );
    let mut unknown_thread_lifecycle_event =
        encode_reply_v2(&thread_lifecycle_change_reply(), limits()).unwrap();
    unknown_thread_lifecycle_event[36] = 0x7f;
    assert_eq!(
        decode_reply_v2(&unknown_thread_lifecycle_event, limits()),
        Err(WireDecodeError::UnknownTag)
    );
    let mut unknown_thread_state = encode_reply_v2(&thread_status_reply(), limits()).unwrap();
    *unknown_thread_state.last_mut().unwrap() = 0x7f;
    assert_eq!(
        decode_reply_v2(&unknown_thread_state, limits()),
        Err(WireDecodeError::UnknownTag)
    );
    let mut unknown_revision_availability =
        encode_reply_v2(&message_revision_page_reply(), limits()).unwrap();
    unknown_revision_availability[70] = 0x7f;
    assert_eq!(
        decode_reply_v2(&unknown_revision_availability, limits()),
        Err(WireDecodeError::UnknownTag)
    );
    let mut unknown_thread_root_tag = encode_reply_v2(&thread_replies_reply(), limits()).unwrap();
    unknown_thread_root_tag[23] = 0x7f;
    assert_eq!(
        decode_reply_v2(&unknown_thread_root_tag, limits()),
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
    let mut invalid_revision_page = message_revision_page_reply();
    let WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::MessageRevisions(page))) =
        &mut invalid_revision_page
    else {
        unreachable!();
    };
    page.revisions[0].availability = MessageRevisionAvailabilityV1::Tombstoned;
    assert_eq!(
        encode_reply_v2(&invalid_revision_page, limits()),
        Err(WireEncodeError::Invalid)
    );

    let mut invalid_root_page = thread_roots_reply();
    let WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::ThreadRoots(page))) =
        &mut invalid_root_page
    else {
        unreachable!()
    };
    page.messages[0].reply_to_root = Some(MessageIdV1::new([0x90; 16]));
    assert_eq!(
        encode_reply_v2(&invalid_root_page, limits()),
        Err(WireEncodeError::Invalid)
    );

    let mut mismatched_reply = encode_reply_v2(&thread_replies_reply(), limits()).unwrap();
    let relation_offsets: Vec<_> = mismatched_reply
        .windows(16)
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == [0x90; 16]).then_some(offset))
        .collect();
    assert_eq!(relation_offsets.len(), 2);
    mismatched_reply[relation_offsets[1]] = 0x92;
    assert_eq!(
        decode_reply_v2(&mismatched_reply, limits()),
        Err(WireDecodeError::Malformed)
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

#[test]
fn v2_topic_commands_pages_and_mutation_outcomes_are_canonical() {
    let topic_id = TopicIdV1::new([0x44; 16]);
    let message_id = MessageIdV1::new([0x55; 16]);
    let requests = [
        CommandV2::Topics {
            room_id: "room".to_owned(),
            cursor: Some(PageCursorV2(7)),
            limit: 2,
        },
        CommandV2::TopicMessages {
            room_id: "room".to_owned(),
            topic_id,
            cursor: None,
            limit: 3,
        },
        CommandV2::CreateTopic {
            room_id: "room".to_owned(),
            expected_revision: 8,
            name: "incidents".to_owned(),
        },
        CommandV2::SpeakInTopic {
            room_id: "room".to_owned(),
            expected_revision: 9,
            topic_id,
            logical_time: 10,
            body: "evidence".to_owned(),
            addressed_to: vec!["alice".to_owned()],
        },
        CommandV2::OpenTopicChanges {
            room_id: "room".to_owned(),
            observed_revision: 10,
        },
        CommandV2::TopicChanges {
            room_id: "room".to_owned(),
            cursor: PageCursorV2(12),
            limit: 2,
        },
    ];
    for (offset, command) in requests.into_iter().enumerate() {
        let request = WireRequestV2::Execute {
            client: ClientIdV2(1),
            request: RequestV2 {
                request_id: RequestIdV2(offset as u64),
                command,
            },
        };
        let encoded = encode_request_v2(&request, limits()).unwrap();
        assert_eq!(decode_request_v2(&encoded, limits()).unwrap(), request);
    }

    let replies = [
        OutcomeV2::Topics(TopicPageV1 {
            revision: 10,
            room_id: "room".to_owned(),
            topics: vec![TopicV1 {
                topic_id,
                name: "incidents".to_owned(),
                creator_id: "alice".to_owned(),
            }],
            next: Some(PageCursorV2(11)),
        }),
        OutcomeV2::TopicMessages(TopicMessagePageV1 {
            revision: 10,
            room_id: "room".to_owned(),
            topic_id,
            messages: vec![TopicMessageV1 {
                message_id,
                sequence: 4,
                author: "bot".to_owned(),
                logical_time: 9,
                body: "evidence".to_owned(),
                addressed_to: vec!["alice".to_owned()],
                reply_to_root: None,
                topic_id: Some(topic_id),
            }],
            oldest_retained_sequence: 3,
            next_message_sequence: 5,
            next: None,
        }),
        OutcomeV2::TopicCreated {
            revision: 9,
            topic_id,
        },
        OutcomeV2::TopicMessageAccepted {
            revision: 10,
            message_id,
            topic_id,
            sequence: 4,
        },
        OutcomeV2::TopicChanges(TopicRoomChangePageV1 {
            revision: 10,
            changes: vec![TopicRoomChangeV1 {
                event_sequence: 8,
                revision: 10,
                event: TopicRoomEventV1::MessageAccepted(TopicMessageV1 {
                    message_id,
                    sequence: 4,
                    author: "bot".to_owned(),
                    logical_time: 9,
                    body: "evidence".to_owned(),
                    addressed_to: vec!["alice".to_owned()],
                    reply_to_root: None,
                    topic_id: Some(topic_id),
                }),
            }],
            next: PageCursorV2(13),
            has_more: false,
        }),
    ];
    for outcome in replies {
        let reply = WireReplyV2::Executed(ReplyV2::Accepted(outcome));
        let encoded = encode_reply_v2(&reply, limits()).unwrap();
        assert_eq!(decode_reply_v2(&encoded, limits()).unwrap(), reply);
    }

    let invalid = WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::TopicMessages(
        TopicMessagePageV1 {
            revision: 10,
            room_id: "room".to_owned(),
            topic_id,
            messages: vec![TopicMessageV1 {
                message_id,
                sequence: 4,
                author: "bot".to_owned(),
                logical_time: 9,
                body: "evidence".to_owned(),
                addressed_to: vec![],
                reply_to_root: None,
                topic_id: Some(TopicIdV1::new([0x66; 16])),
            }],
            oldest_retained_sequence: 3,
            next_message_sequence: 5,
            next: None,
        },
    )));
    assert_eq!(
        encode_reply_v2(&invalid, limits()),
        Err(WireEncodeError::Invalid)
    );

    let refusal = WireReplyV2::Executed(ReplyV2::Refused(RefusalV2::UnknownTopic));
    let encoded = encode_reply_v2(&refusal, limits()).unwrap();
    assert_eq!(decode_reply_v2(&encoded, limits()).unwrap(), refusal);
}
