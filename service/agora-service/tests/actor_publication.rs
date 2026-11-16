use agora_actor_publication_port::{
    ActorPublicationIdV1, ActorPublicationIntentV1, ActorPublicationObservationRefusalV1,
    ActorPublicationObservationV1, ActorPublicationRefusalV1, ActorPublicationRequestV1,
};
use agora_engine::{
    AdmissionPolicy, CommandV2 as EngineCommand, Engine, Participant, ParticipantKind,
};
use agora_service::v2::{ServiceLimitsV2, ServiceV2};

fn setup() -> ServiceV2 {
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
            name: "Room".into(),
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
    ServiceV2::new(engine, ServiceLimitsV2::default())
}

fn request(id: u8, body: &str, addressed_to: Vec<String>) -> ActorPublicationRequestV1 {
    ActorPublicationRequestV1 {
        id: ActorPublicationIdV1::new([id; 16]).unwrap(),
        intent: ActorPublicationIntentV1::new("room", 4, 42, body, addressed_to),
    }
}

#[test]
fn stable_actor_operation_replays_across_client_sessions_with_exact_semantics() {
    let mut service = setup();
    let first_client = service.connect("agent").unwrap();
    let exact = request(1, "answer", vec!["human".into()]);
    let accepted = service
        .publish_actor_at(first_client, exact.clone(), 1_000)
        .unwrap();
    assert_eq!(accepted.revision, 5);
    assert_eq!(accepted.message_sequence, 0);
    assert_eq!(
        service
            .engine()
            .message(&"room".into(), 0)
            .unwrap()
            .addressed_to(),
        ["human".into()]
    );

    service.disconnect(first_client).unwrap();
    let reconnected = service.connect("agent").unwrap();
    assert_eq!(
        service.observe_actor(reconnected, exact.clone()),
        Ok(ActorPublicationObservationV1::Accepted(accepted))
    );
    assert_eq!(
        service.publish_actor_at(reconnected, exact, 1),
        Ok(accepted)
    );
    assert_eq!(service.engine().revision().0, 5);

    assert_eq!(
        service.observe_actor(reconnected, request(1, "answer", Vec::new())),
        Err(ActorPublicationObservationRefusalV1::Conflict)
    );
    assert_eq!(
        service.publish_actor_at(
            reconnected,
            request(1, "changed", vec!["human".into()]),
            1_001,
        ),
        Err(ActorPublicationRefusalV1::Conflict)
    );
}

#[test]
fn observation_absence_is_read_only_and_actor_scoped() {
    let mut service = setup();
    let agent = service.connect("agent").unwrap();
    let human = service.connect("human").unwrap();
    let exact = request(2, "answer", vec!["human".into()]);
    let revision = service.engine().revision();
    assert_eq!(
        service.observe_actor(agent, exact.clone()),
        Ok(ActorPublicationObservationV1::Absent)
    );
    assert_eq!(service.engine().revision(), revision);
    service
        .publish_actor_at(agent, exact.clone(), 1_000)
        .unwrap();
    assert_eq!(
        service.observe_actor(human, exact),
        Err(ActorPublicationObservationRefusalV1::Conflict)
    );
}
