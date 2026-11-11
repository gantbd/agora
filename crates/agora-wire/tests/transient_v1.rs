use agora_protocol::transient::{
    PresenceEntryV1, PresencePageCursorV1, PresencePageV1, PresenceSourceClassV1, PresenceStateV1,
    PresenceViewStateV1, TransientCommandV1, TransientOutcomeV1, TransientRefusalV1,
    TransientReplyV1, TransientRequestIdV1, TransientRequestV1,
};
use agora_protocol::v2::ParticipantKindV2;
use agora_wire::transient::{
    decode_transient_reply_v1, decode_transient_request_v1, encode_transient_reply_v1,
    encode_transient_request_v1,
};
use agora_wire::{WireDecodeError, WireEncodeError, WireLimits};

fn limits() -> WireLimits {
    WireLimits {
        max_frame_bytes: 4_096,
        max_text_bytes: 64,
        max_fields: 128,
        max_list_items: 8,
    }
}

fn request(command: TransientCommandV1) -> TransientRequestV1 {
    TransientRequestV1 {
        request_id: TransientRequestIdV1(7),
        command,
    }
}

fn page() -> PresencePageV1 {
    PresencePageV1 {
        entries: vec![
            PresenceEntryV1 {
                participant_id: "alice".to_owned(),
                participant_kind: ParticipantKindV2::Human,
                state: PresenceViewStateV1::Available,
                source: Some(PresenceSourceClassV1::ParticipantSession),
                remaining_lifetime: Some(5),
            },
            PresenceEntryV1 {
                participant_id: "bot".to_owned(),
                participant_kind: ParticipantKindV2::Agent,
                state: PresenceViewStateV1::Unknown,
                source: None,
                remaining_lifetime: None,
            },
        ],
        next: Some(PresencePageCursorV1(9)),
    }
}

fn fixture(input: &str) -> Vec<u8> {
    let compact: String = input
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    assert_eq!(compact.len() % 2, 0);
    (0..compact.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&compact[index..index + 2], 16).unwrap())
        .collect()
}

#[test]
fn every_request_and_reply_value_round_trips() {
    let requests = [
        request(TransientCommandV1::PublishPresence {
            room_id: "room".to_owned(),
            state: PresenceStateV1::Away,
            lifetime: 30,
        }),
        request(TransientCommandV1::ClearPresence {
            room_id: "room".to_owned(),
        }),
        request(TransientCommandV1::PresencePage {
            room_id: "room".to_owned(),
            cursor: Some(PresencePageCursorV1(9)),
            limit: 2,
        }),
    ];
    for request in requests {
        let encoded = encode_transient_request_v1(&request, limits()).unwrap();
        assert_eq!(
            decode_transient_request_v1(&encoded, limits()).unwrap(),
            request
        );
    }

    let mut replies = vec![
        TransientReplyV1::Accepted(TransientOutcomeV1::PresencePublished {
            accepted_lifetime: 30,
        }),
        TransientReplyV1::Accepted(TransientOutcomeV1::PresenceCleared { was_present: true }),
        TransientReplyV1::Accepted(TransientOutcomeV1::PresencePage(page())),
    ];
    for refusal in [
        TransientRefusalV1::NotMember,
        TransientRefusalV1::InvalidLifetime,
        TransientRefusalV1::InvalidPage,
        TransientRefusalV1::InvalidCursor,
        TransientRefusalV1::StaleCursor,
        TransientRefusalV1::Capacity,
        TransientRefusalV1::FidelityLost,
        TransientRefusalV1::Unavailable,
        TransientRefusalV1::RequestConflict,
        TransientRefusalV1::RequestOutOfSequence,
    ] {
        replies.push(TransientReplyV1::Refused(refusal));
    }
    for reply in replies {
        let encoded = encode_transient_reply_v1(&reply, limits()).unwrap();
        assert_eq!(
            decode_transient_reply_v1(&encoded, limits()).unwrap(),
            reply
        );
    }
}

#[test]
fn frozen_presence_frames_are_canonical() {
    let publish = request(TransientCommandV1::PublishPresence {
        room_id: "room".to_owned(),
        state: PresenceStateV1::Away,
        lifetime: 30,
    });
    assert_eq!(
        encode_transient_request_v1(&publish, limits()).unwrap(),
        fixture(include_str!("fixtures/transient-publish-request-v1.hex"))
    );
    let page = TransientReplyV1::Accepted(TransientOutcomeV1::PresencePage(page()));
    assert_eq!(
        encode_transient_reply_v1(&page, limits()).unwrap(),
        fixture(include_str!("fixtures/transient-presence-page-v1.hex"))
    );
}

#[test]
fn malformed_noncanonical_and_over_limit_frames_fail_closed() {
    let mut unknown_command = fixture(include_str!("fixtures/transient-publish-request-v1.hex"));
    unknown_command[9] = 0xff;
    assert_eq!(
        decode_transient_request_v1(&unknown_command, limits()),
        Err(WireDecodeError::UnknownTag)
    );

    let mut trailing = fixture(include_str!("fixtures/transient-publish-request-v1.hex"));
    trailing.push(0);
    assert_eq!(
        decode_transient_request_v1(&trailing, limits()),
        Err(WireDecodeError::TrailingData)
    );
    assert_eq!(
        decode_transient_request_v1(&trailing[..trailing.len() - 2], limits()),
        Err(WireDecodeError::Malformed)
    );

    let mut invalid = page();
    invalid.entries[1].participant_id = "alice".to_owned();
    assert_eq!(
        encode_transient_reply_v1(
            &TransientReplyV1::Accepted(TransientOutcomeV1::PresencePage(invalid)),
            limits(),
        ),
        Err(WireEncodeError::Invalid)
    );
    let mut invalid = page();
    invalid.entries[0].remaining_lifetime = None;
    assert_eq!(
        encode_transient_reply_v1(
            &TransientReplyV1::Accepted(TransientOutcomeV1::PresencePage(invalid)),
            limits(),
        ),
        Err(WireEncodeError::Invalid)
    );

    let tiny = WireLimits {
        max_frame_bytes: 8,
        ..limits()
    };
    assert_eq!(
        encode_transient_request_v1(
            &request(TransientCommandV1::ClearPresence {
                room_id: "room".to_owned(),
            }),
            tiny,
        ),
        Err(WireEncodeError::Limit)
    );
}

#[test]
fn diagnostics_hide_participant_room_and_cursor_values() {
    let request = request(TransientCommandV1::PresencePage {
        room_id: "secret-room".to_owned(),
        cursor: Some(PresencePageCursorV1(0xdead_beef)),
        limit: 2,
    });
    let debug = format!("{request:?} {:?}", page());
    assert!(!debug.contains("secret-room"));
    assert!(!debug.contains("alice"));
    assert!(!debug.contains("3735928559"));
}

#[test]
fn frozen_frames_have_bounded_deterministic_mutation_closure() {
    for (name, source, request_frame) in [
        (
            "publish-request",
            include_str!("fixtures/transient-publish-request-v1.hex"),
            true,
        ),
        (
            "presence-page",
            include_str!("fixtures/transient-presence-page-v1.hex"),
            false,
        ),
    ] {
        let bytes = fixture(source);
        assert_transient_canonical_if_accepted(name, &bytes, request_frame);
        for end in 0..bytes.len() {
            assert_transient_canonical_if_accepted(name, &bytes[..end], request_frame);
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_transient_canonical_if_accepted(name, &trailing, request_frame);
        for index in 0..bytes.len() {
            for bit in 0..8 {
                let mut candidate = bytes.clone();
                candidate[index] ^= 1 << bit;
                assert_transient_canonical_if_accepted(name, &candidate, request_frame);
            }
        }
    }
}

fn assert_transient_canonical_if_accepted(name: &str, candidate: &[u8], request_frame: bool) {
    if request_frame {
        if let Ok(value) = decode_transient_request_v1(candidate, limits()) {
            assert_eq!(
                encode_transient_request_v1(&value, limits()).unwrap(),
                candidate,
                "accepted request mutation must be canonical: {name}"
            );
        }
    } else if let Ok(value) = decode_transient_reply_v1(candidate, limits()) {
        assert_eq!(
            encode_transient_reply_v1(&value, limits()).unwrap(),
            candidate,
            "accepted reply mutation must be canonical: {name}"
        );
    }
}
