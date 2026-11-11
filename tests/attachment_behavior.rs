use agora_engine::persistence::RecoveryLimits;
use agora_engine::{
    AdmissionPolicy, AttachmentDescriptorV1, AttachmentIdV1, AttachmentIntegrityV1,
    AttachmentMessageTargetV1, AttachmentObjectKeyV1, AttachmentStoreIdV1, CommandV2,
    DomainEventV2, Engine, Limits, LogicalTime, ModerationCausationV1, ModerationReasonV1,
    OutcomeV2, Participant, ParticipantKind, Refusal, Role, Sha256DigestV1,
};

fn setup(limits: Limits) -> Engine {
    let mut engine = Engine::with_limits(limits);
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

fn attachment(id: u8, store: u8, object: u8, label: &str) -> AttachmentDescriptorV1 {
    AttachmentDescriptorV1::new(
        AttachmentIdV1::new([id; 16]),
        AttachmentStoreIdV1::new([store; 16]),
        AttachmentObjectKeyV1::new([object; 32]),
        7,
        "application/octet-stream",
        label,
        AttachmentIntegrityV1::Sha256(Sha256DigestV1::new([id; 32])),
    )
}

fn speak(
    author: &str,
    target: AttachmentMessageTargetV1,
    attachments: Vec<AttachmentDescriptorV1>,
) -> CommandV2 {
    CommandV2::SpeakWithAttachments {
        room_id: "room".into(),
        author: author.into(),
        target,
        at: LogicalTime(10),
        body: "with attachments".to_owned(),
        addressed_to: vec![],
        attachments,
    }
}

#[test]
fn humans_and_agents_admit_ordered_attachments_under_all_targets() {
    let mut engine = setup(Limits::default());
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

    let first = engine
        .apply(speak(
            "agent",
            AttachmentMessageTargetV1::Topic(topic_id),
            vec![attachment(1, 1, 1, "first"), attachment(2, 1, 2, "second")],
        ))
        .unwrap();
    let OutcomeV2::AttachmentMessageAccepted {
        message_id: root,
        sequence: 0,
        attachment_count: 2,
        ..
    } = first
    else {
        panic!("attachment acceptance expected");
    };
    let root_message = engine.message(&"room".into(), 0).unwrap();
    assert_eq!(root_message.author().as_str(), "agent");
    assert_eq!(root_message.topic_id(), Some(topic_id));
    assert_eq!(root_message.attachments().len(), 2);
    assert_eq!(
        root_message.attachments()[0]
            .descriptor()
            .unwrap()
            .display_label(),
        "first"
    );
    assert_eq!(
        root_message.attachments()[1]
            .descriptor()
            .unwrap()
            .display_label(),
        "second"
    );

    engine
        .apply(speak(
            "owner",
            AttachmentMessageTargetV1::Reply(root),
            vec![attachment(3, 1, 3, "reply")],
        ))
        .unwrap();
    let reply = engine.message(&"room".into(), 1).unwrap();
    assert_eq!(reply.reply_to_root(), Some(root));
    assert_eq!(reply.topic_id(), Some(topic_id));

    engine
        .apply(speak(
            "owner",
            AttachmentMessageTargetV1::Room,
            vec![attachment(4, 1, 4, "room")],
        ))
        .unwrap();
    let room_message = engine.message(&"room".into(), 2).unwrap();
    assert_eq!(room_message.reply_to_root(), None);
    assert_eq!(room_message.topic_id(), None);
}

#[test]
fn attachment_shape_and_uniqueness_refuse_before_any_message_mutation() {
    let limits = Limits {
        max_attachment_declared_bytes: 8,
        max_attachment_media_type_bytes: 32,
        max_attachment_display_label_bytes: 8,
        ..Limits::default()
    };
    let mut engine = setup(limits);
    let before_revision = engine.revision();
    let before_events = engine.pending_events().len();
    let before_room = engine.room(&"room".into()).unwrap();

    let mut too_many = Vec::new();
    for id in 1..=9 {
        too_many.push(attachment(id, 1, id, "item"));
    }
    let invalid_length = AttachmentDescriptorV1::new(
        AttachmentIdV1::new([10; 16]),
        AttachmentStoreIdV1::new([1; 16]),
        AttachmentObjectKeyV1::new([10; 32]),
        9,
        "text/plain",
        "item",
        AttachmentIntegrityV1::Sha256(Sha256DigestV1::new([10; 32])),
    );
    let invalid_media = AttachmentDescriptorV1::new(
        AttachmentIdV1::new([11; 16]),
        AttachmentStoreIdV1::new([1; 16]),
        AttachmentObjectKeyV1::new([11; 32]),
        1,
        "Text/plain",
        "item",
        AttachmentIntegrityV1::Sha256(Sha256DigestV1::new([11; 32])),
    );
    let invalid_label = AttachmentDescriptorV1::new(
        AttachmentIdV1::new([12; 16]),
        AttachmentStoreIdV1::new([1; 16]),
        AttachmentObjectKeyV1::new([12; 32]),
        1,
        "text/plain",
        "bad\nlabel",
        AttachmentIntegrityV1::Sha256(Sha256DigestV1::new([12; 32])),
    );
    let duplicate_id = vec![attachment(13, 1, 13, "one"), attachment(13, 1, 14, "two")];
    let duplicate_object = vec![attachment(14, 1, 15, "one"), attachment(15, 1, 15, "two")];

    for (attachments, refusal) in [
        (vec![], Refusal::InvalidAttachmentCount),
        (too_many, Refusal::InvalidAttachmentCount),
        (
            vec![invalid_length],
            Refusal::InvalidAttachmentDeclaredLength,
        ),
        (vec![invalid_media], Refusal::InvalidAttachmentMediaType),
        (vec![invalid_label], Refusal::InvalidAttachmentDisplayLabel),
        (duplicate_id, Refusal::DuplicateAttachmentId),
        (duplicate_object, Refusal::DuplicateAttachmentObject),
    ] {
        assert_eq!(
            engine.apply(speak("owner", AttachmentMessageTargetV1::Room, attachments,)),
            Err(refusal)
        );
        assert_eq!(engine.revision(), before_revision);
        assert_eq!(engine.pending_events().len(), before_events);
        assert_eq!(engine.room(&"room".into()).unwrap(), before_room);
    }
}

#[test]
fn edits_preserve_and_tombstones_scrub_attachment_metadata_in_every_event_copy() {
    let mut engine = setup(Limits::default());
    let OutcomeV2::AttachmentMessageAccepted { message_id, .. } = engine
        .apply(speak(
            "owner",
            AttachmentMessageTargetV1::Room,
            vec![attachment(1, 2, 3, "secret-label")],
        ))
        .unwrap()
    else {
        panic!("attachment acceptance expected");
    };
    engine
        .apply(CommandV2::EditMessage {
            room_id: "room".into(),
            actor: "owner".into(),
            message_id,
            at: LogicalTime(11),
            body: "edited".to_owned(),
        })
        .unwrap();
    assert!(
        engine
            .message_by_id(&"room".into(), message_id)
            .unwrap()
            .attachments()[0]
            .descriptor()
            .is_some()
    );

    engine
        .apply(CommandV2::WithdrawMessage {
            room_id: "room".into(),
            actor: "owner".into(),
            message_id,
        })
        .unwrap();
    let tombstone = engine.message_by_id(&"room".into(), message_id).unwrap();
    assert!(tombstone.attachments()[0].is_tombstoned());
    assert_eq!(
        tombstone.attachments()[0].attachment_id(),
        AttachmentIdV1::new([1; 16])
    );
    for envelope in engine.pending_events() {
        if let DomainEventV2::MessageAccepted { message, .. } = &envelope.event
            && message.id() == message_id
        {
            assert!(message.attachments()[0].is_tombstoned());
        }
    }

    let OutcomeV2::AttachmentMessageAccepted {
        message_id: redacted,
        ..
    } = engine
        .apply(speak(
            "agent",
            AttachmentMessageTargetV1::Room,
            vec![attachment(4, 5, 6, "redact-me")],
        ))
        .unwrap()
    else {
        panic!("attachment acceptance expected");
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
            .attachments()[0]
            .is_tombstoned()
    );
}

#[test]
fn attachment_relations_recover_canonically_and_continue() {
    let mut engine = setup(Limits::default());
    let OutcomeV2::AttachmentMessageAccepted { message_id, .. } = engine
        .apply(speak(
            "agent",
            AttachmentMessageTargetV1::Room,
            vec![attachment(1, 2, 3, "retained")],
        ))
        .unwrap()
    else {
        panic!("attachment acceptance expected");
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
    let restored = recovered.message_by_id(&"room".into(), message_id).unwrap();
    let descriptor = restored.attachments()[0].descriptor().unwrap();
    assert_eq!(descriptor.store_id(), AttachmentStoreIdV1::new([2; 16]));
    assert_eq!(descriptor.object_key(), AttachmentObjectKeyV1::new([3; 32]));
    assert_eq!(descriptor.display_label(), "retained");
    assert_eq!(
        recovered
            .prepare_checkpoint(2 * 1024 * 1024)
            .unwrap()
            .bytes(),
        checkpoint.bytes()
    );
}
