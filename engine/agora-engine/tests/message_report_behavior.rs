use agora_engine::persistence::RecoveryLimits;
use agora_engine::*;

fn setup() -> (Engine, MessageId) {
    let mut engine = Engine::new();
    for (id, kind) in [
        ("owner", ParticipantKind::Human),
        ("moderator", ParticipantKind::Agent),
        ("reporter", ParticipantKind::Human),
        ("other", ParticipantKind::Agent),
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
            name: "Room".into(),
            owner: "owner".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    for participant in ["moderator", "reporter", "other"] {
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
        .apply(CommandV2::Speak {
            room_id: "room".into(),
            author: "other".into(),
            at: LogicalTime(1),
            body: "reported message".into(),
        })
        .unwrap();
    let message_id = engine.message(&"room".into(), 0).unwrap().id();
    (engine, message_id)
}

#[test]
fn reports_are_private_role_gated_and_review_has_no_action_side_effect() {
    let (mut engine, message_id) = setup();
    let revision = engine.revision();
    let report = engine
        .submit_message_report(
            &"reporter".into(),
            &"room".into(),
            message_id,
            revision,
            ModerationReasonV1::new("unsafe content"),
            ModerationCausationV1::new([1; 16]),
        )
        .unwrap();
    assert_eq!(engine.revision(), revision);
    assert_eq!(
        engine.message_reports(&"reporter".into(), &"room".into()),
        Ok(vec![report.clone()])
    );
    assert_eq!(
        engine.message_reports(&"other".into(), &"room".into()),
        Ok(Vec::new())
    );
    assert_eq!(
        engine.moderation_reports(&"other".into(), &"room".into()),
        Err(MessageReportRefusalV1::NotAuthorized)
    );
    assert_eq!(
        engine
            .moderation_reports(&"moderator".into(), &"room".into())
            .unwrap(),
        vec![report.clone()]
    );

    let reviewed = engine
        .review_message_report(
            &"moderator".into(),
            &"room".into(),
            report.report_id,
            MessageReportDispositionV1::Escalated,
            ModerationReasonV1::new("manual review"),
            ModerationCausationV1::new([2; 16]),
        )
        .unwrap();
    assert_eq!(reviewed.disposition, MessageReportDispositionV1::Escalated);
    assert_eq!(engine.revision(), revision);
    engine
        .apply(CommandV2::WithdrawMessage {
            room_id: "room".into(),
            actor: "other".into(),
            message_id,
        })
        .unwrap();
    assert_eq!(
        engine
            .message_reports(&"reporter".into(), &"room".into())
            .unwrap()[0]
            .target_availability,
        StableReferenceAvailabilityV1::Tombstoned
    );
    engine
        .apply(CommandV2::Speak {
            room_id: "room".into(),
            author: "other".into(),
            at: LogicalTime(2),
            body: "still permitted".into(),
        })
        .unwrap();
    assert_eq!(
        engine.review_message_report(
            &"owner".into(),
            &"room".into(),
            report.report_id,
            MessageReportDispositionV1::Dismissed,
            ModerationReasonV1::new("second review"),
            ModerationCausationV1::new([3; 16]),
        ),
        Err(MessageReportRefusalV1::InvalidChange)
    );
}

#[test]
fn report_capacity_refuses_when_open_and_evicts_only_the_oldest_closed_report() {
    let (mut engine, first_message_id) = setup();
    let mut message_ids = vec![first_message_id];
    for sequence in 1..=MAX_MESSAGE_REPORTS_PER_ROOM_V1 {
        engine
            .apply(CommandV2::Speak {
                room_id: "room".into(),
                author: "other".into(),
                at: LogicalTime(sequence as u64 + 1),
                body: format!("message {sequence}"),
            })
            .unwrap();
        message_ids.push(
            engine
                .message(&"room".into(), sequence as u64)
                .unwrap()
                .id(),
        );
    }
    let mut reports = Vec::new();
    for (index, message_id) in message_ids
        .iter()
        .take(MAX_MESSAGE_REPORTS_PER_ROOM_V1)
        .enumerate()
    {
        reports.push(
            engine
                .submit_message_report(
                    &"reporter".into(),
                    &"room".into(),
                    *message_id,
                    engine.revision(),
                    ModerationReasonV1::new(format!("reason {index}")),
                    ModerationCausationV1::new([index as u8; 16]),
                )
                .unwrap(),
        );
    }
    assert_eq!(
        engine.submit_message_report(
            &"reporter".into(),
            &"room".into(),
            *message_ids.last().unwrap(),
            engine.revision(),
            ModerationReasonV1::new("at capacity"),
            ModerationCausationV1::new([8; 16]),
        ),
        Err(MessageReportRefusalV1::Capacity)
    );
    engine
        .review_message_report(
            &"owner".into(),
            &"room".into(),
            reports[0].report_id,
            MessageReportDispositionV1::Dismissed,
            ModerationReasonV1::new("closed"),
            ModerationCausationV1::new([9; 16]),
        )
        .unwrap();
    let replacement = engine
        .submit_message_report(
            &"reporter".into(),
            &"room".into(),
            *message_ids.last().unwrap(),
            engine.revision(),
            ModerationReasonV1::new("replacement"),
            ModerationCausationV1::new([10; 16]),
        )
        .unwrap();
    let visible = engine
        .message_reports(&"reporter".into(), &"room".into())
        .unwrap();
    assert_eq!(visible.len(), MAX_MESSAGE_REPORTS_PER_ROOM_V1);
    assert_eq!(visible.last(), Some(&replacement));
    assert_ne!(visible[0].report_id, reports[0].report_id);
}

#[test]
fn duplicate_and_invalid_reports_refuse_atomically_and_recover_canonically() {
    let (mut engine, message_id) = setup();
    let revision = engine.revision();
    let report = engine
        .submit_message_report(
            &"reporter".into(),
            &"room".into(),
            message_id,
            revision,
            ModerationReasonV1::new("reason"),
            ModerationCausationV1::new([1; 16]),
        )
        .unwrap();
    assert_eq!(
        engine.submit_message_report(
            &"reporter".into(),
            &"room".into(),
            message_id,
            revision,
            ModerationReasonV1::new("again"),
            ModerationCausationV1::new([2; 16]),
        ),
        Err(MessageReportRefusalV1::DuplicateReport)
    );
    assert_eq!(
        engine.submit_message_report(
            &"other".into(),
            &"room".into(),
            MessageId::from_bytes([9; 16]),
            revision,
            ModerationReasonV1::new("reason"),
            ModerationCausationV1::new([3; 16]),
        ),
        Err(MessageReportRefusalV1::UnknownMessage)
    );

    let checkpoint = engine.prepare_checkpoint(1024 * 1024).unwrap();
    let recovered = Engine::recover_checkpoint(
        checkpoint.bytes(),
        RecoveryLimits {
            max_snapshot_bytes: 1024 * 1024,
            maximum_engine_limits: Limits::default(),
        },
    )
    .unwrap();
    assert_eq!(
        recovered.prepare_checkpoint(1024 * 1024).unwrap().bytes(),
        checkpoint.bytes()
    );
    assert_eq!(
        recovered.message_reports(&"reporter".into(), &"room".into()),
        Ok(vec![report])
    );
}
