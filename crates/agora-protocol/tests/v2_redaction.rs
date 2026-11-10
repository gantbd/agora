use agora_protocol::v2::{
    AdmissionPolicyV2, CommandV2, MemberPageV2, MemberV2, MessagePageV2, MessageV2, OutcomeV2,
    PageCursorV2, ParticipantKindV2, ReplyV2, RoleV2, RoomChangePageV2, RoomChangeV2, RoomEventV2,
    RoomPageV2, RoomSummaryV2,
};

#[test]
fn v2_debug_output_is_content_safe() {
    let command = CommandV2::Speak {
        room_id: "secret-room".to_owned(),
        expected_revision: 1,
        logical_time: 2,
        body: "secret-body".to_owned(),
        addressed_to: vec!["secret-recipient".to_owned()],
    };
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
    ];
    let rendered = format!("{command:?} {replies:?} {:?}", PageCursorV2(42));
    for secret in [
        "secret-room",
        "secret-body",
        "secret-recipient",
        "secret-name",
        "secret-participant",
        "secret-display",
        "secret-author",
        "42",
        "43",
    ] {
        assert!(!rendered.contains(secret));
    }
}
