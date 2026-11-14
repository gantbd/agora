use agora_engine::{
    AdmissionPolicy, CommandV2, Engine, LogicalTime, OutcomeV2, Participant, ParticipantId,
    ParticipantKind, PrivateSaveRefusalV1, Refusal, RoomId,
};

#[derive(Clone, Copy, Debug)]
enum Operation {
    Save(bool),
    Pin(bool),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Step {
    accepted: bool,
    saved: bool,
    pinned: bool,
    revision_delta: u64,
    event_delta: usize,
}

fn run(kind: ParticipantKind, schedule: [Operation; 3]) -> Vec<Step> {
    let mut engine = Engine::new();
    engine
        .apply(CommandV2::RegisterParticipant(Participant::new(
            "actor", "Actor", kind,
        )))
        .unwrap();
    engine
        .apply(CommandV2::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
            owner: "actor".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    let OutcomeV2::MessageAccepted { sequence, .. } = engine
        .apply(CommandV2::Speak {
            room_id: "room".into(),
            author: "actor".into(),
            at: LogicalTime(1),
            body: "body".to_owned(),
        })
        .unwrap()
    else {
        panic!("message accepted expected");
    };
    let message_id = engine.message(&RoomId::new("room"), sequence).unwrap().id();
    let baseline_revision = engine.revision().0;
    let baseline_events = engine.pending_events().len();
    let mut saved = false;
    let mut pinned = false;
    let mut steps = Vec::with_capacity(schedule.len());

    for operation in schedule {
        let accepted = match operation {
            Operation::Save(target) => {
                let expected = target != saved;
                let result = engine.set_saved_reference(
                    &ParticipantId::new("actor"),
                    &RoomId::new("room"),
                    message_id,
                    engine.revision(),
                    target,
                );
                assert_eq!(
                    result.is_ok(),
                    expected,
                    "unexpected save result: {result:?}"
                );
                if !expected {
                    assert_eq!(result, Err(PrivateSaveRefusalV1::InvalidChange));
                } else {
                    saved = target;
                }
                expected
            }
            Operation::Pin(target) => {
                let expected = target != pinned;
                let result = engine.apply(CommandV2::SetPin {
                    room_id: "room".into(),
                    actor: "actor".into(),
                    message_id,
                    pinned: target,
                });
                assert_eq!(
                    result.is_ok(),
                    expected,
                    "unexpected pin result: {result:?}"
                );
                if !expected {
                    assert_eq!(result, Err(Refusal::InvalidPinChange));
                } else {
                    pinned = target;
                }
                expected
            }
        };
        assert_eq!(
            engine
                .saved_references(&ParticipantId::new("actor"), &RoomId::new("room"))
                .unwrap()
                .len(),
            usize::from(saved)
        );
        assert_eq!(
            engine
                .pinned_references(&ParticipantId::new("actor"), &RoomId::new("room"))
                .unwrap()
                .len(),
            usize::from(pinned)
        );
        steps.push(Step {
            accepted,
            saved,
            pinned,
            revision_delta: engine.revision().0 - baseline_revision,
            event_delta: engine.pending_events().len() - baseline_events,
        });
    }
    steps
}

#[test]
fn all_three_step_pin_save_schedules_are_exact_and_kind_neutral() {
    let operations = [
        Operation::Save(false),
        Operation::Save(true),
        Operation::Pin(false),
        Operation::Pin(true),
    ];
    for first in operations {
        for second in operations {
            for third in operations {
                let schedule = [first, second, third];
                let human = run(ParticipantKind::Human, schedule);
                let agent = run(ParticipantKind::Agent, schedule);
                assert_eq!(human, agent, "schedule={schedule:?}");
                for step in human {
                    assert_eq!(step.revision_delta as usize, step.event_delta);
                }
            }
        }
    }
}
