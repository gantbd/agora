use agora_engine::{
    AdmissionPolicy, CommandV2 as EngineCommand, Engine, Limits, Participant, ParticipantKind,
};
use agora_protocol::v2::{
    AdmissionControlStatusV1, CapabilityV2, CommandV2, MessageReportDispositionV1, OutcomeV2,
    RefusalV2, ReplyV2, RequestIdV2, RequestV2,
};
use agora_service::v2::{
    ServiceLimitsV2, ServiceV2,
    persistence::{ServiceCheckpointScopeV1, ServiceRecoveryLimitsV2},
};

const SCOPE: ServiceCheckpointScopeV1 = ServiceCheckpointScopeV1::new(*b"admission-scope!");

fn recovery_limits() -> ServiceRecoveryLimitsV2 {
    ServiceRecoveryLimitsV2 {
        max_snapshot_bytes: 2 * 1_024 * 1_024,
        max_engine_snapshot_bytes: 2 * 1_024 * 1_024,
        maximum_service_limits: ServiceLimitsV2::default(),
        maximum_engine_limits: Limits::default(),
    }
}

fn request(id: u64, command: CommandV2) -> RequestV2 {
    RequestV2 {
        request_id: RequestIdV2(id),
        command,
    }
}

#[test]
fn reports_are_private_reviewable_and_do_not_imply_moderation_action() {
    let mut service = setup();
    let owner = service.connect("human").unwrap();
    let reporter = service.connect("agent").unwrap();
    let ReplyV2::Accepted(OutcomeV2::IdentifiedMessageAccepted {
        message_id,
        revision: 5,
        ..
    }) = service.execute(
        owner,
        request(
            1,
            CommandV2::SpeakIdentified {
                room_id: "room".to_owned(),
                expected_revision: 4,
                logical_time: 1,
                body: "report target".to_owned(),
                addressed_to: Vec::new(),
            },
        ),
    )
    else {
        panic!("identified message must succeed")
    };
    let ReplyV2::Accepted(OutcomeV2::MessageReportSubmitted(report)) = service.execute(
        reporter,
        request(
            1,
            CommandV2::SubmitMessageReport {
                room_id: "room".to_owned(),
                observed_revision: 5,
                message_id,
                reason: "unsafe".to_owned(),
                causation: [4; 16],
            },
        ),
    ) else {
        panic!("report must succeed")
    };
    assert_eq!(report.reporter_id, "agent");
    assert_eq!(report.disposition, MessageReportDispositionV1::Pending);
    assert_eq!(
        service.execute(
            owner,
            request(
                2,
                CommandV2::MessageReports {
                    room_id: "room".to_owned(),
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::MessageReports(Vec::new()))
    );
    assert_eq!(
        service.execute(
            owner,
            request(
                3,
                CommandV2::ModerationReports {
                    room_id: "room".to_owned(),
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::MessageReports(vec![report.clone()]))
    );
    let ReplyV2::Accepted(OutcomeV2::MessageReportReviewed(reviewed)) = service.execute(
        owner,
        request(
            4,
            CommandV2::ReviewMessageReport {
                room_id: "room".to_owned(),
                report_id: report.report_id,
                disposition: MessageReportDispositionV1::Escalated,
                reason: "manual review".to_owned(),
                causation: [5; 16],
            },
        ),
    ) else {
        panic!("review must succeed")
    };
    assert_eq!(reviewed.disposition, MessageReportDispositionV1::Escalated);
    let checkpoint = service
        .prepare_checkpoint(SCOPE, 2 * 1_024 * 1_024)
        .unwrap();
    let mut service =
        ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, recovery_limits()).unwrap();
    assert_eq!(
        service
            .prepare_checkpoint(SCOPE, 2 * 1_024 * 1_024)
            .unwrap()
            .bytes(),
        checkpoint.bytes()
    );
    assert_eq!(
        service.execute(
            reporter,
            request(
                2,
                CommandV2::MessageReports {
                    room_id: "room".to_owned(),
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::MessageReports(vec![reviewed]))
    );
    assert_eq!(
        service.execute_at(reporter, request(3, speak(5, "still allowed")), 0),
        ReplyV2::Accepted(OutcomeV2::MessageAccepted {
            revision: 6,
            sequence: 1,
        })
    );
}

fn speak(expected_revision: u64, body: &str) -> CommandV2 {
    CommandV2::Speak {
        room_id: "room".to_owned(),
        expected_revision,
        logical_time: expected_revision,
        body: body.to_owned(),
        addressed_to: Vec::new(),
    }
}

fn setup() -> ServiceV2 {
    let mut engine = Engine::new();
    for (id, kind) in [
        ("human", ParticipantKind::Human),
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
            owner: "human".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(EngineCommand::Join {
            room_id: "room".into(),
            participant: "agent".into(),
        })
        .unwrap();
    ServiceV2::new(engine, ServiceLimitsV2::default())
}

#[test]
fn trusted_time_throttles_humans_and_agents_per_participant_with_exact_replay() {
    let mut service = setup();
    let human = service.connect("human").unwrap();
    let agent = service.connect("agent").unwrap();

    assert!(matches!(
        service.execute(human, request(1, CommandV2::Capabilities)),
        ReplyV2::Accepted(OutcomeV2::Capabilities(capabilities))
            if capabilities.last() == Some(&CapabilityV2::AdmissionModerationControlsV1)
    ));
    assert_eq!(
        service.execute(
            human,
            request(
                2,
                CommandV2::SetMinimumMessageInterval {
                    room_id: "room".to_owned(),
                    minimum_interval_ms: 100,
                    expected_revision: 4,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::MessageIntervalChanged { revision: 5 })
    );

    let first = request(3, speak(5, "first"));
    let first_reply = service.execute_at(human, first.clone(), 1_000);
    assert_eq!(
        first_reply,
        ReplyV2::Accepted(OutcomeV2::MessageAccepted {
            revision: 6,
            sequence: 0,
        })
    );
    assert_eq!(service.execute_at(human, first, 1), first_reply);

    assert_eq!(
        service.execute_at(agent, request(1, speak(6, "agent")), 1_000),
        ReplyV2::Accepted(OutcomeV2::MessageAccepted {
            revision: 7,
            sequence: 1,
        })
    );
    assert_eq!(
        service.execute_at(human, request(4, speak(7, "too soon")), 1_049),
        ReplyV2::Refused(RefusalV2::MessageThrottled { retry_after_ms: 51 })
    );
    assert_eq!(
        service.execute_at(human, request(5, speak(7, "on boundary")), 1_100),
        ReplyV2::Accepted(OutcomeV2::MessageAccepted {
            revision: 8,
            sequence: 2,
        })
    );
}

#[test]
fn posting_restriction_is_private_to_observer_and_clock_independent() {
    let mut service = setup();
    let human = service.connect("human").unwrap();
    let agent = service.connect("agent").unwrap();

    assert_eq!(
        service.execute(
            human,
            request(
                1,
                CommandV2::SetPostingRestricted {
                    room_id: "room".to_owned(),
                    participant_id: "agent".to_owned(),
                    restricted: true,
                    reason: "cooldown".to_owned(),
                    causation: [7; 16],
                    expected_revision: 4,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::PostingRestrictionChanged { revision: 5 })
    );
    assert_eq!(
        service.execute(
            agent,
            request(
                1,
                CommandV2::AdmissionControl {
                    room_id: "room".to_owned(),
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::AdmissionControl(AdmissionControlStatusV1 {
            revision: 5,
            room_id: "room".to_owned(),
            minimum_message_interval_ms: 0,
            observer_posting_restricted: true,
        }))
    );
    assert_eq!(
        service.execute(
            human,
            request(
                2,
                CommandV2::AdmissionControl {
                    room_id: "room".to_owned(),
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::AdmissionControl(AdmissionControlStatusV1 {
            revision: 5,
            room_id: "room".to_owned(),
            minimum_message_interval_ms: 0,
            observer_posting_restricted: false,
        }))
    );
    assert_eq!(
        service.execute_at(agent, request(2, speak(5, "blocked")), 0),
        ReplyV2::Refused(RefusalV2::PostingRestricted)
    );
}
