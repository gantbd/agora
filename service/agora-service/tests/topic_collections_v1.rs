use agora_engine::{
    AdmissionPolicy, CommandV2 as EngineCommand, Engine, Limits, OutcomeV2 as EngineOutcome,
    Participant, ParticipantKind,
};
use agora_protocol::v2::{
    CapabilityV2, CommandV2, OutcomeV2, RefusalV2, ReplyV2, RequestIdV2, RequestV2,
    TopicCollectionFilterV1, TopicCollectionLifecycleV1, TopicIdV1,
};
use agora_service::v2::{
    ServiceLimitsV2, ServiceV2,
    persistence::{ServiceCheckpointScopeV1, ServiceRecoveryLimitsV2},
};

const SCOPE: ServiceCheckpointScopeV1 = ServiceCheckpointScopeV1::new(*b"collection-scope");

fn request(request_id: u64, command: CommandV2) -> RequestV2 {
    RequestV2 {
        request_id: RequestIdV2(request_id),
        command,
    }
}

fn filter() -> TopicCollectionFilterV1 {
    TopicCollectionFilterV1 {
        lifecycle: TopicCollectionLifecycleV1::Active,
        tags: vec!["blue".to_owned(), "urgent".to_owned()],
    }
}

fn topic(engine: &mut Engine, name: &str) -> agora_engine::TopicId {
    let EngineOutcome::TopicCreated { topic_id, .. } = engine
        .apply(EngineCommand::CreateTopic {
            room_id: "room".into(),
            actor: "human".into(),
            name: name.to_owned(),
        })
        .unwrap()
    else {
        unreachable!()
    };
    topic_id
}

fn setup() -> (ServiceV2, [agora_engine::TopicId; 3]) {
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
    let topics = [
        topic(&mut engine, "first"),
        topic(&mut engine, "second"),
        topic(&mut engine, "third"),
    ];
    for topic_id in topics {
        engine
            .apply(EngineCommand::SetTopicTag {
                room_id: "room".into(),
                actor: "human".into(),
                topic_id,
                tag: "blue".to_owned(),
                added: true,
            })
            .unwrap();
    }
    for topic_id in [topics[0], topics[1]] {
        engine
            .apply(EngineCommand::SetTopicTag {
                room_id: "room".into(),
                actor: "human".into(),
                topic_id,
                tag: "urgent".to_owned(),
                added: true,
            })
            .unwrap();
    }
    engine
        .apply(EngineCommand::SetTopicAlias {
            room_id: "room".into(),
            actor: "human".into(),
            topic_id: topics[0],
            alias: "former".to_owned(),
            added: true,
        })
        .unwrap();
    engine
        .apply(EngineCommand::SetTopicResolved {
            room_id: "room".into(),
            actor: "human".into(),
            topic_id: topics[2],
            resolved: true,
        })
        .unwrap();
    engine
        .apply(EngineCommand::SetTopicSuccessor {
            room_id: "room".into(),
            actor: "human".into(),
            topic_id: topics[2],
            successor_topic_id: Some(topics[0]),
        })
        .unwrap();
    (ServiceV2::new(engine, ServiceLimitsV2::default()), topics)
}

fn recovery_limits() -> ServiceRecoveryLimitsV2 {
    ServiceRecoveryLimitsV2 {
        max_snapshot_bytes: 2 * 1_024 * 1_024,
        max_engine_snapshot_bytes: 2 * 1_024 * 1_024,
        maximum_service_limits: ServiceLimitsV2::default(),
        maximum_engine_limits: Limits::default(),
    }
}

#[test]
fn filtered_collection_cursor_is_client_filter_and_checkpoint_bound() {
    let (mut service, topics) = setup();
    let human = service.connect("human").unwrap();
    let agent = service.connect("agent").unwrap();

    assert!(matches!(
        service.execute(agent, request(1, CommandV2::Capabilities)),
        ReplyV2::Accepted(OutcomeV2::Capabilities(capabilities))
            if capabilities.contains(&CapabilityV2::TopicCollectionsV1)
    ));
    let ReplyV2::Accepted(OutcomeV2::TopicCollection(first)) = service.execute(
        agent,
        request(
            2,
            CommandV2::TopicCollection {
                room_id: "room".to_owned(),
                filter: filter(),
                cursor: None,
                limit: 1,
            },
        ),
    ) else {
        panic!("first collection page must succeed")
    };
    assert_eq!(first.topics.len(), 1);
    assert_eq!(
        first.topics[0].topic_id,
        TopicIdV1::new(topics[0].into_bytes())
    );
    assert_eq!(first.topics[0].aliases, vec!["former"]);
    let cursor = first.next.unwrap();

    assert_eq!(
        service.execute(
            human,
            request(
                1,
                CommandV2::TopicCollection {
                    room_id: "room".to_owned(),
                    filter: filter(),
                    cursor: Some(cursor),
                    limit: 1,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::InvalidCursor)
    );
    assert_eq!(
        service.execute(
            agent,
            request(
                3,
                CommandV2::TopicCollection {
                    room_id: "room".to_owned(),
                    filter: TopicCollectionFilterV1 {
                        lifecycle: TopicCollectionLifecycleV1::Any,
                        tags: vec!["blue".to_owned()],
                    },
                    cursor: Some(cursor),
                    limit: 1,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::InvalidCursor)
    );

    let checkpoint = service
        .prepare_checkpoint(SCOPE, 2 * 1_024 * 1_024)
        .unwrap();
    assert_eq!(checkpoint.bytes()[8], 32);
    let mut recovered =
        ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, recovery_limits()).unwrap();
    let ReplyV2::Accepted(OutcomeV2::TopicCollection(second)) = recovered.execute(
        agent,
        request(
            4,
            CommandV2::TopicCollection {
                room_id: "room".to_owned(),
                filter: filter(),
                cursor: Some(cursor),
                limit: 1,
            },
        ),
    ) else {
        panic!("recovered collection page must succeed")
    };
    assert_eq!(second.topics.len(), 1);
    assert_eq!(
        second.topics[0].topic_id,
        TopicIdV1::new(topics[1].into_bytes())
    );
    assert_eq!(second.next, None);
}

#[test]
fn collection_shape_and_membership_fail_closed() {
    let (mut service, _) = setup();
    let human = service.connect("human").unwrap();
    assert_eq!(
        service.execute(
            human,
            request(
                1,
                CommandV2::TopicCollection {
                    room_id: "room".to_owned(),
                    filter: TopicCollectionFilterV1 {
                        lifecycle: TopicCollectionLifecycleV1::Any,
                        tags: vec!["urgent".to_owned(), "blue".to_owned()],
                    },
                    cursor: None,
                    limit: 1,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::InvalidInput)
    );
}
