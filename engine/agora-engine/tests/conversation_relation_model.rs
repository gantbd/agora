use agora_engine::{
    AdmissionPolicy, CommandV2, Engine, Limits, LogicalTime, MessageId,
    MessageRevisionAvailabilityV1, ModerationCausationV1, ModerationReasonV1, ObservationRefusal,
    OutcomeV2, Participant, ParticipantId, ParticipantKind, Refusal, Revision, RevisionedCommandV2,
    RoomId, ThreadLifecycleStateV1,
};

const INITIAL_REVISION: u64 = 5;
const MESSAGE_RETENTION: usize = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Operation {
    Reply,
    Archive,
    Reopen,
    React,
    Unreact,
    Edit,
    Withdraw,
    UnrelatedSpeak,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Step {
    Accepted {
        operation: Operation,
        message_sequence: Option<u64>,
        message_revision: Option<u32>,
    },
    Refused(Refusal),
}

struct Model {
    root_retained: bool,
    archived: bool,
    reacted: bool,
    tombstoned: bool,
    root_body: String,
    root_revisions: usize,
    revision: u64,
    events: usize,
    next_message_sequence: u64,
    retained_messages: Vec<u64>,
}

impl Model {
    fn new() -> Self {
        Self {
            root_retained: true,
            archived: false,
            reacted: false,
            tombstoned: false,
            root_body: "root model".to_owned(),
            root_revisions: 1,
            revision: INITIAL_REVISION,
            events: INITIAL_REVISION as usize,
            next_message_sequence: 1,
            retained_messages: vec![0],
        }
    }

    fn accepted(
        &mut self,
        operation: Operation,
        message_sequence: Option<u64>,
        message_revision: Option<u32>,
    ) -> Step {
        self.revision += 1;
        self.events += 1;
        Step::Accepted {
            operation,
            message_sequence,
            message_revision,
        }
    }

    fn append_message(&mut self, operation: Operation) -> Step {
        let sequence = self.next_message_sequence;
        self.next_message_sequence += 1;
        self.retained_messages.push(sequence);
        if self.retained_messages.len() > MESSAGE_RETENTION {
            let evicted = self.retained_messages.remove(0);
            if evicted == 0 {
                self.root_retained = false;
                self.archived = false;
                self.reacted = false;
            }
        }
        self.accepted(operation, Some(sequence), None)
    }

    fn apply(&mut self, operation: Operation, step: usize) -> Step {
        match operation {
            Operation::Reply => {
                if !self.root_retained {
                    Step::Refused(Refusal::UnknownMessage)
                } else if self.archived {
                    Step::Refused(Refusal::ThreadArchived)
                } else {
                    self.append_message(operation)
                }
            }
            Operation::Archive => {
                if !self.root_retained {
                    Step::Refused(Refusal::UnknownMessage)
                } else if self.archived {
                    Step::Refused(Refusal::InvalidThreadLifecycleChange)
                } else {
                    self.archived = true;
                    self.accepted(operation, None, None)
                }
            }
            Operation::Reopen => {
                if !self.root_retained {
                    Step::Refused(Refusal::UnknownMessage)
                } else if !self.archived {
                    Step::Refused(Refusal::InvalidThreadLifecycleChange)
                } else {
                    self.archived = false;
                    self.accepted(operation, None, None)
                }
            }
            Operation::React => {
                if !self.root_retained {
                    Step::Refused(Refusal::UnknownMessage)
                } else if self.reacted {
                    Step::Refused(Refusal::InvalidReactionChange)
                } else {
                    self.reacted = true;
                    self.accepted(operation, None, None)
                }
            }
            Operation::Unreact => {
                if !self.root_retained {
                    Step::Refused(Refusal::UnknownMessage)
                } else if !self.reacted {
                    Step::Refused(Refusal::InvalidReactionChange)
                } else {
                    self.reacted = false;
                    self.accepted(operation, None, None)
                }
            }
            Operation::Edit => {
                if !self.root_retained {
                    Step::Refused(Refusal::UnknownMessage)
                } else if self.tombstoned {
                    Step::Refused(Refusal::InvalidMessageEdit)
                } else {
                    self.root_body = format!("edit-{step}");
                    let message_revision = self.root_revisions as u32;
                    self.root_revisions += 1;
                    self.accepted(operation, None, Some(message_revision))
                }
            }
            Operation::Withdraw => {
                if !self.root_retained {
                    Step::Refused(Refusal::UnknownMessage)
                } else if self.tombstoned {
                    Step::Refused(Refusal::MessageAlreadyTombstoned)
                } else {
                    self.tombstoned = true;
                    self.root_body.clear();
                    self.accepted(operation, None, None)
                }
            }
            Operation::UnrelatedSpeak => self.append_message(operation),
        }
    }
}

fn setup(actor_kind: ParticipantKind) -> (Engine, MessageId) {
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
    let OutcomeV2::MessageAccepted { sequence, .. } = engine
        .apply(CommandV2::Speak {
            room_id: RoomId::from("room"),
            author: ParticipantId::from("actor"),
            at: LogicalTime(0),
            body: "root model".to_owned(),
        })
        .unwrap()
    else {
        panic!("root acceptance expected");
    };
    let root = engine
        .message(&RoomId::from("room"), sequence)
        .unwrap()
        .id();
    (engine, root)
}

fn command(operation: Operation, root: MessageId, step: usize) -> CommandV2 {
    let reason = ModerationReasonV1::new("model lifecycle");
    let causation = ModerationCausationV1::new([step as u8; 16]);
    match operation {
        Operation::Reply => CommandV2::SpeakReply {
            room_id: RoomId::from("room"),
            author: ParticipantId::from("actor"),
            root_message_id: root,
            at: LogicalTime(step as u64 + 1),
            body: format!("reply-{step}"),
            addressed_to: Vec::new(),
        },
        Operation::Archive | Operation::Reopen => CommandV2::SetThreadArchived {
            room_id: RoomId::from("room"),
            actor: ParticipantId::from("owner"),
            root_message_id: root,
            archived: operation == Operation::Archive,
            reason,
            causation,
        },
        Operation::React | Operation::Unreact => CommandV2::SetReaction {
            room_id: RoomId::from("room"),
            actor: ParticipantId::from("actor"),
            message_id: root,
            key: "spark".to_owned(),
            reacted: operation == Operation::React,
        },
        Operation::Edit => CommandV2::EditMessage {
            room_id: RoomId::from("room"),
            actor: ParticipantId::from("actor"),
            message_id: root,
            at: LogicalTime(step as u64 + 1),
            body: format!("edit-{step}"),
        },
        Operation::Withdraw => CommandV2::WithdrawMessage {
            room_id: RoomId::from("room"),
            actor: ParticipantId::from("actor"),
            message_id: root,
        },
        Operation::UnrelatedSpeak => CommandV2::Speak {
            room_id: RoomId::from("room"),
            author: ParticipantId::from("owner"),
            at: LogicalTime(step as u64 + 1),
            body: format!("unrelated-{step}"),
        },
    }
}

fn normalize(operation: Operation, result: Result<OutcomeV2, Refusal>) -> Step {
    match result {
        Err(refusal) => Step::Refused(refusal),
        Ok(OutcomeV2::ReplyAccepted { sequence, .. })
        | Ok(OutcomeV2::MessageAccepted { sequence, .. }) => Step::Accepted {
            operation,
            message_sequence: Some(sequence),
            message_revision: None,
        },
        Ok(OutcomeV2::MessageEdited {
            message_revision, ..
        }) => Step::Accepted {
            operation,
            message_sequence: None,
            message_revision: Some(message_revision),
        },
        Ok(
            OutcomeV2::ThreadLifecycleChanged { .. }
            | OutcomeV2::ReactionChanged { .. }
            | OutcomeV2::MessageWithdrawn { .. },
        ) => Step::Accepted {
            operation,
            message_sequence: None,
            message_revision: None,
        },
        Ok(other) => panic!("unexpected modeled outcome: {other:?}"),
    }
}

fn run(actor_kind: ParticipantKind, schedule: [Operation; 4]) -> Vec<Step> {
    let (mut engine, root) = setup(actor_kind);
    let mut model = Model::new();
    let mut transcript = Vec::new();
    let actor = ParticipantId::from("actor");
    let room_id = RoomId::from("room");

    for (step, operation) in schedule.into_iter().enumerate() {
        let before_room = engine.room(&room_id).unwrap();
        let before_outbox: Vec<_> = engine.pending_events().cloned().collect();
        let before_journal: Vec<_> = engine.retained_changes().cloned().collect();
        let result = engine
            .apply_revisioned(RevisionedCommandV2 {
                expected_revision: engine.revision(),
                command: command(operation, root, step),
            })
            .map(|transition| transition.outcome);
        let actual = normalize(operation, result);
        let expected = model.apply(operation, step);
        assert_eq!(actual, expected, "operation={operation:?}");

        if matches!(actual, Step::Refused(_)) {
            assert_eq!(engine.room(&room_id).unwrap(), before_room);
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

        let room = engine.room(&room_id).unwrap();
        assert_eq!(room.next_message_sequence, model.next_message_sequence);
        assert_eq!(
            room.messages
                .iter()
                .map(|message| message.sequence())
                .collect::<Vec<_>>(),
            model.retained_messages
        );
        if model.root_retained {
            let message = engine.message_by_id(&room_id, root).unwrap();
            assert_eq!(message.body(), model.root_body);
            assert_eq!(message.revisions().len(), model.root_revisions);
            assert_eq!(message.is_withdrawn(), model.tombstoned);
            assert_eq!(
                message.revisions().iter().all(|revision| {
                    revision.availability == MessageRevisionAvailabilityV1::Tombstoned
                        && revision.body.is_empty()
                }),
                model.tombstoned
            );
            assert_eq!(
                engine.thread_status(&actor, &room_id, root).unwrap().state,
                if model.archived {
                    ThreadLifecycleStateV1::Archived
                } else {
                    ThreadLifecycleStateV1::Active
                }
            );
            let reactions = engine
                .reaction_status(&actor, &room_id, root)
                .unwrap()
                .reactions;
            assert_eq!(reactions.len(), usize::from(model.reacted));
            if model.reacted {
                assert_eq!(reactions[0].key, "spark");
                assert_eq!(reactions[0].count, 1);
                assert!(reactions[0].caller_reacted);
            }
        } else {
            assert!(engine.message_by_id(&room_id, root).is_none());
            assert_eq!(
                engine.thread_status(&actor, &room_id, root),
                Err(ObservationRefusal::InvalidTarget)
            );
            assert_eq!(
                engine.reaction_status(&actor, &room_id, root),
                Err(ObservationRefusal::InvalidTarget)
            );
        }
        transcript.push(actual);
    }
    transcript
}

#[test]
fn all_four_step_relation_schedules_match_reference_and_kind_symmetry() {
    const OPERATIONS: [Operation; 8] = [
        Operation::Reply,
        Operation::Archive,
        Operation::Reopen,
        Operation::React,
        Operation::Unreact,
        Operation::Edit,
        Operation::Withdraw,
        Operation::UnrelatedSpeak,
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
