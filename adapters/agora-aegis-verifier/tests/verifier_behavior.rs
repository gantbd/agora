use aegis_client::ServicePort;
use aegis_protocol::{
    ActionV1, CommandV1, DecisionHandleV1, DecisionSummaryV1, EngineNamespaceV1, GrantHandleV1,
    OutcomeV1, RefusalV1, ReplyV1, SessionHandleV1,
};
use agora_aegis_verifier::{
    AegisContextV1, AegisOperatorRestoreAuthorityV1, AegisOperatorRestoreContextV1, AegisVerifierV1,
};
use agora_durable::{OperatorRestoreAuthorityV1, OperatorRestoreRequestV1};
use agora_engine::{CommandV2 as EngineCommand, Engine, Participant, ParticipantKind};
use agora_host::v2::HostV2;
use agora_host::v2::persistence::HostCheckpointScopeV1;
use agora_host::{
    HostLimits, HostRefusal, HostTimeV1, ScheduledHostActionV1, ScheduledPrincipalVerifier,
    VerifiedSubjectV1,
};
use agora_service::v2::{ServiceLimitsV2, ServiceV2};
use agora_wire::WireLimits;
use agora_wire::v2::{WireReplyV2, WireRequestV2, decode_reply_v2, encode_request_v2};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

#[derive(Default)]
struct PortState {
    commands: Vec<(SessionHandleV1, CommandV1)>,
    replies: VecDeque<Result<ReplyV1, ()>>,
}

#[derive(Clone, Default)]
struct Port(Rc<RefCell<PortState>>);

impl ServicePort for Port {
    type Error = ();

    fn execute(
        &mut self,
        session: SessionHandleV1,
        command: CommandV1,
    ) -> Result<ReplyV1, Self::Error> {
        let mut state = self.0.borrow_mut();
        state.commands.push((session, command));
        state.replies.pop_front().unwrap_or(Err(()))
    }
}

fn authorized(expires_at: u64) -> ReplyV1 {
    ReplyV1::Accepted(OutcomeV1::Authorized(DecisionSummaryV1 {
        handle: DecisionHandleV1(9),
        epoch: 3,
        expires_at,
        engine: EngineNamespaceV1::Agora,
        action: ActionV1::Submit,
    }))
}

fn scheduled_authorized(action: ActionV1, expires_at: u64) -> ReplyV1 {
    ReplyV1::Accepted(OutcomeV1::Authorized(DecisionSummaryV1 {
        handle: DecisionHandleV1(12),
        epoch: 3,
        expires_at,
        engine: EngineNamespaceV1::Agora,
        action,
    }))
}

#[test]
fn scheduled_owner_authority_maps_publish_and_observe_to_distinct_actions() {
    let port = Port::default();
    port.0.borrow_mut().replies = VecDeque::from([
        Ok(scheduled_authorized(ActionV1::Execute, 10)),
        Ok(scheduled_authorized(ActionV1::Observe, 10)),
        Ok(scheduled_authorized(ActionV1::Submit, 10)),
    ]);
    let mut verifier = AegisVerifierV1::new(port.clone(), [0x44; 16]);
    let context = context("alice", 1);
    let subject = VerifiedSubjectV1::from_verifier([1; 16]);
    verifier
        .authorize_scheduled(
            subject,
            "alice",
            &context,
            HostTimeV1(2),
            ScheduledHostActionV1::Publish,
        )
        .unwrap();
    verifier
        .authorize_scheduled(
            subject,
            "alice",
            &context,
            HostTimeV1(3),
            ScheduledHostActionV1::Observe,
        )
        .unwrap();
    assert!(
        verifier
            .authorize_scheduled(
                subject,
                "alice",
                &context,
                HostTimeV1(4),
                ScheduledHostActionV1::Publish,
            )
            .is_err(),
        "an inexact Submit reply cannot satisfy Execute"
    );
    let actions: Vec<ActionV1> = port
        .0
        .borrow()
        .commands
        .iter()
        .map(|(_, command)| match command {
            CommandV1::Authorize { action, .. } => *action,
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(
        actions,
        [ActionV1::Execute, ActionV1::Observe, ActionV1::Execute]
    );
}

fn restore_authorized(expires_at: u64) -> ReplyV1 {
    ReplyV1::Accepted(OutcomeV1::Authorized(DecisionSummaryV1 {
        handle: DecisionHandleV1(10),
        epoch: 3,
        expires_at,
        engine: EngineNamespaceV1::Agora,
        action: ActionV1::Administer,
    }))
}

fn limits() -> HostLimits {
    HostLimits {
        max_sessions: 2,
        max_retired_sessions: 4,
        wire: WireLimits::reference(),
    }
}

fn context(participant: &str, subject: u8) -> AegisContextV1 {
    AegisContextV1::new(
        participant,
        VerifiedSubjectV1::from_verifier([subject; 16]),
        SessionHandleV1(7),
        GrantHandleV1(11),
    )
}

fn host(
    replies: impl IntoIterator<Item = Result<ReplyV1, ()>>,
) -> (HostV2<AegisVerifierV1<Port>>, Port) {
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
    let port = Port::default();
    port.0.borrow_mut().replies = replies.into_iter().collect();
    let host = HostV2::new(
        ServiceV2::new(engine, ServiceLimitsV2::default()),
        AegisVerifierV1::new(port.clone(), [0x44; 16]),
        limits(),
    );
    (host, port)
}

fn connect(participant: &str) -> Vec<u8> {
    encode_request_v2(
        &WireRequestV2::Connect {
            participant_id: participant.to_owned(),
        },
        limits().wire,
    )
    .unwrap()
}

#[test]
fn authorization_maps_exact_session_grant_time_namespace_resource_and_action() {
    let (mut host, port) = host([Ok(authorized(10)), Ok(authorized(10))]);
    let context = context("alice", 1);
    let opened = host
        .open(&connect("alice"), &context, HostTimeV1(2))
        .unwrap();
    let session = opened.session().expect("Aegis-authorized session expected");
    assert!(matches!(
        decode_reply_v2(opened.frame(), limits().wire).unwrap(),
        WireReplyV2::Connected(_)
    ));
    assert!(host.close(session));

    let state = port.0.borrow();
    assert_eq!(state.commands.len(), 1);
    assert_eq!(state.commands[0].0, SessionHandleV1(7));
    assert_eq!(
        state.commands[0].1,
        CommandV1::Authorize {
            grant: GrantHandleV1(11),
            now: 2,
            engine: EngineNamespaceV1::Agora,
            resource: [0x44; 16],
            action: ActionV1::Submit,
        }
    );
}

#[test]
fn participant_mismatch_refusal_expiry_and_port_failure_are_non_enumerating() {
    for replies in [
        vec![Ok(ReplyV1::Refused(RefusalV1::Revoked))],
        vec![Ok(authorized(2))],
        vec![Err(())],
    ] {
        let (mut host, _) = host(replies);
        let opened = host
            .open(&connect("alice"), &context("alice", 1), HostTimeV1(2))
            .unwrap();
        assert!(opened.session().is_none());
        assert!(matches!(
            decode_reply_v2(opened.frame(), limits().wire).unwrap(),
            WireReplyV2::ConnectRefused(_)
        ));
    }

    let (mut host, port) = host([Ok(authorized(10))]);
    let opened = host
        .open(&connect("bot"), &context("alice", 1), HostTimeV1(2))
        .unwrap();
    assert!(opened.session().is_none());
    assert!(port.0.borrow().commands.is_empty());
}

#[test]
fn revalidation_refusal_retires_the_session_and_service_client() {
    let (mut host, _) = host([
        Ok(authorized(10)),
        Ok(ReplyV1::Refused(RefusalV1::Revoked)),
        Ok(authorized(10)),
    ]);
    let context = context("bot", 2);
    let opened = host.open(&connect("bot"), &context, HostTimeV1(1)).unwrap();
    let session = opened.session().expect("connection expected");
    let WireReplyV2::Connected(client) = decode_reply_v2(opened.frame(), limits().wire).unwrap()
    else {
        panic!("connected frame expected");
    };
    let frame = encode_request_v2(&WireRequestV2::Disconnect(client), limits().wire).unwrap();
    assert_eq!(
        host.handle(session, &frame, &context, HostTimeV1(2)),
        Err(HostRefusal::Unavailable)
    );
    assert!(!host.close(session));
    assert!(
        host.open(&connect("bot"), &context, HostTimeV1(3))
            .unwrap()
            .session()
            .is_some()
    );
}

#[test]
fn diagnostics_redact_participant_resource_session_grant_and_port() {
    let context = context("private-participant", 7);
    let verifier = AegisVerifierV1::new(Port::default(), [0x44; 16]);
    let context_debug = format!("{context:?}");
    let verifier_debug = format!("{verifier:?}");
    assert!(!context_debug.contains("private-participant"));
    assert!(!context_debug.contains("SessionHandleV1"));
    assert!(!context_debug.contains("GrantHandleV1"));
    assert!(!verifier_debug.contains("44"));
    assert!(!verifier_debug.contains("Port"));
}

#[test]
fn operator_restore_maps_administer_and_rejects_target_mismatch_locally() {
    const SCOPE: HostCheckpointScopeV1 = HostCheckpointScopeV1::new(*b"restore-scope-v1");
    let port = Port::default();
    port.0
        .borrow_mut()
        .replies
        .push_back(Ok(restore_authorized(10)));
    let mut authority =
        AegisOperatorRestoreAuthorityV1::new(port.clone(), [0x66; 16], "host", SCOPE);
    let restore_context = AegisOperatorRestoreContextV1::new(context("operator", 8), HostTimeV1(4));

    authority
        .authorize(
            &OperatorRestoreRequestV1::new("host", SCOPE, 17),
            &restore_context,
        )
        .unwrap();
    let evidence = authority.evidence().expect("restore evidence expected");
    assert_eq!(evidence.agora_revision, 17);
    assert_eq!(evidence.decision.action, ActionV1::Administer);
    assert_eq!(
        port.0.borrow().commands.as_slice(),
        &[(
            (SessionHandleV1(7)),
            CommandV1::Authorize {
                grant: GrantHandleV1(11),
                now: 4,
                engine: EngineNamespaceV1::Agora,
                resource: [0x66; 16],
                action: ActionV1::Administer,
            }
        )]
    );

    assert!(
        authority
            .authorize(
                &OperatorRestoreRequestV1::new("other", SCOPE, 17),
                &restore_context,
            )
            .is_err()
    );
    assert!(
        authority.evidence().is_none(),
        "failure clears stale evidence"
    );
    assert_eq!(port.0.borrow().commands.len(), 1);
}

#[test]
fn operator_restore_refusal_expiry_wrong_action_and_port_failure_are_erased() {
    const SCOPE: HostCheckpointScopeV1 = HostCheckpointScopeV1::new(*b"restore-scope-v1");
    for reply in [
        Ok(ReplyV1::Refused(RefusalV1::Revoked)),
        Ok(restore_authorized(4)),
        Ok(authorized(10)),
        Err(()),
    ] {
        let port = Port::default();
        port.0.borrow_mut().replies.push_back(reply);
        let mut authority = AegisOperatorRestoreAuthorityV1::new(port, [0x66; 16], "host", SCOPE);
        assert!(
            authority
                .authorize(
                    &OperatorRestoreRequestV1::new("host", SCOPE, 17),
                    &AegisOperatorRestoreContextV1::new(context("operator", 8), HostTimeV1(4),),
                )
                .is_err()
        );
    }
}
