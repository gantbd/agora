use agora_protocol::search::{
    SearchCommandV1, SearchHitV1, SearchMatchProfileV1, SearchOfferV1, SearchOutcomeV1,
    SearchPageV1, SearchRefusalV1, SearchReplyV1, SearchRequestIdV1, SearchRequestV1,
    ServiceSearchCursorV1,
};
use agora_protocol::v2::{ClientIdV2, MessageIdV1};
use agora_wire::search::{
    decode_search_offer_v1, decode_search_reply_v1, encode_search_offer_v1, encode_search_reply_v1,
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

fn fixture(source: &str) -> Vec<u8> {
    source
        .split_ascii_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).unwrap())
        .collect()
}

fn offer(cursor: Option<ServiceSearchCursorV1>) -> SearchOfferV1 {
    SearchOfferV1 {
        client: ClientIdV2(7),
        request: SearchRequestV1 {
            request_id: SearchRequestIdV1(9),
            command: SearchCommandV1 {
                room_id: "room".into(),
                profile: SearchMatchProfileV1::ExactUtf8Substring,
                term: "needle".into(),
                cursor,
                limit: 2,
            },
        },
    }
}

fn page() -> SearchPageV1 {
    SearchPageV1 {
        source_revision: 11,
        index_generation: 3,
        hits: vec![
            SearchHitV1 {
                message_id: MessageIdV1::new([0x11; 16]),
                message_sequence: 2,
            },
            SearchHitV1 {
                message_id: MessageIdV1::new([0x22; 16]),
                message_sequence: 5,
            },
        ],
        next: Some(ServiceSearchCursorV1::new([0x33; 16])),
    }
}

#[test]
fn every_search_value_round_trips() {
    for request in [
        offer(None),
        offer(Some(ServiceSearchCursorV1::new([0x44; 16]))),
    ] {
        let encoded = encode_search_offer_v1(&request, limits()).unwrap();
        assert_eq!(decode_search_offer_v1(&encoded, limits()).unwrap(), request);
    }

    let mut replies = vec![
        SearchReplyV1::Accepted(SearchOutcomeV1::Available(page())),
        SearchReplyV1::Accepted(SearchOutcomeV1::Incomplete(page())),
        SearchReplyV1::Accepted(SearchOutcomeV1::Stale {
            source_revision: 10,
            index_generation: 4,
        }),
        SearchReplyV1::Accepted(SearchOutcomeV1::Unavailable),
    ];
    for refusal in [
        SearchRefusalV1::UnknownClient,
        SearchRefusalV1::RequestConflict,
        SearchRefusalV1::RequestLimit,
        SearchRefusalV1::UnknownRoom,
        SearchRefusalV1::NotMember,
        SearchRefusalV1::InvalidInput,
        SearchRefusalV1::InvalidCursor,
        SearchRefusalV1::CursorLimit,
        SearchRefusalV1::IndexCapacity,
    ] {
        replies.push(SearchReplyV1::Refused(refusal));
    }
    for reply in replies {
        let encoded = encode_search_reply_v1(&reply, limits()).unwrap();
        assert_eq!(decode_search_reply_v1(&encoded, limits()).unwrap(), reply);
    }
}

#[test]
fn search_v1_golden_fixtures_are_canonical() {
    assert_eq!(
        encode_search_offer_v1(&offer(None), limits()).unwrap(),
        fixture(include_str!("fixtures/search-offer-v1.hex"))
    );
    assert_eq!(
        encode_search_reply_v1(
            &SearchReplyV1::Accepted(SearchOutcomeV1::Available(page())),
            limits()
        )
        .unwrap(),
        fixture(include_str!("fixtures/search-page-reply-v1.hex"))
    );
}

#[test]
fn frozen_search_frames_have_bounded_deterministic_mutation_closure() {
    let offer = fixture(include_str!("fixtures/search-offer-v1.hex"));
    let reply = fixture(include_str!("fixtures/search-page-reply-v1.hex"));

    assert_mutation_closure(&offer, |candidate| {
        if let Ok(value) = decode_search_offer_v1(candidate, limits()) {
            assert_eq!(encode_search_offer_v1(&value, limits()).unwrap(), candidate);
        }
    });
    assert_mutation_closure(&reply, |candidate| {
        if let Ok(value) = decode_search_reply_v1(candidate, limits()) {
            assert_eq!(encode_search_reply_v1(&value, limits()).unwrap(), candidate);
        }
    });
}

fn assert_mutation_closure(frame: &[u8], mut check: impl FnMut(&[u8])) {
    check(frame);
    for end in 0..frame.len() {
        check(&frame[..end]);
    }
    let mut trailing = frame.to_vec();
    trailing.push(0);
    check(&trailing);
    for index in 0..frame.len() {
        for bit in 0..8 {
            let mut changed = frame.to_vec();
            changed[index] ^= 1 << bit;
            check(&changed);
        }
    }
}

#[test]
fn malformed_versions_tags_framing_utf8_and_limits_fail_closed() {
    let encoded = encode_search_offer_v1(&offer(None), limits()).unwrap();
    for prefix in 0..encoded.len() {
        assert!(decode_search_offer_v1(&encoded[..prefix], limits()).is_err());
    }
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        decode_search_offer_v1(&trailing, limits()),
        Err(WireDecodeError::TrailingData)
    );
    let mut version = encoded.clone();
    version[0] = 2;
    assert_eq!(
        decode_search_offer_v1(&version, limits()),
        Err(WireDecodeError::UnsupportedVersion)
    );
    let mut profile = encoded.clone();
    profile[25] = 0xff;
    assert_eq!(
        decode_search_offer_v1(&profile, limits()),
        Err(WireDecodeError::UnknownTag)
    );
    let mut invalid_utf8 = encoded;
    invalid_utf8[21] = 0xff;
    assert_eq!(
        decode_search_offer_v1(&invalid_utf8, limits()),
        Err(WireDecodeError::InvalidUtf8)
    );
    let tight = WireLimits {
        max_frame_bytes: 8,
        ..limits()
    };
    assert_eq!(
        encode_search_offer_v1(&offer(None), tight),
        Err(WireEncodeError::Limit)
    );
}

#[test]
fn noncanonical_pages_refuse_encode_and_decode() {
    let mut noncanonical = page();
    noncanonical.hits.swap(0, 1);
    assert_eq!(
        encode_search_reply_v1(
            &SearchReplyV1::Accepted(SearchOutcomeV1::Available(noncanonical)),
            limits()
        ),
        Err(WireEncodeError::Invalid)
    );
    let mut empty_continuation = page();
    empty_continuation.hits.clear();
    assert_eq!(
        encode_search_reply_v1(
            &SearchReplyV1::Accepted(SearchOutcomeV1::Available(empty_continuation)),
            limits()
        ),
        Err(WireEncodeError::Invalid)
    );
    let mut duplicate_id = page();
    duplicate_id.hits[1].message_id = duplicate_id.hits[0].message_id;
    assert_eq!(
        encode_search_reply_v1(
            &SearchReplyV1::Accepted(SearchOutcomeV1::Available(duplicate_id)),
            limits()
        ),
        Err(WireEncodeError::Invalid)
    );

    let canonical = encode_search_reply_v1(
        &SearchReplyV1::Accepted(SearchOutcomeV1::Available(page())),
        limits(),
    )
    .unwrap();
    let mut changed = canonical;
    // The second hit's sequence is the final u64 before the optional cursor.
    let second_sequence = 2 + 1 + 8 + 8 + 8 + 16 + 8 + 16;
    changed[second_sequence..second_sequence + 8].copy_from_slice(&1u64.to_be_bytes());
    assert_eq!(
        decode_search_reply_v1(&changed, limits()),
        Err(WireDecodeError::Malformed)
    );
}

#[test]
fn diagnostics_do_not_disclose_search_content() {
    let debug = format!("{:?}", offer(None));
    assert!(!debug.contains("room\""));
    assert!(!debug.contains("\"needle\""));
}
