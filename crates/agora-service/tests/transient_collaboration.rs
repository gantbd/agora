use agora_engine::{
    AdmissionPolicy, CommandV2 as EngineCommand, Engine, Participant, ParticipantId,
    ParticipantKind, Revision, RoomId,
};
use agora_service::transient::{
    PresenceSourceV1, PresenceStateV1, PresenceViewStateV1, TransientCollaborationV1,
    TransientLimitsV1, TransientRefusalV1, TransientTimeV1,
};

fn fixture() -> (Engine, ParticipantId, ParticipantId, RoomId) {
    let mut engine = Engine::new();
    let human = ParticipantId::new("human");
    let agent = ParticipantId::new("agent");
    let room = RoomId::new("room");
    engine
        .apply(EngineCommand::RegisterParticipant(Participant::new(
            "human",
            "human",
            ParticipantKind::Human,
        )))
        .unwrap();
    engine
        .apply(EngineCommand::RegisterParticipant(Participant::new(
            "agent",
            "agent",
            ParticipantKind::Agent,
        )))
        .unwrap();
    engine
        .apply(EngineCommand::CreateRoom {
            room_id: room.clone(),
            name: "room".to_owned(),
            owner: human.clone(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(EngineCommand::Join {
            room_id: room.clone(),
            participant: agent.clone(),
        })
        .unwrap();
    (engine, human, agent, room)
}

fn owner(max_signals: usize, max_sources: usize) -> TransientCollaborationV1 {
    TransientCollaborationV1::new(TransientLimitsV1 {
        max_signals,
        max_sources_per_participant: max_sources,
        max_lifetime: 10,
        max_page_items: 8,
    })
    .unwrap()
}

#[test]
fn presence_is_kind_neutral_private_expiring_and_revision_free() {
    for actor_is_agent in [false, true] {
        let (engine, human, agent, room) = fixture();
        let actor = if actor_is_agent { &agent } else { &human };
        let observer = if actor_is_agent { &human } else { &agent };
        let source = PresenceSourceV1::new([actor_is_agent as u8 + 1; 16]);
        let revision = engine.revision();
        let mut transient = owner(4, 2);
        assert_eq!(
            transient
                .publish_presence(
                    &engine,
                    &room,
                    actor,
                    source,
                    PresenceStateV1::Available,
                    TransientTimeV1(5),
                    3,
                )
                .unwrap(),
            TransientTimeV1(8)
        );
        let page = transient
            .presence_page(&engine, observer, &room, None, 8, TransientTimeV1(7))
            .unwrap();
        let actor_entry = page
            .entries
            .iter()
            .find(|entry| entry.participant_id == *actor)
            .unwrap();
        assert_eq!(actor_entry.state, PresenceViewStateV1::Available);
        assert!(actor_entry.source.is_some());
        assert_eq!(actor_entry.expires_at, Some(TransientTimeV1(8)));
        assert_eq!(engine.revision(), revision);

        let expired = transient
            .presence_page(&engine, observer, &room, None, 8, TransientTimeV1(8))
            .unwrap();
        let actor_entry = expired
            .entries
            .iter()
            .find(|entry| entry.participant_id == *actor)
            .unwrap();
        assert_eq!(actor_entry.state, PresenceViewStateV1::Unknown);
        assert!(actor_entry.source.is_none());
        assert!(actor_entry.expires_at.is_none());
        assert_eq!(engine.revision(), revision);
    }
}

#[test]
fn source_and_membership_retirement_prevent_resurrection() {
    let (mut engine, human, agent, room) = fixture();
    let source = PresenceSourceV1::new([3; 16]);
    let mut transient = owner(4, 2);
    transient
        .publish_presence(
            &engine,
            &room,
            &agent,
            source,
            PresenceStateV1::Busy,
            TransientTimeV1(1),
            10,
        )
        .unwrap();
    assert_eq!(transient.retire_source(source), 1);
    assert_eq!(transient.retire_source(source), 0);

    transient
        .publish_presence(
            &engine,
            &room,
            &agent,
            source,
            PresenceStateV1::Away,
            TransientTimeV1(2),
            9,
        )
        .unwrap();
    assert_eq!(transient.retire_participant_room(&room, &agent), 1);
    engine
        .apply(EngineCommand::Leave {
            room_id: room.clone(),
            participant: agent.clone(),
        })
        .unwrap();
    engine
        .apply(EngineCommand::Join {
            room_id: room.clone(),
            participant: agent.clone(),
        })
        .unwrap();
    let page = transient
        .presence_page(&engine, &human, &room, None, 8, TransientTimeV1(3))
        .unwrap();
    assert_eq!(
        page.entries
            .iter()
            .find(|entry| entry.participant_id == agent)
            .unwrap()
            .state,
        PresenceViewStateV1::Unknown
    );
}

#[test]
fn bounds_clock_and_snapshot_cursors_fail_closed() {
    let (engine, human, agent, room) = fixture();
    let mut transient = owner(1, 1);
    transient
        .publish_presence(
            &engine,
            &room,
            &human,
            PresenceSourceV1::new([1; 16]),
            PresenceStateV1::Available,
            TransientTimeV1(1),
            5,
        )
        .unwrap();
    let generation = transient.generation();
    assert_eq!(
        transient.publish_presence(
            &engine,
            &room,
            &agent,
            PresenceSourceV1::new([2; 16]),
            PresenceStateV1::Busy,
            TransientTimeV1(1),
            5,
        ),
        Err(TransientRefusalV1::Capacity)
    );
    assert_eq!(transient.generation(), generation);
    assert_eq!(
        transient.presence_page(&engine, &human, &room, None, 8, TransientTimeV1(0)),
        Err(TransientRefusalV1::ClockRegression)
    );
    assert_eq!(
        transient.expire(TransientTimeV1(6), 0),
        Err(TransientRefusalV1::InvalidWorkBudget)
    );
    assert_eq!(transient.expire(TransientTimeV1(6), 1).unwrap(), 1);
    transient
        .publish_presence(
            &engine,
            &room,
            &agent,
            PresenceSourceV1::new([2; 16]),
            PresenceStateV1::Busy,
            TransientTimeV1(6),
            4,
        )
        .unwrap();

    let first = transient
        .presence_page(&engine, &human, &room, None, 1, TransientTimeV1(7))
        .unwrap();
    let cursor = first.next.unwrap();
    assert_eq!(
        transient.presence_page(&engine, &human, &room, Some(&cursor), 1, TransientTimeV1(8),),
        Err(TransientRefusalV1::StaleCursor)
    );
    transient
        .clear_presence(
            &engine,
            &room,
            &agent,
            PresenceSourceV1::new([2; 16]),
            TransientTimeV1(7),
        )
        .unwrap();
    assert_eq!(
        transient.presence_page(&engine, &human, &room, Some(&cursor), 1, TransientTimeV1(7),),
        Err(TransientRefusalV1::StaleCursor)
    );
    assert_eq!(format!("{cursor:?}"), "PresenceCursorV1(<opaque>)");
    assert_eq!(
        format!("{:?}", PresenceSourceV1::new([0x73; 16])),
        "PresenceSourceV1(<opaque>)"
    );
}

#[test]
fn empty_restarted_owner_projects_unknown() {
    let (engine, human, agent, room) = fixture();
    let mut restarted = owner(4, 2);
    let page = restarted
        .presence_page(&engine, &human, &room, None, 8, TransientTimeV1(100))
        .unwrap();
    assert_eq!(page.entries.len(), 2);
    assert!(
        page.entries
            .iter()
            .all(|entry| entry.state == PresenceViewStateV1::Unknown)
    );
    assert!(page.entries.iter().any(|entry| {
        entry.participant_id == human && entry.participant_kind == ParticipantKind::Human
    }));
    assert!(page.entries.iter().any(|entry| {
        entry.participant_id == agent && entry.participant_kind == ParticipantKind::Agent
    }));
    assert_eq!(engine.revision(), Revision(4));
}

#[test]
fn multiple_sources_fall_back_and_pages_bind_privacy_and_engine_revision() {
    let (mut engine, human, agent, room) = fixture();
    let outsider = ParticipantId::new("outsider");
    engine
        .apply(EngineCommand::RegisterParticipant(Participant::new(
            "outsider",
            "outsider",
            ParticipantKind::Human,
        )))
        .unwrap();
    let first_source = PresenceSourceV1::new([1; 16]);
    let second_source = PresenceSourceV1::new([2; 16]);
    let mut transient = owner(4, 2);
    transient
        .publish_presence(
            &engine,
            &room,
            &human,
            first_source,
            PresenceStateV1::Available,
            TransientTimeV1(1),
            8,
        )
        .unwrap();
    transient
        .publish_presence(
            &engine,
            &room,
            &human,
            second_source,
            PresenceStateV1::Away,
            TransientTimeV1(2),
            6,
        )
        .unwrap();

    let latest = transient
        .presence_page(&engine, &agent, &room, None, 8, TransientTimeV1(3))
        .unwrap();
    let human_entry = latest
        .entries
        .iter()
        .find(|entry| entry.participant_id == human)
        .unwrap();
    assert_eq!(human_entry.state, PresenceViewStateV1::Away);
    assert_eq!(human_entry.expires_at, Some(TransientTimeV1(8)));

    let fallback = transient
        .presence_page(&engine, &agent, &room, None, 8, TransientTimeV1(8))
        .unwrap();
    let human_entry = fallback
        .entries
        .iter()
        .find(|entry| entry.participant_id == human)
        .unwrap();
    assert_eq!(human_entry.state, PresenceViewStateV1::Available);
    assert_eq!(human_entry.expires_at, Some(TransientTimeV1(9)));
    let debug = format!("{fallback:?}");
    assert!(!debug.contains("human"));
    assert!(!debug.contains("outsider"));

    assert_eq!(
        transient.publish_presence(
            &engine,
            &room,
            &outsider,
            PresenceSourceV1::new([3; 16]),
            PresenceStateV1::Busy,
            TransientTimeV1(8),
            1,
        ),
        Err(TransientRefusalV1::NotMember)
    );
    assert_eq!(
        transient.presence_page(&engine, &outsider, &room, None, 1, TransientTimeV1(8)),
        Err(TransientRefusalV1::NotMember)
    );

    let first = transient
        .presence_page(&engine, &human, &room, None, 1, TransientTimeV1(8))
        .unwrap();
    let cursor = first.next.unwrap();
    assert_eq!(
        transient.presence_page(&engine, &agent, &room, Some(&cursor), 1, TransientTimeV1(8),),
        Err(TransientRefusalV1::ForeignCursor)
    );
    engine
        .apply(EngineCommand::Leave {
            room_id: room.clone(),
            participant: agent,
        })
        .unwrap();
    assert_eq!(
        transient.presence_page(&engine, &human, &room, Some(&cursor), 1, TransientTimeV1(8),),
        Err(TransientRefusalV1::EngineChanged)
    );
}
