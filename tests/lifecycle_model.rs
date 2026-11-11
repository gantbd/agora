use agora_engine::{
    AdmissionPolicy, CommandV2, Engine, Limits, LogicalTime, ModerationCausationV1,
    ModerationReasonV1, OutcomeV2, Participant, ParticipantId, ParticipantKind, Refusal, Revision,
    RevisionedCommandV2, RoomId,
};

const INITIAL_REVISION: u64 = 4;
const MESSAGE_RETENTION: usize = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Operation {
    OwnerSpeak,
    ActorSpeak,
    StaleActorSpeak,
    ActorLeave,
    ActorJoin,
    OwnerKick,
    OwnerBan,
    OwnerUnban,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Step {
    Accepted(Operation, Option<u64>),
    Refused(Refusal),
}

struct Model {
    member: bool,
    banned: bool,
    revision: u64,
    events: usize,
    next_message_sequence: u64,
    retained_messages: Vec<u64>,
}

impl Model {
    fn new() -> Self {
        Self {
            member: true,
            banned: false,
            revision: INITIAL_REVISION,
            events: INITIAL_REVISION as usize,
            next_message_sequence: 0,
            retained_messages: Vec::new(),
        }
    }

    fn apply(&mut self, operation: Operation) -> Step {
        if operation == Operation::StaleActorSpeak && self.revision != INITIAL_REVISION {
            return Step::Refused(Refusal::RevisionConflict {
                expected: Revision(INITIAL_REVISION),
                actual: Revision(self.revision),
            });
        }

        let result = match operation {
            Operation::OwnerSpeak => self.accept_message(operation),
            Operation::ActorSpeak | Operation::StaleActorSpeak => {
                if self.member {
                    self.accept_message(operation)
                } else {
                    Step::Refused(Refusal::NotMember(ParticipantId::from("actor")))
                }
            }
            Operation::ActorLeave => {
                if self.member {
                    self.member = false;
                    Step::Accepted(operation, None)
                } else {
                    Step::Refused(Refusal::NotMember(ParticipantId::from("actor")))
                }
            }
            Operation::ActorJoin => {
                if self.member {
                    Step::Refused(Refusal::AlreadyMember(ParticipantId::from("actor")))
                } else if self.banned {
                    Step::Refused(Refusal::ParticipantBanned(ParticipantId::from("actor")))
                } else {
                    self.member = true;
                    Step::Accepted(operation, None)
                }
            }
            Operation::OwnerKick => {
                if self.member {
                    self.member = false;
                    Step::Accepted(operation, None)
                } else {
                    Step::Refused(Refusal::NotMember(ParticipantId::from("actor")))
                }
            }
            Operation::OwnerBan => {
                if self.banned {
                    Step::Refused(Refusal::ParticipantBanned(ParticipantId::from("actor")))
                } else {
                    self.member = false;
                    self.banned = true;
                    Step::Accepted(operation, None)
                }
            }
            Operation::OwnerUnban => {
                if self.banned {
                    self.banned = false;
                    Step::Accepted(operation, None)
                } else {
                    Step::Refused(Refusal::NotBanned(ParticipantId::from("actor")))
                }
            }
        };
        if matches!(result, Step::Accepted(_, _)) {
            self.revision += 1;
            self.events += 1;
        }
        result
    }

    fn accept_message(&mut self, operation: Operation) -> Step {
        let sequence = self.next_message_sequence;
        self.next_message_sequence += 1;
        self.retained_messages.push(sequence);
        if self.retained_messages.len() > MESSAGE_RETENTION {
            self.retained_messages.remove(0);
        }
        Step::Accepted(operation, Some(sequence))
    }
}

fn setup(actor_kind: ParticipantKind) -> Engine {
    let mut engine = Engine::with_limits(Limits {
        max_messages_per_room: MESSAGE_RETENTION,
        max_pending_events: 128,
        max_retained_changes: 128,
        ..Limits::default()
    });
    for (id, kind) in [("owner", ParticipantKind::Human), ("actor", actor_kind)] {
        engine
            .apply(CommandV2::RegisterParticipant(Participant::new(
                id, id, kind,
            )))
            .unwrap();
    }
    engine
        .apply(CommandV2::CreateRoom {
            room_id: RoomId::from("room"),
            name: "Room".to_owned(),
            owner: ParticipantId::from("owner"),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(CommandV2::Join {
            room_id: RoomId::from("room"),
            participant: ParticipantId::from("actor"),
        })
        .unwrap();
    engine
}

fn command(operation: Operation, step: usize) -> RevisionedCommandV2 {
    let expected_revision = if operation == Operation::StaleActorSpeak {
        Revision(INITIAL_REVISION)
    } else {
        Revision::default()
    };
    let reason = ModerationReasonV1::new("model moderation");
    let causation = ModerationCausationV1::new([step as u8; 16]);
    let command = match operation {
        Operation::OwnerSpeak => CommandV2::Speak {
            room_id: RoomId::from("room"),
            author: ParticipantId::from("owner"),
            at: LogicalTime(step as u64),
            body: "owner model".to_owned(),
        },
        Operation::ActorSpeak | Operation::StaleActorSpeak => CommandV2::Speak {
            room_id: RoomId::from("room"),
            author: ParticipantId::from("actor"),
            at: LogicalTime(step as u64),
            body: "actor model".to_owned(),
        },
        Operation::ActorLeave => CommandV2::Leave {
            room_id: RoomId::from("room"),
            participant: ParticipantId::from("actor"),
        },
        Operation::ActorJoin => CommandV2::Join {
            room_id: RoomId::from("room"),
            participant: ParticipantId::from("actor"),
        },
        Operation::OwnerKick => CommandV2::Kick {
            room_id: RoomId::from("room"),
            actor: ParticipantId::from("owner"),
            participant: ParticipantId::from("actor"),
            reason,
            causation,
        },
        Operation::OwnerBan => CommandV2::Ban {
            room_id: RoomId::from("room"),
            actor: ParticipantId::from("owner"),
            participant: ParticipantId::from("actor"),
            reason,
            causation,
        },
        Operation::OwnerUnban => CommandV2::Unban {
            room_id: RoomId::from("room"),
            actor: ParticipantId::from("owner"),
            participant: ParticipantId::from("actor"),
            reason,
            causation,
        },
    };
    RevisionedCommandV2 {
        expected_revision,
        command,
    }
}

fn normalize(operation: Operation, result: Result<OutcomeV2, Refusal>) -> Step {
    match result {
        Err(refusal) => Step::Refused(refusal),
        Ok(OutcomeV2::MessageAccepted { sequence, .. }) => {
            Step::Accepted(operation, Some(sequence))
        }
        Ok(
            OutcomeV2::Left { .. }
            | OutcomeV2::Joined { .. }
            | OutcomeV2::Kicked { .. }
            | OutcomeV2::Banned { .. }
            | OutcomeV2::Unbanned { .. },
        ) => Step::Accepted(operation, None),
        Ok(other) => panic!("unexpected modeled outcome: {other:?}"),
    }
}

fn run(actor_kind: ParticipantKind, schedule: [Operation; 4]) -> Vec<Step> {
    let mut engine = setup(actor_kind);
    let mut model = Model::new();
    let mut transcript = Vec::new();
    for (step, operation) in schedule.into_iter().enumerate() {
        let before_room = engine.room(&RoomId::from("room")).unwrap();
        let before_outbox: Vec<_> = engine.pending_events().cloned().collect();
        let before_journal: Vec<_> = engine.retained_changes().cloned().collect();
        let mut envelope = command(operation, step);
        if operation != Operation::StaleActorSpeak {
            envelope.expected_revision = engine.revision();
        }
        let actual = normalize(
            operation,
            engine
                .apply_revisioned(envelope)
                .map(|transition| transition.outcome),
        );
        let expected = model.apply(operation);
        assert_eq!(actual, expected, "operation={operation:?}");

        if matches!(actual, Step::Refused(_)) {
            assert_eq!(engine.room(&RoomId::from("room")).unwrap(), before_room);
            assert_eq!(
                engine.pending_events().cloned().collect::<Vec<_>>(),
                before_outbox
            );
            assert_eq!(
                engine.retained_changes().cloned().collect::<Vec<_>>(),
                before_journal
            );
        }
        assert_eq!(engine.revision(), Revision(model.revision));
        assert_eq!(engine.pending_events().len(), model.events);
        assert_eq!(engine.retained_changes().len(), model.events);
        for (index, event) in engine.pending_events().enumerate() {
            assert_eq!(event.sequence.0 as usize, index);
            assert_eq!(event.revision.0, event.sequence.0 + 1);
        }

        let room = engine.room(&RoomId::from("room")).unwrap();
        assert_eq!(
            room.members
                .iter()
                .any(|(participant, _)| participant.as_str() == "actor"),
            model.member
        );
        assert_eq!(
            room.banned_participants
                .iter()
                .any(|participant| participant.as_str() == "actor"),
            model.banned
        );
        assert_eq!(room.next_message_sequence, model.next_message_sequence);
        assert_eq!(
            room.messages
                .iter()
                .map(|message| message.sequence())
                .collect::<Vec<_>>(),
            model.retained_messages
        );
        transcript.push(actual);
    }
    transcript
}

#[test]
fn all_four_step_lifecycle_schedules_match_reference_and_kind_symmetry() {
    const OPERATIONS: [Operation; 8] = [
        Operation::OwnerSpeak,
        Operation::ActorSpeak,
        Operation::StaleActorSpeak,
        Operation::ActorLeave,
        Operation::ActorJoin,
        Operation::OwnerKick,
        Operation::OwnerBan,
        Operation::OwnerUnban,
    ];
    for encoded in 0_usize..4_096 {
        let schedule = [
            OPERATIONS[encoded % 8],
            OPERATIONS[(encoded / 8) % 8],
            OPERATIONS[(encoded / 64) % 8],
            OPERATIONS[(encoded / 512) % 8],
        ];
        let human = run(ParticipantKind::Human, schedule);
        let agent = run(ParticipantKind::Agent, schedule);
        assert_eq!(human, agent, "schedule={schedule:?}");
    }
}
