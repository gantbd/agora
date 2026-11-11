use agora_engine::persistence::RecoveryLimits;
use agora_engine::{
    AdmissionPolicy, CommandV2, Engine, EventSequence, LogicalTime, ModerationCausationV1,
    ModerationReasonV1, OutcomeV2, Participant, ParticipantKind, PublicationCommandV2,
    PublicationKeyV2, RoomId,
};

fn fixture_engine() -> Engine {
    let mut engine = Engine::new();
    for (id, name, kind) in [
        ("human", "Human", ParticipantKind::Human),
        ("agent", "Agent", ParticipantKind::Agent),
    ] {
        engine
            .apply(CommandV2::RegisterParticipant(Participant::new(
                id, name, kind,
            )))
            .unwrap();
    }
    engine
        .apply(CommandV2::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
            owner: "human".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(CommandV2::Join {
            room_id: "room".into(),
            participant: "agent".into(),
        })
        .unwrap();
    let OutcomeV2::TopicCreated { topic_id, .. } = engine
        .apply(CommandV2::CreateTopic {
            room_id: "room".into(),
            actor: "human".into(),
            name: "General".to_owned(),
        })
        .unwrap()
    else {
        panic!("topic creation expected");
    };
    engine
        .apply(CommandV2::RenameTopic {
            room_id: "room".into(),
            actor: "human".into(),
            topic_id,
            name: "Operations".to_owned(),
        })
        .unwrap();
    engine
        .apply(CommandV2::SetTopicAlias {
            room_id: "room".into(),
            actor: "human".into(),
            topic_id,
            alias: "General".to_owned(),
            added: true,
        })
        .unwrap();
    engine
        .apply(CommandV2::SetTopicTag {
            room_id: "room".into(),
            actor: "human".into(),
            topic_id,
            tag: "urgent".to_owned(),
            added: true,
        })
        .unwrap();
    let OutcomeV2::TopicCreated {
        topic_id: successor_topic_id,
        ..
    } = engine
        .apply(CommandV2::CreateTopic {
            room_id: "room".into(),
            actor: "human".into(),
            name: "Current".to_owned(),
        })
        .unwrap()
    else {
        panic!("successor topic creation expected");
    };
    engine
        .apply(CommandV2::SpeakInTopic {
            room_id: "room".into(),
            author: "human".into(),
            topic_id,
            at: LogicalTime(17),
            body: "hello".to_owned(),
            addressed_to: vec!["agent".into()],
        })
        .unwrap();
    engine
        .publish(PublicationCommandV2::new(
            engine.revision(),
            PublicationKeyV2::new([0x2a; 16]),
            "room".into(),
            "agent".into(),
            LogicalTime(18),
            "reply",
        ))
        .unwrap();
    let root_message_id = engine.message(&RoomId::new("room"), 0).unwrap().id();
    engine
        .apply(CommandV2::SetThreadArchived {
            room_id: "room".into(),
            actor: "human".into(),
            root_message_id,
            archived: true,
            reason: ModerationReasonV1::new("pause"),
            causation: ModerationCausationV1::new([0x5b; 16]),
        })
        .unwrap();
    engine
        .apply(CommandV2::SetReaction {
            room_id: "room".into(),
            actor: "agent".into(),
            message_id: root_message_id,
            key: "spark".to_owned(),
            reacted: true,
        })
        .unwrap();
    engine
        .apply(CommandV2::EditMessage {
            room_id: "room".into(),
            actor: "human".into(),
            message_id: root_message_id,
            at: LogicalTime(19),
            body: "hello revised".to_owned(),
        })
        .unwrap();
    engine
        .advance_read_position(
            &"human".into(),
            &"room".into(),
            agora_engine::ReadScopeV1::Room,
            engine.revision(),
            root_message_id,
        )
        .unwrap();
    engine
        .apply(CommandV2::SetTopicResolved {
            room_id: "room".into(),
            actor: "human".into(),
            topic_id,
            resolved: true,
        })
        .unwrap();
    engine
        .apply(CommandV2::SetTopicSuccessor {
            room_id: "room".into(),
            actor: "human".into(),
            topic_id,
            successor_topic_id: Some(successor_topic_id),
        })
        .unwrap();
    engine.acknowledge_events(EventSequence(2)).unwrap();
    engine
        .apply(CommandV2::Ban {
            room_id: "room".into(),
            actor: "human".into(),
            participant: "agent".into(),
            reason: ModerationReasonV1::new("policy"),
            causation: ModerationCausationV1::new([0x5a; 16]),
        })
        .unwrap();
    engine
        .apply(CommandV2::SetAdmissionPolicy {
            room_id: "room".into(),
            actor: "human".into(),
            admission: AdmissionPolicy::InviteOnly,
        })
        .unwrap();
    engine
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

fn unhex(value: &str) -> Vec<u8> {
    fn digit(value: u8) -> u8 {
        match value {
            b'0'..=b'9' => value - b'0',
            b'a'..=b'f' => value - b'a' + 10,
            _ => panic!("fixture contains non-canonical hex"),
        }
    }
    let value = value.trim().as_bytes();
    assert_eq!(value.len() % 2, 0, "fixture has a partial byte");
    value
        .chunks_exact(2)
        .map(|pair| (digit(pair[0]) << 4) | digit(pair[1]))
        .collect()
}

#[test]
fn canonical_v19_checkpoint_is_immutable_and_recovers() {
    let source = fixture_engine();
    let topic_id = topic_id_from_fixture(&source);
    let prepared = source.prepare_checkpoint(65_536).unwrap();
    let expected_hex = include_str!("fixtures/checkpoint-v19.hex").trim();
    assert_eq!(hex(prepared.bytes()), expected_hex);

    let recovered = Engine::recover_checkpoint(
        &unhex(expected_hex),
        RecoveryLimits {
            max_snapshot_bytes: 65_536,
            maximum_engine_limits: Default::default(),
        },
    )
    .unwrap();
    assert_eq!(
        recovered.prepare_checkpoint(65_536).unwrap().bytes(),
        prepared.bytes()
    );
    assert!(
        recovered
            .topic(&RoomId::new("room"), topic_id)
            .unwrap()
            .is_resolved()
    );
    assert!(
        recovered
            .topic_supersession_status(&"human".into(), &"room".into(), topic_id)
            .unwrap()
            .successor_topic_id
            .is_some()
    );
    assert_eq!(
        recovered
            .topic_alias_status(&"human".into(), &"room".into(), topic_id)
            .unwrap()
            .aliases,
        vec!["General".to_owned()]
    );
    assert_eq!(
        recovered
            .topic_tag_status(&"human".into(), &"room".into(), topic_id)
            .unwrap()
            .tags,
        vec!["urgent".to_owned()]
    );

    let mut broken_history = prepared.bytes().to_vec();
    let offset = broken_history
        .windows(b"General".len())
        .enumerate()
        .filter(|(_, bytes)| *bytes == b"General")
        .nth(1)
        .map(|(offset, _)| offset)
        .expect("fixture retains the creation/rename name chain");
    broken_history[offset] = b'X';
    assert!(matches!(
        Engine::recover_checkpoint(
            &broken_history,
            RecoveryLimits {
                max_snapshot_bytes: 65_536,
                maximum_engine_limits: Default::default(),
            },
        ),
        Err(agora_engine::persistence::CheckpointError::Invalid)
    ));

    let mut lifecycle_pattern = vec![18];
    for value in ["room", "human"] {
        lifecycle_pattern.extend_from_slice(&(value.len() as u32).to_be_bytes());
        lifecycle_pattern.extend_from_slice(value.as_bytes());
    }
    lifecycle_pattern.extend_from_slice(&topic_id.into_bytes());
    lifecycle_pattern.extend_from_slice(&[0, 1]);
    let mut broken_lifecycle = prepared.bytes().to_vec();
    let offset = broken_lifecycle
        .windows(lifecycle_pattern.len())
        .position(|bytes| bytes == lifecycle_pattern)
        .expect("fixture retains the topic lifecycle chain");
    broken_lifecycle[offset + lifecycle_pattern.len() - 1] = 0;
    assert!(matches!(
        Engine::recover_checkpoint(
            &broken_lifecycle,
            RecoveryLimits {
                max_snapshot_bytes: 65_536,
                maximum_engine_limits: Default::default(),
            },
        ),
        Err(agora_engine::persistence::CheckpointError::Invalid)
    ));

    let mut supersession_pattern = vec![19];
    for value in ["room", "human"] {
        supersession_pattern.extend_from_slice(&(value.len() as u32).to_be_bytes());
        supersession_pattern.extend_from_slice(value.as_bytes());
    }
    supersession_pattern.extend_from_slice(&topic_id.into_bytes());
    supersession_pattern.push(0);
    supersession_pattern.push(1);
    let mut broken_supersession = prepared.bytes().to_vec();
    let offset = broken_supersession
        .windows(supersession_pattern.len())
        .position(|bytes| bytes.starts_with(&supersession_pattern))
        .expect("fixture retains the topic supersession chain");
    broken_supersession[offset + supersession_pattern.len()] ^= 1;
    assert!(matches!(
        Engine::recover_checkpoint(
            &broken_supersession,
            RecoveryLimits {
                max_snapshot_bytes: 65_536,
                maximum_engine_limits: Default::default(),
            },
        ),
        Err(agora_engine::persistence::CheckpointError::Invalid)
    ));

    let mut alias_pattern = vec![20];
    for value in ["room", "human"] {
        alias_pattern.extend_from_slice(&(value.len() as u32).to_be_bytes());
        alias_pattern.extend_from_slice(value.as_bytes());
    }
    alias_pattern.extend_from_slice(&topic_id.into_bytes());
    alias_pattern.extend_from_slice(&("General".len() as u32).to_be_bytes());
    alias_pattern.extend_from_slice(b"General");
    alias_pattern.push(1);
    let mut broken_alias = prepared.bytes().to_vec();
    let offset = broken_alias
        .windows(alias_pattern.len())
        .position(|bytes| bytes == alias_pattern)
        .expect("fixture retains the topic alias event");
    broken_alias[offset + alias_pattern.len() - 1] = 2;
    assert!(matches!(
        Engine::recover_checkpoint(
            &broken_alias,
            RecoveryLimits {
                max_snapshot_bytes: 65_536,
                maximum_engine_limits: Default::default(),
            },
        ),
        Err(agora_engine::persistence::CheckpointError::Invalid)
    ));
}

fn topic_id_from_fixture(engine: &Engine) -> agora_engine::TopicId {
    engine
        .topic_page(&"human".into(), &"room".into(), None, 1)
        .unwrap()
        .topics[0]
        .id()
}
