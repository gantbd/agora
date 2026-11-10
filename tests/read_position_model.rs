use agora_engine::{
    AdmissionPolicy, CommandV2, Engine, LogicalTime, MessageId, OutcomeV2, Participant,
    ParticipantId, ParticipantKind, ReadPositionRefusalV1, ReadScopeV1, RoomId,
};

#[derive(Clone, Copy, Debug)]
enum ScopeKind {
    Room,
    Thread,
    Topic,
}

fn accepted_message(engine: &Engine, sequence: u64) -> MessageId {
    engine.message(&RoomId::new("room"), sequence).unwrap().id()
}

fn setup(kind: ParticipantKind, scope_kind: ScopeKind) -> (Engine, ReadScopeV1, [MessageId; 3]) {
    let mut engine = Engine::new();
    engine
        .apply(CommandV2::RegisterParticipant(Participant::new(
            "owner",
            "Owner",
            ParticipantKind::Human,
        )))
        .unwrap();
    engine
        .apply(CommandV2::RegisterParticipant(Participant::new(
            "actor", "Actor", kind,
        )))
        .unwrap();
    engine
        .apply(CommandV2::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
            owner: "owner".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(CommandV2::Join {
            room_id: "room".into(),
            participant: "actor".into(),
        })
        .unwrap();

    match scope_kind {
        ScopeKind::Room => {
            let mut targets = Vec::new();
            for (author, body) in [("owner", "zero"), ("actor", "one"), ("owner", "two")] {
                let OutcomeV2::MessageAccepted { sequence, .. } = engine
                    .apply(CommandV2::Speak {
                        room_id: "room".into(),
                        author: author.into(),
                        at: LogicalTime(sequence_time(targets.len())),
                        body: body.to_owned(),
                    })
                    .unwrap()
                else {
                    panic!("room message expected");
                };
                targets.push(accepted_message(&engine, sequence));
            }
            (engine, ReadScopeV1::Room, targets.try_into().unwrap())
        }
        ScopeKind::Thread => {
            let OutcomeV2::MessageAccepted { sequence, .. } = engine
                .apply(CommandV2::Speak {
                    room_id: "room".into(),
                    author: "owner".into(),
                    at: LogicalTime(0),
                    body: "root".to_owned(),
                })
                .unwrap()
            else {
                panic!("thread root expected");
            };
            let root = accepted_message(&engine, sequence);
            let mut targets = vec![root];
            for (author, body) in [("actor", "one"), ("owner", "two")] {
                let OutcomeV2::ReplyAccepted { sequence, .. } = engine
                    .apply(CommandV2::SpeakReply {
                        room_id: "room".into(),
                        author: author.into(),
                        root_message_id: root,
                        at: LogicalTime(sequence_time(targets.len())),
                        body: body.to_owned(),
                        addressed_to: vec![],
                    })
                    .unwrap()
                else {
                    panic!("thread reply expected");
                };
                targets.push(accepted_message(&engine, sequence));
            }
            (
                engine,
                ReadScopeV1::Thread(root),
                targets.try_into().unwrap(),
            )
        }
        ScopeKind::Topic => {
            let OutcomeV2::TopicCreated { topic_id, .. } = engine
                .apply(CommandV2::CreateTopic {
                    room_id: "room".into(),
                    actor: "owner".into(),
                    name: "topic".to_owned(),
                })
                .unwrap()
            else {
                panic!("topic expected");
            };
            let mut targets = Vec::new();
            for (author, body) in [("owner", "zero"), ("actor", "one"), ("owner", "two")] {
                let OutcomeV2::TopicMessageAccepted { sequence, .. } = engine
                    .apply(CommandV2::SpeakInTopic {
                        room_id: "room".into(),
                        author: author.into(),
                        topic_id,
                        at: LogicalTime(sequence_time(targets.len())),
                        body: body.to_owned(),
                        addressed_to: vec![],
                    })
                    .unwrap()
                else {
                    panic!("topic message expected");
                };
                targets.push(accepted_message(&engine, sequence));
            }
            (
                engine,
                ReadScopeV1::Topic(topic_id),
                targets.try_into().unwrap(),
            )
        }
    }
}

fn sequence_time(index: usize) -> u64 {
    u64::try_from(index).unwrap()
}

#[test]
fn bounded_schedules_match_the_monotone_reference_model_for_humans_and_agents() {
    for kind in [ParticipantKind::Human, ParticipantKind::Agent] {
        for scope_kind in [ScopeKind::Room, ScopeKind::Thread, ScopeKind::Topic] {
            for encoded in 0_u8..27 {
                let schedule = [
                    usize::from(encoded % 3),
                    usize::from((encoded / 3) % 3),
                    usize::from((encoded / 9) % 3),
                ];
                let (mut engine, scope, targets) = setup(kind, scope_kind);
                let revision = engine.revision();
                let events = engine.pending_events().count();
                let mut reference = None;

                for target_index in schedule {
                    let actual = engine.advance_read_position(
                        &ParticipantId::new("actor"),
                        &RoomId::new("room"),
                        scope,
                        revision,
                        targets[target_index],
                    );
                    match reference {
                        Some(current) if target_index < current => {
                            assert_eq!(actual, Err(ReadPositionRefusalV1::Regression));
                        }
                        _ => {
                            let marker = actual.unwrap();
                            assert_eq!(marker.target_message_id(), targets[target_index]);
                            assert_eq!(marker.target_sequence(), target_index as u64);
                            reference = Some(
                                reference.map_or(target_index, |current| current.max(target_index)),
                            );
                        }
                    }
                    assert_eq!(engine.revision(), revision);
                    assert_eq!(engine.pending_events().count(), events);
                }

                let marker = engine
                    .read_position(&ParticipantId::new("actor"), &RoomId::new("room"), &scope)
                    .unwrap()
                    .unwrap();
                assert_eq!(marker.target_message_id(), targets[reference.unwrap()]);
            }
        }
    }
}
