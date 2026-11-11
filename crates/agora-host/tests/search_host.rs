use agora_engine::{
    AdmissionPolicy, CommandV2 as EngineCommand, Engine, LogicalTime, Participant, ParticipantKind,
    RoomId,
};
use agora_host::search::SearchHostV1;
use agora_host::v2::{HostSessionIdV2, HostV2};
use agora_host::{HostLimits, HostRefusal, HostTimeV1, PrincipalVerifier, VerifiedSubjectV1};
use agora_protocol::search::{
    SearchCommandV1, SearchMatchProfileV1, SearchOfferV1, SearchOutcomeV1, SearchReplyV1,
    SearchRequestIdV1, SearchRequestV1,
};
use agora_protocol::v2::{ClientIdV2, CommandV2, RequestIdV2, RequestV2};
use agora_service::search::port::{
    SearchCursorV1, SearchGenerationV1, SearchHitV1, SearchObservationV1, SearchPageV1,
    SearchQueryV1, SearchRefusalV1,
};
use agora_service::search::{SearchPortV1, SearchServiceLimitsV1};
use agora_service::v2::{ServiceLimitsV2, ServiceV2};
use agora_wire::WireLimits;
use agora_wire::search::{decode_search_reply_v1, encode_search_offer_v1};
use agora_wire::v2::{WireReplyV2, WireRequestV2, decode_reply_v2, encode_request_v2};
use std::cell::Cell;
use std::rc::Rc;

#[derive(Clone)]
struct Context {
    participant: &'static str,
    active: bool,
}

struct Verifier;

impl PrincipalVerifier for Verifier {
    type Context = Context;
    type Error = ();

    fn authenticate(
        &mut self,
        claimed_participant: &str,
        context: &Self::Context,
        _now: HostTimeV1,
    ) -> Result<VerifiedSubjectV1, Self::Error> {
        if context.active && claimed_participant == context.participant {
            Ok(VerifiedSubjectV1::from_verifier([7; 16]))
        } else {
            Err(())
        }
    }

    fn revalidate(
        &mut self,
        subject: VerifiedSubjectV1,
        participant: &str,
        context: &Self::Context,
        _now: HostTimeV1,
    ) -> Result<(), Self::Error> {
        if context.active
            && participant == context.participant
            && subject == VerifiedSubjectV1::from_verifier([7; 16])
        {
            Ok(())
        } else {
            Err(())
        }
    }
}

struct Port {
    message_id: [u8; 16],
    message_sequence: u64,
    retired: Rc<Cell<u64>>,
}

impl SearchPortV1 for Port {
    fn search(&mut self, query: SearchQueryV1) -> Result<SearchObservationV1, SearchRefusalV1> {
        Ok(SearchObservationV1::Available(SearchPageV1 {
            source_revision: query.observed_revision,
            generation: SearchGenerationV1::new(1),
            hits: vec![SearchHitV1 {
                message_id: self.message_id,
                message_sequence: self.message_sequence,
            }],
            next: Some(SearchCursorV1::new([9; 16])),
        }))
    }

    fn retire_cursor(&mut self, _cursor: SearchCursorV1) -> bool {
        self.retired.set(self.retired.get() + 1);
        true
    }
}

fn wire_limits() -> WireLimits {
    WireLimits {
        max_frame_bytes: 8_192,
        max_text_bytes: 128,
        max_fields: 512,
        max_list_items: 32,
    }
}

fn fixture() -> (SearchHostV1<Verifier, Port>, Rc<Cell<u64>>) {
    let mut engine = Engine::new();
    engine
        .apply(EngineCommand::RegisterParticipant(Participant::new(
            "alice",
            "Alice",
            ParticipantKind::Human,
        )))
        .unwrap();
    engine
        .apply(EngineCommand::CreateRoom {
            room_id: "room".into(),
            name: "Room".into(),
            owner: "alice".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(EngineCommand::Speak {
            room_id: "room".into(),
            author: "alice".into(),
            at: LogicalTime(1),
            body: "needle".into(),
        })
        .unwrap();
    let message = &engine.room(&RoomId::new("room")).unwrap().messages[0];
    let message_id = message.id().into_bytes();
    let message_sequence = message.sequence();
    let retired = Rc::new(Cell::new(0));
    let host = HostV2::new(
        ServiceV2::new(engine, ServiceLimitsV2::default()),
        Verifier,
        HostLimits {
            max_sessions: 8,
            max_retired_sessions: 8,
            wire: wire_limits(),
        },
    );
    (
        SearchHostV1::new(
            host,
            Port {
                message_id,
                message_sequence,
                retired: retired.clone(),
            },
            SearchServiceLimitsV1::default(),
        ),
        retired,
    )
}

fn open(host: &mut SearchHostV1<Verifier, Port>) -> (HostSessionIdV2, ClientIdV2, Context) {
    let context = Context {
        participant: "alice",
        active: true,
    };
    let frame = encode_request_v2(
        &WireRequestV2::Connect {
            participant_id: "alice".into(),
        },
        wire_limits(),
    )
    .unwrap();
    let reply = host.open(&frame, &context, HostTimeV1(1)).unwrap();
    let WireReplyV2::Connected(client) = decode_reply_v2(reply.frame(), wire_limits()).unwrap()
    else {
        panic!("connected reply expected")
    };
    (reply.session().unwrap(), client, context)
}

fn search_frame(client: ClientIdV2, request_id: u64) -> Vec<u8> {
    encode_search_offer_v1(
        &SearchOfferV1 {
            client,
            request: SearchRequestV1 {
                request_id: SearchRequestIdV1(request_id),
                command: SearchCommandV1 {
                    room_id: "room".into(),
                    profile: SearchMatchProfileV1::ExactUtf8Substring,
                    term: "needle".into(),
                    cursor: None,
                    limit: 1,
                },
            },
        },
        wire_limits(),
    )
    .unwrap()
}

#[test]
fn authenticated_session_binds_client_and_exact_search_replay() {
    let (mut host, retired) = fixture();
    let (session, client, context) = open(&mut host);
    let frame = search_frame(client, 0);
    let first = host
        .handle_search(session, &frame, &context, HostTimeV1(2))
        .unwrap();
    let decoded = decode_search_reply_v1(&first, wire_limits()).unwrap();
    assert!(
        matches!(
            decoded,
            SearchReplyV1::Accepted(SearchOutcomeV1::Available(_))
        ),
        "unexpected search reply: {decoded:?}"
    );
    assert_eq!(
        host.handle_search(session, &frame, &context, HostTimeV1(3))
            .unwrap(),
        first
    );
    assert_eq!(host.search_usage().replayed, 1);

    assert_eq!(
        host.handle_search(
            session,
            &search_frame(ClientIdV2(client.0 + 1), 1),
            &context,
            HostTimeV1(4),
        ),
        Err(HostRefusal::Unavailable)
    );
    assert_eq!(retired.get(), 1);
    assert!(matches!(
        host.handle_search(session, &frame, &context, HostTimeV1(5)),
        Err(HostRefusal::Unavailable)
    ));
}

#[test]
fn revocation_and_close_retire_adapter_continuations() {
    let (mut host, retired) = fixture();
    let (session, client, mut context) = open(&mut host);
    host.handle_search(session, &search_frame(client, 0), &context, HostTimeV1(2))
        .unwrap();
    context.active = false;
    assert_eq!(
        host.handle_search(session, &search_frame(client, 1), &context, HostTimeV1(3),),
        Err(HostRefusal::Unavailable)
    );
    assert_eq!(retired.get(), 1);

    let (session, client, context) = open(&mut host);
    host.handle_search(session, &search_frame(client, 0), &context, HostTimeV1(4))
        .unwrap();
    assert!(host.close(session));
    assert_eq!(retired.get(), 2);
}

#[test]
fn authoritative_conversation_mutation_retires_search_continuations() {
    let (mut host, retired) = fixture();
    let (session, client, context) = open(&mut host);
    host.handle_search(session, &search_frame(client, 0), &context, HostTimeV1(2))
        .unwrap();
    let revision = host.host().service().engine().revision().0;
    let frame = encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(0),
                command: CommandV2::SpeakIdentified {
                    room_id: "room".into(),
                    expected_revision: revision,
                    logical_time: 2,
                    body: "needle two".into(),
                    addressed_to: vec![],
                },
            },
        },
        wire_limits(),
    )
    .unwrap();
    host.handle(session, &frame, &context, HostTimeV1(3))
        .unwrap();
    assert_eq!(retired.get(), 1);
}
