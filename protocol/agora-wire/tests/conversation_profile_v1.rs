use agora_protocol::v2::{
    CapabilityV2, ClientIdV2, CommandV2, ConversationProfileStatusV1, ConversationProfileV1,
    IdentifiedRoomChangePageV1, IdentifiedRoomChangeV1, IdentifiedRoomEventV1, OutcomeV2,
    PageCursorV2, PeerConversationCreatedV1, ReplyV2, RequestIdV2, RequestV2, RoomChangePageV2,
    RoomChangeV2, RoomEventV2, ThreadedRoomChangePageV1, ThreadedRoomChangeV1, ThreadedRoomEventV1,
    TopicRoomChangePageV1, TopicRoomChangeV1, TopicRoomEventV1,
};
use agora_wire::v2::{
    WireReplyV2, WireRequestV2, decode_reply_v2, decode_request_v2, encode_reply_v2,
    encode_request_v2,
};
use agora_wire::{WireDecodeError, WireEncodeError, WireLimits};

fn limits() -> WireLimits {
    WireLimits::reference()
}

fn decode_hex(source: &str) -> Vec<u8> {
    let compact = source.split_whitespace().collect::<String>();
    compact
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn peer_event() -> PeerConversationCreatedV1 {
    PeerConversationCreatedV1 {
        initiator_id: "alice".to_owned(),
        profile: ConversationProfileV1::Direct,
        founding_participant_ids: vec!["alice".to_owned(), "bot".to_owned()],
    }
}

#[test]
fn conversation_commands_status_capability_and_refusal_round_trip() {
    let values = [
        WireRequestV2::Execute {
            client: ClientIdV2(3),
            request: RequestV2 {
                request_id: RequestIdV2(5),
                command: CommandV2::CreatePeerConversation {
                    room_id: "direct".to_owned(),
                    name: "Direct".to_owned(),
                    profile: ConversationProfileV1::Direct,
                    founding_participant_ids: vec!["alice".to_owned(), "bot".to_owned()],
                    expected_revision: 8,
                },
            },
        },
        WireRequestV2::Execute {
            client: ClientIdV2(3),
            request: RequestV2 {
                request_id: RequestIdV2(6),
                command: CommandV2::ConversationProfile {
                    room_id: "direct".to_owned(),
                },
            },
        },
    ];
    assert_eq!(
        encode_request_v2(&values[0], limits()).unwrap(),
        decode_hex(include_str!(
            "fixtures/create-peer-conversation-request-v2.hex"
        ))
    );
    for value in values {
        let encoded = encode_request_v2(&value, limits()).unwrap();
        assert_eq!(decode_request_v2(&encoded, limits()).unwrap(), value);
    }

    let replies = [
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::PeerConversationCreated {
            revision: 9,
            profile: ConversationProfileV1::Direct,
        })),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::ConversationProfile(
            ConversationProfileStatusV1 {
                revision: 9,
                room_id: "direct".to_owned(),
                profile: ConversationProfileV1::Direct,
                founding_participant_ids: vec!["alice".to_owned(), "bot".to_owned()],
            },
        ))),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::Capabilities(vec![
            CapabilityV2::ConversationProfilesV1,
        ]))),
        WireReplyV2::Executed(ReplyV2::Refused(
            agora_protocol::v2::RefusalV2::ImmutableConversationCohort,
        )),
    ];
    assert_eq!(
        encode_reply_v2(&replies[1], limits()).unwrap(),
        decode_hex(include_str!(
            "fixtures/conversation-profile-status-reply-v2.hex"
        ))
    );
    for value in replies {
        let encoded = encode_reply_v2(&value, limits()).unwrap();
        assert_eq!(decode_reply_v2(&encoded, limits()).unwrap(), value);
    }
}

#[test]
fn peer_creation_event_round_trips_in_every_change_profile() {
    let replies = [
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::Changes(RoomChangePageV2 {
            revision: 2,
            changes: vec![RoomChangeV2 {
                event_sequence: 1,
                revision: 2,
                event: RoomEventV2::PeerConversationCreated(peer_event()),
            }],
            next: PageCursorV2(7),
            has_more: false,
        }))),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::IdentifiedChanges(
            IdentifiedRoomChangePageV1 {
                revision: 2,
                changes: vec![IdentifiedRoomChangeV1 {
                    event_sequence: 1,
                    revision: 2,
                    event: IdentifiedRoomEventV1::PeerConversationCreated(peer_event()),
                }],
                next: PageCursorV2(7),
                has_more: false,
            },
        ))),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::ThreadedChanges(
            ThreadedRoomChangePageV1 {
                revision: 2,
                changes: vec![ThreadedRoomChangeV1 {
                    event_sequence: 1,
                    revision: 2,
                    event: ThreadedRoomEventV1::PeerConversationCreated(peer_event()),
                }],
                next: PageCursorV2(7),
                has_more: false,
            },
        ))),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::TopicChanges(
            TopicRoomChangePageV1 {
                revision: 2,
                changes: vec![TopicRoomChangeV1 {
                    event_sequence: 1,
                    revision: 2,
                    event: TopicRoomEventV1::PeerConversationCreated(peer_event()),
                }],
                next: PageCursorV2(7),
                has_more: false,
            },
        ))),
    ];
    for value in replies {
        let encoded = encode_reply_v2(&value, limits()).unwrap();
        assert_eq!(decode_reply_v2(&encoded, limits()).unwrap(), value);
    }
}

#[test]
fn noncanonical_peer_cohorts_refuse_encoding() {
    let request = |profile, founding_participant_ids| WireRequestV2::Execute {
        client: ClientIdV2(1),
        request: RequestV2 {
            request_id: RequestIdV2(1),
            command: CommandV2::CreatePeerConversation {
                room_id: "direct".to_owned(),
                name: "Direct".to_owned(),
                profile,
                founding_participant_ids,
                expected_revision: 0,
            },
        },
    };
    for value in [
        request(ConversationProfileV1::Ordinary, vec!["alice".to_owned()]),
        request(
            ConversationProfileV1::Direct,
            vec!["bot".to_owned(), "alice".to_owned()],
        ),
        request(
            ConversationProfileV1::Direct,
            vec!["alice".to_owned(), "alice".to_owned()],
        ),
        request(ConversationProfileV1::Direct, vec!["alice".to_owned()]),
        request(
            ConversationProfileV1::GroupDirect,
            vec!["alice".to_owned(), "bot".to_owned()],
        ),
    ] {
        assert_eq!(
            encode_request_v2(&value, limits()),
            Err(WireEncodeError::Invalid)
        );
    }
}

#[test]
fn malformed_peer_cohort_cardinality_refuses_decoding() {
    let empty_direct = decode_hex(
        "02 02 00 00 00 00 00 00 00 03 00 00 00 00 00 00 00 05
         40 00 00 00 06 64 69 72 65 63 74 00 00 00 06 44 69 72
         65 63 74 02 00 00 00 00 00 00 00 00 00 00 00 00 00 00
         00 08",
    );
    assert_eq!(
        decode_request_v2(&empty_direct, limits()),
        Err(WireDecodeError::Malformed)
    );
}
