use agora_engine::persistence::RecoveryLimits;
use agora_engine::*;

fn setup() -> Engine {
    let mut engine = Engine::new();
    for (id, kind) in [
        ("owner", ParticipantKind::Human),
        ("member", ParticipantKind::Human),
        ("agent", ParticipantKind::Agent),
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
    for participant in ["member", "agent"] {
        engine
            .apply(CommandV2::Join {
                room_id: "room".into(),
                participant: participant.into(),
            })
            .unwrap();
    }
    engine
}

fn speak(author: &str, body: &str) -> CommandV2 {
    CommandV2::Speak {
        room_id: "room".into(),
        author: author.into(),
        at: LogicalTime(1),
        body: body.into(),
    }
}

#[test]
fn interval_uses_trusted_per_participant_time_without_role_or_kind_bypass() {
    let mut engine = setup();
    assert_eq!(
        engine.apply(CommandV2::SetMinimumMessageInterval {
            room_id: "room".into(),
            actor: "member".into(),
            minimum_interval_ms: 100,
        }),
        Err(Refusal::NotAuthorized("member".into()))
    );
    assert_eq!(
        engine.apply(CommandV2::SetMinimumMessageInterval {
            room_id: "room".into(),
            actor: "owner".into(),
            minimum_interval_ms: MAX_MESSAGE_INTERVAL_MS_V1 + 1,
        }),
        Err(Refusal::InvalidMessageInterval)
    );
    engine
        .apply(CommandV2::SetMinimumMessageInterval {
            room_id: "room".into(),
            actor: "owner".into(),
            minimum_interval_ms: 100,
        })
        .unwrap();

    assert_eq!(
        engine.apply(speak("member", "missing time")),
        Err(Refusal::AdmissionTimeRequired)
    );
    let at_1_000 = engine.advance_admission_time(1_000).unwrap();
    engine
        .apply_at(speak("member", "human first"), &at_1_000)
        .unwrap();
    engine
        .apply_at(speak("agent", "agent first"), &at_1_000)
        .unwrap();
    engine
        .apply_at(speak("owner", "owner first"), &at_1_000)
        .unwrap();

    let before = engine.revision();
    let at_1_050 = engine.advance_admission_time(1_050).unwrap();
    for participant in ["owner", "member", "agent"] {
        assert_eq!(
            engine.apply_at(speak(participant, "too soon"), &at_1_050),
            Err(Refusal::MessageThrottled { retry_after_ms: 50 })
        );
        assert_eq!(engine.revision(), before);
    }

    let at_1_100 = engine.advance_admission_time(1_100).unwrap();
    for participant in ["owner", "member", "agent"] {
        engine
            .apply_at(speak(participant, "on boundary"), &at_1_100)
            .unwrap();
    }
}

#[test]
fn posting_restriction_fences_every_message_form_and_leaks_only_self_state() {
    let mut engine = setup();
    engine
        .apply(CommandV2::SetPostingRestricted {
            room_id: "room".into(),
            actor: "owner".into(),
            participant: "agent".into(),
            restricted: true,
            reason: ModerationReasonV1::new("bounded restriction"),
            causation: ModerationCausationV1::new([7; 16]),
        })
        .unwrap();

    assert!(
        engine
            .admission_control(&"agent".into(), &"room".into())
            .unwrap()
            .observer_posting_restricted
    );
    assert!(
        !engine
            .admission_control(&"member".into(), &"room".into())
            .unwrap()
            .observer_posting_restricted
    );

    let commands = vec![
        speak("agent", "room"),
        CommandV2::SpeakAddressed {
            room_id: "room".into(),
            author: "agent".into(),
            at: LogicalTime(2),
            body: "addressed".into(),
            addressed_to: vec!["member".into()],
        },
        CommandV2::SpeakReply {
            room_id: "room".into(),
            author: "agent".into(),
            root_message_id: MessageId::from_bytes([1; 16]),
            at: LogicalTime(2),
            body: "reply".into(),
            addressed_to: Vec::new(),
        },
        CommandV2::SpeakInTopic {
            room_id: "room".into(),
            author: "agent".into(),
            topic_id: TopicId::from_bytes([2; 16]),
            at: LogicalTime(2),
            body: "topic".into(),
            addressed_to: Vec::new(),
        },
        CommandV2::SpeakWithAttachments {
            room_id: "room".into(),
            author: "agent".into(),
            target: AttachmentMessageTargetV1::Room,
            at: LogicalTime(2),
            body: "attachment".into(),
            addressed_to: Vec::new(),
            attachments: Vec::new(),
        },
        CommandV2::SpeakWithVellumArtifacts {
            room_id: "room".into(),
            author: "agent".into(),
            target: VellumArtifactMessageTargetV1::Room,
            at: LogicalTime(2),
            body: "artifact".into(),
            addressed_to: Vec::new(),
            references: Vec::new(),
        },
    ];
    let revision = engine.revision();
    for command in commands {
        assert_eq!(engine.apply(command), Err(Refusal::PostingRestricted));
        assert_eq!(engine.revision(), revision);
    }

    engine
        .apply(CommandV2::SetPostingRestricted {
            room_id: "room".into(),
            actor: "owner".into(),
            participant: "agent".into(),
            restricted: false,
            reason: ModerationReasonV1::new("restore"),
            causation: ModerationCausationV1::new([8; 16]),
        })
        .unwrap();
    engine.apply(speak("agent", "restored")).unwrap();
}

#[test]
fn admission_controls_recover_canonically_and_old_time_is_foreign() {
    let mut engine = setup();
    engine
        .apply(CommandV2::SetMinimumMessageInterval {
            room_id: "room".into(),
            actor: "owner".into(),
            minimum_interval_ms: 25,
        })
        .unwrap();
    let time = engine.advance_admission_time(500).unwrap();
    engine
        .apply_at(speak("agent", "before recovery"), &time)
        .unwrap();
    engine
        .apply(CommandV2::SetPostingRestricted {
            room_id: "room".into(),
            actor: "owner".into(),
            participant: "member".into(),
            restricted: true,
            reason: ModerationReasonV1::new("recover"),
            causation: ModerationCausationV1::new([9; 16]),
        })
        .unwrap();

    let checkpoint = engine.prepare_checkpoint(1024 * 1024).unwrap();
    let mut recovered = Engine::recover_checkpoint(
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
        recovered.apply_at(speak("agent", "foreign"), &time),
        Err(Refusal::AdmissionTimeForeign)
    );
    assert!(
        recovered
            .admission_control(&"member".into(), &"room".into())
            .unwrap()
            .observer_posting_restricted
    );
    let current = recovered.advance_admission_time(525).unwrap();
    recovered
        .apply_at(speak("agent", "boundary"), &current)
        .unwrap();
}
