use agora_engine::{
    AdmissionPolicy, CommandV2, Engine, OutcomeV2, Participant, ParticipantId, ParticipantKind,
    Refusal, RoomId, TopicCollectionFilterV1, TopicCollectionLifecycleV1, TopicId,
};

#[derive(Clone, Copy, Debug)]
enum Action {
    ResolveFirst,
    ReopenFirst,
    AddBlueFirst,
    RemoveBlueFirst,
    AddBlueSecond,
    AddUrgentSecond,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Model {
    first_resolved: bool,
    first_blue: bool,
    second_blue: bool,
    second_urgent: bool,
}

impl Model {
    fn apply(&mut self, action: Action) -> bool {
        let target = match action {
            Action::ResolveFirst => &mut self.first_resolved,
            Action::ReopenFirst => {
                if !self.first_resolved {
                    return false;
                }
                self.first_resolved = false;
                return true;
            }
            Action::AddBlueFirst => &mut self.first_blue,
            Action::RemoveBlueFirst => {
                if !self.first_blue {
                    return false;
                }
                self.first_blue = false;
                return true;
            }
            Action::AddBlueSecond => &mut self.second_blue,
            Action::AddUrgentSecond => &mut self.second_urgent,
        };
        if *target {
            false
        } else {
            *target = true;
            true
        }
    }

    fn expected(
        &self,
        lifecycle: TopicCollectionLifecycleV1,
        tags: &[String],
        topics: [TopicId; 2],
    ) -> Vec<TopicId> {
        [
            (topics[0], self.first_resolved, self.first_blue, false),
            (topics[1], false, self.second_blue, self.second_urgent),
        ]
        .into_iter()
        .filter(|(_, resolved, blue, urgent)| {
            let lifecycle_matches = match lifecycle {
                TopicCollectionLifecycleV1::Any => true,
                TopicCollectionLifecycleV1::Active => !resolved,
                TopicCollectionLifecycleV1::Resolved => *resolved,
            };
            lifecycle_matches
                && tags.iter().all(|tag| match tag.as_str() {
                    "blue" => *blue,
                    "urgent" => *urgent,
                    _ => false,
                })
        })
        .map(|(topic_id, _, _, _)| topic_id)
        .collect()
    }
}

fn setup(observer_kind: ParticipantKind) -> (Engine, RoomId, ParticipantId, [TopicId; 2]) {
    let mut engine = Engine::new();
    for (id, kind) in [
        ("owner", ParticipantKind::Human),
        ("observer", observer_kind),
    ] {
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
            owner: "owner".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    let observer = ParticipantId::new("observer");
    engine
        .apply(CommandV2::Join {
            room_id: room.clone(),
            participant: observer.clone(),
        })
        .unwrap();
    let topics = ["first", "second"].map(|name| {
        let OutcomeV2::TopicCreated { topic_id, .. } = engine
            .apply(CommandV2::CreateTopic {
                room_id: room.clone(),
                actor: "owner".into(),
                name: name.to_owned(),
            })
            .unwrap()
        else {
            unreachable!()
        };
        topic_id
    });
    (engine, room, observer, topics)
}

fn assert_projection(
    engine: &Engine,
    room: &RoomId,
    observer: &ParticipantId,
    topics: [TopicId; 2],
    model: &Model,
) {
    for lifecycle in [
        TopicCollectionLifecycleV1::Any,
        TopicCollectionLifecycleV1::Active,
        TopicCollectionLifecycleV1::Resolved,
    ] {
        for tags in [
            vec![],
            vec!["blue".to_owned()],
            vec!["urgent".to_owned()],
            vec!["blue".to_owned(), "urgent".to_owned()],
        ] {
            let page = engine
                .topic_collection_page(
                    observer,
                    room,
                    &TopicCollectionFilterV1 {
                        lifecycle,
                        tags: tags.clone(),
                    },
                    None,
                    2,
                )
                .unwrap();
            assert_eq!(
                page.topics
                    .into_iter()
                    .map(|entry| entry.topic_id)
                    .collect::<Vec<_>>(),
                model.expected(lifecycle, &tags, topics)
            );
        }
    }
}

fn run(observer_kind: ParticipantKind, schedule: [Action; 3]) -> Model {
    let (mut engine, room, observer, topics) = setup(observer_kind);
    let mut model = Model {
        first_resolved: false,
        first_blue: false,
        second_blue: false,
        second_urgent: false,
    };
    assert_projection(&engine, &room, &observer, topics, &model);
    for action in schedule {
        let expected_accepted = model.apply(action);
        let result = engine.apply(match action {
            Action::ResolveFirst => CommandV2::SetTopicResolved {
                room_id: room.clone(),
                actor: "owner".into(),
                topic_id: topics[0],
                resolved: true,
            },
            Action::ReopenFirst => CommandV2::SetTopicResolved {
                room_id: room.clone(),
                actor: "owner".into(),
                topic_id: topics[0],
                resolved: false,
            },
            Action::AddBlueFirst => CommandV2::SetTopicTag {
                room_id: room.clone(),
                actor: "owner".into(),
                topic_id: topics[0],
                tag: "blue".to_owned(),
                added: true,
            },
            Action::RemoveBlueFirst => CommandV2::SetTopicTag {
                room_id: room.clone(),
                actor: "owner".into(),
                topic_id: topics[0],
                tag: "blue".to_owned(),
                added: false,
            },
            Action::AddBlueSecond => CommandV2::SetTopicTag {
                room_id: room.clone(),
                actor: "owner".into(),
                topic_id: topics[1],
                tag: "blue".to_owned(),
                added: true,
            },
            Action::AddUrgentSecond => CommandV2::SetTopicTag {
                room_id: room.clone(),
                actor: "owner".into(),
                topic_id: topics[1],
                tag: "urgent".to_owned(),
                added: true,
            },
        });
        assert_eq!(result.is_ok(), expected_accepted, "action={action:?}");
        if let Err(refusal) = result {
            assert!(matches!(
                refusal,
                Refusal::InvalidTopicLifecycleChange
                    | Refusal::DuplicateTopicTag
                    | Refusal::UnknownTopicTag
            ));
        }
        assert_projection(&engine, &room, &observer, topics, &model);
    }
    model
}

#[test]
fn all_three_step_collection_schedules_match_reference_and_kind_symmetry() {
    let actions = [
        Action::ResolveFirst,
        Action::ReopenFirst,
        Action::AddBlueFirst,
        Action::RemoveBlueFirst,
        Action::AddBlueSecond,
        Action::AddUrgentSecond,
    ];
    for first in actions {
        for second in actions {
            for third in actions {
                let schedule = [first, second, third];
                let human = run(ParticipantKind::Human, schedule);
                let agent = run(ParticipantKind::Agent, schedule);
                assert_eq!(human, agent, "schedule={schedule:?}");
            }
        }
    }
}
