use agora_search_memory::{
    IndexedMessageV1, ProjectionRefusalV1, RoomProjectionV1, SearchMemoryLimitsV1, SearchMemoryV1,
};
use agora_search_port::{
    SearchCoverageV1, SearchMatchProfileV1, SearchObservationV1, SearchPortV1, SearchQueryV1,
    SearchRefusalV1,
};

fn message(id: u8, sequence: u64, body: &str) -> IndexedMessageV1 {
    IndexedMessageV1 {
        message_id: [id; 16],
        message_sequence: sequence,
        body: body.into(),
    }
}

fn projection(revision: u64, coverage: SearchCoverageV1) -> RoomProjectionV1 {
    RoomProjectionV1 {
        room_id: "room".into(),
        source_revision: revision,
        coverage,
        messages: vec![
            message(1, 1, "alpha one"),
            message(2, 2, "beta"),
            message(3, 4, "alpha two"),
        ],
    }
}

fn query(revision: u64, limit: u32) -> SearchQueryV1 {
    SearchQueryV1 {
        room_id: "room".into(),
        observed_revision: revision,
        profile: SearchMatchProfileV1::ExactUtf8Substring,
        term: "alpha".into(),
        cursor: None,
        limit,
    }
}

#[test]
fn exact_identity_only_search_pages_and_consumes_bound_cursor() {
    let mut index = SearchMemoryV1::new(SearchMemoryLimitsV1::default());
    let generation = index
        .replace_room(projection(7, SearchCoverageV1::Complete))
        .unwrap();
    let first = index.search(query(7, 1)).unwrap();
    let SearchObservationV1::Available(first) = first else {
        panic!("expected available page")
    };
    assert_eq!(first.source_revision, 7);
    assert_eq!(first.generation, generation);
    assert_eq!(first.hits.len(), 1);
    assert_eq!(first.hits[0].message_id, [1; 16]);
    assert_eq!(first.hits[0].message_sequence, 1);
    let cursor = first.next.unwrap();

    let mut continuation = query(7, 1);
    continuation.cursor = Some(cursor);
    let SearchObservationV1::Available(second) = index.search(continuation.clone()).unwrap() else {
        panic!("expected available continuation")
    };
    assert_eq!(second.hits[0].message_id, [3; 16]);
    assert_eq!(second.hits[0].message_sequence, 4);
    assert_eq!(second.next, None);
    assert_eq!(
        index.search(continuation),
        Err(SearchRefusalV1::InvalidCursor)
    );
}

#[test]
fn source_lag_incomplete_coverage_and_unavailability_are_distinct() {
    let mut index = SearchMemoryV1::new(SearchMemoryLimitsV1::default());
    assert_eq!(
        index.search(query(7, 2)).unwrap(),
        SearchObservationV1::Unavailable
    );
    let generation = index
        .replace_room(projection(6, SearchCoverageV1::Incomplete))
        .unwrap();
    assert_eq!(
        index.search(query(7, 2)).unwrap(),
        SearchObservationV1::Stale {
            source_revision: 6,
            generation
        }
    );
    let SearchObservationV1::Incomplete(page) = index.search(query(6, 2)).unwrap() else {
        panic!("expected incomplete page")
    };
    assert_eq!(page.hits.len(), 2);
    index.fence();
    assert_eq!(
        index.search(query(6, 2)).unwrap(),
        SearchObservationV1::Unavailable
    );
    assert_eq!(
        index.replace_room(projection(7, SearchCoverageV1::Complete)),
        Err(ProjectionRefusalV1::Fenced)
    );
}

#[test]
fn replacement_invalidates_cursor_and_advances_generation() {
    let mut index = SearchMemoryV1::new(SearchMemoryLimitsV1::default());
    let first_generation = index
        .replace_room(projection(7, SearchCoverageV1::Complete))
        .unwrap();
    let SearchObservationV1::Available(page) = index.search(query(7, 1)).unwrap() else {
        panic!("expected page")
    };
    let old_cursor = page.next.unwrap();
    let second_generation = index
        .replace_room(projection(8, SearchCoverageV1::Complete))
        .unwrap();
    assert_ne!(first_generation, second_generation);
    let mut old = query(8, 1);
    old.cursor = Some(old_cursor);
    assert_eq!(index.search(old), Err(SearchRefusalV1::InvalidCursor));
}

#[test]
fn source_revision_cannot_regress() {
    let mut index = SearchMemoryV1::new(SearchMemoryLimitsV1::default());
    let generation = index
        .replace_room(projection(7, SearchCoverageV1::Complete))
        .unwrap();
    let before = (index.room_count(), index.total_content_bytes());
    assert_eq!(
        index.replace_room(projection(6, SearchCoverageV1::Complete)),
        Err(ProjectionRefusalV1::RevisionRegression)
    );
    assert_eq!((index.room_count(), index.total_content_bytes()), before);
    let SearchObservationV1::Available(page) = index.search(query(7, 2)).unwrap() else {
        panic!("expected original projection")
    };
    assert_eq!(page.generation, generation);
}

#[test]
fn cursor_is_bound_to_exact_query_and_page_shape() {
    let mut index = SearchMemoryV1::new(SearchMemoryLimitsV1::default());
    index
        .replace_room(projection(7, SearchCoverageV1::Complete))
        .unwrap();
    let SearchObservationV1::Available(page) = index.search(query(7, 1)).unwrap() else {
        panic!("expected page")
    };
    let cursor = page.next.unwrap();
    let mut changed_term = query(7, 1);
    changed_term.term = "two".into();
    changed_term.cursor = Some(cursor);
    assert_eq!(
        index.search(changed_term),
        Err(SearchRefusalV1::InvalidCursor)
    );
    let mut changed_limit = query(7, 2);
    changed_limit.cursor = Some(cursor);
    assert_eq!(
        index.search(changed_limit),
        Err(SearchRefusalV1::InvalidCursor)
    );
}

#[test]
fn projection_validation_and_capacity_refuse_atomically() {
    let limits = SearchMemoryLimitsV1 {
        max_rooms: 1,
        max_messages_per_room: 3,
        max_body_bytes: 16,
        max_total_content_bytes: 32,
        max_room_id_bytes: 8,
        max_term_bytes: 8,
        max_page_size: 2,
        max_cursors: 1,
    };
    let mut index = SearchMemoryV1::new(limits);
    index
        .replace_room(projection(7, SearchCoverageV1::Complete))
        .unwrap();
    let before = (index.room_count(), index.total_content_bytes());
    let mut duplicate = projection(8, SearchCoverageV1::Complete);
    duplicate.messages[1].message_id = duplicate.messages[0].message_id;
    assert_eq!(
        index.replace_room(duplicate),
        Err(ProjectionRefusalV1::InvalidProjection)
    );
    assert_eq!((index.room_count(), index.total_content_bytes()), before);
    let other = RoomProjectionV1 {
        room_id: "other".into(),
        source_revision: 1,
        coverage: SearchCoverageV1::Complete,
        messages: vec![],
    };
    assert_eq!(
        index.replace_room(other),
        Err(ProjectionRefusalV1::Capacity)
    );
    assert_eq!((index.room_count(), index.total_content_bytes()), before);
}

#[test]
fn query_bounds_and_cursor_capacity_are_explicit() {
    let limits = SearchMemoryLimitsV1 {
        max_page_size: 1,
        max_cursors: 0,
        ..SearchMemoryLimitsV1::default()
    };
    let mut index = SearchMemoryV1::new(limits);
    index
        .replace_room(projection(7, SearchCoverageV1::Complete))
        .unwrap();
    assert_eq!(
        index.search(query(7, 0)),
        Err(SearchRefusalV1::InvalidLimit)
    );
    assert_eq!(
        index.search(query(7, 2)),
        Err(SearchRefusalV1::InvalidLimit)
    );
    let mut empty = query(7, 1);
    empty.term.clear();
    assert_eq!(index.search(empty), Err(SearchRefusalV1::InvalidQuery));
    assert_eq!(index.search(query(7, 1)), Err(SearchRefusalV1::Capacity));
}

#[test]
fn debug_does_not_disclose_indexed_content() {
    let projection = RoomProjectionV1 {
        room_id: "secret-room".into(),
        source_revision: 1,
        coverage: SearchCoverageV1::Complete,
        messages: vec![message(7, 1, "secret-body")],
    };
    let message_debug = format!("{:?}", projection.messages[0]);
    let projection_debug = format!("{projection:?}");
    assert!(!message_debug.contains("secret-body"));
    assert!(!projection_debug.contains("secret-room"));
    assert!(message_debug.contains("body_bytes"));
}
