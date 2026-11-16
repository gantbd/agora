use agora_delivery_projection_port::{
    DeliveryProjectionRefusalV1, DeliveryProjectionV1, DeliverySourceV1,
};
use agora_engine::retention::{
    ContentDigestProfileV1, ContentDigestV1, ExportProfileV1, ExportRecordV1, ExportRequestIdV1,
    ExportRetirementV1, ExportScopeBindingV1, RECORD_CLASSES_V1, RecordClassV1, RetentionBoundV1,
    RetentionFrontierV1, RetentionPolicyV1, RetentionRecordIdV1, materialize_export_v1,
};
use agora_engine::{
    AdmissionPolicy, CommandV2 as EngineCommand, Engine, LogicalTime, OutcomeV2 as EngineOutcome,
    Participant, ParticipantId, ParticipantKind, ReadScopeV1, Revision, RoomId, TopicId,
};
use agora_export_custody_port::{
    ExportCustodyAcceptedV1, ExportCustodyApplyV1, ExportCustodyDurabilityV1, ExportCustodyPortV1,
    ExportCustodyRefusalV1, ExportCustodyRequestV1, ExportCustodyRetirementAcceptedV1,
    ExportCustodyRetirementApplyV1, ExportCustodyRetirementRefusalV1,
    ExportCustodyRetirementRequestV1,
};
use agora_federated_publication_port::{
    FederatedPublicationIdV1, FederatedPublicationIntentV1, FederatedPublicationRefusalV1,
    FederatedPublicationRequestV1, FederatedPublicationScopeV1,
};
use agora_host::v2::persistence::{
    HostCheckpointErrorV2, HostCheckpointScopeV1, HostRecoveryLimitsV2,
};
use agora_host::v2::{
    AuthoritativeChangeRetentionHostRefusalV1, ExportHostRefusalV1, ExportRetirementHostRefusalV1,
    HostV2, ReceiptRetirementHostRefusalV1, RetentionPolicyHostRefusalV1,
};
use agora_host::{HostLimits, HostRefusal, HostTimeV1, PrincipalVerifier, VerifiedSubjectV1};
use agora_interaction_port::{
    InteractionContextRefusalV1, InteractionContextV1, InteractionInvocationIdV1,
    InteractionPromotionIntentV1, InteractionPromotionRequestV1, InteractionSourceV1,
};
use agora_protocol::v2::{
    AdmissionPolicyV2, AttachmentDescriptorV1, AttachmentIdV1, AttachmentIntegrityV1,
    AttachmentMessageTargetV1, AttachmentObjectKeyV1, AttachmentStoreIdV1, AttentionPreferenceV1,
    AttentionScopeV1, ClientIdV2, CommandV2, ConversationProfileV1, NotificationIntentV1,
    OutcomeV2, RefusalV2, ReplyV2, RequestIdV2, RequestV2, Sha256DigestV1, TopicCollectionFilterV1,
    TopicCollectionLifecycleV1, VellumArtifactMessageTargetV1, VellumArtifactReferenceIdV1,
    VellumArtifactReferenceV1, VellumDocumentTargetV1,
};
use agora_retention_authority_port::{
    AuthoritativeChangeRetentionAuthorityRequestV1, AuthorityDecisionV1, ExportAuthorityRequestV1,
    ExportRetirementAuthorityRequestV1, PolicyAuthorityRequestV1,
    ReceiptRetirementAuthorityRequestV1, RetentionAuthorityPortV1, RetentionPolicyRevisionV1,
};
use agora_scheduled_publication_port::{
    ScheduledPublicationIdV1, ScheduledPublicationIntentV1, ScheduledPublicationObservationV1,
    ScheduledPublicationRefusalV1, ScheduledPublicationRequestV1,
};
use agora_service::v2::persistence::ServiceRecoveryLimitsV2;
use agora_service::v2::{RetentionPolicyInstallV1, ServiceLimitsV2, ServiceV2};
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

struct RetentionAuthority {
    decision: AuthorityDecisionV1,
    policy_calls: usize,
    export_calls: usize,
    last_export: Option<ExportAuthorityRequestV1>,
    export_retirement_calls: usize,
    last_export_retirement: Option<ExportRetirementAuthorityRequestV1>,
    retirement_calls: usize,
    change_calls: usize,
    last_change: Option<AuthoritativeChangeRetentionAuthorityRequestV1>,
}

impl RetentionAuthorityPortV1 for RetentionAuthority {
    fn authorize_policy(&mut self, _request: PolicyAuthorityRequestV1) -> AuthorityDecisionV1 {
        self.policy_calls += 1;
        self.decision
    }

    fn authorize_export(&mut self, request: ExportAuthorityRequestV1) -> AuthorityDecisionV1 {
        self.export_calls += 1;
        self.last_export = Some(request);
        self.decision
    }

    fn authorize_authoritative_change_retention(
        &mut self,
        request: AuthoritativeChangeRetentionAuthorityRequestV1,
    ) -> AuthorityDecisionV1 {
        self.change_calls += 1;
        self.last_change = Some(request);
        self.decision
    }

    fn authorize_export_retirement(
        &mut self,
        request: ExportRetirementAuthorityRequestV1,
    ) -> AuthorityDecisionV1 {
        self.export_retirement_calls += 1;
        self.last_export_retirement = Some(request);
        self.decision
    }

    fn authorize_receipt_retirement(
        &mut self,
        request: &ReceiptRetirementAuthorityRequestV1,
    ) -> AuthorityDecisionV1 {
        self.retirement_calls += 1;
        assert!(!request.selection().is_empty());
        self.decision
    }
}

struct Custody {
    requests: Vec<ExportCustodyRequestV1>,
    result: Result<ExportCustodyAcceptedV1, ExportCustodyRefusalV1>,
    retirement_requests: Vec<ExportCustodyRetirementRequestV1>,
    retirement_result: Result<ExportCustodyRetirementAcceptedV1, ExportCustodyRetirementRefusalV1>,
}

impl ExportCustodyPortV1 for Custody {
    fn admit(
        &mut self,
        request: ExportCustodyRequestV1,
    ) -> Result<ExportCustodyAcceptedV1, ExportCustodyRefusalV1> {
        self.requests.push(request);
        self.result
    }

    fn retire(
        &mut self,
        request: ExportCustodyRetirementRequestV1,
    ) -> Result<ExportCustodyRetirementAcceptedV1, ExportCustodyRetirementRefusalV1> {
        self.retirement_requests.push(request);
        self.retirement_result
    }
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

fn export_digest(bytes: &[u8]) -> ContentDigestV1 {
    let mut digest = [0_u8; 32];
    for (index, byte) in bytes.iter().enumerate() {
        digest[index % 32] = digest[index % 32].wrapping_add(*byte);
    }
    ContentDigestV1::new(digest)
}

fn prepared_export(
    request: u8,
    source_engine_revision: u64,
    policy_revision: RetentionPolicyRevisionV1,
) -> agora_export_custody_port::ExportMaterializationV1 {
    let record = ExportRecordV1::new(
        RetentionRecordIdV1::new(RecordClassV1::RoomProjection, 1).unwrap(),
        vec![request],
        1,
    )
    .unwrap();
    let frontiers = RECORD_CLASSES_V1.map(|class| {
        RetentionFrontierV1::new(
            class,
            0,
            (class == RecordClassV1::RoomProjection).then_some(1),
        )
        .unwrap()
    });
    materialize_export_v1(
        ExportRequestIdV1::new([request; 16]).unwrap(),
        ExportProfileV1::RoomVisible,
        ExportScopeBindingV1::new([8; 32]).unwrap(),
        ContentDigestProfileV1::Sha256,
        source_engine_revision,
        policy_revision,
        [true, false, false, false, false, false],
        frontiers,
        &[record],
        1,
        1,
        1,
        32,
        export_digest,
    )
    .unwrap()
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

#[test]
fn scheduled_publication_binds_revalidated_session_actor_and_current_host_time() {
    let mut engine = engine();
    engine
        .apply(EngineCommand::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
            owner: "alice".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    let calls = Rc::new(Cell::new(0));
    let mut host = HostV2::new(
        ServiceV2::new(engine, ServiceLimitsV2::default()),
        Verifier {
            calls: calls.clone(),
        },
        limits(4),
    );
    let (session, _, mut context) = open(&mut host, "alice", 1);
    let request = ScheduledPublicationRequestV1 {
        id: ScheduledPublicationIdV1::new([3; 16]).unwrap(),
        intent: ScheduledPublicationIntentV1::new("room", 10, "scheduled"),
    };
    let accepted = host
        .publish_scheduled(session, request.clone(), &context, HostTimeV1(2))
        .unwrap();
    assert_eq!(accepted.revision, 4);
    assert_eq!(
        host.service()
            .engine()
            .message(&"room".into(), accepted.message_sequence)
            .unwrap()
            .author(),
        &"alice".into()
    );
    assert_eq!(
        host.publish_scheduled(session, request, &context, HostTimeV1(3)),
        Ok(accepted)
    );
    assert_eq!(
        host.observe_scheduled(
            session,
            ScheduledPublicationRequestV1 {
                id: ScheduledPublicationIdV1::new([3; 16]).unwrap(),
                intent: ScheduledPublicationIntentV1::new("room", 10, "scheduled"),
            },
            &context,
            HostTimeV1(3),
        ),
        Ok(ScheduledPublicationObservationV1::Accepted(accepted))
    );
    context.active = false;
    assert_eq!(
        host.publish_scheduled(
            session,
            ScheduledPublicationRequestV1 {
                id: ScheduledPublicationIdV1::new([4; 16]).unwrap(),
                intent: ScheduledPublicationIntentV1::new("room", 11, "revoked"),
            },
            &context,
            HostTimeV1(4),
        ),
        Err(ScheduledPublicationRefusalV1::NotAuthorized)
    );
    assert_eq!(host.service().active_client_count(), 0);
    assert_eq!(calls.get(), 5);
}

#[test]
fn federated_publication_binds_revalidated_session_actor_and_current_host_time() {
    let mut engine = engine();
    engine
        .apply(EngineCommand::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
            owner: "alice".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    let calls = Rc::new(Cell::new(0));
    let mut host = HostV2::new(
        ServiceV2::new(engine, ServiceLimitsV2::default()),
        Verifier {
            calls: calls.clone(),
        },
        limits(4),
    );
    let (session, _, mut context) = open(&mut host, "alice", 1);
    let request = FederatedPublicationRequestV1 {
        id: FederatedPublicationIdV1::new([13; 16]).unwrap(),
        scope: FederatedPublicationScopeV1::new([1; 64]),
        intent: FederatedPublicationIntentV1::new("room", "federated"),
    };
    let accepted = host
        .publish_federated(session, request.clone(), &context, HostTimeV1(2))
        .unwrap();
    assert_eq!(accepted.revision, 4);
    assert_eq!(accepted.event_sequence, 3);
    let message = host
        .service()
        .engine()
        .message(&"room".into(), accepted.message_sequence)
        .unwrap();
    assert_eq!(message.author(), &"alice".into());
    assert_eq!(message.at(), LogicalTime(2));
    assert_eq!(
        host.publish_federated(session, request, &context, HostTimeV1(3)),
        Ok(accepted)
    );
    context.active = false;
    assert_eq!(
        host.publish_federated(
            session,
            FederatedPublicationRequestV1 {
                id: FederatedPublicationIdV1::new([14; 16]).unwrap(),
                scope: FederatedPublicationScopeV1::new([1; 64]),
                intent: FederatedPublicationIntentV1::new("room", "revoked"),
            },
            &context,
            HostTimeV1(4),
        ),
        Err(FederatedPublicationRefusalV1::NotAuthorized)
    );
    assert_eq!(host.service().active_client_count(), 0);
    assert_eq!(calls.get(), 4);
}

#[test]
fn delivery_projection_binds_current_authenticated_observer_without_content() {
    let mut engine = engine();
    engine
        .apply(EngineCommand::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
            owner: "alice".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(EngineCommand::Speak {
            room_id: "room".into(),
            author: "alice".into(),
            at: LogicalTime(1),
            body: "host secret".to_owned(),
        })
        .unwrap();
    let calls = Rc::new(Cell::new(0));
    let mut host = HostV2::new(
        ServiceV2::new(engine, ServiceLimitsV2::default()),
        Verifier {
            calls: calls.clone(),
        },
        limits(4),
    );
    let (session, _, mut context) = open(&mut host, "alice", 1);
    let source = DeliverySourceV1 { event_sequence: 3 };
    let projection = host
        .project_delivery(session, source, &context, HostTimeV1(2))
        .unwrap();
    assert!(matches!(projection, DeliveryProjectionV1::Visible(_)));
    assert!(!format!("{projection:?}").contains("host secret"));
    context.active = false;
    assert_eq!(
        host.project_delivery(session, source, &context, HostTimeV1(3)),
        Err(DeliveryProjectionRefusalV1::NotAuthorized)
    );
    assert_eq!(host.service().active_client_count(), 0);
}

#[test]
fn interaction_context_and_promotion_bind_current_authenticated_participant() {
    let mut engine = engine();
    engine
        .apply(EngineCommand::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
            owner: "alice".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(EngineCommand::Speak {
            room_id: "room".into(),
            author: "alice".into(),
            at: LogicalTime(1),
            body: "source".to_owned(),
        })
        .unwrap();
    let calls = Rc::new(Cell::new(0));
    let mut host = HostV2::new(
        ServiceV2::new(engine, ServiceLimitsV2::default()),
        Verifier {
            calls: calls.clone(),
        },
        limits(4),
    );
    let (session, _, mut context) = open(&mut host, "alice", 1);
    let source = InteractionSourceV1 { event_sequence: 3 };
    assert_eq!(
        host.interaction_context(session, source, &context, HostTimeV1(2)),
        Ok(InteractionContextV1::Available { source })
    );
    let request = InteractionPromotionRequestV1 {
        id: InteractionInvocationIdV1::new([8; 16]).unwrap(),
        intent: InteractionPromotionIntentV1::new(source, "room", 2, "proposal"),
    };
    let accepted = host
        .promote_interaction(session, request, &context, HostTimeV1(3))
        .unwrap();
    assert_eq!(accepted.revision, 5);
    assert_eq!(accepted.message_sequence, 1);
    context.active = false;
    assert_eq!(
        host.interaction_context(session, source, &context, HostTimeV1(4)),
        Err(InteractionContextRefusalV1::NotAuthorized)
    );
    assert_eq!(host.service().active_client_count(), 0);
}

#[test]
fn v2_host_supplies_trusted_time_for_symmetric_message_admission() {
    let mut engine = engine();
    engine
        .apply(EngineCommand::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
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
    let calls = Rc::new(Cell::new(0));
    let mut host = HostV2::new(
        ServiceV2::new(engine, ServiceLimitsV2::default()),
        Verifier {
            calls: calls.clone(),
        },
        limits(4),
    );
    let (alice_session, alice_client, alice_context) = open(&mut host, "alice", 1);
    let (bot_session, bot_client, bot_context) = open(&mut host, "bot", 2);
    let frame = |client, request_id, command| {
        encode_request_v2(
            &WireRequestV2::Execute {
                client,
                request: RequestV2 {
                    request_id: RequestIdV2(request_id),
                    command,
                },
            },
            limits(4).wire,
        )
        .unwrap()
    };
    let decode = |bytes: Vec<u8>| decode_reply_v2(&bytes, limits(4).wire).unwrap();

    assert!(matches!(
        decode(
            host.handle(
                alice_session,
                &frame(
                    alice_client,
                    1,
                    CommandV2::SetMinimumMessageInterval {
                        room_id: "room".to_owned(),
                        minimum_interval_ms: 100,
                        expected_revision: 4,
                    },
                ),
                &alice_context,
                HostTimeV1(10),
            )
            .unwrap()
        ),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::MessageIntervalChanged {
            revision: 5
        }))
    ));
    let speak = |expected_revision, body: &str| CommandV2::Speak {
        room_id: "room".to_owned(),
        expected_revision,
        logical_time: 0,
        body: body.to_owned(),
        addressed_to: Vec::new(),
    };
    for (session, client, context, request_id, revision, body) in [
        (alice_session, alice_client, &alice_context, 2, 5, "alice"),
        (bot_session, bot_client, &bot_context, 1, 6, "bot"),
    ] {
        assert!(matches!(
            decode(
                host.handle(
                    session,
                    &frame(client, request_id, speak(revision, body)),
                    context,
                    HostTimeV1(100),
                )
                .unwrap()
            ),
            WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::MessageAccepted { .. }))
        ));
    }
    let throttled = frame(alice_client, 3, speak(7, "too soon"));
    let expected = WireReplyV2::Executed(ReplyV2::Refused(RefusalV2::MessageThrottled {
        retry_after_ms: 51,
    }));
    assert_eq!(
        decode(
            host.handle(alice_session, &throttled, &alice_context, HostTimeV1(149),)
                .unwrap()
        ),
        expected
    );
    assert_eq!(
        decode(
            host.handle(alice_session, &throttled, &alice_context, HostTimeV1(150),)
                .unwrap()
        ),
        expected
    );
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
fn v2_change_retention_revalidates_exact_authority_and_preserves_delivery() {
    let (mut host, _) = make_host(2);
    let (session, _, context) = open(&mut host, "alice", 1);
    let revision = host.service().engine().revision();
    let policy_revision = RetentionPolicyRevisionV1::new(1).unwrap();
    let mut bounds = [RetentionBoundV1::Disabled; 6];
    bounds[RecordClassV1::AuthoritativeChanges as usize - 1] =
        RetentionBoundV1::entries(1).unwrap();
    let policy = RetentionPolicyV1::new(policy_revision, revision.0, bounds);
    let mut authority = RetentionAuthority {
        decision: AuthorityDecisionV1::Authorized,
        policy_calls: 0,
        export_calls: 0,
        last_export: None,
        export_retirement_calls: 0,
        last_export_retirement: None,
        retirement_calls: 0,
        change_calls: 0,
        last_change: None,
    };
    host.install_retention_policy(session, policy, &mut authority, &context, HostTimeV1(2))
        .unwrap();

    authority.decision = AuthorityDecisionV1::Denied;
    assert_eq!(
        host.apply_authoritative_change_retention(
            session,
            policy_revision,
            &mut authority,
            &context,
            HostTimeV1(3),
        ),
        Err(AuthoritativeChangeRetentionHostRefusalV1::AuthorityDenied)
    );
    assert_eq!(host.service().engine().retained_changes().len(), 2);
    authority.decision = AuthorityDecisionV1::Authorized;
    let applied = host
        .apply_authoritative_change_retention(
            session,
            policy_revision,
            &mut authority,
            &context,
            HostTimeV1(4),
        )
        .unwrap();
    assert_eq!(applied.discarded_records, 1);
    assert_eq!(host.service().engine().retained_changes().len(), 1);
    assert_eq!(host.service().engine().pending_events().len(), 2);
    assert_eq!(authority.change_calls, 2);
    let request = authority.last_change.unwrap();
    assert_eq!(request.observed_engine_revision, 2);
    assert_eq!(request.prior_discarded_through, 0);
    assert_eq!(request.retained_records, 2);
    assert!(!format!("{request:?}").contains("alice"));
}

#[test]
fn v2_receipt_retirement_requires_current_authority_and_is_participant_symmetric() {
    let (mut host, _) = make_host(4);
    let (alice_session, alice_client, alice_context) = open(&mut host, "alice", 1);
    let (bot_session, bot_client, bot_context) = open(&mut host, "bot", 2);
    let create = |client, request_id, room_id: &str, expected_revision| {
        encode_request_v2(
            &WireRequestV2::Execute {
                client,
                request: RequestV2 {
                    request_id,
                    command: CommandV2::CreateRoom {
                        room_id: room_id.to_owned(),
                        name: room_id.to_owned(),
                        admission: AdmissionPolicyV2::Open,
                        expected_revision,
                    },
                },
            },
            limits(4).wire,
        )
        .unwrap()
    };
    let alice_frame = create(alice_client, RequestIdV2(1), "alice-room", 2);
    let bot_frame = create(bot_client, RequestIdV2(1), "bot-room", 3);
    host.handle(alice_session, &alice_frame, &alice_context, HostTimeV1(2))
        .unwrap();
    host.handle(bot_session, &bot_frame, &bot_context, HostTimeV1(2))
        .unwrap();
    let selection = [(alice_client, RequestIdV2(1)), (bot_client, RequestIdV2(1))];
    let policy_revision = RetentionPolicyRevisionV1::new(1).unwrap();
    let mut authority = RetentionAuthority {
        decision: AuthorityDecisionV1::Denied,
        policy_calls: 0,
        export_calls: 0,
        last_export: None,
        export_retirement_calls: 0,
        last_export_retirement: None,
        retirement_calls: 0,
        change_calls: 0,
        last_change: None,
    };
    let policy = RetentionPolicyV1::new(
        policy_revision,
        host.service().engine().revision().0,
        [RetentionBoundV1::entries(4_096).unwrap(); 6],
    );
    assert_eq!(
        host.install_retention_policy(
            alice_session,
            policy,
            &mut authority,
            &alice_context,
            HostTimeV1(3),
        ),
        Err(RetentionPolicyHostRefusalV1::AuthorityDenied)
    );
    authority.decision = AuthorityDecisionV1::Stale;
    assert_eq!(
        host.install_retention_policy(
            alice_session,
            policy,
            &mut authority,
            &alice_context,
            HostTimeV1(4),
        ),
        Err(RetentionPolicyHostRefusalV1::AuthorityStale)
    );
    authority.decision = AuthorityDecisionV1::Authorized;
    assert_eq!(
        host.install_retention_policy(
            alice_session,
            policy,
            &mut authority,
            &alice_context,
            HostTimeV1(5),
        ),
        Ok(RetentionPolicyInstallV1::Installed)
    );
    assert_eq!(authority.policy_calls, 3);
    authority.decision = AuthorityDecisionV1::Denied;
    assert_eq!(
        host.retire_request_receipts(
            alice_session,
            policy_revision,
            &selection,
            &mut authority,
            &alice_context,
            HostTimeV1(6),
        ),
        Err(ReceiptRetirementHostRefusalV1::AuthorityDenied)
    );
    assert_eq!(host.service().retired_receipt_count(), 0);
    authority.decision = AuthorityDecisionV1::Stale;
    assert_eq!(
        host.retire_request_receipts(
            alice_session,
            policy_revision,
            &selection,
            &mut authority,
            &alice_context,
            HostTimeV1(7),
        ),
        Err(ReceiptRetirementHostRefusalV1::AuthorityStale)
    );
    assert_eq!(host.service().retired_receipt_count(), 0);
    authority.decision = AuthorityDecisionV1::Authorized;
    assert_eq!(
        host.retire_request_receipts(
            alice_session,
            policy_revision,
            &selection,
            &mut authority,
            &alice_context,
            HostTimeV1(8),
        ),
        Ok(2)
    );
    assert_eq!(authority.retirement_calls, 3);
    assert_eq!(host.service().retired_receipt_count(), 2);
    for (session, frame, context) in [
        (alice_session, &alice_frame, &alice_context),
        (bot_session, &bot_frame, &bot_context),
    ] {
        let reply = host.handle(session, frame, context, HostTimeV1(9)).unwrap();
        assert_eq!(
            decode_reply_v2(&reply, limits(4).wire).unwrap(),
            WireReplyV2::Executed(ReplyV2::Refused(RefusalV2::RequestConflict))
        );
    }
    let checkpoint = host
        .prepare_checkpoint(CHECKPOINT_SCOPE, MAX_CHECKPOINT)
        .unwrap();
    assert_eq!(checkpoint.bytes()[8], 29);
    let recovered = HostV2::recover_checkpoint(
        checkpoint.bytes(),
        CHECKPOINT_SCOPE,
        recovery_limits(),
        Verifier {
            calls: Rc::new(Cell::new(0)),
        },
    )
    .unwrap();
    assert_eq!(recovered.service().retention_policy(), Some(policy));
    assert_eq!(recovered.service().retired_receipt_count(), 2);
}

#[test]
fn v2_export_admission_revalidates_exact_authority_before_separate_custody() {
    let (mut host, _) = make_host(2);
    let (alice_session, _, alice_context) = open(&mut host, "alice", 1);
    let (bot_session, _, bot_context) = open(&mut host, "bot", 2);
    let engine_revision = host.service().engine().revision().0;
    let policy_revision = RetentionPolicyRevisionV1::new(1).unwrap();
    let policy = RetentionPolicyV1::new(
        policy_revision,
        engine_revision,
        [RetentionBoundV1::entries(4_096).unwrap(); 6],
    );
    let mut authority = RetentionAuthority {
        decision: AuthorityDecisionV1::Authorized,
        policy_calls: 0,
        export_calls: 0,
        last_export: None,
        export_retirement_calls: 0,
        last_export_retirement: None,
        retirement_calls: 0,
        change_calls: 0,
        last_change: None,
    };
    host.install_retention_policy(
        alice_session,
        policy,
        &mut authority,
        &alice_context,
        HostTimeV1(3),
    )
    .unwrap();
    let accepted = ExportCustodyAcceptedV1 {
        apply: ExportCustodyApplyV1::Materialized,
        durability: ExportCustodyDurabilityV1::Committed { store_revision: 4 },
    };
    let mut custody = Custody {
        requests: Vec::new(),
        result: Ok(accepted),
        retirement_requests: Vec::new(),
        retirement_result: Ok(ExportCustodyRetirementAcceptedV1 {
            apply: ExportCustodyRetirementApplyV1::Retired,
            durability: ExportCustodyDurabilityV1::Committed { store_revision: 5 },
        }),
    };

    assert_eq!(
        host.admit_export(
            alice_session,
            prepared_export(1, engine_revision - 1, policy_revision),
            &mut authority,
            &mut custody,
            &alice_context,
            HostTimeV1(4),
        ),
        Err(ExportHostRefusalV1::StaleSource)
    );
    assert_eq!(authority.export_calls, 0);
    assert!(custody.requests.is_empty());

    authority.decision = AuthorityDecisionV1::Denied;
    assert_eq!(
        host.admit_export(
            alice_session,
            prepared_export(1, engine_revision, policy_revision),
            &mut authority,
            &mut custody,
            &alice_context,
            HostTimeV1(5),
        ),
        Err(ExportHostRefusalV1::AuthorityDenied)
    );
    assert_eq!(authority.export_calls, 1);
    assert!(custody.requests.is_empty());

    authority.decision = AuthorityDecisionV1::Authorized;
    assert_eq!(
        host.admit_export(
            alice_session,
            prepared_export(1, engine_revision, policy_revision),
            &mut authority,
            &mut custody,
            &alice_context,
            HostTimeV1(6),
        ),
        Ok(accepted)
    );
    let alice_principal = authority.last_export.unwrap().principal;
    assert_eq!(
        host.admit_export(
            bot_session,
            prepared_export(2, engine_revision, policy_revision),
            &mut authority,
            &mut custody,
            &bot_context,
            HostTimeV1(6),
        ),
        Ok(accepted)
    );
    let bot_request = authority.last_export.unwrap();
    assert_ne!(alice_principal, bot_request.principal);
    assert_eq!(
        bot_request.request,
        ExportRequestIdV1::new([2; 16]).unwrap()
    );
    assert_eq!(bot_request.source_engine_revision, engine_revision);
    assert_eq!(bot_request.policy_revision, policy_revision);
    assert_eq!(authority.export_calls, 3);
    assert_eq!(custody.requests.len(), 2);
    assert!(custody.requests.iter().all(|request| {
        request.current_engine_revision == engine_revision
            && request.current_policy_revision == policy_revision
    }));
}

#[test]
fn v2_export_retirement_revalidates_exact_authority_before_custody() {
    let (mut host, _) = make_host(2);
    let (alice_session, _, alice_context) = open(&mut host, "alice", 1);
    let (bot_session, _, bot_context) = open(&mut host, "bot", 2);
    let engine_revision = host.service().engine().revision().0;
    let policy_revision = RetentionPolicyRevisionV1::new(1).unwrap();
    let policy = RetentionPolicyV1::new(
        policy_revision,
        engine_revision,
        [RetentionBoundV1::entries(4_096).unwrap(); 6],
    );
    let mut authority = RetentionAuthority {
        decision: AuthorityDecisionV1::Authorized,
        policy_calls: 0,
        export_calls: 0,
        last_export: None,
        export_retirement_calls: 0,
        last_export_retirement: None,
        retirement_calls: 0,
        change_calls: 0,
        last_change: None,
    };
    host.install_retention_policy(
        alice_session,
        policy,
        &mut authority,
        &alice_context,
        HostTimeV1(3),
    )
    .unwrap();
    let export = prepared_export(1, engine_revision, policy_revision);
    let retirement = ExportRetirementV1::from_manifest(&export.manifest);
    let accepted = ExportCustodyRetirementAcceptedV1 {
        apply: ExportCustodyRetirementApplyV1::Retired,
        durability: ExportCustodyDurabilityV1::Committed { store_revision: 5 },
    };
    let mut custody = Custody {
        requests: Vec::new(),
        result: Err(ExportCustodyRefusalV1::Unavailable),
        retirement_requests: Vec::new(),
        retirement_result: Ok(accepted),
    };

    assert_eq!(
        host.retire_export(
            alice_session,
            ExportRetirementV1 {
                source_engine_revision: engine_revision - 1,
                ..retirement
            },
            &mut authority,
            &mut custody,
            &alice_context,
            HostTimeV1(4),
        ),
        Err(ExportRetirementHostRefusalV1::StaleSource)
    );
    assert_eq!(authority.export_retirement_calls, 0);
    assert!(custody.retirement_requests.is_empty());

    authority.decision = AuthorityDecisionV1::Denied;
    assert_eq!(
        host.retire_export(
            alice_session,
            retirement,
            &mut authority,
            &mut custody,
            &alice_context,
            HostTimeV1(5),
        ),
        Err(ExportRetirementHostRefusalV1::AuthorityDenied)
    );
    assert_eq!(authority.export_retirement_calls, 1);
    assert!(custody.retirement_requests.is_empty());

    authority.decision = AuthorityDecisionV1::Authorized;
    assert_eq!(
        host.retire_export(
            alice_session,
            retirement,
            &mut authority,
            &mut custody,
            &alice_context,
            HostTimeV1(6),
        ),
        Ok(accepted)
    );
    let alice_principal = authority.last_export_retirement.unwrap().principal;
    assert_eq!(
        host.retire_export(
            bot_session,
            retirement,
            &mut authority,
            &mut custody,
            &bot_context,
            HostTimeV1(6),
        ),
        Ok(accepted)
    );
    let bot_request = authority.last_export_retirement.unwrap();
    assert_ne!(alice_principal, bot_request.principal);
    assert_eq!(bot_request.retirement, retirement);
    assert_eq!(authority.export_retirement_calls, 3);
    assert_eq!(custody.retirement_requests.len(), 2);
    assert!(custody.retirement_requests.iter().all(|request| {
        request.current_engine_revision == engine_revision
            && request.current_policy_revision == policy_revision
            && request.retirement == retirement
    }));
}

#[test]
fn v2_peer_creation_binds_the_authenticated_initiator() {
    let (mut host, _) = make_host(2);
    let (session, client, context) = open(&mut host, "alice", 1);
    let create = encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(1),
                command: CommandV2::CreatePeerConversation {
                    room_id: "direct".to_owned(),
                    name: "Direct".to_owned(),
                    profile: ConversationProfileV1::Direct,
                    founding_participant_ids: vec!["alice".to_owned(), "bot".to_owned()],
                    expected_revision: 2,
                },
            },
        },
        limits(8).wire,
    )
    .unwrap();
    let reply = host
        .handle(session, &create, &context, HostTimeV1(2))
        .unwrap();
    assert_eq!(
        decode_reply_v2(&reply, limits(8).wire).unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::PeerConversationCreated {
            revision: 3,
            profile: ConversationProfileV1::Direct,
        }))
    );
    let event = host.service().engine().pending_events().last().unwrap();
    assert!(matches!(
        &event.event,
        agora_engine::DomainEventV2::PeerConversationCreated { initiator, .. }
            if initiator.as_str() == "alice"
    ));
}

#[test]
fn v2_attention_preferences_bind_the_authenticated_participant() {
    let mut engine = engine();
    engine
        .apply(EngineCommand::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
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
    let calls = Rc::new(Cell::new(0));
    let mut host = HostV2::new(
        ServiceV2::new(engine, ServiceLimitsV2::default()),
        Verifier {
            calls: calls.clone(),
        },
        limits(2),
    );
    let (alice_session, alice_client, alice_context) = open(&mut host, "alice", 1);
    let (bot_session, bot_client, bot_context) = open(&mut host, "bot", 2);
    let preference = AttentionPreferenceV1 {
        following: true,
        muted: true,
        notification_intent: NotificationIntentV1::AddressedOnly,
    };
    let set = encode_request_v2(
        &WireRequestV2::Execute {
            client: alice_client,
            request: RequestV2 {
                request_id: RequestIdV2(1),
                command: CommandV2::SetAttentionPreference {
                    room_id: "room".to_owned(),
                    observed_revision: 4,
                    scope: AttentionScopeV1::Room,
                    preference,
                },
            },
        },
        limits(2).wire,
    )
    .unwrap();
    assert_eq!(
        decode_reply_v2(
            &host
                .handle(alice_session, &set, &alice_context, HostTimeV1(3))
                .unwrap(),
            limits(2).wire,
        )
        .unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::AttentionPreferenceSet(
            preference
        )))
    );
    let observe = encode_request_v2(
        &WireRequestV2::Execute {
            client: bot_client,
            request: RequestV2 {
                request_id: RequestIdV2(1),
                command: CommandV2::AttentionPreference {
                    room_id: "room".to_owned(),
                    scope: AttentionScopeV1::Room,
                },
            },
        },
        limits(2).wire,
    )
    .unwrap();
    assert_eq!(
        decode_reply_v2(
            &host
                .handle(bot_session, &observe, &bot_context, HostTimeV1(3))
                .unwrap(),
            limits(2).wire,
        )
        .unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::AttentionPreference(
            AttentionPreferenceV1::default()
        )))
    );
    assert_eq!(host.service().engine().revision(), Revision(4));
    assert!(calls.get() >= 4);
}

#[test]
fn v2_pins_and_saves_bind_authenticated_participants_without_merging_visibility() {
    let mut engine = engine();
    engine
        .apply(EngineCommand::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
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
    let EngineOutcome::MessageAccepted { sequence, .. } = engine
        .apply(EngineCommand::Speak {
            room_id: "room".into(),
            author: "alice".into(),
            at: LogicalTime(1),
            body: "body".to_owned(),
        })
        .unwrap()
    else {
        panic!("message accepted expected");
    };
    let message_id = agora_protocol::v2::MessageIdV1::new(
        engine
            .message(&RoomId::new("room"), sequence)
            .unwrap()
            .id()
            .into_bytes(),
    );
    let revision = engine.revision().0;
    let calls = Rc::new(Cell::new(0));
    let mut host = HostV2::new(
        ServiceV2::new(engine, ServiceLimitsV2::default()),
        Verifier {
            calls: calls.clone(),
        },
        limits(2),
    );
    let (alice_session, alice_client, alice_context) = open(&mut host, "alice", 1);
    let (bot_session, bot_client, bot_context) = open(&mut host, "bot", 2);

    let save = encode_request_v2(
        &WireRequestV2::Execute {
            client: alice_client,
            request: RequestV2 {
                request_id: RequestIdV2(1),
                command: CommandV2::SetSavedReference {
                    room_id: "room".to_owned(),
                    observed_revision: revision,
                    message_id,
                    saved: true,
                },
            },
        },
        limits(2).wire,
    )
    .unwrap();
    assert!(matches!(
        decode_reply_v2(
            &host
                .handle(alice_session, &save, &alice_context, HostTimeV1(3))
                .unwrap(),
            limits(2).wire,
        )
        .unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::SavedReferenceSet {
            saved: true,
            ..
        }))
    ));

    let bot_saves = encode_request_v2(
        &WireRequestV2::Execute {
            client: bot_client,
            request: RequestV2 {
                request_id: RequestIdV2(1),
                command: CommandV2::SavedReferences {
                    room_id: "room".to_owned(),
                },
            },
        },
        limits(2).wire,
    )
    .unwrap();
    assert_eq!(
        decode_reply_v2(
            &host
                .handle(bot_session, &bot_saves, &bot_context, HostTimeV1(3))
                .unwrap(),
            limits(2).wire,
        )
        .unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::SavedReferences(vec![])))
    );

    let pin = encode_request_v2(
        &WireRequestV2::Execute {
            client: bot_client,
            request: RequestV2 {
                request_id: RequestIdV2(2),
                command: CommandV2::SetPin {
                    room_id: "room".to_owned(),
                    expected_revision: revision,
                    message_id,
                    pinned: true,
                },
            },
        },
        limits(2).wire,
    )
    .unwrap();
    assert!(matches!(
        decode_reply_v2(
            &host
                .handle(bot_session, &pin, &bot_context, HostTimeV1(4))
                .unwrap(),
            limits(2).wire,
        )
        .unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::PinChanged {
            pinned: true,
            ..
        }))
    ));

    let pins = encode_request_v2(
        &WireRequestV2::Execute {
            client: alice_client,
            request: RequestV2 {
                request_id: RequestIdV2(2),
                command: CommandV2::Pins {
                    room_id: "room".to_owned(),
                },
            },
        },
        limits(2).wire,
    )
    .unwrap();
    assert!(matches!(
        decode_reply_v2(
            &host
                .handle(alice_session, &pins, &alice_context, HostTimeV1(4))
                .unwrap(),
            limits(2).wire,
        )
        .unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::Pins(pins)))
            if pins.len() == 1 && pins[0].pinned_by == "bot"
    ));
    assert!(calls.get() >= 6);
}

#[test]
fn v2_polls_bind_authenticated_human_and_agent_without_disclosing_ballots() {
    let mut engine = engine();
    engine
        .apply(EngineCommand::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
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
    let revision = engine.revision().0;
    let calls = Rc::new(Cell::new(0));
    let mut host = HostV2::new(
        ServiceV2::new(engine, ServiceLimitsV2::default()),
        Verifier {
            calls: calls.clone(),
        },
        limits(2),
    );
    let (alice_session, alice_client, alice_context) = open(&mut host, "alice", 1);
    let (bot_session, bot_client, bot_context) = open(&mut host, "bot", 2);

    let create = encode_request_v2(
        &WireRequestV2::Execute {
            client: alice_client,
            request: RequestV2 {
                request_id: RequestIdV2(1),
                command: CommandV2::CreatePoll {
                    room_id: "room".to_owned(),
                    expected_revision: revision,
                    prompt: "Ship?".to_owned(),
                    options: vec!["yes".to_owned(), "no".to_owned()],
                    eligible_participant_ids: vec!["alice".to_owned(), "bot".to_owned()],
                    max_choices: 1,
                },
            },
        },
        limits(2).wire,
    )
    .unwrap();
    let WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::PollCreated { revision, poll_id })) =
        decode_reply_v2(
            &host
                .handle(alice_session, &create, &alice_context, HostTimeV1(3))
                .unwrap(),
            limits(2).wire,
        )
        .unwrap()
    else {
        panic!("poll creation expected")
    };

    let vote = encode_request_v2(
        &WireRequestV2::Execute {
            client: bot_client,
            request: RequestV2 {
                request_id: RequestIdV2(1),
                command: CommandV2::SetPollBallot {
                    room_id: "room".to_owned(),
                    expected_revision: revision,
                    poll_id,
                    selections: vec![0],
                },
            },
        },
        limits(2).wire,
    )
    .unwrap();
    assert!(matches!(
        decode_reply_v2(
            &host.handle(bot_session, &vote, &bot_context, HostTimeV1(4)).unwrap(),
            limits(2).wire,
        ).unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::PollBallotChanged {
            totals, total_ballots: 1, ..
        })) if totals == vec![1, 0]
    ));

    let observe = encode_request_v2(
        &WireRequestV2::Execute {
            client: alice_client,
            request: RequestV2 {
                request_id: RequestIdV2(2),
                command: CommandV2::PollBallot {
                    room_id: "room".to_owned(),
                    poll_id,
                },
            },
        },
        limits(2).wire,
    )
    .unwrap();
    assert!(matches!(
        decode_reply_v2(
            &host.handle(alice_session, &observe, &alice_context, HostTimeV1(5)).unwrap(),
            limits(2).wire,
        ).unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::PollBallot(ballot)))
            if ballot.selections.is_empty()
    ));
    assert!(calls.get() >= 5);
}

#[test]
fn v2_topic_collections_bind_the_authenticated_observer_and_exact_filter() {
    let mut engine = engine();
    engine
        .apply(EngineCommand::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
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
    let EngineOutcome::TopicCreated { topic_id, .. } = engine
        .apply(EngineCommand::CreateTopic {
            room_id: "room".into(),
            actor: "alice".into(),
            name: "work".to_owned(),
        })
        .unwrap()
    else {
        unreachable!()
    };
    engine
        .apply(EngineCommand::SetTopicTag {
            room_id: "room".into(),
            actor: "alice".into(),
            topic_id,
            tag: "urgent".to_owned(),
            added: true,
        })
        .unwrap();
    let calls = Rc::new(Cell::new(0));
    let mut host = HostV2::new(
        ServiceV2::new(engine, ServiceLimitsV2::default()),
        Verifier {
            calls: calls.clone(),
        },
        limits(1),
    );
    let (session, client, context) = open(&mut host, "bot", 2);
    let frame = encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(1),
                command: CommandV2::TopicCollection {
                    room_id: "room".to_owned(),
                    filter: TopicCollectionFilterV1 {
                        lifecycle: TopicCollectionLifecycleV1::Active,
                        tags: vec!["urgent".to_owned()],
                    },
                    cursor: None,
                    limit: 8,
                },
            },
        },
        limits(1).wire,
    )
    .unwrap();
    let reply = host
        .handle(session, &frame, &context, HostTimeV1(3))
        .unwrap();
    assert!(matches!(
        decode_reply_v2(&reply, limits(1).wire).unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::TopicCollection(page)))
            if page.topics.len() == 1
                && page.topics[0].topic_id.into_bytes() == topic_id.into_bytes()
                && page.filter.tags == vec!["urgent"]
    ));
    let checkpoint = host
        .prepare_checkpoint(CHECKPOINT_SCOPE, MAX_CHECKPOINT)
        .unwrap();
    assert_eq!(checkpoint.bytes()[8], 29);
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
            .handle(session, &frame, &context, HostTimeV1(4))
            .unwrap(),
        reply
    );
    assert!(calls.get() >= 2);
}

#[test]
fn v2_attention_receipt_and_private_state_recover_through_host_checkpoint() {
    let mut engine = engine();
    engine
        .apply(EngineCommand::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
            owner: "alice".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    let calls = Rc::new(Cell::new(0));
    let mut host = HostV2::new(
        ServiceV2::new(engine, ServiceLimitsV2::default()),
        Verifier {
            calls: calls.clone(),
        },
        limits(1),
    );
    let (session, client, context) = open(&mut host, "alice", 1);
    let preference = AttentionPreferenceV1 {
        following: true,
        muted: false,
        notification_intent: NotificationIntentV1::AllActivity,
    };
    let frame = encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(1),
                command: CommandV2::SetAttentionPreference {
                    room_id: "room".to_owned(),
                    observed_revision: 3,
                    scope: AttentionScopeV1::Room,
                    preference,
                },
            },
        },
        limits(1).wire,
    )
    .unwrap();
    let accepted = host
        .handle(session, &frame, &context, HostTimeV1(2))
        .unwrap();
    let checkpoint = host
        .prepare_checkpoint(CHECKPOINT_SCOPE, MAX_CHECKPOINT)
        .unwrap();
    assert_eq!(checkpoint.bytes()[8], 29);

    let mut recovered = HostV2::recover_checkpoint(
        checkpoint.bytes(),
        CHECKPOINT_SCOPE,
        recovery_limits(),
        Verifier { calls },
    )
    .unwrap();
    assert_eq!(
        recovered
            .handle(session, &frame, &context, HostTimeV1(3))
            .unwrap(),
        accepted
    );
    assert_eq!(
        decode_reply_v2(&accepted, limits(1).wire).unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::AttentionPreferenceSet(
            preference
        )))
    );
    assert_eq!(recovered.service().engine().revision(), Revision(3));
}

#[test]
fn v2_attachment_admission_binds_the_authenticated_actor() {
    let mut engine = engine();
    engine
        .apply(EngineCommand::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
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
    let calls = Rc::new(Cell::new(0));
    let mut host = HostV2::new(
        ServiceV2::new(engine, ServiceLimitsV2::default()),
        Verifier {
            calls: calls.clone(),
        },
        limits(2),
    );
    let (session, client, context) = open(&mut host, "bot", 2);
    let frame = encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(1),
                command: CommandV2::SpeakWithAttachments {
                    room_id: "room".to_owned(),
                    expected_revision: 4,
                    target: AttachmentMessageTargetV1::Room,
                    logical_time: 5,
                    body: "body".to_owned(),
                    addressed_to: vec![],
                    attachments: vec![AttachmentDescriptorV1 {
                        attachment_id: AttachmentIdV1::new([1; 16]),
                        store_id: AttachmentStoreIdV1::new([2; 16]),
                        object_key: AttachmentObjectKeyV1::new([3; 32]),
                        declared_bytes: 4,
                        media_type: "text/plain".to_owned(),
                        display_label: "note.txt".to_owned(),
                        integrity: AttachmentIntegrityV1::Sha256(Sha256DigestV1::new([4; 32])),
                    }],
                },
            },
        },
        limits(2).wire,
    )
    .unwrap();
    assert!(matches!(
        decode_reply_v2(
            &host
                .handle(session, &frame, &context, HostTimeV1(2))
                .unwrap(),
            limits(2).wire,
        )
        .unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::AttachmentMessageAccepted {
            revision: 5,
            ..
        }))
    ));
    assert_eq!(
        host.service()
            .engine()
            .message(&RoomId::new("room"), 0)
            .unwrap()
            .author()
            .as_str(),
        "bot"
    );
    assert!(calls.get() >= 2);
}

#[test]
fn v2_vellum_artifact_admission_binds_the_authenticated_actor() {
    let mut engine = engine();
    engine
        .apply(EngineCommand::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
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
    let calls = Rc::new(Cell::new(0));
    let mut host = HostV2::new(
        ServiceV2::new(engine, ServiceLimitsV2::default()),
        Verifier {
            calls: calls.clone(),
        },
        limits(2),
    );
    let (session, client, context) = open(&mut host, "bot", 2);
    let frame = encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(1),
                command: CommandV2::SpeakWithVellumArtifacts {
                    room_id: "room".to_owned(),
                    expected_revision: 4,
                    target: VellumArtifactMessageTargetV1::Room,
                    logical_time: 5,
                    body: "body".to_owned(),
                    addressed_to: vec![],
                    references: vec![VellumArtifactReferenceV1 {
                        reference_id: VellumArtifactReferenceIdV1::new([1; 16]),
                        target: VellumDocumentTargetV1 {
                            workspace_id: 2,
                            document_id: 3,
                            source_revision: 4,
                        },
                    }],
                },
            },
        },
        limits(2).wire,
    )
    .unwrap();
    assert!(matches!(
        decode_reply_v2(
            &host
                .handle(session, &frame, &context, HostTimeV1(2))
                .unwrap(),
            limits(2).wire,
        )
        .unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(
            OutcomeV2::VellumArtifactMessageAccepted { revision: 5, .. }
        ))
    ));
    let message = host
        .service()
        .engine()
        .message(&RoomId::new("room"), 0)
        .unwrap();
    assert_eq!(message.author().as_str(), "bot");
    assert_eq!(message.vellum_artifacts().len(), 1);
    assert!(calls.get() >= 2);
}

#[test]
fn v2_vellum_artifact_receipt_and_actor_binding_recover_through_host_checkpoint() {
    let mut engine = engine();
    engine
        .apply(EngineCommand::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
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
    let calls = Rc::new(Cell::new(0));
    let mut host = HostV2::new(
        ServiceV2::new(engine, ServiceLimitsV2::default()),
        Verifier {
            calls: calls.clone(),
        },
        limits(2),
    );
    let (session, client, context) = open(&mut host, "bot", 2);
    let frame = encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(1),
                command: CommandV2::SpeakWithVellumArtifacts {
                    room_id: "room".to_owned(),
                    expected_revision: 4,
                    target: VellumArtifactMessageTargetV1::Room,
                    logical_time: 5,
                    body: "body".to_owned(),
                    addressed_to: vec![],
                    references: vec![VellumArtifactReferenceV1 {
                        reference_id: VellumArtifactReferenceIdV1::new([1; 16]),
                        target: VellumDocumentTargetV1 {
                            workspace_id: 2,
                            document_id: 3,
                            source_revision: 4,
                        },
                    }],
                },
            },
        },
        limits(2).wire,
    )
    .unwrap();
    let accepted = host
        .handle(session, &frame, &context, HostTimeV1(2))
        .unwrap();
    let checkpoint = host
        .prepare_checkpoint(CHECKPOINT_SCOPE, MAX_CHECKPOINT)
        .unwrap();
    assert_eq!(checkpoint.bytes()[8], 29);

    let mut recovered = HostV2::recover_checkpoint(
        checkpoint.bytes(),
        CHECKPOINT_SCOPE,
        recovery_limits(),
        Verifier { calls },
    )
    .unwrap();
    assert_eq!(
        recovered
            .handle(session, &frame, &context, HostTimeV1(3))
            .unwrap(),
        accepted
    );
    assert_eq!(recovered.service().engine().revision(), Revision(5));
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
    let create_successor_topic = encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(13),
                command: CommandV2::CreateTopic {
                    room_id: "private".to_owned(),
                    expected_revision: 8,
                    name: "current".to_owned(),
                },
            },
        },
        limits(8).wire,
    )
    .unwrap();
    let WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::TopicCreated {
        topic_id: successor_topic_id,
        revision: 9,
    })) = decode_reply_v2(
        &host
            .handle(session, &create_successor_topic, &context, HostTimeV1(2))
            .unwrap(),
        limits(8).wire,
    )
    .unwrap()
    else {
        panic!("successor topic creation expected");
    };
    let topic_message = encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(14),
                command: CommandV2::SpeakInTopic {
                    room_id: "private".to_owned(),
                    expected_revision: 9,
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
            revision: 10,
            ..
        }))
    ));
    let rename_topic = encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(15),
                command: CommandV2::RenameTopic {
                    room_id: "private".to_owned(),
                    expected_revision: 10,
                    topic_id,
                    name: "operations".to_owned(),
                },
            },
        },
        limits(8).wire,
    )
    .unwrap();
    assert!(matches!(
        decode_reply_v2(
            &host
                .handle(session, &rename_topic, &context, HostTimeV1(2))
                .unwrap(),
            limits(8).wire,
        )
        .unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::TopicRenamed {
            revision: 11,
            ..
        }))
    ));
    let resolve_topic = encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(16),
                command: CommandV2::ResolveTopic {
                    room_id: "private".to_owned(),
                    expected_revision: 11,
                    topic_id,
                },
            },
        },
        limits(8).wire,
    )
    .unwrap();
    assert!(matches!(
        decode_reply_v2(
            &host
                .handle(session, &resolve_topic, &context, HostTimeV1(2))
                .unwrap(),
            limits(8).wire,
        )
        .unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::TopicResolved {
            revision: 12,
            ..
        }))
    ));
    let supersede_topic = encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(17),
                command: CommandV2::SupersedeTopic {
                    room_id: "private".to_owned(),
                    expected_revision: 12,
                    topic_id,
                    successor_topic_id,
                },
            },
        },
        limits(8).wire,
    )
    .unwrap();
    assert!(matches!(
        decode_reply_v2(
            &host
                .handle(session, &supersede_topic, &context, HostTimeV1(2))
                .unwrap(),
            limits(8).wire,
        )
        .unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::TopicSuperseded {
            revision: 13,
            ..
        }))
    ));
    let add_topic_alias = encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(18),
                command: CommandV2::AddTopicAlias {
                    room_id: "private".to_owned(),
                    expected_revision: 13,
                    topic_id,
                    alias: "incidents".to_owned(),
                },
            },
        },
        limits(8).wire,
    )
    .unwrap();
    assert!(matches!(
        decode_reply_v2(
            &host
                .handle(session, &add_topic_alias, &context, HostTimeV1(2))
                .unwrap(),
            limits(8).wire,
        )
        .unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::TopicAliasAdded {
            revision: 14,
            ..
        }))
    ));
    let add_topic_tag = encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: RequestV2 {
                request_id: RequestIdV2(19),
                command: CommandV2::AddTopicTag {
                    room_id: "private".to_owned(),
                    expected_revision: 14,
                    topic_id,
                    tag: "urgent".to_owned(),
                },
            },
        },
        limits(8).wire,
    )
    .unwrap();
    assert!(matches!(
        decode_reply_v2(
            &host
                .handle(session, &add_topic_tag, &context, HostTimeV1(2))
                .unwrap(),
            limits(8).wire,
        )
        .unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::TopicTagAdded {
            revision: 15,
            ..
        }))
    ));
    let checkpoint = host
        .prepare_checkpoint(CHECKPOINT_SCOPE, MAX_CHECKPOINT)
        .unwrap();
    assert_eq!(checkpoint.bytes()[8], 29);
    assert_eq!(host.service().receipt_revision(), Some(11));

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
            .service()
            .engine()
            .topic_supersession_status(
                &ParticipantId::new("alice"),
                &RoomId::new("private"),
                TopicId::from_bytes(topic_id.into_bytes()),
            )
            .unwrap()
            .successor_topic_id,
        Some(TopicId::from_bytes(successor_topic_id.into_bytes()))
    );
    assert_eq!(
        recovered
            .service()
            .engine()
            .topic_alias_status(
                &ParticipantId::new("alice"),
                &RoomId::new("private"),
                TopicId::from_bytes(topic_id.into_bytes()),
            )
            .unwrap()
            .aliases,
        vec!["incidents".to_owned()]
    );
    assert_eq!(
        recovered
            .service()
            .engine()
            .topic(
                &RoomId::new("private"),
                TopicId::from_bytes(topic_id.into_bytes()),
            )
            .unwrap()
            .name(),
        "operations"
    );
    assert!(
        recovered
            .service()
            .engine()
            .topic(
                &RoomId::new("private"),
                TopicId::from_bytes(topic_id.into_bytes()),
            )
            .unwrap()
            .is_resolved()
    );
    assert_eq!(
        recovered
            .service()
            .engine()
            .topic_tag_status(
                &ParticipantId::new("alice"),
                &RoomId::new("private"),
                TopicId::from_bytes(topic_id.into_bytes()),
            )
            .unwrap()
            .tags,
        vec!["urgent".to_owned()]
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
    assert_eq!(recovered.service().engine().revision(), Revision(15));
    assert_eq!(calls.get(), 1, "recovered session must be revalidated");
}

#[test]
fn operator_restore_invalidates_sessions_and_private_client_state() {
    let (mut host, _) = make_host(2);
    let (old_session, old_client, context) = open(&mut host, "alice", 1);
    let create = create_frame(old_client, 0, 2);
    assert!(matches!(
        decode_reply_v2(
            &host
                .handle(old_session, &create, &context, HostTimeV1(2))
                .unwrap(),
            limits(8).wire,
        )
        .unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::RoomCreated { revision: 3 }))
    ));
    let checkpoint = host
        .prepare_checkpoint(CHECKPOINT_SCOPE, MAX_CHECKPOINT)
        .unwrap();

    let calls = Rc::new(Cell::new(0));
    let mut restored = HostV2::recover_checkpoint_after_restore(
        checkpoint.bytes(),
        CHECKPOINT_SCOPE,
        recovery_limits(),
        Verifier {
            calls: calls.clone(),
        },
    )
    .unwrap();
    assert_eq!(restored.service().engine().revision(), Revision(3));
    assert_eq!(restored.service().active_client_count(), 0);
    assert_eq!(
        restored
            .handle(old_session, &create, &context, HostTimeV1(3))
            .unwrap_err(),
        HostRefusal::Unavailable
    );
    assert_eq!(calls.get(), 0, "invalidated session must not revalidate");

    let (new_session, new_client, _) = open(&mut restored, "alice", 1);
    assert_ne!(new_session, old_session);
    assert_eq!(new_client.0, 1, "client frontier must not rewind");
    assert_eq!(restored.service().engine().revision(), Revision(3));
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
    let checkpoint = decode_hex(include_str!("fixtures/host-checkpoint-v24.hex"));
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
            let migrated = recovered
                .prepare_checkpoint(CHECKPOINT_SCOPE, MAX_CHECKPOINT)
                .unwrap();
            let round_trip = HostV2::recover_checkpoint(
                migrated.bytes(),
                CHECKPOINT_SCOPE,
                recovery_limits(),
                Verifier {
                    calls: Rc::new(Cell::new(0)),
                },
            )
            .unwrap_or_else(|error| {
                panic!(
                    "accepted legacy mutation must migrate canonically at sample {sample} bit {bit}: {error:?}"
                )
            });
            assert_eq!(
                round_trip
                    .prepare_checkpoint(CHECKPOINT_SCOPE, MAX_CHECKPOINT)
                    .unwrap()
                    .bytes(),
                migrated.bytes(),
                "accepted legacy mutation must stabilize after migration at sample {sample} bit {bit}"
            );
        }
    }
}

#[test]
fn legacy_host_checkpoint_migrates_to_current_host_and_service_envelopes() {
    let legacy = decode_hex(include_str!("fixtures/host-checkpoint-v24.hex"));
    let recovered = HostV2::recover_checkpoint(
        &legacy,
        CHECKPOINT_SCOPE,
        recovery_limits(),
        Verifier {
            calls: Rc::new(Cell::new(0)),
        },
    )
    .unwrap();
    let migrated = recovered
        .prepare_checkpoint(CHECKPOINT_SCOPE, MAX_CHECKPOINT)
        .unwrap();
    assert_eq!(migrated.bytes()[8], 25);
    let service_offset = migrated
        .bytes()
        .windows(8)
        .position(|window| window == b"AGORASV\0")
        .unwrap();
    assert_eq!(migrated.bytes()[service_offset + 8], 26);
    assert!(
        HostV2::recover_checkpoint(
            migrated.bytes(),
            CHECKPOINT_SCOPE,
            recovery_limits(),
            Verifier {
                calls: Rc::new(Cell::new(0)),
            },
        )
        .is_ok()
    );
}
