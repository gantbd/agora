use agora_engine::retention::{
    RecordClassV1, RetentionBoundV1, RetentionPolicyRevisionV1, RetentionPolicyV1,
};
use agora_engine::{
    AdmissionPolicy, CommandV2 as EngineCommand, Engine, Participant, ParticipantKind,
};
use agora_protocol::v2::{CommandV2, ReplyV2, RequestIdV2, RequestV2};
use agora_scheduled_publication_port::{
    ScheduledPublicationIdV1, ScheduledPublicationIntentV1,
    ScheduledPublicationObservationRefusalV1, ScheduledPublicationObservationV1,
    ScheduledPublicationRefusalV1, ScheduledPublicationRequestV1,
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

fn scheduled(id: u8, body: &str) -> ScheduledPublicationRequestV1 {
    ScheduledPublicationRequestV1 {
        id: ScheduledPublicationIdV1::new([id; 16]).unwrap(),
        intent: ScheduledPublicationIntentV1::new("room", 42, body),
    }
}

fn install_single_change_policy(service: &mut ServiceV2) {
    let mut bounds = [RetentionBoundV1::Disabled; 6];
    bounds[RecordClassV1::AuthoritativeChanges as usize - 1] =
        RetentionBoundV1::entries(1).unwrap();
    service
        .install_retention_policy(RetentionPolicyV1::new(
            RetentionPolicyRevisionV1::new(1).unwrap(),
            service.engine().revision().0,
            bounds,
        ))
        .unwrap();
}

#[test]
fn publication_is_actor_bound_exactly_replayable_and_conflict_safe() {
    let mut service = setup();
    let agent = service.connect("agent").unwrap();
    let first = service
        .publish_scheduled_at(agent, scheduled(1, "scheduled"), 1_000)
        .unwrap();
    assert_eq!(first.revision, 5);
    assert_eq!(first.message_sequence, 0);
    assert_eq!(
        service
            .engine()
            .message(&"room".into(), 0)
            .unwrap()
            .author(),
        &"agent".into()
    );
    assert_eq!(
        service.publish_scheduled_at(agent, scheduled(1, "scheduled"), 1),
        Ok(first)
    );
    assert_eq!(service.engine().revision().0, 5);
    assert_eq!(
        service.publish_scheduled_at(agent, scheduled(1, "changed"), 1_001),
        Err(ScheduledPublicationRefusalV1::Conflict)
    );
    assert_eq!(
        service.publish_scheduled_at(
            agora_protocol::v2::ClientIdV2(u64::MAX),
            scheduled(2, "x"),
            1_002
        ),
        Err(ScheduledPublicationRefusalV1::NotAuthorized)
    );
}

#[test]
fn retained_observation_is_read_only_actor_bound_and_authoritative() {
    let mut service = setup();
    let agent = service.connect("agent").unwrap();
    let human = service.connect("human").unwrap();
    let request = scheduled(7, "observed");

    assert_eq!(
        service.observe_scheduled(agent, request.clone()),
        Ok(ScheduledPublicationObservationV1::Absent)
    );
    let accepted = service
        .publish_scheduled_at(agent, request.clone(), 1_000)
        .unwrap();
    let revision = service.engine().revision();
    assert_eq!(
        service.observe_scheduled(agent, request.clone()),
        Ok(ScheduledPublicationObservationV1::Accepted(accepted))
    );
    assert_eq!(service.engine().revision(), revision);
    assert_eq!(
        service.observe_scheduled(agent, scheduled(7, "changed")),
        Err(ScheduledPublicationObservationRefusalV1::Conflict)
    );
    assert_eq!(
        service.observe_scheduled(human, request),
        Err(ScheduledPublicationObservationRefusalV1::Conflict)
    );
}

#[test]
fn scheduled_publication_continuously_enforces_the_installed_change_bound() {
    let mut service = setup();
    install_single_change_policy(&mut service);
    let agent = service.connect("agent").unwrap();

    service
        .publish_scheduled_at(agent, scheduled(9, "bounded"), 1_000)
        .unwrap();

    assert_eq!(service.engine().retained_changes().len(), 1);
    assert_eq!(
        service
            .engine()
            .retained_changes()
            .next()
            .unwrap()
            .sequence
            .0,
        4
    );
    assert_eq!(service.engine().discarded_changes_through().unwrap().0, 3);
    assert_eq!(service.engine().pending_events().len(), 5);
}

#[test]
fn current_admission_controls_are_rechecked_at_dispatch() {
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
            .publish_scheduled_at(agent, scheduled(2, "first"), 1_000)
            .is_ok()
    );
    assert_eq!(
        service.publish_scheduled_at(agent, scheduled(3, "early"), 1_050),
        Err(ScheduledPublicationRefusalV1::AdmissionDeferred { retry_after_ms: 50 })
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
        service.publish_scheduled_at(agent, scheduled(4, "blocked"), 1_100),
        Err(ScheduledPublicationRefusalV1::NotAuthorized)
    );
}
