use agora_engine::{
    AdmissionPolicy, CommandV2 as EngineCommand, Engine, Participant, ParticipantKind,
};
use agora_federated_publication_port::{
    FederatedPublicationIdV1, FederatedPublicationIntentV1, FederatedPublicationRefusalV1,
    FederatedPublicationRequestV1,
};
use agora_protocol::v2::{CommandV2, ReplyV2, RequestIdV2, RequestV2};
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
    ServiceV2::new(engine, ServiceLimitsV2::default())
}

fn federated(id: u8, body: &str) -> FederatedPublicationRequestV1 {
    FederatedPublicationRequestV1 {
        id: FederatedPublicationIdV1::new([id; 16]).unwrap(),
        intent: FederatedPublicationIntentV1::new("room", body),
    }
}

#[test]
fn publication_is_actor_bound_host_timed_and_exactly_replayable() {
    let mut service = setup();
    let agent = service.connect("agent").unwrap();
    let first = service
        .publish_federated_at(agent, federated(1, "federated"), 1_000)
        .unwrap();
    assert_eq!(first.revision, 5);
    assert_eq!(first.event_sequence, 4);
    assert_eq!(first.message_sequence, 0);
    let message = service.engine().message(&"room".into(), 0).unwrap();
    assert_eq!(message.author(), &"agent".into());
    assert_eq!(message.at().0, 1_000);
    assert_eq!(
        service.publish_federated_at(agent, federated(1, "federated"), 1),
        Ok(first)
    );
    assert_eq!(service.engine().revision().0, 5);
    assert_eq!(
        service.publish_federated_at(agent, federated(1, "changed"), 1_001),
        Err(FederatedPublicationRefusalV1::Conflict)
    );
    assert_eq!(
        service.publish_federated_at(
            agora_protocol::v2::ClientIdV2(u64::MAX),
            federated(2, "x"),
            1_002,
        ),
        Err(FederatedPublicationRefusalV1::NotAuthorized)
    );
}

#[test]
fn current_admission_controls_are_rechecked_before_first_publication() {
    let mut service = setup();
    let human = service.connect("human").unwrap();
    let agent = service.connect("agent").unwrap();
    assert!(matches!(
        service.execute(
            human,
            RequestV2 {
                request_id: RequestIdV2(1),
                command: CommandV2::SetMinimumMessageInterval {
                    room_id: "room".to_owned(),
                    minimum_interval_ms: 100,
                    expected_revision: 4,
                },
            },
        ),
        ReplyV2::Accepted(_)
    ));
    assert!(
        service
            .publish_federated_at(agent, federated(2, "first"), 1_000)
            .is_ok()
    );
    assert_eq!(
        service.publish_federated_at(agent, federated(3, "early"), 1_050),
        Err(FederatedPublicationRefusalV1::AdmissionDeferred { retry_after_ms: 50 })
    );
    assert!(matches!(
        service.execute(
            human,
            RequestV2 {
                request_id: RequestIdV2(2),
                command: CommandV2::SetPostingRestricted {
                    room_id: "room".to_owned(),
                    participant_id: "agent".to_owned(),
                    restricted: true,
                    reason: "review".to_owned(),
                    causation: [9; 16],
                    expected_revision: 6,
                },
            },
        ),
        ReplyV2::Accepted(_)
    ));
    assert_eq!(
        service.publish_federated_at(agent, federated(4, "blocked"), 1_100),
        Err(FederatedPublicationRefusalV1::NotAuthorized)
    );
}
