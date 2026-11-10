use agora_engine::{
    AdmissionPolicy, CommandV2 as EngineCommand, Engine, LogicalTime, OutcomeV2 as EngineOutcome,
    Participant, ParticipantId, ParticipantKind, ReadScopeV1, Revision, RoomId,
};
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

fn next_mutation_bit(state: &mut u64, byte_len: usize) -> usize {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    (*state as usize) % (byte_len * 8)
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
    let mut seeded = engine();
    seeded
        .apply(EngineCommand::CreateRoom {
            room_id: "seed".into(),
            name: "Seed".to_owned(),
            owner: "alice".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    let EngineOutcome::MessageAccepted { sequence, .. } = seeded
        .apply(EngineCommand::Speak {
            room_id: "seed".into(),
            author: "alice".into(),
            at: LogicalTime(1),
            body: "private marker".to_owned(),
        })
        .unwrap()
    else {
        panic!("seed message expected");
    };
    let target = seeded.message(&RoomId::new("seed"), sequence).unwrap().id();
    seeded
        .advance_read_position(
            &ParticipantId::new("alice"),
            &RoomId::new("seed"),
            ReadScopeV1::Room,
            seeded.revision(),
            target,
        )
        .unwrap();
    let calls = Rc::new(Cell::new(0));
    let service = ServiceV2::new(seeded, ServiceLimitsV2::default());
    let mut host = HostV2::new(
        service,
        Verifier {
            calls: calls.clone(),
        },
        limits(2),
    );
    let (session, client, context) = open(&mut host, "alice", 1);
    let request = create_frame(client, 9, 4);
    let first = host
        .handle(session, &request, &context, HostTimeV1(2))
        .unwrap();
    let speak = encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(10),
                command: CommandV2::SpeakIdentified {
                    room_id: "private".to_owned(),
                    expected_revision: 5,
                    logical_time: 10,
                    body: "checkpoint original".to_owned(),
                    addressed_to: vec![],
                },
            },
        },
        limits(8).wire,
    )
    .unwrap();
    let speak_reply = host
        .handle(session, &speak, &context, HostTimeV1(2))
        .unwrap();
    let WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::IdentifiedMessageAccepted {
        message_id,
        ..
    })) = decode_reply_v2(&speak_reply, limits(8).wire).unwrap()
    else {
        panic!("identified message expected");
    };
    let edit = encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(11),
                command: CommandV2::EditMessage {
                    room_id: "private".to_owned(),
                    expected_revision: 6,
                    message_id,
                    logical_time: 20,
                    body: "checkpoint revised".to_owned(),
                },
            },
        },
        limits(8).wire,
    )
    .unwrap();
    assert!(matches!(
        decode_reply_v2(
            &host
                .handle(session, &edit, &context, HostTimeV1(2))
                .unwrap(),
            limits(8).wire
        )
        .unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::MessageEdited {
            revision: 7,
            ..
        }))
    ));
    let create_topic = encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(12),
                command: CommandV2::CreateTopic {
                    room_id: "private".to_owned(),
                    expected_revision: 7,
                    name: "incidents".to_owned(),
                },
            },
        },
        limits(8).wire,
    )
    .unwrap();
    let WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::TopicCreated { topic_id, .. })) =
        decode_reply_v2(
            &host
                .handle(session, &create_topic, &context, HostTimeV1(2))
                .unwrap(),
            limits(8).wire,
        )
        .unwrap()
    else {
        panic!("topic creation expected");
    };
    let topic_message = encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(13),
                command: CommandV2::SpeakInTopic {
                    room_id: "private".to_owned(),
                    expected_revision: 8,
                    topic_id,
                    logical_time: 21,
                    body: "checkpoint topic".to_owned(),
                    addressed_to: vec![],
                },
            },
        },
        limits(8).wire,
    )
    .unwrap();
    assert!(matches!(
        decode_reply_v2(
            &host
                .handle(session, &topic_message, &context, HostTimeV1(2))
                .unwrap(),
            limits(8).wire,
        )
        .unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::TopicMessageAccepted {
            revision: 9,
            ..
        }))
    ));
    let checkpoint = host
        .prepare_checkpoint(CHECKPOINT_SCOPE, MAX_CHECKPOINT)
        .unwrap();
    let fixture = include_str!("fixtures/host-checkpoint-v15.hex").trim();
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
    assert!(
        recovered
            .service()
            .engine()
            .read_position(
                &ParticipantId::new("alice"),
                &RoomId::new("seed"),
                &ReadScopeV1::Room,
            )
            .unwrap()
            .is_some()
    );
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
    assert_eq!(recovered.service().engine().revision(), Revision(9));
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

#[test]
fn fixed_seed_host_checkpoint_mutations_refuse_or_remain_canonical() {
    let checkpoint = decode_hex(include_str!("fixtures/host-checkpoint-v15.hex"));
    let mut seed = 0x7057_c4ae_c0de_0015u64;

    for sample in 0..512 {
        let bit = next_mutation_bit(&mut seed, checkpoint.len());
        let mut candidate = checkpoint.clone();
        candidate[bit / 8] ^= 1 << (bit % 8);
        if let Ok(recovered) = HostV2::recover_checkpoint(
            &candidate,
            CHECKPOINT_SCOPE,
            recovery_limits(),
            Verifier {
                calls: Rc::new(Cell::new(0)),
            },
        ) {
            assert_eq!(
                recovered
                    .prepare_checkpoint(CHECKPOINT_SCOPE, MAX_CHECKPOINT)
                    .unwrap()
                    .bytes(),
                candidate,
                "accepted mutation must remain canonical at sample {sample} bit {bit}"
            );
        }
    }
}
