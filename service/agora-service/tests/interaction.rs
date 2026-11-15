use agora_engine::retention::{
    RecordClassV1, RetentionBoundV1, RetentionPolicyRevisionV1, RetentionPolicyV1,
};
use agora_engine::{
    AdmissionPolicy, CommandV2 as EngineCommand, Engine, Limits, LogicalTime, Participant,
    ParticipantKind,
};
use agora_interaction_port::{
    InteractionContextV1, InteractionInvocationIdV1, InteractionPromotionIntentV1,
    InteractionPromotionRefusalV1, InteractionPromotionRequestV1, InteractionSourceV1,
};
use agora_scheduled_publication_port::{
    ScheduledPublicationIdV1, ScheduledPublicationIntentV1, ScheduledPublicationRequestV1,
};
use agora_service::v2::{ServiceLimitsV2, ServiceV2};

fn setup() -> ServiceV2 {
    let mut engine = Engine::with_limits(Limits {
        max_retained_changes: 2,
        ..Limits::default()
    });
    for (id, kind) in [
        ("alice", ParticipantKind::Human),
        ("bot", ParticipantKind::Agent),
        ("outsider", ParticipantKind::Human),
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
            owner: "alice".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(EngineCommand::Join {
            room_id: "room".into(),
            participant: "bot".into(),
        })
        .unwrap();
    engine
        .apply(EngineCommand::Speak {
            room_id: "room".into(),
            author: "alice".into(),
            at: LogicalTime(1),
            body: "source body".to_owned(),
        })
        .unwrap();
    ServiceV2::new(engine, ServiceLimitsV2::default())
}

fn promotion(id: u8, body: &str) -> InteractionPromotionRequestV1 {
    InteractionPromotionRequestV1 {
        id: InteractionInvocationIdV1::new([id; 16]).unwrap(),
        intent: InteractionPromotionIntentV1::new(
            InteractionSourceV1 { event_sequence: 5 },
            "room",
            2,
            body,
        ),
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
fn context_is_current_member_bound_message_only_and_domain_neutral() {
    let mut service = setup();
    let alice = service.connect("alice").unwrap();
    let bot = service.connect("bot").unwrap();
    let outsider = service.connect("outsider").unwrap();
    let source = InteractionSourceV1 { event_sequence: 5 };
    let revision = service.engine().revision();
    assert_eq!(
        service.interaction_context(alice, source),
        Ok(InteractionContextV1::Available { source })
    );
    assert_eq!(
        service.interaction_context(bot, source),
        Ok(InteractionContextV1::Available { source })
    );
    assert_eq!(
        service.interaction_context(outsider, source),
        Ok(InteractionContextV1::NotVisible)
    );
    assert_eq!(
        service.interaction_context(alice, InteractionSourceV1 { event_sequence: 4 }),
        Ok(InteractionContextV1::NotVisible)
    );
    assert_eq!(
        service.interaction_context(alice, InteractionSourceV1 { event_sequence: 0 }),
        Ok(InteractionContextV1::Gap { retained_from: 4 })
    );
    assert_eq!(service.engine().revision(), revision);
}

#[test]
fn promotion_is_actor_bound_exact_and_replays_after_source_eviction() {
    let mut service = setup();
    let alice = service.connect("alice").unwrap();
    let bot = service.connect("bot").unwrap();
    let outsider = service.connect("outsider").unwrap();
    let first = service
        .promote_interaction_at(alice, promotion(1, "human proposal"), 10)
        .unwrap();
    assert_eq!(first.revision, 7);
    assert_eq!(first.message_sequence, 1);
    let second = service
        .promote_interaction_at(bot, promotion(2, "agent proposal"), 11)
        .unwrap();
    assert_eq!(second.revision, 8);
    assert_eq!(second.message_sequence, 2);
    assert_eq!(
        service.promote_interaction_at(alice, promotion(1, "human proposal"), 1),
        Ok(first),
        "exact replay resolves before source retention and Host time"
    );
    assert_eq!(
        service.promote_interaction_at(alice, promotion(1, "changed"), 12),
        Err(InteractionPromotionRefusalV1::Conflict)
    );
    assert_eq!(
        service.promote_interaction_at(outsider, promotion(3, "outsider"), 12),
        Err(InteractionPromotionRefusalV1::SourceUnavailable)
    );
    assert!(!format!("{:?}", promotion(4, "diagnostic secret")).contains("diagnostic secret"));
}

#[test]
fn interaction_promotion_continuously_enforces_the_installed_change_bound() {
    let mut service = setup();
    install_single_change_policy(&mut service);
    let alice = service.connect("alice").unwrap();

    service
        .promote_interaction_at(alice, promotion(9, "bounded proposal"), 10)
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
        6
    );
    assert_eq!(service.engine().discarded_changes_through().unwrap().0, 5);
    assert_eq!(service.engine().pending_events().len(), 7);
}

#[test]
fn scheduled_and_interaction_ids_are_source_scoped() {
    let mut service = setup();
    let alice = service.connect("alice").unwrap();
    service
        .publish_scheduled_at(
            alice,
            ScheduledPublicationRequestV1 {
                id: ScheduledPublicationIdV1::new([8; 16]).unwrap(),
                intent: ScheduledPublicationIntentV1::new("room", 2, "scheduled"),
            },
            10,
        )
        .unwrap();
    service
        .promote_interaction_at(alice, promotion(8, "interaction"), 11)
        .unwrap();
    assert_eq!(
        service
            .engine()
            .room(&"room".into())
            .unwrap()
            .messages
            .len(),
        3
    );
}
