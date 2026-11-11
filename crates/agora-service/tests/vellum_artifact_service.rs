use agora_engine::{
    AdmissionPolicy, CommandV2 as EngineCommand, Engine, Participant, ParticipantKind,
};
use agora_protocol::v2::{
    CommandV2, MessageVellumArtifactReferenceV1, OutcomeV2, ReplyV2, RequestIdV2, RequestV2,
    VellumArtifactMessageTargetV1, VellumArtifactReferenceIdV1, VellumArtifactReferenceV1,
    VellumDocumentTargetV1,
};
use agora_service::v2::persistence::{ServiceCheckpointScopeV1, ServiceRecoveryLimitsV2};
use agora_service::v2::{ServiceLimitsV2, ServiceV2};

fn request(request_id: u64, command: CommandV2) -> RequestV2 {
    RequestV2 {
        request_id: RequestIdV2(request_id),
        command,
    }
}

fn reference(id: u8, document: u64) -> VellumArtifactReferenceV1 {
    VellumArtifactReferenceV1 {
        reference_id: VellumArtifactReferenceIdV1::new([id; 16]),
        target: VellumDocumentTargetV1 {
            workspace_id: 7,
            document_id: document,
            source_revision: 11,
        },
    }
}

fn setup() -> (
    ServiceV2,
    agora_protocol::v2::ClientIdV2,
    agora_protocol::v2::ClientIdV2,
) {
    let mut engine = Engine::new();
    for (id, kind) in [
        ("owner", ParticipantKind::Human),
        ("agent", ParticipantKind::Agent),
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
            name: "Room".to_owned(),
            owner: "owner".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(EngineCommand::Join {
            room_id: "room".into(),
            participant: "agent".into(),
        })
        .unwrap();
    let mut service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let owner = service.connect("owner").unwrap();
    let agent = service.connect("agent").unwrap();
    (service, owner, agent)
}

fn speak(expected_revision: u64) -> CommandV2 {
    CommandV2::SpeakWithVellumArtifacts {
        room_id: "room".to_owned(),
        expected_revision,
        target: VellumArtifactMessageTargetV1::Room,
        logical_time: 9,
        body: "discuss exact Vellum revision".to_owned(),
        addressed_to: vec!["owner".to_owned()],
        references: vec![reference(1, 9), reference(2, 10)],
    }
}

#[test]
fn agent_admission_replays_and_history_scrubs_after_withdrawal() {
    let (mut service, _owner, agent) = setup();
    let command = speak(4);
    let accepted = service.execute(agent, request(1, command.clone()));
    let ReplyV2::Accepted(OutcomeV2::VellumArtifactMessageAccepted {
        message_id,
        revision: 5,
        sequence: 0,
        reference_count: 2,
    }) = accepted.clone()
    else {
        panic!("Vellum artifact acceptance expected");
    };
    assert_eq!(service.execute(agent, request(1, command)), accepted);

    let ReplyV2::Accepted(OutcomeV2::VellumArtifactMessages(page)) = service.execute(
        agent,
        request(
            2,
            CommandV2::VellumArtifactMessages {
                room_id: "room".to_owned(),
                cursor: None,
                limit: 4,
            },
        ),
    ) else {
        panic!("Vellum artifact history expected");
    };
    assert_eq!(page.messages.len(), 1);
    assert_eq!(page.messages[0].references.len(), 2);
    assert!(
        page.messages[0]
            .references
            .iter()
            .all(|item| item.target.is_some())
    );

    assert!(matches!(
        service.execute(
            agent,
            request(
                3,
                CommandV2::WithdrawMessage {
                    room_id: "room".to_owned(),
                    expected_revision: 5,
                    message_id,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::MessageWithdrawn { revision: 6, .. })
    ));
    let ReplyV2::Accepted(OutcomeV2::VellumArtifactMessages(page)) = service.execute(
        agent,
        request(
            4,
            CommandV2::VellumArtifactMessages {
                room_id: "room".to_owned(),
                cursor: None,
                limit: 4,
            },
        ),
    ) else {
        panic!("tombstoned Vellum artifact history expected");
    };
    assert!(page.messages[0].body.is_empty());
    assert!(
        page.messages[0]
            .references
            .iter()
            .all(|item| item.target.is_none())
    );
}

#[test]
fn actor_binding_is_kind_neutral_and_request_conflicts_do_not_mutate() {
    let (mut service, owner, agent) = setup();
    assert!(matches!(
        service.execute(owner, request(1, speak(4))),
        ReplyV2::Accepted(OutcomeV2::VellumArtifactMessageAccepted { revision: 5, .. })
    ));
    assert!(matches!(
        service.execute(agent, request(1, speak(5))),
        ReplyV2::Accepted(OutcomeV2::VellumArtifactMessageAccepted { revision: 6, .. })
    ));
    let before = service.engine().revision();
    assert!(matches!(
        service.execute(agent, request(1, speak(6))),
        ReplyV2::Refused(agora_protocol::v2::RefusalV2::RequestConflict)
    ));
    assert_eq!(service.engine().revision(), before);
}

#[test]
fn history_cursor_and_artifact_receipt_recover_exactly() {
    let (mut service, _owner, agent) = setup();
    assert!(matches!(
        service.execute(
            agent,
            request(
                1,
                CommandV2::SpeakIdentified {
                    room_id: "room".to_owned(),
                    expected_revision: 4,
                    logical_time: 1,
                    body: "ordinary".to_owned(),
                    addressed_to: vec![],
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::IdentifiedMessageAccepted { revision: 5, .. })
    ));
    let artifact_request = request(2, speak(5));
    let receipt = service.execute(agent, artifact_request.clone());
    assert!(matches!(
        receipt,
        ReplyV2::Accepted(OutcomeV2::VellumArtifactMessageAccepted { revision: 6, .. })
    ));
    let ReplyV2::Accepted(OutcomeV2::VellumArtifactMessages(first)) = service.execute(
        agent,
        request(
            3,
            CommandV2::VellumArtifactMessages {
                room_id: "room".to_owned(),
                cursor: None,
                limit: 1,
            },
        ),
    ) else {
        panic!("first Vellum artifact page expected");
    };
    assert!(first.messages.is_empty());
    let cursor = first.next.unwrap();
    assert!(matches!(
        service.execute(
            agent,
            request(
                4,
                CommandV2::IdentifiedMessages {
                    room_id: "room".to_owned(),
                    cursor: Some(cursor),
                    limit: 1,
                },
            ),
        ),
        ReplyV2::Refused(agora_protocol::v2::RefusalV2::InvalidCursor)
    ));

    const SCOPE: ServiceCheckpointScopeV1 = ServiceCheckpointScopeV1::new(*b"vellum-scope-v1!");
    const MAX: usize = 2 * 1024 * 1024;
    let checkpoint = service.prepare_checkpoint(SCOPE, MAX).unwrap();
    assert_eq!(checkpoint.bytes()[8], 25);
    let mut recovered = ServiceV2::recover_checkpoint(
        checkpoint.bytes(),
        SCOPE,
        ServiceRecoveryLimitsV2 {
            max_snapshot_bytes: MAX,
            max_engine_snapshot_bytes: MAX,
            maximum_service_limits: ServiceLimitsV2::default(),
            maximum_engine_limits: agora_engine::Limits::default(),
        },
    )
    .unwrap();
    assert_eq!(recovered.execute(agent, artifact_request), receipt);
    let ReplyV2::Accepted(OutcomeV2::VellumArtifactMessages(second)) = recovered.execute(
        agent,
        request(
            5,
            CommandV2::VellumArtifactMessages {
                room_id: "room".to_owned(),
                cursor: Some(cursor),
                limit: 1,
            },
        ),
    ) else {
        panic!("second Vellum artifact page expected");
    };
    assert_eq!(second.messages.len(), 1);
    assert_eq!(second.messages[0].references.len(), 2);
    assert!(second.next.is_none());
}

#[test]
fn protocol_reference_tombstones_retain_only_relation_identity() {
    let tombstone = MessageVellumArtifactReferenceV1 {
        reference_id: VellumArtifactReferenceIdV1::new([3; 16]),
        target: None,
    };
    assert!(tombstone.target.is_none());
}
