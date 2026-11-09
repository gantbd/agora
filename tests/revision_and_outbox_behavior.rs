use agora_engine::{
    AcknowledgeError, CommandV1, DomainEventV1, Engine, EventSequence, Limits, Participant,
    ParticipantKind, Refusal, Revision, RevisionedCommandV1,
};

fn registration(id: &str) -> CommandV1 {
    CommandV1::RegisterParticipant(Participant::new(id, id, ParticipantKind::Human))
}

#[test]
fn stale_revision_refuses_without_mutating_or_publishing() {
    let mut engine = Engine::new();
    let accepted = engine
        .apply_revisioned(RevisionedCommandV1 {
            expected_revision: Revision(0),
            command: registration("alice"),
        })
        .unwrap();
    assert_eq!(accepted.revision, Revision(1));
    assert_eq!(accepted.event_sequence, EventSequence(0));

    assert_eq!(
        engine.apply_revisioned(RevisionedCommandV1 {
            expected_revision: Revision(0),
            command: registration("bob"),
        }),
        Err(Refusal::RevisionConflict {
            expected: Revision(0),
            actual: Revision(1),
        })
    );
    assert_eq!(engine.revision(), Revision(1));
    assert_eq!(engine.participants().len(), 1);
    assert_eq!(engine.pending_events().len(), 1);
}

#[test]
fn full_outbox_backpressures_before_state_mutation_and_ack_releases_capacity() {
    let mut engine = Engine::with_limits(Limits {
        max_pending_events: 1,
        ..Limits::default()
    });
    engine.apply(registration("alice")).unwrap();

    assert_eq!(
        engine.apply(registration("bob")),
        Err(Refusal::EventOutboxFull)
    );
    assert_eq!(engine.revision(), Revision(1));
    assert_eq!(engine.participants().len(), 1);

    assert_eq!(engine.acknowledge_events(EventSequence(0)), Ok(1));
    assert_eq!(engine.acknowledge_events(EventSequence(0)), Ok(0));
    let accepted = engine.apply(registration("bob")).unwrap();
    assert!(matches!(
        accepted,
        agora_engine::OutcomeV1::ParticipantRegistered(_)
    ));
    let event = engine.pending_events().next().unwrap();
    assert_eq!(event.sequence, EventSequence(1));
    assert_eq!(event.revision, Revision(2));
    assert!(matches!(
        event.event,
        DomainEventV1::ParticipantRegistered { .. }
    ));
}

#[test]
fn acknowledgement_cannot_skip_beyond_the_published_edge() {
    let mut engine = Engine::new();
    assert_eq!(
        engine.acknowledge_events(EventSequence(0)),
        Err(AcknowledgeError::BeyondPublished {
            requested: EventSequence(0),
            last_published: None,
        })
    );
    engine.apply(registration("alice")).unwrap();
    assert_eq!(
        engine.acknowledge_events(EventSequence(1)),
        Err(AcknowledgeError::BeyondPublished {
            requested: EventSequence(1),
            last_published: Some(EventSequence(0)),
        })
    );
    assert_eq!(engine.pending_events().len(), 1);
}

#[test]
fn acknowledgement_cursor_is_monotone() {
    let mut engine = Engine::new();
    engine.apply(registration("alice")).unwrap();
    engine.apply(registration("bob")).unwrap();
    engine.acknowledge_events(EventSequence(1)).unwrap();

    assert_eq!(
        engine.acknowledge_events(EventSequence(0)),
        Err(AcknowledgeError::Regressed {
            requested: EventSequence(0),
            acknowledged_through: EventSequence(1),
        })
    );
    assert_eq!(engine.acknowledged_through(), Some(EventSequence(1)));
}
