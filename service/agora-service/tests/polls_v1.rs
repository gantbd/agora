use agora_engine::Limits;
use agora_engine::{
    AdmissionPolicy, CommandV2 as EngineCommand, Engine, Participant, ParticipantKind,
};
use agora_protocol::v2::{CommandV2, OutcomeV2, RefusalV2, ReplyV2, RequestIdV2, RequestV2};
use agora_service::v2::{
    ServiceLimitsV2, ServiceV2,
    persistence::{ServiceCheckpointScopeV1, ServiceRecoveryLimitsV2},
};

const SCOPE: ServiceCheckpointScopeV1 = ServiceCheckpointScopeV1::new(*b"poll-test-scope_");

fn request(id: u64, command: CommandV2) -> RequestV2 {
    RequestV2 {
        request_id: RequestIdV2(id),
        command,
    }
}

#[test]
fn polls_keep_human_and_agent_ballots_private_while_publishing_aggregates() {
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

    let mut service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    let bot = service.connect("bot").unwrap();
    let revision = service.engine().revision().0;
    let ReplyV2::Accepted(OutcomeV2::PollCreated { revision, poll_id }) = service.execute(
        alice,
        request(
            1,
            CommandV2::CreatePoll {
                room_id: "room".to_owned(),
                expected_revision: revision,
                prompt: "Ship it?".to_owned(),
                options: vec!["yes".to_owned(), "no".to_owned()],
                eligible_participant_ids: vec!["alice".to_owned(), "bot".to_owned()],
                max_choices: 1,
            },
        ),
    ) else {
        panic!("poll creation must succeed")
    };

    assert!(matches!(
        service.execute(bot, request(1, CommandV2::SetPollBallot {
            room_id: "room".to_owned(), expected_revision: revision, poll_id, selections: vec![0],
        })),
        ReplyV2::Accepted(OutcomeV2::PollBallotChanged { totals, total_ballots: 1, .. })
            if totals == vec![1, 0]
    ));
    assert!(matches!(
        service.execute(alice, request(2, CommandV2::PollBallot { room_id: "room".to_owned(), poll_id })),
        ReplyV2::Accepted(OutcomeV2::PollBallot(ballot)) if ballot.selections.is_empty()
    ));
    assert!(matches!(
        service.execute(alice, request(3, CommandV2::Poll { room_id: "room".to_owned(), poll_id })),
        ReplyV2::Accepted(OutcomeV2::Poll(poll)) if poll.totals == vec![1, 0] && poll.total_ballots == 1
    ));

    let checkpoint = service
        .prepare_checkpoint(SCOPE, 2 * 1_024 * 1_024)
        .unwrap();
    let mut service = ServiceV2::recover_checkpoint(
        checkpoint.bytes(),
        SCOPE,
        ServiceRecoveryLimitsV2 {
            max_snapshot_bytes: 2 * 1_024 * 1_024,
            max_engine_snapshot_bytes: 2 * 1_024 * 1_024,
            maximum_service_limits: ServiceLimitsV2::default(),
            maximum_engine_limits: Limits::default(),
        },
    )
    .unwrap();
    assert!(matches!(
        service.execute(bot, request(3, CommandV2::PollBallot { room_id: "room".to_owned(), poll_id })),
        ReplyV2::Accepted(OutcomeV2::PollBallot(ballot)) if ballot.selections == vec![0]
    ));

    let close_revision = service.engine().revision().0;
    assert!(matches!(
        service.execute(
            alice,
            request(
                4,
                CommandV2::SetPollOpen {
                    room_id: "room".to_owned(),
                    expected_revision: close_revision,
                    poll_id,
                    open: false,
                }
            )
        ),
        ReplyV2::Accepted(OutcomeV2::PollLifecycleChanged { open: false, .. })
    ));
    let closed_revision = service.engine().revision().0;
    assert_eq!(
        service.execute(
            bot,
            request(
                2,
                CommandV2::SetPollBallot {
                    room_id: "room".to_owned(),
                    expected_revision: closed_revision,
                    poll_id,
                    selections: vec![1],
                }
            )
        ),
        ReplyV2::Refused(RefusalV2::PollClosed)
    );
}
