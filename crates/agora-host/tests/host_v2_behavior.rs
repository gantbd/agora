use agora_engine::{CommandV2 as EngineCommand, Engine, Participant, ParticipantKind, Revision};
use agora_host::v2::HostV2;
use agora_host::v2::persistence::{
    HostCheckpointErrorV2, HostCheckpointScopeV1, HostRecoveryLimitsV2,
};
use agora_host::{HostLimits, HostRefusal, HostTimeV1, PrincipalVerifier, VerifiedSubjectV1};
use agora_protocol::v2::{
    AdmissionPolicyV2, ClientIdV2, CommandV2, OutcomeV2, RefusalV2, ReplyV2, RequestIdV2, RequestV2,
};
use agora_service::v2::persistence::ServiceRecoveryLimitsV2;
use agora_service::v2::{ServiceLimitsV2, ServiceV2};
use agora_wire::v2::{WireReplyV2, WireRequestV2, decode_reply_v2, encode_request_v2};
use agora_wire::{WireLimits, WireRequestV1, encode_request_v1};
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
            .apply(EngineCommand::RegisterParticipant(Participant::new(
                id, id, kind,
            )))
            .unwrap();
    }
    engine
}

fn limits(max_sessions: usize) -> HostLimits {
    HostLimits {
        max_sessions,
        max_retired_sessions: 2,
        wire: WireLimits {
            max_frame_bytes: 8_192,
            max_text_bytes: 128,
            max_fields: 512,
            max_list_items: 32,
        },
    }
}

fn make_host(max_sessions: usize) -> (HostV2<Verifier>, Rc<Cell<u64>>) {
    let calls = Rc::new(Cell::new(0));
    let service = ServiceV2::new(engine(), ServiceLimitsV2::default());
    (
        HostV2::new(
            service,
            Verifier {
                calls: calls.clone(),
            },
            limits(max_sessions),
        ),
        calls,
    )
}

fn connect_frame(participant: &str) -> Vec<u8> {
    encode_request_v2(
        &WireRequestV2::Connect {
            participant_id: participant.to_owned(),
        },
        limits(8).wire,
    )
    .unwrap()
}

fn open(
    host: &mut HostV2<Verifier>,
    participant: &'static str,
    subject: u8,
) -> (agora_host::v2::HostSessionIdV2, ClientIdV2, Context) {
    let context = Context {
        participant,
        subject,
        active: true,
    };
    let reply = host
        .open(&connect_frame(participant), &context, HostTimeV1(1))
        .unwrap();
    let session = reply.session().unwrap();
    let WireReplyV2::Connected(client) = decode_reply_v2(reply.frame(), limits(8).wire).unwrap()
    else {
        panic!("connect should succeed");
    };
    (session, client, context)
}

fn create_frame(client: ClientIdV2, request_id: u64, revision: u64) -> Vec<u8> {
    encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(request_id),
                command: CommandV2::CreateRoom {
                    room_id: "private".to_owned(),
                    name: "Private".to_owned(),
                    admission: AdmissionPolicyV2::InviteOnly,
                    expected_revision: revision,
                },
            },
        },
        limits(8).wire,
    )
    .unwrap()
}

const CHECKPOINT_SCOPE: HostCheckpointScopeV1 = HostCheckpointScopeV1::new(*b"host-test-scope!");
const MAX_CHECKPOINT: usize = 2 * 1_024 * 1_024;

fn recovery_limits() -> HostRecoveryLimitsV2 {
    HostRecoveryLimitsV2 {
        max_snapshot_bytes: MAX_CHECKPOINT,
        max_service_snapshot_bytes: MAX_CHECKPOINT,
        maximum_host_limits: limits(8),
        service: ServiceRecoveryLimitsV2 {
            max_snapshot_bytes: MAX_CHECKPOINT,
            max_engine_snapshot_bytes: MAX_CHECKPOINT,
            maximum_service_limits: ServiceLimitsV2::default(),
            maximum_engine_limits: agora_engine::Limits::default(),
        },
    }
}

#[test]
fn v2_verified_connection_binds_actor_and_exact_replay() {
    let (mut host, _) = make_host(2);
    let (session, client, context) = open(&mut host, "alice", 1);
    let frame = create_frame(client, 9, 2);
    let first = host
        .handle(session, &frame, &context, HostTimeV1(2))
        .unwrap();
    let second = host
        .handle(session, &frame, &context, HostTimeV1(3))
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(
        decode_reply_v2(&first, limits(8).wire).unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::RoomCreated { revision: 3 }))
    );
    assert_eq!(host.service().engine().revision(), Revision(3));
    let room = host.service().engine().room(&"private".into()).unwrap();
    assert_eq!(room.members[0].0.as_str(), "alice");
    assert_eq!(host.usage().executed, 2);
    assert!(!format!("{host:?}").contains("alice"));
}

#[test]
fn v2_authentication_failure_is_non_enumerating() {
    let (mut host, calls) = make_host(2);
    let context = Context {
        participant: "alice",
        subject: 1,
        active: false,
    };
    let first = host
        .open(&connect_frame("alice"), &context, HostTimeV1(1))
        .unwrap();
    let second = host
        .open(&connect_frame("missing"), &context, HostTimeV1(1))
        .unwrap();
    assert_eq!(first.session(), None);
    assert_eq!(second.session(), None);
    assert_eq!(first.frame(), second.frame());
    assert_eq!(
        decode_reply_v2(first.frame(), limits(2).wire).unwrap(),
        WireReplyV2::ConnectRefused(RefusalV2::Unavailable)
    );
    assert_eq!(calls.get(), 2);
}

#[test]
fn v2_context_revocation_and_client_substitution_retire_only_caller() {
    let (mut host, _) = make_host(2);
    let (alice_session, alice_client, mut alice_context) = open(&mut host, "alice", 1);
    let (bot_session, bot_client, bot_context) = open(&mut host, "bot", 2);

    assert_eq!(
        host.handle(
            alice_session,
            &create_frame(bot_client, 0, 2),
            &alice_context,
            HostTimeV1(2),
        ),
        Err(HostRefusal::Unavailable)
    );
    assert!(!host.close(alice_session));
    assert!(
        host.handle(
            bot_session,
            &create_frame(bot_client, 0, 2),
            &bot_context,
            HostTimeV1(2),
        )
        .is_ok()
    );

    let (replacement, replacement_client, _) = open(&mut host, "alice", 1);
    assert_ne!(replacement_client, alice_client);
    alice_context.active = false;
    assert_eq!(
        host.handle(
            replacement,
            &create_frame(replacement_client, 0, 3),
            &alice_context,
            HostTimeV1(2),
        ),
        Err(HostRefusal::Unavailable)
    );
}

#[test]
fn v2_capacity_and_version_mismatch_preflight_authentication() {
    let (mut host, calls) = make_host(1);
    let _ = open(&mut host, "alice", 1);
    let before = calls.get();
    let context = Context {
        participant: "bot",
        subject: 2,
        active: true,
    };
    let refused = host
        .open(&connect_frame("bot"), &context, HostTimeV1(2))
        .unwrap();
    assert_eq!(calls.get(), before);
    assert_eq!(
        decode_reply_v2(refused.frame(), limits(1).wire).unwrap(),
        WireReplyV2::ConnectRefused(RefusalV2::ClientLimit)
    );

    let (mut empty, empty_calls) = make_host(1);
    let v1 = encode_request_v1(
        &WireRequestV1::Connect {
            participant_id: "bot".to_owned(),
        },
        limits(1).wire,
    )
    .unwrap();
    assert!(matches!(
        empty.open(&v1, &context, HostTimeV1(1)),
        Err(HostRefusal::InvalidFrame(_))
    ));
    assert_eq!(empty_calls.get(), 0);
}

#[test]
fn v2_disconnect_and_transport_close_reclaim_service_state() {
    let (mut host, _) = make_host(2);
    let (session, client, context) = open(&mut host, "alice", 1);
    let frame = encode_request_v2(&WireRequestV2::Disconnect(client), limits(2).wire).unwrap();
    let reply = host
        .handle(session, &frame, &context, HostTimeV1(2))
        .unwrap();
    assert_eq!(
        decode_reply_v2(&reply, limits(2).wire).unwrap(),
        WireReplyV2::Disconnected
    );
    assert!(!host.close(session));

    let (session, _, _) = open(&mut host, "bot", 2);
    assert!(host.close(session));
    assert!(!host.close(session));
}

#[test]
fn v2_host_checkpoint_recovers_session_service_receipt_and_verifier_continuity() {
    let (mut host, _) = make_host(2);
    let (session, client, context) = open(&mut host, "alice", 1);
    let request = create_frame(client, 9, 2);
    let first = host
        .handle(session, &request, &context, HostTimeV1(2))
        .unwrap();
    let checkpoint = host
        .prepare_checkpoint(CHECKPOINT_SCOPE, MAX_CHECKPOINT)
        .unwrap();
    let fixture = include_str!("fixtures/host-checkpoint-v10.hex").trim();
    let actual = checkpoint
        .bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, fixture);

    let calls = Rc::new(Cell::new(0));
    let mut recovered = HostV2::recover_checkpoint(
        checkpoint.bytes(),
        CHECKPOINT_SCOPE,
        recovery_limits(),
        Verifier {
            calls: calls.clone(),
        },
    )
    .unwrap();
    assert_eq!(
        recovered
            .prepare_checkpoint(CHECKPOINT_SCOPE, MAX_CHECKPOINT)
            .unwrap()
            .bytes(),
        checkpoint.bytes()
    );
    assert_eq!(
        recovered
            .handle(session, &request, &context, HostTimeV1(3))
            .unwrap(),
        first
    );
    assert_eq!(recovered.service().engine().revision(), Revision(3));
    assert_eq!(calls.get(), 1, "recovered session must be revalidated");
}

#[test]
fn v2_host_checkpoint_is_scope_bound_bounded_and_closed_to_partial_state() {
    let (mut host, _) = make_host(2);
    let _ = open(&mut host, "alice", 1);
    let checkpoint = host
        .prepare_checkpoint(CHECKPOINT_SCOPE, MAX_CHECKPOINT)
        .unwrap();
    let calls = Rc::new(Cell::new(0));

    for end in 0..checkpoint.bytes().len() {
        assert!(
            HostV2::recover_checkpoint(
                &checkpoint.bytes()[..end],
                CHECKPOINT_SCOPE,
                recovery_limits(),
                Verifier {
                    calls: calls.clone(),
                },
            )
            .is_err()
        );
    }
    assert_eq!(
        HostV2::recover_checkpoint(
            checkpoint.bytes(),
            HostCheckpointScopeV1::new(*b"foreign-host-v1!"),
            recovery_limits(),
            Verifier { calls },
        )
        .unwrap_err(),
        HostCheckpointErrorV2::Scope
    );
    assert_eq!(format!("{checkpoint:?}"), "HostCheckpointV2(<redacted>)");

    let mut orphaned_service = ServiceV2::new(engine(), ServiceLimitsV2::default());
    orphaned_service.connect("alice").unwrap();
    let orphaned = HostV2::new(
        orphaned_service,
        Verifier {
            calls: Rc::new(Cell::new(0)),
        },
        limits(2),
    );
    assert_eq!(
        orphaned
            .prepare_checkpoint(CHECKPOINT_SCOPE, MAX_CHECKPOINT)
            .unwrap_err(),
        HostCheckpointErrorV2::Invalid
    );
}
