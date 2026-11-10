use agora_protocol::v2::{
    AdmissionPolicyV2, BanPageV2, BannedParticipantV2, CommandV2, MemberPageV2, MemberV2,
    MessagePageV2, MessageV2, OutcomeV2, PageCursorV2, ParticipantKindV2, ReplyV2, RoleV2,
    RoomChangePageV2, RoomChangeV2, RoomEventV2, RoomPageV2, RoomSummaryV2,
};

#[test]
fn v2_debug_output_is_content_safe() {
    let commands = [
        CommandV2::Speak {
            room_id: "secret-room".to_owned(),
            expected_revision: 1,
            logical_time: 2,
            body: "secret-body".to_owned(),
            addressed_to: vec!["secret-recipient".to_owned()],
        },
        CommandV2::Ban {
            room_id: "secret-room".to_owned(),
            participant_id: "secret-participant".to_owned(),
            reason: "secret-reason".to_owned(),
            causation: *b"secret-cause-123",
            expected_revision: 2,
        },
        CommandV2::SetAdmissionPolicy {
            room_id: "secret-room".to_owned(),
            admission: AdmissionPolicyV2::Open,
            expected_revision: 3,
        },
        CommandV2::Bans {
            room_id: "secret-room".to_owned(),
            cursor: Some(PageCursorV2(46)),
            limit: 1,
        },
    ];
    let replies = [
        ReplyV2::Accepted(OutcomeV2::Rooms(RoomPageV2 {
            revision: 1,
            rooms: vec![RoomSummaryV2 {
                room_id: "secret-room".to_owned(),
                name: "secret-name".to_owned(),
                admission: AdmissionPolicyV2::InviteOnly,
                observer_role: RoleV2::Owner,
                member_count: 1,
                retained_message_count: 1,
                next_message_sequence: 1,
            }],
            next: Some(PageCursorV2(42)),
        })),
        ReplyV2::Accepted(OutcomeV2::Members(MemberPageV2 {
            revision: 1,
            room_id: "secret-room".to_owned(),
            members: vec![MemberV2 {
                participant_id: "secret-participant".to_owned(),
                display_name: "secret-display".to_owned(),
                kind: ParticipantKindV2::Agent,
                role: RoleV2::Member,
            }],
            next: None,
        })),
        ReplyV2::Accepted(OutcomeV2::Bans(BanPageV2 {
            revision: 1,
            room_id: "secret-room".to_owned(),
            banned: vec![BannedParticipantV2 {
                participant_id: "secret-participant".to_owned(),
                display_name: "secret-display".to_owned(),
                kind: ParticipantKindV2::Agent,
                prior_role: RoleV2::Moderator,
            }],
            next: Some(PageCursorV2(47)),
        })),
        ReplyV2::Accepted(OutcomeV2::Messages(MessagePageV2 {
            revision: 1,
            room_id: "secret-room".to_owned(),
            messages: vec![MessageV2 {
                sequence: 0,
                author: "secret-author".to_owned(),
                logical_time: 2,
                body: "secret-body".to_owned(),
                addressed_to: vec!["secret-recipient".to_owned()],
            }],
            oldest_retained_sequence: 0,
            next_message_sequence: 1,
            next: None,
        })),
        ReplyV2::Accepted(OutcomeV2::Changes(RoomChangePageV2 {
            revision: 2,
            changes: vec![RoomChangeV2 {
                event_sequence: 1,
                revision: 2,
                event: RoomEventV2::MessageAccepted(MessageV2 {
                    sequence: 0,
                    author: "secret-author".to_owned(),
                    logical_time: 2,
                    body: "secret-body".to_owned(),
                    addressed_to: vec!["secret-recipient".to_owned()],
                }),
            }],
            next: PageCursorV2(43),
            has_more: false,
        })),
        ReplyV2::Accepted(OutcomeV2::Changes(RoomChangePageV2 {
            revision: 4,
            changes: vec![RoomChangeV2 {
                event_sequence: 3,
                revision: 4,
                event: RoomEventV2::RoomPolicyChanged {
                    actor_id: "secret-actor".to_owned(),
                    previous: AdmissionPolicyV2::InviteOnly,
                    admission: AdmissionPolicyV2::Open,
                },
            }],
            next: PageCursorV2(45),
            has_more: false,
        })),
        ReplyV2::Accepted(OutcomeV2::Changes(RoomChangePageV2 {
            revision: 3,
            changes: vec![RoomChangeV2 {
                event_sequence: 2,
                revision: 3,
                event: RoomEventV2::ParticipantBanned {
                    actor_id: "secret-actor".to_owned(),
                    participant_id: "secret-participant".to_owned(),
                    reason: "secret-reason".to_owned(),
                    causation: *b"secret-cause-123",
                },
            }],
            next: PageCursorV2(44),
            has_more: false,
        })),
    ];
    let rendered = format!("{commands:?} {replies:?} {:?}", PageCursorV2(42));
    for secret in [
        "secret-room",
        "secret-body",
        "secret-recipient",
        "secret-name",
        "secret-participant",
        "secret-display",
        "secret-author",
        "secret-reason",
        "secret-cause-123",
        "secret-actor",
        "42",
        "43",
        "44",
        "45",
        "46",
        "47",
    ] {
        assert!(!rendered.contains(secret));
    }
}
