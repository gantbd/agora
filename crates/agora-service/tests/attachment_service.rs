use agora_engine::{
    AdmissionPolicy, CommandV2 as EngineCommand, Engine, Participant, ParticipantKind,
};
use agora_protocol::v2::{
    AttachmentDescriptorV1, AttachmentIdV1, AttachmentIntegrityV1, AttachmentMessageTargetV1,
    AttachmentObjectKeyV1, AttachmentStoreIdV1, CommandV2, OutcomeV2, ReplyV2, RequestIdV2,
    RequestV2, Sha256DigestV1,
};
use agora_service::v2::persistence::{ServiceCheckpointScopeV1, ServiceRecoveryLimitsV2};
use agora_service::v2::{ServiceLimitsV2, ServiceV2};

fn request(request_id: u64, command: CommandV2) -> RequestV2 {
    RequestV2 {
        request_id: RequestIdV2(request_id),
        command,
    }
}

fn descriptor(id: u8, object: u8) -> AttachmentDescriptorV1 {
    AttachmentDescriptorV1 {
        attachment_id: AttachmentIdV1::new([id; 16]),
        store_id: AttachmentStoreIdV1::new([7; 16]),
        object_key: AttachmentObjectKeyV1::new([object; 32]),
        declared_bytes: 42,
        media_type: "text/plain".to_owned(),
        display_label: format!("attachment-{id}"),
        integrity: AttachmentIntegrityV1::Sha256(Sha256DigestV1::new([id; 32])),
    }
}

fn setup() -> (ServiceV2, agora_protocol::v2::ClientIdV2) {
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
    let agent = service.connect("agent").unwrap();
    (service, agent)
}

#[test]
fn attachment_admission_replays_and_history_scrubs_after_withdrawal() {
    let (mut service, agent) = setup();
    let command = CommandV2::SpeakWithAttachments {
        room_id: "room".to_owned(),
        expected_revision: 4,
        target: AttachmentMessageTargetV1::Room,
        logical_time: 9,
        body: "attached".to_owned(),
        addressed_to: vec!["owner".to_owned()],
        attachments: vec![descriptor(1, 2), descriptor(2, 3)],
    };
    let accepted = service.execute(agent, request(1, command.clone()));
    let ReplyV2::Accepted(OutcomeV2::AttachmentMessageAccepted {
        message_id,
        revision: 5,
        sequence: 0,
        attachment_count: 2,
    }) = accepted.clone()
    else {
        panic!("attachment acceptance expected");
    };
    assert_eq!(service.execute(agent, request(1, command)), accepted);

    let ReplyV2::Accepted(OutcomeV2::AttachmentMessages(page)) = service.execute(
        agent,
        request(
            2,
            CommandV2::AttachmentMessages {
                room_id: "room".to_owned(),
                cursor: None,
                limit: 4,
            },
        ),
    ) else {
        panic!("attachment history expected");
    };
    assert_eq!(page.messages.len(), 1);
    assert_eq!(page.messages[0].attachments.len(), 2);
    assert!(
        page.messages[0]
            .attachments
            .iter()
            .all(|item| item.descriptor.is_some())
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
    let ReplyV2::Accepted(OutcomeV2::AttachmentMessages(page)) = service.execute(
        agent,
        request(
            4,
            CommandV2::AttachmentMessages {
                room_id: "room".to_owned(),
                cursor: None,
                limit: 4,
            },
        ),
    ) else {
        panic!("tombstoned attachment history expected");
    };
    assert!(page.messages[0].body.is_empty());
    assert!(
        page.messages[0]
            .attachments
            .iter()
            .all(|item| item.descriptor.is_none())
    );
}

#[test]
fn attachment_history_cursor_and_receipts_recover_exactly() {
    let (mut service, agent) = setup();
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
    let attachment_request = request(
        2,
        CommandV2::SpeakWithAttachments {
            room_id: "room".to_owned(),
            expected_revision: 5,
            target: AttachmentMessageTargetV1::Room,
            logical_time: 2,
            body: "attached".to_owned(),
            addressed_to: vec![],
            attachments: vec![descriptor(3, 4)],
        },
    );
    let receipt = service.execute(agent, attachment_request.clone());
    assert!(matches!(
        receipt,
        ReplyV2::Accepted(OutcomeV2::AttachmentMessageAccepted { revision: 6, .. })
    ));
    let ReplyV2::Accepted(OutcomeV2::AttachmentMessages(first)) = service.execute(
        agent,
        request(
            3,
            CommandV2::AttachmentMessages {
                room_id: "room".to_owned(),
                cursor: None,
                limit: 1,
            },
        ),
    ) else {
        panic!("first attachment page expected");
    };
    assert!(first.messages.is_empty());
    let cursor = first.next.unwrap();

    const SCOPE: ServiceCheckpointScopeV1 = ServiceCheckpointScopeV1::new(*b"attach-scope-v1!");
    const MAX: usize = 2 * 1024 * 1024;
    let checkpoint = service.prepare_checkpoint(SCOPE, MAX).unwrap();
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
    assert_eq!(recovered.execute(agent, attachment_request), receipt);
    let ReplyV2::Accepted(OutcomeV2::AttachmentMessages(second)) = recovered.execute(
        agent,
        request(
            4,
            CommandV2::AttachmentMessages {
                room_id: "room".to_owned(),
                cursor: Some(cursor),
                limit: 1,
            },
        ),
    ) else {
        panic!("second attachment page expected");
    };
    assert_eq!(second.messages.len(), 1);
    assert_eq!(second.messages[0].attachments.len(), 1);
    assert!(second.next.is_none());
}
