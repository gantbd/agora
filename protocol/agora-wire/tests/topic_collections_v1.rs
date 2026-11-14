use agora_protocol::v2::{
    CapabilityV2, ClientIdV2, CommandV2, OutcomeV2, PageCursorV2, ReplyV2, RequestIdV2, RequestV2,
    TopicCollectionEntryV1, TopicCollectionFilterV1, TopicCollectionLifecycleV1,
    TopicCollectionPageV1, TopicIdV1,
};
use agora_wire::v2::{
    WireReplyV2, WireRequestV2, decode_reply_v2, decode_request_v2, encode_reply_v2,
    encode_request_v2,
};
use agora_wire::{WireEncodeError, WireLimits};

fn decode_hex(source: &str) -> Vec<u8> {
    source
        .split_whitespace()
        .map(|pair| u8::from_str_radix(pair, 16).unwrap())
        .collect()
}

fn filter() -> TopicCollectionFilterV1 {
    TopicCollectionFilterV1 {
        lifecycle: TopicCollectionLifecycleV1::Active,
        tags: vec!["blue".to_owned(), "urgent".to_owned()],
    }
}

fn request() -> WireRequestV2 {
    WireRequestV2::Execute {
        client: ClientIdV2(3),
        request: RequestV2 {
            request_id: RequestIdV2(5),
            command: CommandV2::TopicCollection {
                room_id: "room".to_owned(),
                filter: filter(),
                cursor: Some(PageCursorV2(9)),
                limit: 4,
            },
        },
    }
}

fn reply() -> WireReplyV2 {
    WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::TopicCollection(
        TopicCollectionPageV1 {
            revision: 11,
            room_id: "room".to_owned(),
            filter: filter(),
            topics: vec![TopicCollectionEntryV1 {
                topic_id: TopicIdV1::new([1; 16]),
                name: "current".to_owned(),
                creator_id: "owner".to_owned(),
                resolved: false,
                aliases: vec!["former".to_owned()],
                tags: vec!["blue".to_owned(), "urgent".to_owned()],
                successor_topic_id: Some(TopicIdV1::new([2; 16])),
            }],
            next: Some(PageCursorV2(10)),
        },
    )))
}

#[test]
fn collection_request_and_reply_round_trip_canonically() {
    let request = request();
    let request_bytes = encode_request_v2(&request, WireLimits::reference()).unwrap();
    assert_eq!(
        request_bytes,
        decode_hex(include_str!("fixtures/topic-collection-request-v2.hex"))
    );
    assert_eq!(
        decode_request_v2(&request_bytes, WireLimits::reference()).unwrap(),
        request
    );

    let reply = reply();
    let reply_bytes = encode_reply_v2(&reply, WireLimits::reference()).unwrap();
    assert_eq!(
        reply_bytes,
        decode_hex(include_str!("fixtures/topic-collection-reply-v2.hex"))
    );
    assert_eq!(
        decode_reply_v2(&reply_bytes, WireLimits::reference()).unwrap(),
        reply
    );

    let capability = WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::Capabilities(vec![
        CapabilityV2::TopicCollectionsV1,
    ])));
    let bytes = encode_reply_v2(&capability, WireLimits::reference()).unwrap();
    assert_eq!(
        decode_reply_v2(&bytes, WireLimits::reference()).unwrap(),
        capability
    );
}

#[test]
fn collection_wire_rejects_noncanonical_filters_and_entries() {
    let mut request = request();
    let WireRequestV2::Execute {
        request:
            RequestV2 {
                command: CommandV2::TopicCollection { filter, .. },
                ..
            },
        ..
    } = &mut request
    else {
        unreachable!()
    };
    filter.tags.reverse();
    assert_eq!(
        encode_request_v2(&request, WireLimits::reference()),
        Err(WireEncodeError::Invalid)
    );

    let mut reply = reply();
    let WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::TopicCollection(page))) = &mut reply
    else {
        unreachable!()
    };
    page.topics[0].resolved = true;
    assert_eq!(
        encode_reply_v2(&reply, WireLimits::reference()),
        Err(WireEncodeError::Invalid)
    );
}
