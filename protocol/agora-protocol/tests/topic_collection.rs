use agora_protocol::v2::{
    CapabilityV2, CommandV2, OutcomeV2, PageCursorV2, TopicCollectionEntryV1,
    TopicCollectionFilterV1, TopicCollectionLifecycleV1, TopicCollectionPageV1, TopicIdV1,
};

#[test]
fn topic_collection_values_are_closed_and_content_safe() {
    let filter = TopicCollectionFilterV1 {
        lifecycle: TopicCollectionLifecycleV1::Active,
        tags: vec!["secret-blue".to_owned(), "secret-urgent".to_owned()],
    };
    let command = CommandV2::TopicCollection {
        room_id: "secret-room".to_owned(),
        filter: filter.clone(),
        cursor: Some(PageCursorV2(7)),
        limit: 8,
    };
    let outcome = OutcomeV2::TopicCollection(TopicCollectionPageV1 {
        revision: 12,
        room_id: "secret-room".to_owned(),
        filter,
        topics: vec![TopicCollectionEntryV1 {
            topic_id: TopicIdV1::new([1; 16]),
            name: "secret-name".to_owned(),
            creator_id: "secret-creator".to_owned(),
            resolved: false,
            aliases: vec!["secret-alias".to_owned()],
            tags: vec!["secret-blue".to_owned(), "secret-urgent".to_owned()],
            successor_topic_id: Some(TopicIdV1::new([2; 16])),
        }],
        next: Some(PageCursorV2(8)),
    });

    let debug = format!("{command:?} {outcome:?}");
    for secret in [
        "secret-room",
        "secret-blue",
        "secret-urgent",
        "secret-name",
        "secret-creator",
        "secret-alias",
    ] {
        assert!(!debug.contains(secret));
    }
    assert_eq!(
        CapabilityV2::TopicCollectionsV1,
        CapabilityV2::TopicCollectionsV1
    );
}
