use agora_engine::{
    AdmissionPolicy, CommandV2 as EngineCommand, Engine, Participant, ParticipantKind, Revision,
};
use agora_host::transient::{TransientHostRefusalV1, TransientHostV1};
use agora_host::v2::{HostSessionIdV2, HostV2};
use agora_host::{HostLimits, HostRefusal, HostTimeV1, PrincipalVerifier, VerifiedSubjectV1};
use agora_protocol::transient::{
    PresencePageCursorV1, PresenceStateV1 as ProtocolPresenceStateV1, TransientCommandV1,
    TransientOutcomeV1, TransientRefusalV1 as ProtocolTransientRefusalV1, TransientReplyV1,
    TransientRequestIdV1, TransientRequestV1,
};
use agora_protocol::v2::{ClientIdV2, CommandV2, OutcomeV2, ReplyV2, RequestIdV2, RequestV2};
use agora_service::transient::{
    PresenceStateV1, PresenceViewStateV1, TransientLimitsV1, TransientRefusalV1, TransientTimeV1,
};
use agora_service::v2::{ServiceLimitsV2, ServiceV2};
use agora_wire::WireLimits;
use agora_wire::transient::{decode_transient_reply_v1, encode_transient_request_v1};
use agora_wire::v2::{WireReplyV2, WireRequestV2, decode_reply_v2, encode_request_v2};

#[derive(Clone)]
struct Context {
    participant: &'static str,
    subject: u8,
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

fn wire_limits() -> WireLimits {
    WireLimits {
        max_frame_bytes: 8_192,
        max_text_bytes: 128,
        max_fields: 512,
        max_list_items: 32,
    }
}

fn host() -> TransientHostV1<Verifier> {
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
        .apply(EngineCommand::CreateRoom {
            room_id: "room".into(),
            name: "room".to_owned(),
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
    let host = HostV2::new(
        ServiceV2::new(engine, ServiceLimitsV2::default()),
        Verifier,
        HostLimits {
            max_sessions: 8,
            max_retired_sessions: 8,
            wire: wire_limits(),
        },
    );
    TransientHostV1::new(
        host,
        TransientLimitsV1 {
            max_signals: 8,
            max_sources_per_participant: 2,
            max_lifetime: 10,
            max_page_items: 8,
        },
        2,
    )
    .unwrap()
}

fn open(
    host: &mut TransientHostV1<Verifier>,
    participant: &'static str,
    subject: u8,
) -> (HostSessionIdV2, ClientIdV2, Context) {
    let context = Context {
        participant,
        subject,
        active: true,
    };
    let frame = encode_request_v2(
        &WireRequestV2::Connect {
            participant_id: participant.to_owned(),
        },
        wire_limits(),
    )
    .unwrap();
    let opened = host.open(&frame, &context, HostTimeV1(1)).unwrap();
    let WireReplyV2::Connected(client) = decode_reply_v2(opened.frame(), wire_limits()).unwrap()
    else {
        panic!("connected reply expected");
    };
    (opened.session().unwrap(), client, context)
}

fn execute(client: ClientIdV2, request_id: u64, command: CommandV2) -> Vec<u8> {
    encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(request_id),
                command,
            },
        },
        wire_limits(),
    )
    .unwrap()
}

fn transient(request_id: u64, command: TransientCommandV1) -> Vec<u8> {
    encode_transient_request_v1(
        &TransientRequestV1 {
            request_id: TransientRequestIdV1(request_id),
            command,
        },
        wire_limits(),
    )
    .unwrap()
}

fn state(
    page: &agora_service::transient::PresencePageV1,
    participant: &str,
) -> PresenceViewStateV1 {
    page.entries
        .iter()
        .find(|entry| entry.participant_id.as_str() == participant)
        .map_or(PresenceViewStateV1::Unknown, |entry| entry.state)
}

#[test]
fn authenticated_sessions_publish_observe_and_close_without_domain_mutation() {
    let mut host = host();
    let (alice_session, _, alice_context) = open(&mut host, "alice", 1);
    let (bot_session, _, bot_context) = open(&mut host, "bot", 2);
    let revision = host.host().service().engine().revision();
    assert_eq!(
        host.publish_presence(
            alice_session,
            "room",
            PresenceStateV1::Available,
            5,
            &alice_context,
            HostTimeV1(2),
        )
        .unwrap(),
        TransientTimeV1(7)
    );
    let page = host
        .presence_page(bot_session, "room", None, 8, &bot_context, HostTimeV1(2))
        .unwrap();
    assert_eq!(state(&page, "alice"), PresenceViewStateV1::Available);
    assert_eq!(host.host().service().engine().revision(), revision);
    assert!(!format!("{host:?}").contains("alice"));

    assert!(host.close(alice_session));
    let page = host
        .presence_page(bot_session, "room", None, 8, &bot_context, HostTimeV1(3))
        .unwrap();
    assert_eq!(state(&page, "alice"), PresenceViewStateV1::Unknown);
}

#[test]
fn revocation_and_clock_regression_retire_the_session_source_before_refusal() {
    for regress_clock in [false, true] {
        let mut host = host();
        let (alice_session, _, mut alice_context) = open(&mut host, "alice", 1);
        let (bot_session, _, bot_context) = open(&mut host, "bot", 2);
        host.publish_presence(
            alice_session,
            "room",
            PresenceStateV1::Busy,
            5,
            &alice_context,
            HostTimeV1(2),
        )
        .unwrap();
        let refusal = if regress_clock {
            host.clear_presence(alice_session, "room", &alice_context, HostTimeV1(1))
        } else {
            alice_context.active = false;
            host.clear_presence(alice_session, "room", &alice_context, HostTimeV1(3))
        };
        assert!(matches!(
            refusal,
            Err(TransientHostRefusalV1::Host(HostRefusal::Unavailable))
        ));
        assert!(!host.close(alice_session));
        let page = host
            .presence_page(bot_session, "room", None, 8, &bot_context, HostTimeV1(3))
            .unwrap();
        assert_eq!(state(&page, "alice"), PresenceViewStateV1::Unknown);
    }
}

#[test]
fn leave_is_reconciled_before_reply_and_rejoin_cannot_resurrect_presence() {
    let mut host = host();
    let (alice_session, _, alice_context) = open(&mut host, "alice", 1);
    let (bot_session, bot_client, bot_context) = open(&mut host, "bot", 2);
    host.publish_presence(
        bot_session,
        "room",
        PresenceStateV1::Away,
        8,
        &bot_context,
        HostTimeV1(2),
    )
    .unwrap();
    let left = host
        .handle(
            bot_session,
            &execute(
                bot_client,
                1,
                CommandV2::Leave {
                    room_id: "room".to_owned(),
                    expected_revision: 4,
                },
            ),
            &bot_context,
            HostTimeV1(3),
        )
        .unwrap();
    assert_eq!(
        decode_reply_v2(&left, wire_limits()).unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::Left { revision: 5 }))
    );
    host.handle(
        bot_session,
        &execute(
            bot_client,
            2,
            CommandV2::Join {
                room_id: "room".to_owned(),
                expected_revision: 5,
            },
        ),
        &bot_context,
        HostTimeV1(4),
    )
    .unwrap();
    let page = host
        .presence_page(
            alice_session,
            "room",
            None,
            8,
            &alice_context,
            HostTimeV1(4),
        )
        .unwrap();
    assert_eq!(state(&page, "bot"), PresenceViewStateV1::Unknown);
    assert_eq!(host.host().service().engine().revision(), Revision(6));
}

#[test]
fn wrapper_reconstruction_is_explicit_restart_to_unknown() {
    let mut host = host();
    let (alice_session, _, alice_context) = open(&mut host, "alice", 1);
    let (bot_session, _, bot_context) = open(&mut host, "bot", 2);
    host.publish_presence(
        alice_session,
        "room",
        PresenceStateV1::Available,
        8,
        &alice_context,
        HostTimeV1(2),
    )
    .unwrap();
    let mut restarted =
        TransientHostV1::new(host.into_host(), TransientLimitsV1::default(), 2).unwrap();
    let page = restarted
        .presence_page(bot_session, "room", None, 8, &bot_context, HostTimeV1(3))
        .unwrap();
    assert_eq!(state(&page, "alice"), PresenceViewStateV1::Unknown);
}

#[test]
fn runtime_expiry_is_bounded_and_clock_regression_is_explicit() {
    let mut host = host();
    let (alice_session, _, alice_context) = open(&mut host, "alice", 1);
    host.publish_presence(
        alice_session,
        "room",
        PresenceStateV1::Available,
        1,
        &alice_context,
        HostTimeV1(2),
    )
    .unwrap();
    assert_eq!(host.expire(HostTimeV1(3), 1).unwrap(), 1);
    assert!(matches!(
        host.expire(HostTimeV1(2), 1),
        Err(TransientRefusalV1::ClockRegression)
    ));
}

#[test]
fn public_presence_frames_replay_exactly_and_fence_conflict_and_sequence() {
    let mut host = host();
    let (alice_session, _, alice_context) = open(&mut host, "alice", 1);
    let (bot_session, _, bot_context) = open(&mut host, "bot", 2);
    let revision = host.host().service().engine().revision();
    let publish = transient(
        0,
        TransientCommandV1::PublishPresence {
            room_id: "room".to_owned(),
            state: ProtocolPresenceStateV1::Available,
            lifetime: 5,
        },
    );
    let first = host
        .handle_transient(alice_session, &publish, &alice_context, HostTimeV1(2))
        .unwrap();
    assert_eq!(
        decode_transient_reply_v1(&first, wire_limits()).unwrap(),
        TransientReplyV1::Accepted(TransientOutcomeV1::PresencePublished {
            accepted_lifetime: 5,
        })
    );
    let replay = host
        .handle_transient(alice_session, &publish, &alice_context, HostTimeV1(3))
        .unwrap();
    assert_eq!(replay, first);

    let conflict = host
        .handle_transient(
            alice_session,
            &transient(
                0,
                TransientCommandV1::ClearPresence {
                    room_id: "room".to_owned(),
                },
            ),
            &alice_context,
            HostTimeV1(3),
        )
        .unwrap();
    assert_eq!(
        decode_transient_reply_v1(&conflict, wire_limits()).unwrap(),
        TransientReplyV1::Refused(ProtocolTransientRefusalV1::RequestConflict)
    );
    let future = host
        .handle_transient(
            alice_session,
            &transient(
                2,
                TransientCommandV1::ClearPresence {
                    room_id: "room".to_owned(),
                },
            ),
            &alice_context,
            HostTimeV1(3),
        )
        .unwrap();
    assert_eq!(
        decode_transient_reply_v1(&future, wire_limits()).unwrap(),
        TransientReplyV1::Refused(ProtocolTransientRefusalV1::RequestOutOfSequence)
    );

    let observed = host
        .handle_transient(
            bot_session,
            &transient(
                0,
                TransientCommandV1::PresencePage {
                    room_id: "room".to_owned(),
                    cursor: None,
                    limit: 8,
                },
            ),
            &bot_context,
            HostTimeV1(3),
        )
        .unwrap();
    let TransientReplyV1::Accepted(TransientOutcomeV1::PresencePage(page)) =
        decode_transient_reply_v1(&observed, wire_limits()).unwrap()
    else {
        panic!("presence page expected");
    };
    assert_eq!(
        page.entries
            .iter()
            .find(|entry| entry.participant_id == "alice")
            .unwrap()
            .state,
        agora_protocol::transient::PresenceViewStateV1::Available
    );
    assert_eq!(host.host().service().engine().revision(), revision);
}

#[test]
fn public_cursor_continues_one_bounded_snapshot_and_then_expires() {
    let mut host = host();
    let (alice_session, _, alice_context) = open(&mut host, "alice", 1);
    let (bot_session, _, bot_context) = open(&mut host, "bot", 2);
    host.publish_presence(
        bot_session,
        "room",
        PresenceStateV1::Busy,
        5,
        &bot_context,
        HostTimeV1(2),
    )
    .unwrap();

    let first = host
        .handle_transient(
            alice_session,
            &transient(
                0,
                TransientCommandV1::PresencePage {
                    room_id: "room".to_owned(),
                    cursor: None,
                    limit: 1,
                },
            ),
            &alice_context,
            HostTimeV1(2),
        )
        .unwrap();
    let TransientReplyV1::Accepted(TransientOutcomeV1::PresencePage(first)) =
        decode_transient_reply_v1(&first, wire_limits()).unwrap()
    else {
        panic!("presence page expected");
    };
    let cursor = first.next.unwrap();
    let second = host
        .handle_transient(
            alice_session,
            &transient(
                1,
                TransientCommandV1::PresencePage {
                    room_id: "room".to_owned(),
                    cursor: Some(cursor),
                    limit: 1,
                },
            ),
            &alice_context,
            HostTimeV1(3),
        )
        .unwrap();
    let TransientReplyV1::Accepted(TransientOutcomeV1::PresencePage(second)) =
        decode_transient_reply_v1(&second, wire_limits()).unwrap()
    else {
        panic!("presence page expected");
    };
    assert_eq!(second.entries[0].participant_id, "bot");
    assert_eq!(second.entries[0].remaining_lifetime, Some(5));

    let fresh = host
        .handle_transient(
            alice_session,
            &transient(
                2,
                TransientCommandV1::PresencePage {
                    room_id: "room".to_owned(),
                    cursor: None,
                    limit: 1,
                },
            ),
            &alice_context,
            HostTimeV1(3),
        )
        .unwrap();
    let TransientReplyV1::Accepted(TransientOutcomeV1::PresencePage(fresh)) =
        decode_transient_reply_v1(&fresh, wire_limits()).unwrap()
    else {
        panic!("presence page expected");
    };
    let expired = host
        .handle_transient(
            alice_session,
            &transient(
                3,
                TransientCommandV1::PresencePage {
                    room_id: "room".to_owned(),
                    cursor: fresh.next,
                    limit: 1,
                },
            ),
            &alice_context,
            HostTimeV1(6),
        )
        .unwrap();
    assert_eq!(
        decode_transient_reply_v1(&expired, wire_limits()).unwrap(),
        TransientReplyV1::Refused(ProtocolTransientRefusalV1::StaleCursor)
    );
}

#[test]
fn public_cursor_is_bound_to_session_and_room() {
    let mut host = host();
    let (alice_session, _, alice_context) = open(&mut host, "alice", 1);
    let (bot_session, _, bot_context) = open(&mut host, "bot", 2);
    let first = host
        .handle_transient(
            alice_session,
            &transient(
                0,
                TransientCommandV1::PresencePage {
                    room_id: "room".to_owned(),
                    cursor: None,
                    limit: 1,
                },
            ),
            &alice_context,
            HostTimeV1(2),
        )
        .unwrap();
    let TransientReplyV1::Accepted(TransientOutcomeV1::PresencePage(first)) =
        decode_transient_reply_v1(&first, wire_limits()).unwrap()
    else {
        panic!("presence page expected");
    };
    let foreign = host
        .handle_transient(
            bot_session,
            &transient(
                0,
                TransientCommandV1::PresencePage {
                    room_id: "room".to_owned(),
                    cursor: Some(first.next.unwrap_or(PresencePageCursorV1(0))),
                    limit: 1,
                },
            ),
            &bot_context,
            HostTimeV1(2),
        )
        .unwrap();
    assert_eq!(
        decode_transient_reply_v1(&foreign, wire_limits()).unwrap(),
        TransientReplyV1::Refused(ProtocolTransientRefusalV1::InvalidCursor)
    );
}

#[test]
fn refused_public_offer_still_fences_cross_session_clock_regression() {
    let mut host = host();
    let (alice_session, _, alice_context) = open(&mut host, "alice", 1);
    let (bot_session, _, bot_context) = open(&mut host, "bot", 2);
    let invalid = host
        .handle_transient(
            alice_session,
            &transient(
                0,
                TransientCommandV1::PresencePage {
                    room_id: "room".to_owned(),
                    cursor: Some(PresencePageCursorV1(77)),
                    limit: 1,
                },
            ),
            &alice_context,
            HostTimeV1(5),
        )
        .unwrap();
    assert_eq!(
        decode_transient_reply_v1(&invalid, wire_limits()).unwrap(),
        TransientReplyV1::Refused(ProtocolTransientRefusalV1::InvalidCursor)
    );

    assert_eq!(
        host.handle_transient(
            bot_session,
            &transient(
                0,
                TransientCommandV1::PublishPresence {
                    room_id: "room".to_owned(),
                    state: ProtocolPresenceStateV1::Busy,
                    lifetime: 2,
                },
            ),
            &bot_context,
            HostTimeV1(4),
        ),
        Err(HostRefusal::Unavailable)
    );
    assert!(!host.close(bot_session));
}
