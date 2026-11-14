use agora_protocol::v2::{
    CapabilityV2, ClientIdV2, CommandV2, IdentifiedRoomChangePageV1, IdentifiedRoomChangeV1,
    IdentifiedRoomEventV1, MessageIdV1, OutcomeV2, PageCursorV2, PinnedReferenceV1, RefusalV2,
    ReplyV2, RequestIdV2, RequestV2, RoomChangePageV2, RoomChangeV2, RoomEventV2, SavedReferenceV1,
    StableReferenceAvailabilityV1, ThreadedRoomChangePageV1, ThreadedRoomChangeV1,
    ThreadedRoomEventV1, TopicRoomChangePageV1, TopicRoomChangeV1, TopicRoomEventV1,
};
use agora_wire::WireLimits;
use agora_wire::v2::{
    WireReplyV2, WireRequestV2, decode_reply_v2, decode_request_v2, encode_reply_v2,
    encode_request_v2,
};

fn limits() -> WireLimits {
    WireLimits::reference()
}

fn decode_hex(source: &str) -> Vec<u8> {
    source
        .split_whitespace()
        .map(|pair| u8::from_str_radix(pair, 16).unwrap())
        .collect()
}

#[test]
fn stable_reference_commands_outcomes_capabilities_and_refusals_are_closed() {
    let message_id = MessageIdV1::new([7; 16]);
    let commands = [
        CommandV2::Pins {
            room_id: "room".to_owned(),
        },
        CommandV2::SetPin {
            room_id: "room".to_owned(),
            expected_revision: 9,
            message_id,
            pinned: true,
        },
        CommandV2::SavedReference {
            room_id: "room".to_owned(),
            message_id,
        },
        CommandV2::SavedReferences {
            room_id: "room".to_owned(),
        },
        CommandV2::SetSavedReference {
            room_id: "room".to_owned(),
            observed_revision: 9,
            message_id,
            saved: true,
        },
    ];
    for (index, command) in commands.into_iter().enumerate() {
        let value = WireRequestV2::Execute {
            client: ClientIdV2(2),
            request: RequestV2 {
                request_id: RequestIdV2(index as u64 + 1),
                command,
            },
        };
        let encoded = encode_request_v2(&value, limits()).unwrap();
        if index == 1 {
            assert_eq!(
                encoded,
                decode_hex(include_str!("fixtures/set-pin-request-v2.hex"))
            );
        }
        assert_eq!(decode_request_v2(&encoded, limits()).unwrap(), value);
    }

    let saved = SavedReferenceV1 {
        message_id,
        original_sequence: 3,
        availability: StableReferenceAvailabilityV1::Tombstoned,
    };
    let pinned = PinnedReferenceV1 {
        message_id,
        original_sequence: 3,
        pinned_by: "agent".to_owned(),
        availability: StableReferenceAvailabilityV1::Evicted,
    };
    let outcomes = [
        OutcomeV2::Pins(vec![pinned]),
        OutcomeV2::PinChanged {
            revision: 10,
            message_id,
            original_sequence: 3,
            pinned: true,
        },
        OutcomeV2::SavedReference(Some(saved)),
        OutcomeV2::SavedReferences(vec![saved]),
        OutcomeV2::SavedReferenceSet {
            reference: saved,
            saved: true,
        },
        OutcomeV2::Capabilities(vec![
            CapabilityV2::SharedPinsV1,
            CapabilityV2::PrivateSavedReferencesV1,
        ]),
        OutcomeV2::Changes(RoomChangePageV2 {
            revision: 10,
            changes: vec![RoomChangeV2 {
                event_sequence: 4,
                revision: 10,
                event: RoomEventV2::PinChanged {
                    actor_id: "agent".to_owned(),
                    message_id,
                    original_sequence: 3,
                    pinned: true,
                },
            }],
            next: PageCursorV2(11),
            has_more: false,
        }),
        OutcomeV2::IdentifiedChanges(IdentifiedRoomChangePageV1 {
            revision: 10,
            changes: vec![IdentifiedRoomChangeV1 {
                event_sequence: 4,
                revision: 10,
                event: IdentifiedRoomEventV1::PinChanged {
                    actor_id: "agent".to_owned(),
                    message_id,
                    original_sequence: 3,
                    pinned: true,
                },
            }],
            next: PageCursorV2(12),
            has_more: false,
        }),
        OutcomeV2::ThreadedChanges(ThreadedRoomChangePageV1 {
            revision: 10,
            changes: vec![ThreadedRoomChangeV1 {
                event_sequence: 4,
                revision: 10,
                event: ThreadedRoomEventV1::PinChanged {
                    actor_id: "agent".to_owned(),
                    message_id,
                    original_sequence: 3,
                    pinned: true,
                },
            }],
            next: PageCursorV2(13),
            has_more: false,
        }),
        OutcomeV2::TopicChanges(TopicRoomChangePageV1 {
            revision: 10,
            changes: vec![TopicRoomChangeV1 {
                event_sequence: 4,
                revision: 10,
                event: TopicRoomEventV1::PinChanged {
                    actor_id: "agent".to_owned(),
                    message_id,
                    original_sequence: 3,
                    pinned: true,
                },
            }],
            next: PageCursorV2(14),
            has_more: false,
        }),
    ];
    for (index, outcome) in outcomes.into_iter().enumerate() {
        let value = WireReplyV2::Executed(ReplyV2::Accepted(outcome));
        let encoded = encode_reply_v2(&value, limits()).unwrap();
        if index == 3 {
            assert_eq!(
                encoded,
                decode_hex(include_str!("fixtures/saved-references-reply-v2.hex"))
            );
        }
        assert_eq!(decode_reply_v2(&encoded, limits()).unwrap(), value);
    }

    for refusal in [RefusalV2::InvalidPinChange, RefusalV2::InvalidSaveChange] {
        let value = WireReplyV2::Executed(ReplyV2::Refused(refusal));
        let encoded = encode_reply_v2(&value, limits()).unwrap();
        assert_eq!(decode_reply_v2(&encoded, limits()).unwrap(), value);
    }
}

#[test]
fn stable_reference_debug_views_do_not_copy_payloads() {
    let debug = format!(
        "{:?}",
        PinnedReferenceV1 {
            message_id: MessageIdV1::new([5; 16]),
            original_sequence: 7,
            pinned_by: "secret-agent".to_owned(),
            availability: StableReferenceAvailabilityV1::Available,
        }
    );
    assert!(!debug.contains("secret-agent"));
    assert!(debug.contains("pinned_by_id_bytes"));
}

#[test]
fn stable_reference_collection_bounds_refuse_before_reconstruction() {
    let saved = SavedReferenceV1 {
        message_id: MessageIdV1::new([1; 16]),
        original_sequence: 1,
        availability: StableReferenceAvailabilityV1::Available,
    };
    let value = WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::SavedReferences(vec![
        saved;
        4_097
    ])));
    assert!(encode_reply_v2(&value, limits()).is_err());

    let pin = PinnedReferenceV1 {
        message_id: MessageIdV1::new([1; 16]),
        original_sequence: 1,
        pinned_by: "actor".to_owned(),
        availability: StableReferenceAvailabilityV1::Available,
    };
    let value = WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::Pins(vec![pin; 257])));
    assert!(encode_reply_v2(&value, limits()).is_err());
}
