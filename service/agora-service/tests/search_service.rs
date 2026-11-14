use agora_engine::{
    AdmissionPolicy, CommandV2 as EngineCommand, Engine, Limits, LogicalTime, Participant,
    ParticipantKind, RoomId,
};
use agora_protocol::search::{
    SearchCommandV1, SearchMatchProfileV1, SearchOutcomeV1, SearchRefusalV1, SearchReplyV1,
    SearchRequestIdV1, SearchRequestV1,
};
use agora_protocol::v2::{CommandV2, OutcomeV2, ReplyV2, RequestIdV2, RequestV2};
use agora_search_memory::{
    IndexedMessageV1, RoomProjectionV1, SearchMemoryLimitsV1, SearchMemoryV1,
};
use agora_search_port::{
    SearchCoverageV1, SearchCursorV1, SearchGenerationV1, SearchHitV1, SearchObservationV1,
    SearchPageV1, SearchPortV1, SearchQueryV1, SearchRefusalV1 as PortRefusalV1,
};
use agora_service::search::{SearchServiceLimitsV1, SearchServiceV1};
use agora_service::v2::{ServiceLimitsV2, ServiceV2};

fn search_request(
    id: u64,
    cursor: Option<agora_protocol::search::ServiceSearchCursorV1>,
) -> SearchRequestV1 {
    SearchRequestV1 {
        request_id: SearchRequestIdV1(id),
        command: SearchCommandV1 {
            room_id: "room".into(),
            profile: SearchMatchProfileV1::ExactUtf8Substring,
            term: "needle".into(),
            cursor,
            limit: 1,
        },
    }
}

fn fixture() -> SearchServiceV1<SearchMemoryV1> {
    let mut engine = Engine::with_limits(Limits::default());
    for (id, kind) in [
        ("alice", ParticipantKind::Human),
        ("bot", ParticipantKind::Agent),
        ("outsider", ParticipantKind::Human),
    ] {
        engine
            .apply(EngineCommand::RegisterParticipant(Participant::new(
                id, id, kind,
            )))
            .unwrap();
    }
    engine
        .apply(EngineCommand::CreateRoom {
            room_id: "room".into(),
            name: "Room".into(),
            owner: "alice".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(EngineCommand::Join {
            room_id: "room".into(),
            participant: "bot".into(),
        })
        .unwrap();
    for (at, body) in [(1, "needle one"), (2, "other"), (3, "needle two")] {
        engine
            .apply(EngineCommand::Speak {
                room_id: "room".into(),
                author: "alice".into(),
                at: LogicalTime(at),
                body: body.into(),
            })
            .unwrap();
    }
    let revision = engine.revision().0;
    let room = engine.room(&RoomId::new("room")).unwrap();
    let messages = room
        .messages
        .iter()
        .map(|message| IndexedMessageV1 {
            message_id: message.id().into_bytes(),
            message_sequence: message.sequence(),
            body: message.body().into(),
        })
        .collect();
    let mut index = SearchMemoryV1::new(SearchMemoryLimitsV1::default());
    index
        .replace_room(RoomProjectionV1 {
            room_id: "room".into(),
            source_revision: revision,
            coverage: SearchCoverageV1::Complete,
            messages,
        })
        .unwrap();
    SearchServiceV1::new(
        ServiceV2::new(engine, ServiceLimitsV2::default()),
        index,
        SearchServiceLimitsV1::default(),
    )
}

#[test]
fn participant_bound_search_pages_replays_and_is_kind_neutral() {
    let mut service = fixture();
    let alice = service.connect("alice").unwrap();
    let bot = service.connect("bot").unwrap();
    let first_request = search_request(1, None);
    let first_reply = service.search(alice, first_request.clone());
    let SearchReplyV1::Accepted(SearchOutcomeV1::Available(first)) = &first_reply else {
        panic!("available first page expected")
    };
    assert_eq!(first.hits.len(), 1);
    let cursor = first.next.unwrap();
    assert_eq!(service.search(alice, first_request), first_reply);
    assert_eq!(service.usage().replayed, 1);

    assert_eq!(
        service.search(bot, search_request(1, Some(cursor))),
        SearchReplyV1::Refused(SearchRefusalV1::InvalidCursor)
    );
    let SearchReplyV1::Accepted(SearchOutcomeV1::Available(second)) =
        service.search(alice, search_request(2, Some(cursor)))
    else {
        panic!("available continuation expected")
    };
    assert_eq!(second.hits.len(), 1);
    assert_ne!(first.hits[0], second.hits[0]);
    assert_eq!(second.next, None);
}

#[test]
fn membership_and_request_identity_are_enforced_before_index_disclosure() {
    let mut service = fixture();
    let alice = service.connect("alice").unwrap();
    let outsider = service.connect("outsider").unwrap();
    assert_eq!(
        service.search(outsider, search_request(1, None)),
        SearchReplyV1::Refused(SearchRefusalV1::NotMember)
    );
    assert_eq!(
        service.search(agora_protocol::v2::ClientIdV2(99), search_request(1, None)),
        SearchReplyV1::Refused(SearchRefusalV1::UnknownClient)
    );
    let accepted = service.search(alice, search_request(1, None));
    assert!(matches!(
        accepted,
        SearchReplyV1::Accepted(SearchOutcomeV1::Available(_))
    ));
    let mut changed = search_request(1, None);
    changed.command.term = "other".into();
    assert_eq!(
        service.search(alice, changed),
        SearchReplyV1::Refused(SearchRefusalV1::RequestConflict)
    );
}

#[test]
fn authoritative_mutation_retires_search_continuations() {
    let mut service = fixture();
    let alice = service.connect("alice").unwrap();
    let SearchReplyV1::Accepted(SearchOutcomeV1::Available(page)) =
        service.search(alice, search_request(1, None))
    else {
        panic!("available page expected")
    };
    let cursor = page.next.unwrap();
    let revision = service.service().engine().revision().0;
    assert!(matches!(
        service.execute_conversation(
            alice,
            RequestV2 {
                request_id: RequestIdV2(1),
                command: CommandV2::SpeakIdentified {
                    room_id: "room".into(),
                    expected_revision: revision,
                    logical_time: 4,
                    body: "needle three".into(),
                    addressed_to: vec![],
                },
            },
        ),
        ReplyV2::Accepted(OutcomeV2::IdentifiedMessageAccepted { .. })
    ));
    assert_eq!(
        service.search(alice, search_request(2, Some(cursor))),
        SearchReplyV1::Refused(SearchRefusalV1::InvalidCursor)
    );
    assert_eq!(service.usage().cursor_retirements, 1);
}

#[test]
fn disconnect_retires_search_receipts_and_cursors() {
    let mut service = fixture();
    let alice = service.connect("alice").unwrap();
    let SearchReplyV1::Accepted(SearchOutcomeV1::Available(page)) =
        service.search(alice, search_request(1, None))
    else {
        panic!("available page expected")
    };
    assert!(page.next.is_some());
    assert!(service.disconnect(alice).unwrap());
    assert_eq!(service.usage().cursor_retirements, 1);
    assert_eq!(
        service.search(alice, search_request(1, None)),
        SearchReplyV1::Refused(SearchRefusalV1::UnknownClient)
    );
}

#[derive(Default)]
struct LyingPort {
    retired: usize,
}

impl SearchPortV1 for LyingPort {
    fn search(&mut self, query: SearchQueryV1) -> Result<SearchObservationV1, PortRefusalV1> {
        Ok(SearchObservationV1::Available(SearchPageV1 {
            source_revision: query.observed_revision,
            generation: SearchGenerationV1::new(1),
            hits: vec![SearchHitV1 {
                message_id: [99; 16],
                message_sequence: 1,
            }],
            next: Some(SearchCursorV1::new([3; 16])),
        }))
    }

    fn retire_cursor(&mut self, _cursor: SearchCursorV1) -> bool {
        self.retired += 1;
        true
    }
}

#[test]
fn unresolvable_index_hits_fail_closed_without_disclosure() {
    let (service, _) = fixture().into_parts();
    let mut service = SearchServiceV1::new(
        service,
        LyingPort::default(),
        SearchServiceLimitsV1::default(),
    );
    let alice = service.connect("alice").unwrap();
    assert_eq!(
        service.search(alice, search_request(1, None)),
        SearchReplyV1::Accepted(SearchOutcomeV1::Unavailable)
    );
    assert_eq!(service.port().retired, 1);
}
