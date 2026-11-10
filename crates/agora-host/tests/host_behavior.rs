use agora_engine::{AdmissionPolicy, CommandV2, Engine, Participant, ParticipantKind, Revision};
use agora_host::{Host, HostLimits, HostRefusal, HostTimeV1, PrincipalVerifier, VerifiedSubjectV1};
use agora_protocol::{
    ClientIdV1, CommandV1, OutcomeV1, RefusalV1, ReplyV1, RequestIdV1, RequestV1,
};
use agora_service::{Service, ServiceLimits};
use agora_wire::{WireLimits, WireReplyV1, WireRequestV1, decode_reply_v1, encode_request_v1};
use std::cell::Cell;
use std::rc::Rc;

#[derive(Clone)]
struct Context {
    participant: &'static str,
    subject: u8,
    active: bool,
}

struct Verifier {
    calls: Rc<Cell<u64>>,
}

impl PrincipalVerifier for Verifier {
    type Context = Context;
    type Error = ();

    fn authenticate(
        &mut self,
        claimed_participant: &str,
        context: &Self::Context,
        _now: HostTimeV1,
    ) -> Result<VerifiedSubjectV1, Self::Error> {
        self.calls.set(self.calls.get() + 1);
        if context.active && claimed_participant == context.participant {
            Ok(VerifiedSubjectV1::from_verifier([context.subject; 16]))
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
        self.calls.set(self.calls.get() + 1);
        if context.active
            && participant == context.participant
            && subject == VerifiedSubjectV1::from_verifier([context.subject; 16])
        {
            Ok(())
        } else {
            Err(())
        }
    }
}

fn engine() -> Engine {
    let mut engine = Engine::new();
    for (id, kind) in [
        ("alice", ParticipantKind::Human),
        ("bot", ParticipantKind::Agent),
    ] {
        engine
            .apply(CommandV2::RegisterParticipant(Participant::new(
                id, id, kind,
            )))
            .unwrap();
    }
    engine
        .apply(CommandV2::CreateRoom {
            room_id: "general".into(),
            name: "General".to_owned(),
            owner: "alice".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(CommandV2::Join {
            room_id: "general".into(),
            participant: "bot".into(),
        })
        .unwrap();
    engine
}

fn limits(max_sessions: usize) -> HostLimits {
    HostLimits {
        max_sessions,
        max_retired_sessions: 2,
        wire: WireLimits {
            max_frame_bytes: 4_096,
            max_text_bytes: 128,
            max_fields: 256,
            max_list_items: 32,
        },
    }
}

fn make_host(max_sessions: usize) -> (Host<Verifier>, Rc<Cell<u64>>) {
    let calls = Rc::new(Cell::new(0));
    let service = Service::new(engine(), ServiceLimits::default());
    (
        Host::new(
            service,
            Verifier {
                calls: calls.clone(),
            },
            limits(max_sessions),
        ),
        calls,
    )
}

fn connect_frame(participant: &str, limits: HostLimits) -> Vec<u8> {
    encode_request_v1(
        &WireRequestV1::Connect {
            participant_id: participant.to_owned(),
        },
        limits.wire,
    )
    .unwrap()
}

fn open(
    host: &mut Host<Verifier>,
    participant: &'static str,
    subject: u8,
) -> (agora_host::HostSessionIdV1, ClientIdV1, Context) {
    let context = Context {
        participant,
        subject,
        active: true,
    };
    let reply = host
        .open(
            &connect_frame(participant, limits(8)),
            &context,
            HostTimeV1(1),
        )
        .unwrap();
    let session = reply.session().unwrap();
    let WireReplyV1::Connected(client) = decode_reply_v1(reply.frame(), limits(8).wire).unwrap()
    else {
        panic!("connect should succeed");
    };
    (session, client, context)
}

fn execute_frame(client: ClientIdV1, request_id: u64, revision: u64) -> Vec<u8> {
    encode_request_v1(
        &WireRequestV1::Execute {
            client,
            request: RequestV1 {
                request_id: RequestIdV1(request_id),
                command: CommandV1::Speak {
                    room_id: "general".to_owned(),
                    expected_revision: revision,
                    logical_time: 7,
                    body: "hello".to_owned(),
                    addressed_to: vec![],
                },
            },
        },
        limits(8).wire,
    )
    .unwrap()
}

#[test]
fn verified_connection_binds_exact_frames_to_service_identity_and_replay() {
    let (mut host, _) = make_host(2);
    let (session, client, context) = open(&mut host, "alice", 1);
    let frame = execute_frame(client, 9, 4);

    let first = host
        .handle(session, &frame, &context, HostTimeV1(2))
        .unwrap();
    let second = host
        .handle(session, &frame, &context, HostTimeV1(3))
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(
        decode_reply_v1(&first, limits(8).wire).unwrap(),
        WireReplyV1::Executed(ReplyV1::Accepted(OutcomeV1::MessageAccepted {
            revision: 5,
            sequence: 0,
        }))
    );
    assert_eq!(host.service().engine().revision(), Revision(5));
    let room = host.service().engine().room(&"general".into()).unwrap();
    assert_eq!(room.messages[0].author().as_str(), "alice");
    assert_eq!(host.usage().executed, 2);
    assert!(!format!("{host:?}").contains("alice"));
}

#[test]
fn authentication_failure_is_non_enumerating_and_retains_no_session() {
    let (mut host, calls) = make_host(2);
    let context = Context {
        participant: "alice",
        subject: 1,
        active: false,
    };
    let first = host
        .open(&connect_frame("alice", limits(2)), &context, HostTimeV1(1))
        .unwrap();
    let second = host
        .open(
            &connect_frame("not-registered", limits(2)),
            &context,
            HostTimeV1(1),
        )
        .unwrap();
    assert_eq!(first.session(), None);
    assert_eq!(second.session(), None);
    assert_eq!(first.frame(), second.frame());
    assert_eq!(
        decode_reply_v1(first.frame(), limits(2).wire).unwrap(),
        WireReplyV1::ConnectRefused(RefusalV1::Unavailable)
    );
    assert_eq!(calls.get(), 2);
}

#[test]
fn context_revocation_closes_service_client_and_allows_replacement() {
    let (mut host, _) = make_host(1);
    let (session, client, mut context) = open(&mut host, "alice", 1);
    context.active = false;
    assert_eq!(
        host.handle(
            session,
            &execute_frame(client, 0, 4),
            &context,
            HostTimeV1(2)
        ),
        Err(HostRefusal::Unavailable)
    );
    assert!(!host.close(session));

    let replacement = open(&mut host, "alice", 1);
    assert_ne!(replacement.1, client);
}

#[test]
fn host_time_regression_retires_the_session_before_revalidation() {
    let (mut host, calls) = make_host(1);
    let context = Context {
        participant: "alice",
        subject: 1,
        active: true,
    };
    let reply = host
        .open(&connect_frame("alice", limits(1)), &context, HostTimeV1(10))
        .unwrap();
    let session = reply.session().unwrap();
    let WireReplyV1::Connected(client) = decode_reply_v1(reply.frame(), limits(1).wire).unwrap()
    else {
        panic!("connect should succeed");
    };
    let before = calls.get();
    assert_eq!(
        host.handle(
            session,
            &execute_frame(client, 0, 4),
            &context,
            HostTimeV1(9)
        ),
        Err(HostRefusal::Unavailable)
    );
    assert_eq!(calls.get(), before);
    assert!(!host.close(session));
}

#[test]
fn client_handle_substitution_retires_only_the_attacking_session() {
    let (mut host, _) = make_host(2);
    let (alice_session, alice_client, alice_context) = open(&mut host, "alice", 1);
    let (bot_session, bot_client, bot_context) = open(&mut host, "bot", 2);

    assert_eq!(
        host.handle(
            alice_session,
            &execute_frame(bot_client, 0, 4),
            &alice_context,
            HostTimeV1(2)
        ),
        Err(HostRefusal::Unavailable)
    );
    assert_eq!(
        host.handle(
            alice_session,
            &execute_frame(alice_client, 0, 4),
            &alice_context,
            HostTimeV1(2)
        ),
        Err(HostRefusal::Unavailable)
    );
    assert!(
        host.handle(
            bot_session,
            &execute_frame(bot_client, 0, 4),
            &bot_context,
            HostTimeV1(2)
        )
        .is_ok()
    );
}

#[test]
fn capacity_and_malformed_frames_preflight_authentication() {
    let (mut host, calls) = make_host(1);
    let (_session, _client, _context) = open(&mut host, "alice", 1);
    let before = calls.get();
    let context = Context {
        participant: "bot",
        subject: 2,
        active: true,
    };
    let refused = host
        .open(&connect_frame("bot", limits(1)), &context, HostTimeV1(2))
        .unwrap();
    assert_eq!(calls.get(), before);
    assert_eq!(
        decode_reply_v1(refused.frame(), limits(1).wire).unwrap(),
        WireReplyV1::ConnectRefused(RefusalV1::ClientLimit)
    );

    let (mut empty_host, empty_calls) = make_host(1);
    assert!(matches!(
        empty_host.open(&[1], &context, HostTimeV1(1)),
        Err(HostRefusal::InvalidFrame(_))
    ));
    assert_eq!(empty_calls.get(), 0);
}

#[test]
fn explicit_disconnect_and_transport_close_are_cleanup_paths() {
    let (mut host, _) = make_host(2);
    let (session, client, context) = open(&mut host, "alice", 1);
    let frame = encode_request_v1(&WireRequestV1::Disconnect(client), limits(2).wire).unwrap();
    let reply = host
        .handle(session, &frame, &context, HostTimeV1(2))
        .unwrap();
    assert_eq!(
        decode_reply_v1(&reply, limits(2).wire).unwrap(),
        WireReplyV1::Disconnected
    );
    assert!(!host.close(session));

    let (session, _, _) = open(&mut host, "bot", 2);
    assert!(host.close(session));
    assert!(!host.close(session));
}
