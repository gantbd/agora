use agora_delivery_projection_port::{
    DeliveryProjectionRefusalV1, DeliveryProjectionV1, DeliverySourceV1, WakeClassV1,
};
use agora_engine::{
    AdmissionPolicy, CommandV2 as EngineCommand, Engine, Limits, LogicalTime, Participant,
    ParticipantKind,
};
use agora_service::v2::{ServiceLimitsV2, ServiceV2};

fn setup() -> ServiceV2 {
    let mut engine = Engine::with_limits(Limits {
        max_retained_changes: 5,
        ..Limits::default()
    });
    for id in ["alice", "bob", "outsider"] {
        engine
            .apply(EngineCommand::RegisterParticipant(Participant::new(
                id,
                id,
                ParticipantKind::Human,
            )))
            .unwrap();
    }
    engine
        .apply(EngineCommand::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
            owner: "alice".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(EngineCommand::Join {
            room_id: "room".into(),
            participant: "bob".into(),
        })
        .unwrap();
    engine
        .apply(EngineCommand::Speak {
            room_id: "room".into(),
            author: "alice".into(),
            at: LogicalTime(1),
            body: "secret body".to_owned(),
        })
        .unwrap();
    ServiceV2::new(engine, ServiceLimitsV2::default())
}

#[test]
fn projection_is_current_member_bound_content_minimized_and_state_neutral() {
    let mut service = setup();
    let alice = service.connect("alice").unwrap();
    let bob = service.connect("bob").unwrap();
    let outsider = service.connect("outsider").unwrap();
    let revision = service.engine().revision();
    let events = service.engine().pending_events().count();
    let source = DeliverySourceV1 { event_sequence: 5 };
    assert_eq!(
        service.project_delivery(alice, source),
        Ok(DeliveryProjectionV1::Visible(
            agora_delivery_projection_port::WakeProjectionV1 {
                source,
                class: WakeClassV1::ConversationActivity,
            }
        ))
    );
    assert!(matches!(
        service.project_delivery(bob, source),
        Ok(DeliveryProjectionV1::Visible(_))
    ));
    assert_eq!(
        service.project_delivery(outsider, source),
        Ok(DeliveryProjectionV1::NotVisible)
    );
    assert_eq!(service.engine().revision(), revision);
    assert_eq!(service.engine().pending_events().count(), events);
    assert!(!format!("{:?}", service.project_delivery(alice, source)).contains("secret body"));
}

#[test]
fn projection_reports_retention_gap_nonroom_event_and_unknown_client() {
    let mut service = setup();
    let alice = service.connect("alice").unwrap();
    assert_eq!(
        service.project_delivery(alice, DeliverySourceV1 { event_sequence: 0 }),
        Ok(DeliveryProjectionV1::Gap { retained_from: 1 })
    );
    assert_eq!(
        service.project_delivery(alice, DeliverySourceV1 { event_sequence: 2 }),
        Ok(DeliveryProjectionV1::NotVisible)
    );
    assert_eq!(
        service.project_delivery(alice, DeliverySourceV1 { event_sequence: 99 }),
        Ok(DeliveryProjectionV1::Unavailable)
    );
    assert_eq!(
        service.project_delivery(
            agora_protocol::v2::ClientIdV2(u64::MAX),
            DeliverySourceV1 { event_sequence: 5 },
        ),
        Err(DeliveryProjectionRefusalV1::NotAuthorized)
    );
}
