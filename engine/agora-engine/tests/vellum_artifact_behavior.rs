use agora_engine::persistence::RecoveryLimits;
use agora_engine::{
    AdmissionPolicy, CommandV2, DomainEventV2, Engine, Limits, LogicalTime, ModerationCausationV1,
    ModerationReasonV1, OutcomeV2, Participant, ParticipantKind, Refusal, Role,
    VellumArtifactMessageTargetV1, VellumArtifactReferenceIdV1, VellumArtifactReferenceV1,
    VellumDocumentTargetV1,
};

fn setup() -> Engine {
    let mut engine = Engine::new();
    for (id, kind) in [
        ("owner", ParticipantKind::Human),
        ("agent", ParticipantKind::Agent),
        ("moderator", ParticipantKind::Human),
    ] {
        engine
            .apply(CommandV2::RegisterParticipant(Participant::new(
                id, id, kind,
            )))
            .unwrap();
    }
    engine
        .apply(CommandV2::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
            owner: "owner".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    for participant in ["agent", "moderator"] {
        engine
            .apply(CommandV2::Join {
                room_id: "room".into(),
                participant: participant.into(),
            })
            .unwrap();
    }
    engine
        .apply(CommandV2::SetRole {
            room_id: "room".into(),
            actor: "owner".into(),
            participant: "moderator".into(),
            role: Role::Moderator,
        })
        .unwrap();
    engine
}

fn reference(id: u8, workspace: u64, document: u64, revision: u64) -> VellumArtifactReferenceV1 {
    VellumArtifactReferenceV1::new(
        VellumArtifactReferenceIdV1::new([id; 16]),
        VellumDocumentTargetV1::new(workspace, document, revision).unwrap(),
    )
}

fn speak(
    author: &str,
    target: VellumArtifactMessageTargetV1,
    references: Vec<VellumArtifactReferenceV1>,
) -> CommandV2 {
    CommandV2::SpeakWithVellumArtifacts {
        room_id: "room".into(),
        author: author.into(),
        target,
        at: LogicalTime(10),
        body: "discuss exact Vellum revision".to_owned(),
        addressed_to: vec![],
        references,
    }
}

#[test]
fn humans_and_agents_admit_exact_ordered_vellum_references_under_all_targets() {
    let mut engine = setup();
    let OutcomeV2::TopicCreated { topic_id, .. } = engine
        .apply(CommandV2::CreateTopic {
            room_id: "room".into(),
            actor: "owner".into(),
            name: "topic".to_owned(),
        })
        .unwrap()
    else {
        panic!("topic expected");
    };
    let OutcomeV2::VellumArtifactMessageAccepted {
        message_id: root,
        reference_count: 2,
        ..
    } = engine
        .apply(speak(
            "agent",
            VellumArtifactMessageTargetV1::Topic(topic_id),
            vec![reference(1, 7, 11, 13), reference(2, 7, 12, 0)],
        ))
        .unwrap()
    else {
        panic!("artifact message expected");
    };
    let message = engine.message_by_id(&"room".into(), root).unwrap();
    assert_eq!(message.author().as_str(), "agent");
    assert_eq!(message.topic_id(), Some(topic_id));
    assert_eq!(message.vellum_artifacts().len(), 2);
    assert_eq!(
        message.vellum_artifacts()[0]
            .target()
            .unwrap()
            .document_id(),
        11
    );
    assert_eq!(
        message.vellum_artifacts()[1]
            .target()
            .unwrap()
            .source_revision(),
        0
    );

    engine
        .apply(speak(
            "owner",
            VellumArtifactMessageTargetV1::Reply(root),
            vec![reference(3, 7, 13, 17)],
        ))
        .unwrap();
    let reply = engine.message(&"room".into(), 1).unwrap();
    assert_eq!(reply.reply_to_root(), Some(root));
    assert_eq!(reply.topic_id(), Some(topic_id));
}

#[test]
fn artifact_bounds_and_uniqueness_refuse_before_mutation() {
    let mut engine = setup();
    let revision = engine.revision();
    let events = engine.pending_events().len();
    let room = engine.room(&"room".into()).unwrap();
    let too_many = (1..=9)
        .map(|id| reference(id, 1, u64::from(id), 1))
        .collect();
    for (references, refusal) in [
        (vec![], Refusal::InvalidVellumArtifactReferenceCount),
        (too_many, Refusal::InvalidVellumArtifactReferenceCount),
        (
            vec![reference(1, 1, 1, 1), reference(1, 1, 2, 1)],
            Refusal::DuplicateVellumArtifactReferenceId,
        ),
        (
            vec![reference(1, 1, 1, 1), reference(2, 1, 1, 1)],
            Refusal::DuplicateVellumArtifactTarget,
        ),
    ] {
        assert_eq!(
            engine.apply(speak(
                "owner",
                VellumArtifactMessageTargetV1::Room,
                references
            )),
            Err(refusal)
        );
        assert_eq!(engine.revision(), revision);
        assert_eq!(engine.pending_events().len(), events);
        assert_eq!(engine.room(&"room".into()).unwrap(), room);
    }
}

#[test]
fn edits_preserve_and_tombstones_scrub_vellum_targets_in_every_copy() {
    let mut engine = setup();
    let OutcomeV2::VellumArtifactMessageAccepted { message_id, .. } = engine
        .apply(speak(
            "owner",
            VellumArtifactMessageTargetV1::Room,
            vec![reference(1, 77, 88, 99)],
        ))
        .unwrap()
    else {
        panic!("artifact message expected");
    };
    engine
        .apply(CommandV2::EditMessage {
            room_id: "room".into(),
            actor: "owner".into(),
            message_id,
            at: LogicalTime(11),
            body: "edited discussion".to_owned(),
        })
        .unwrap();
    assert_eq!(
        engine
            .message_by_id(&"room".into(), message_id)
            .unwrap()
            .vellum_artifacts()[0]
            .target()
            .unwrap()
            .source_revision(),
        99
    );
    engine
        .apply(CommandV2::WithdrawMessage {
            room_id: "room".into(),
            actor: "owner".into(),
            message_id,
        })
        .unwrap();
    let tombstone = engine.message_by_id(&"room".into(), message_id).unwrap();
    assert!(tombstone.vellum_artifacts()[0].is_tombstoned());
    assert_eq!(
        tombstone.vellum_artifacts()[0].reference_id(),
        VellumArtifactReferenceIdV1::new([1; 16])
    );
    for envelope in engine.pending_events() {
        if let DomainEventV2::MessageAccepted { message, .. } = &envelope.event
            && message.id() == message_id
        {
            assert!(message.vellum_artifacts()[0].is_tombstoned());
        }
    }

    let OutcomeV2::VellumArtifactMessageAccepted {
        message_id: redacted,
        ..
    } = engine
        .apply(speak(
            "agent",
            VellumArtifactMessageTargetV1::Room,
            vec![reference(2, 44, 55, 66)],
        ))
        .unwrap()
    else {
        panic!("artifact message expected");
    };
    engine
        .apply(CommandV2::RedactMessage {
            room_id: "room".into(),
            actor: "moderator".into(),
            message_id: redacted,
            reason: ModerationReasonV1::new("policy"),
            causation: ModerationCausationV1::new([9; 16]),
        })
        .unwrap();
    assert!(
        engine
            .message_by_id(&"room".into(), redacted)
            .unwrap()
            .vellum_artifacts()[0]
            .is_tombstoned()
    );
}

#[test]
fn vellum_artifact_relations_recover_canonically() {
    let mut engine = setup();
    let OutcomeV2::VellumArtifactMessageAccepted { message_id, .. } = engine
        .apply(speak(
            "agent",
            VellumArtifactMessageTargetV1::Room,
            vec![reference(1, 2, 3, 4)],
        ))
        .unwrap()
    else {
        panic!("artifact message expected");
    };
    let checkpoint = engine.prepare_checkpoint(2 * 1024 * 1024).unwrap();
    let recovered = Engine::recover_checkpoint(
        checkpoint.bytes(),
        RecoveryLimits {
            max_snapshot_bytes: 2 * 1024 * 1024,
            maximum_engine_limits: Limits::default(),
        },
    )
    .unwrap();
    let target = recovered
        .message_by_id(&"room".into(), message_id)
        .unwrap()
        .vellum_artifacts()[0]
        .target()
        .unwrap();
    assert_eq!(
        (
            target.workspace_id(),
            target.document_id(),
            target.source_revision()
        ),
        (2, 3, 4)
    );
    assert_eq!(
        recovered
            .prepare_checkpoint(2 * 1024 * 1024)
            .unwrap()
            .bytes(),
        checkpoint.bytes()
    );
}
