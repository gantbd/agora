use agora_engine::*;

#[derive(Clone, Copy)]
enum Action {
    First0,
    First50,
    First100,
    Second0,
    Second50,
    Second100,
    RestrictSecond,
    RestoreSecond,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResultClass {
    Accepted,
    Throttled(u64),
    Restricted,
    InvalidChange,
}

fn setup(first_kind: ParticipantKind, second_kind: ParticipantKind) -> Engine {
    let mut engine = Engine::new();
    for (id, kind) in [
        ("owner", ParticipantKind::Human),
        ("first", first_kind),
        ("second", second_kind),
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
    for participant in ["first", "second"] {
        engine
            .apply(CommandV2::Join {
                room_id: "room".into(),
                participant: participant.into(),
            })
            .unwrap();
    }
    engine
        .apply(CommandV2::SetMinimumMessageInterval {
            room_id: "room".into(),
            actor: "owner".into(),
            minimum_interval_ms: 100,
        })
        .unwrap();
    engine
}

fn run(
    actions: [Action; 3],
    first_kind: ParticipantKind,
    second_kind: ParticipantKind,
) -> Vec<ResultClass> {
    let mut engine = setup(first_kind, second_kind);
    let mut now = 0;
    let mut trace = Vec::new();
    for action in actions {
        let (participant, delta) = match action {
            Action::First0 => (Some("first"), 0),
            Action::First50 => (Some("first"), 50),
            Action::First100 => (Some("first"), 100),
            Action::Second0 => (Some("second"), 0),
            Action::Second50 => (Some("second"), 50),
            Action::Second100 => (Some("second"), 100),
            Action::RestrictSecond | Action::RestoreSecond => (None, 0),
        };
        if let Some(participant) = participant {
            now += delta;
            let time = engine.advance_admission_time(now).unwrap();
            let result = engine.apply_at(
                CommandV2::Speak {
                    room_id: "room".into(),
                    author: participant.into(),
                    at: LogicalTime(now),
                    body: "message".into(),
                },
                &time,
            );
            trace.push(match result {
                Ok(_) => ResultClass::Accepted,
                Err(Refusal::MessageThrottled { retry_after_ms }) => {
                    ResultClass::Throttled(retry_after_ms)
                }
                Err(Refusal::PostingRestricted) => ResultClass::Restricted,
                other => panic!("unexpected model result: {other:?}"),
            });
        } else {
            let restricted = matches!(action, Action::RestrictSecond);
            let result = engine.apply(CommandV2::SetPostingRestricted {
                room_id: "room".into(),
                actor: "owner".into(),
                participant: "second".into(),
                restricted,
                reason: ModerationReasonV1::new("model"),
                causation: ModerationCausationV1::new([1; 16]),
            });
            trace.push(match result {
                Ok(_) => ResultClass::Accepted,
                Err(Refusal::InvalidPostingRestrictionChange) => ResultClass::InvalidChange,
                other => panic!("unexpected model result: {other:?}"),
            });
        }
    }
    trace
}

#[test]
fn all_three_step_admission_schedules_are_human_agent_symmetric() {
    let actions = [
        Action::First0,
        Action::First50,
        Action::First100,
        Action::Second0,
        Action::Second50,
        Action::Second100,
        Action::RestrictSecond,
        Action::RestoreSecond,
    ];
    for first in actions {
        for second in actions {
            for third in actions {
                let schedule = [first, second, third];
                assert_eq!(
                    run(schedule, ParticipantKind::Human, ParticipantKind::Agent),
                    run(schedule, ParticipantKind::Agent, ParticipantKind::Human),
                );
            }
        }
    }
}
