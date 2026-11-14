use agora_engine::{
    AdmissionPolicy, CommandV2, Engine, OutcomeV2, Participant, ParticipantId, ParticipantKind,
    Refusal, Revision, RevisionedCommandV2, Role, RoomId,
};

#[derive(Clone, Copy)]
enum Action {
    AliceVote,
    AliceRetract,
    BotVote,
    BotRetract,
    Close,
    Reopen,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Effect {
    Accepted,
    Closed,
    DuplicateBallot,
    DuplicateLifecycle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Model {
    open: bool,
    alice: Option<u8>,
    bot: Option<u8>,
}

impl Model {
    fn apply(&mut self, action: Action) -> Effect {
        match action {
            Action::Close => {
                if self.open {
                    self.open = false;
                    Effect::Accepted
                } else {
                    Effect::DuplicateLifecycle
                }
            }
            Action::Reopen => {
                if self.open {
                    Effect::DuplicateLifecycle
                } else {
                    self.open = true;
                    Effect::Accepted
                }
            }
            _ if !self.open => Effect::Closed,
            Action::AliceVote => {
                if self.alice == Some(0) {
                    Effect::DuplicateBallot
                } else {
                    self.alice = Some(0);
                    Effect::Accepted
                }
            }
            Action::AliceRetract => {
                if self.alice.is_none() {
                    Effect::DuplicateBallot
                } else {
                    self.alice = None;
                    Effect::Accepted
                }
            }
            Action::BotVote => {
                if self.bot == Some(1) {
                    Effect::DuplicateBallot
                } else {
                    self.bot = Some(1);
                    Effect::Accepted
                }
            }
            Action::BotRetract => {
                if self.bot.is_none() {
                    Effect::DuplicateBallot
                } else {
                    self.bot = None;
                    Effect::Accepted
                }
            }
        }
    }

    fn totals(&self) -> Vec<u16> {
        vec![
            u16::from(self.alice == Some(0)),
            u16::from(self.bot == Some(1)),
        ]
    }
}

fn setup(
    alice_kind: ParticipantKind,
    bot_kind: ParticipantKind,
) -> (Engine, RoomId, agora_engine::PollId) {
    let mut engine = Engine::new();
    for (id, kind) in [("alice", alice_kind), ("bot", bot_kind)] {
        engine
            .apply(CommandV2::RegisterParticipant(Participant::new(
                id, id, kind,
            )))
            .unwrap();
    }
    let room: RoomId = "room".into();
    engine
        .apply(CommandV2::CreateRoom {
            room_id: room.clone(),
            name: "Room".to_owned(),
            owner: "alice".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(CommandV2::Join {
            room_id: room.clone(),
            participant: "bot".into(),
        })
        .unwrap();
    engine
        .apply(CommandV2::SetRole {
            room_id: room.clone(),
            actor: "alice".into(),
            participant: "bot".into(),
            role: Role::Moderator,
        })
        .unwrap();
    let OutcomeV2::PollCreated { poll_id, .. } = engine
        .apply(CommandV2::CreatePoll {
            room_id: room.clone(),
            actor: "alice".into(),
            prompt: "choose".to_owned(),
            options: vec!["a".to_owned(), "b".to_owned()],
            eligible_participants: vec!["alice".into(), "bot".into()],
            max_choices: 1,
        })
        .unwrap()
    else {
        unreachable!()
    };
    (engine, room, poll_id)
}

fn run(schedule: &[Action], alice_kind: ParticipantKind, bot_kind: ParticipantKind) -> Model {
    let (mut engine, room, poll_id) = setup(alice_kind, bot_kind);
    let alice = ParticipantId::new("alice");
    let bot = ParticipantId::new("bot");
    let mut model = Model {
        open: true,
        alice: None,
        bot: None,
    };
    for action in schedule {
        let expected = model.apply(*action);
        let before = engine.revision();
        let command = match action {
            Action::AliceVote => CommandV2::SetPollBallot {
                room_id: room.clone(),
                actor: alice.clone(),
                poll_id,
                selections: vec![0],
            },
            Action::AliceRetract => CommandV2::SetPollBallot {
                room_id: room.clone(),
                actor: alice.clone(),
                poll_id,
                selections: vec![],
            },
            Action::BotVote => CommandV2::SetPollBallot {
                room_id: room.clone(),
                actor: bot.clone(),
                poll_id,
                selections: vec![1],
            },
            Action::BotRetract => CommandV2::SetPollBallot {
                room_id: room.clone(),
                actor: bot.clone(),
                poll_id,
                selections: vec![],
            },
            Action::Close => CommandV2::SetPollOpen {
                room_id: room.clone(),
                actor: alice.clone(),
                poll_id,
                open: false,
            },
            Action::Reopen => CommandV2::SetPollOpen {
                room_id: room.clone(),
                actor: bot.clone(),
                poll_id,
                open: true,
            },
        };
        let result = engine.apply_revisioned(RevisionedCommandV2 {
            expected_revision: before,
            command,
        });
        let actual = match result {
            Ok(_) => Effect::Accepted,
            Err(Refusal::PollClosed) => Effect::Closed,
            Err(Refusal::InvalidPollBallotChange) => Effect::DuplicateBallot,
            Err(Refusal::InvalidPollLifecycleChange) => Effect::DuplicateLifecycle,
            Err(other) => panic!("unexpected refusal: {other:?}"),
        };
        assert_eq!(actual, expected);
        assert_eq!(
            engine.revision(),
            Revision(before.0 + u64::from(actual == Effect::Accepted))
        );
        let status = engine.poll(&alice, &room, poll_id).unwrap();
        assert_eq!(status.is_open(), model.open);
        assert_eq!(status.totals(), model.totals());
        assert_eq!(
            engine
                .poll_ballot(&alice, &room, poll_id)
                .unwrap()
                .selections(),
            model.alice.iter().copied().collect::<Vec<_>>()
        );
        assert_eq!(
            engine
                .poll_ballot(&bot, &room, poll_id)
                .unwrap()
                .selections(),
            model.bot.iter().copied().collect::<Vec<_>>()
        );
    }
    model
}

#[test]
fn all_four_step_poll_schedules_match_reference_and_kind_symmetry() {
    let actions = [
        Action::AliceVote,
        Action::AliceRetract,
        Action::BotVote,
        Action::BotRetract,
        Action::Close,
        Action::Reopen,
    ];
    for first in actions {
        for second in actions {
            for third in actions {
                for fourth in actions {
                    let schedule = [first, second, third, fourth];
                    let human_agent =
                        run(&schedule, ParticipantKind::Human, ParticipantKind::Agent);
                    let agent_human =
                        run(&schedule, ParticipantKind::Agent, ParticipantKind::Human);
                    assert_eq!(human_agent, agent_human);
                }
            }
        }
    }
}
