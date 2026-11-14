use agora_engine::{
    AdmissionPolicy, CommandV2 as EngineCommand, Engine, Participant, ParticipantKind,
};
use agora_protocol::v2::{
    ClientIdV2, CommandV2, OutcomeV2, RefusalV2, ReplyV2, RequestIdV2, RequestV2,
};
use agora_service::v2::{ServiceLimitsV2, ServiceV2};

const INITIAL_REVISION: u64 = 4;

#[derive(Clone, Copy, Debug)]
enum Operation {
    OwnerSpeak,
    ActorSpeak,
    StaleActorSpeak,
    ActorLeave,
    ActorJoin,
    ReconnectActor,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Step {
    Accepted(u64),
    Refused(RefusalV2),
    Reconnected,
}

fn request(id: u64, command: CommandV2) -> RequestV2 {
    RequestV2 {
        request_id: RequestIdV2(id),
        command,
    }
}

fn setup(actor_kind: ParticipantKind) -> (ServiceV2, ClientIdV2, ClientIdV2) {
    let mut engine = Engine::new();
    engine
        .apply(EngineCommand::RegisterParticipant(Participant::new(
            "owner",
            "Owner",
            ParticipantKind::Human,
        )))
        .unwrap();
    engine
        .apply(EngineCommand::RegisterParticipant(Participant::new(
            "actor", "Actor", actor_kind,
        )))
        .unwrap();
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
            participant: "actor".into(),
        })
        .unwrap();
    assert_eq!(engine.revision().0, INITIAL_REVISION);
    let mut service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let owner = service.connect("owner").unwrap();
    let actor = service.connect("actor").unwrap();
    (service, owner, actor)
}

fn accepted_revision(reply: ReplyV2) -> Step {
    match reply {
        ReplyV2::Accepted(OutcomeV2::MessageAccepted { revision, .. })
        | ReplyV2::Accepted(OutcomeV2::Joined { revision })
        | ReplyV2::Accepted(OutcomeV2::Left { revision }) => Step::Accepted(revision),
        ReplyV2::Refused(refusal) => Step::Refused(refusal),
        other => panic!("unexpected modeled reply: {other:?}"),
    }
}

fn run(actor_kind: ParticipantKind, schedule: [Operation; 3]) -> Vec<Step> {
    let (mut service, owner, mut actor) = setup(actor_kind);
    let mut owner_request = 0;
    let mut actor_request = 0;
    let mut member = true;
    let mut revision = INITIAL_REVISION;
    let mut events = service.engine().pending_events().count();
    let mut transcript = Vec::new();

    for (logical_time, operation) in schedule.into_iter().enumerate() {
        let (actual, expected, accepted) = match operation {
            Operation::OwnerSpeak => {
                let reply = service.execute(
                    owner,
                    request(
                        owner_request,
                        CommandV2::Speak {
                            room_id: "room".to_owned(),
                            expected_revision: revision,
                            logical_time: logical_time as u64,
                            body: "owner model".to_owned(),
                            addressed_to: vec![],
                        },
                    ),
                );
                owner_request += 1;
                revision += 1;
                (accepted_revision(reply), Step::Accepted(revision), true)
            }
            Operation::ActorSpeak | Operation::StaleActorSpeak => {
                let expected_revision = if matches!(operation, Operation::StaleActorSpeak) {
                    INITIAL_REVISION
                } else {
                    revision
                };
                let reply = service.execute(
                    actor,
                    request(
                        actor_request,
                        CommandV2::Speak {
                            room_id: "room".to_owned(),
                            expected_revision,
                            logical_time: logical_time as u64,
                            body: "actor model".to_owned(),
                            addressed_to: vec![],
                        },
                    ),
                );
                actor_request += 1;
                if expected_revision != revision {
                    (
                        accepted_revision(reply),
                        Step::Refused(RefusalV2::Stale {
                            actual_revision: revision,
                        }),
                        false,
                    )
                } else if !member {
                    (
                        accepted_revision(reply),
                        Step::Refused(RefusalV2::NotMember),
                        false,
                    )
                } else {
                    revision += 1;
                    (accepted_revision(reply), Step::Accepted(revision), true)
                }
            }
            Operation::ActorLeave => {
                let reply = service.execute(
                    actor,
                    request(
                        actor_request,
                        CommandV2::Leave {
                            room_id: "room".to_owned(),
                            expected_revision: revision,
                        },
                    ),
                );
                actor_request += 1;
                if member {
                    member = false;
                    revision += 1;
                    (accepted_revision(reply), Step::Accepted(revision), true)
                } else {
                    (
                        accepted_revision(reply),
                        Step::Refused(RefusalV2::NotMember),
                        false,
                    )
                }
            }
            Operation::ActorJoin => {
                let reply = service.execute(
                    actor,
                    request(
                        actor_request,
                        CommandV2::Join {
                            room_id: "room".to_owned(),
                            expected_revision: revision,
                        },
                    ),
                );
                actor_request += 1;
                if member {
                    (
                        accepted_revision(reply),
                        Step::Refused(RefusalV2::InvalidInput),
                        false,
                    )
                } else {
                    member = true;
                    revision += 1;
                    (accepted_revision(reply), Step::Accepted(revision), true)
                }
            }
            Operation::ReconnectActor => {
                let retired = actor;
                assert_eq!(service.disconnect(retired), Ok(true));
                assert_eq!(
                    service.execute(
                        retired,
                        request(
                            actor_request,
                            CommandV2::Rooms {
                                cursor: None,
                                limit: 1,
                            },
                        ),
                    ),
                    ReplyV2::Refused(RefusalV2::UnknownClient)
                );
                actor = service.connect("actor").unwrap();
                actor_request = 0;
                (Step::Reconnected, Step::Reconnected, false)
            }
        };
        assert_eq!(actual, expected, "operation={operation:?}");
        if accepted {
            events += 1;
        }
        assert_eq!(service.engine().revision().0, revision);
        assert_eq!(service.engine().pending_events().count(), events);
        transcript.push(actual);
    }
    transcript
}

#[test]
fn all_three_step_multi_client_schedules_match_reference_and_kind_symmetry() {
    const OPERATIONS: [Operation; 6] = [
        Operation::OwnerSpeak,
        Operation::ActorSpeak,
        Operation::StaleActorSpeak,
        Operation::ActorLeave,
        Operation::ActorJoin,
        Operation::ReconnectActor,
    ];
    for encoded in 0_usize..216 {
        let schedule = [
            OPERATIONS[encoded % 6],
            OPERATIONS[(encoded / 6) % 6],
            OPERATIONS[(encoded / 36) % 6],
        ];
        let human = run(ParticipantKind::Human, schedule);
        let agent = run(ParticipantKind::Agent, schedule);
        assert_eq!(human, agent, "schedule={schedule:?}");
    }
}
