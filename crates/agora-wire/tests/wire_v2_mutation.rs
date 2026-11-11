use agora_wire::v2::{decode_reply_v2, decode_request_v2, encode_reply_v2, encode_request_v2};
use agora_wire::{
    WireLimits, decode_reply_v1, decode_request_v1, encode_reply_v1, encode_request_v1,
};

#[derive(Clone, Copy)]
enum FrameKind {
    Request,
    Reply,
}

fn assert_v1_request_canonical_if_accepted(name: &str, candidate: &[u8]) {
    if let Ok(value) = decode_request_v1(candidate, limits()) {
        assert_eq!(
            encode_request_v1(&value, limits()).unwrap(),
            candidate,
            "accepted request mutation must be canonical: {name}"
        );
    }
}

fn assert_v1_reply_canonical_if_accepted(name: &str, candidate: &[u8]) {
    if let Ok(value) = decode_reply_v1(candidate, limits()) {
        assert_eq!(
            encode_reply_v1(&value, limits()).unwrap(),
            candidate,
            "accepted reply mutation must be canonical: {name}"
        );
    }
}

fn limits() -> WireLimits {
    WireLimits {
        max_frame_bytes: 8_192,
        max_text_bytes: 128,
        max_fields: 512,
        max_list_items: 32,
    }
}

fn decode_hex(source: &str) -> Vec<u8> {
    let compact: String = source
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    assert_eq!(compact.len() % 2, 0);
    (0..compact.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&compact[index..index + 2], 16).unwrap())
        .collect()
}

fn assert_canonical_if_accepted(name: &str, kind: FrameKind, candidate: &[u8]) {
    match kind {
        FrameKind::Request => {
            if let Ok(value) = decode_request_v2(candidate, limits()) {
                assert_eq!(
                    encode_request_v2(&value, limits()).unwrap(),
                    candidate,
                    "accepted request mutation must be canonical: {name}"
                );
            }
        }
        FrameKind::Reply => {
            if let Ok(value) = decode_reply_v2(candidate, limits()) {
                assert_eq!(
                    encode_reply_v2(&value, limits()).unwrap(),
                    candidate,
                    "accepted reply mutation must be canonical: {name}"
                );
            }
        }
    }
}

#[test]
fn every_v2_fixture_has_deterministic_bounded_single_bit_and_framing_mutation() {
    let fixtures = [
        (
            "create-room-request",
            include_str!("fixtures/create-room-request-v2.hex"),
            FrameKind::Request,
        ),
        (
            "bans-reply",
            include_str!("fixtures/bans-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "capabilities-reply",
            include_str!("fixtures/capabilities-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "change-retention-gap-reply",
            include_str!("fixtures/change-retention-gap-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "changes-reply",
            include_str!("fixtures/changes-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "identified-changes-reply",
            include_str!("fixtures/identified-changes-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "identified-messages-reply",
            include_str!("fixtures/identified-messages-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "message-edited-change-reply",
            include_str!("fixtures/message-edited-change-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "message-revision-page-reply",
            include_str!("fixtures/message-revision-page-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "messages-reply",
            include_str!("fixtures/messages-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "moderation-change-reply",
            include_str!("fixtures/moderation-change-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "policy-change-reply",
            include_str!("fixtures/policy-change-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "reaction-change-reply",
            include_str!("fixtures/reaction-change-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "reaction-status-reply",
            include_str!("fixtures/reaction-status-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "read-position-reply",
            include_str!("fixtures/read-position-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "redaction-change-reply",
            include_str!("fixtures/redaction-change-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "rooms-reply",
            include_str!("fixtures/rooms-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "thread-lifecycle-change-reply",
            include_str!("fixtures/thread-lifecycle-change-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "thread-replies-reply",
            include_str!("fixtures/thread-replies-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "thread-status-reply",
            include_str!("fixtures/thread-status-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "threaded-changes-reply",
            include_str!("fixtures/threaded-changes-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "topic-rename-request",
            include_str!("fixtures/topic-rename-request-v2.hex"),
            FrameKind::Request,
        ),
        (
            "topic-alias-add-request",
            include_str!("fixtures/topic-alias-add-request-v2.hex"),
            FrameKind::Request,
        ),
        (
            "topic-alias-added-reply",
            include_str!("fixtures/topic-alias-added-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "topic-alias-change-reply",
            include_str!("fixtures/topic-alias-change-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "topic-renamed-change-reply",
            include_str!("fixtures/topic-renamed-change-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "topic-renamed-reply",
            include_str!("fixtures/topic-renamed-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "topic-resolve-request",
            include_str!("fixtures/topic-resolve-request-v2.hex"),
            FrameKind::Request,
        ),
        (
            "topic-resolved-change-reply",
            include_str!("fixtures/topic-resolved-change-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "topic-resolved-reply",
            include_str!("fixtures/topic-resolved-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "topic-supersede-request",
            include_str!("fixtures/topic-supersede-request-v2.hex"),
            FrameKind::Request,
        ),
        (
            "topic-superseded-change-reply",
            include_str!("fixtures/topic-superseded-change-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "topic-superseded-reply",
            include_str!("fixtures/topic-superseded-reply-v2.hex"),
            FrameKind::Reply,
        ),
        (
            "withdrawal-change-reply",
            include_str!("fixtures/withdrawal-change-reply-v2.hex"),
            FrameKind::Reply,
        ),
    ];

    for (name, source, kind) in fixtures {
        let bytes = decode_hex(source);
        assert_canonical_if_accepted(name, kind, &bytes);

        for end in 0..bytes.len() {
            assert_canonical_if_accepted(name, kind, &bytes[..end]);
        }

        let mut trailing = bytes.clone();
        trailing.push(0);
        match kind {
            FrameKind::Request => assert!(decode_request_v2(&trailing, limits()).is_err()),
            FrameKind::Reply => assert!(decode_reply_v2(&trailing, limits()).is_err()),
        }

        for index in 0..bytes.len() {
            for bit in 0..8 {
                let mut candidate = bytes.clone();
                candidate[index] ^= 1 << bit;
                assert_canonical_if_accepted(name, kind, &candidate);
            }
        }
    }
}

#[test]
fn every_v1_fixture_has_deterministic_bounded_single_bit_and_framing_mutation() {
    let request_fixtures = [
        (
            "connect-request-v1",
            include_str!("fixtures/connect-request-v1.hex"),
        ),
        (
            "speak-request-v1",
            include_str!("fixtures/speak-request-v1.hex"),
        ),
    ];
    for (name, source) in request_fixtures {
        let bytes = decode_hex(source);
        assert_v1_request_canonical_if_accepted(name, &bytes);
        for end in 0..bytes.len() {
            assert_v1_request_canonical_if_accepted(name, &bytes[..end]);
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(decode_request_v1(&trailing, limits()).is_err());
        for index in 0..bytes.len() {
            for bit in 0..8 {
                let mut candidate = bytes.clone();
                candidate[index] ^= 1 << bit;
                assert_v1_request_canonical_if_accepted(name, &candidate);
            }
        }
    }

    let name = "snapshot-reply-v1";
    let bytes = decode_hex(include_str!("fixtures/snapshot-reply-v1.hex"));
    assert_v1_reply_canonical_if_accepted(name, &bytes);
    for end in 0..bytes.len() {
        assert_v1_reply_canonical_if_accepted(name, &bytes[..end]);
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(decode_reply_v1(&trailing, limits()).is_err());
    for index in 0..bytes.len() {
        for bit in 0..8 {
            let mut candidate = bytes.clone();
            candidate[index] ^= 1 << bit;
            assert_v1_reply_canonical_if_accepted(name, &candidate);
        }
    }
}
