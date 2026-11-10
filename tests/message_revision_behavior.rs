use agora_engine::{
    AdmissionPolicy, CommandV2, DomainEventV2, Engine, LogicalTime, MAX_MESSAGE_REVISIONS_V1,
    MessageRevisionAvailabilityV1, ModerationCausationV1, ModerationReasonV1, ObservationRefusal,
    OutcomeV2, Participant, ParticipantId, ParticipantKind, Refusal, RevisionedCommandV2, RoomId,
};

fn setup() -> (Engine, agora_engine::MessageId) {
    let mut engine = Engine::new();
    for (id, kind) in [
        ("human", ParticipantKind::Human),
        ("agent", ParticipantKind::Agent),
    ] {
        engine
            .apply(CommandV2::RegisterParticipant(Participant::new(
                id, id, kind,
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
    let OutcomeV2::MessageAccepted { sequence, .. } = engine
        .apply(CommandV2::SpeakAddressed {
            room_id: "room".into(),
            author: "agent".into(),
            at: LogicalTime(10),
            body: "original secret".to_owned(),
            addressed_to: vec![ParticipantId::new("human")],
        })
        .unwrap()
    else {
        panic!("message acceptance expected");
    };
    let id = engine.message(&RoomId::new("room"), sequence).unwrap().id();
    (engine, id)
}

fn edit(engine: &mut Engine, id: agora_engine::MessageId, body: &str, at: u64) -> OutcomeV2 {
    engine
        .apply(CommandV2::EditMessage {
            room_id: "room".into(),
            actor: "agent".into(),
            message_id: id,
            at: LogicalTime(at),
            body: body.to_owned(),
        })
        .unwrap()
}

#[test]
fn edits_are_author_only_append_only_and_preserve_message_relations() {
    let (mut engine, id) = setup();
    engine
        .apply(CommandV2::SetReaction {
            room_id: "room".into(),
            actor: "human".into(),
            message_id: id,
            key: "spark".to_owned(),
            reacted: true,
        })
        .unwrap();
    let before = engine
        .message_by_id(&RoomId::new("room"), id)
        .unwrap()
        .clone();
    let OutcomeV2::MessageEdited {
        message_revision,
        at,
        ..
    } = edit(&mut engine, id, "corrected secret", 11)
    else {
        panic!("edit acceptance expected");
    };
    assert_eq!(message_revision, 1);
    assert_eq!(at, LogicalTime(11));
    let after = engine.message_by_id(&RoomId::new("room"), id).unwrap();
    assert_eq!(after.id(), before.id());
    assert_eq!(after.sequence(), before.sequence());
    assert_eq!(after.author(), before.author());
    assert_eq!(after.at(), before.at());
    assert_eq!(after.addressed_to(), before.addressed_to());
    assert_eq!(after.reply_to_root(), before.reply_to_root());
    assert_eq!(after.body(), "corrected secret");
    assert_eq!(after.current_revision(), 1);
    assert_eq!(
        engine
            .reaction_status(&ParticipantId::new("human"), &RoomId::new("room"), id)
            .unwrap()
            .reactions[0]
            .count,
        1
    );

    let revision = engine.revision();
    let events = engine.pending_events().count();
    for command in [
        CommandV2::EditMessage {
            room_id: "room".into(),
            actor: "human".into(),
            message_id: id,
            at: LogicalTime(12),
            body: "not mine".to_owned(),
        },
        CommandV2::EditMessage {
            room_id: "room".into(),
            actor: "agent".into(),
            message_id: id,
            at: LogicalTime(12),
            body: "corrected secret".to_owned(),
        },
    ] {
        assert!(matches!(
            engine.apply_revisioned(RevisionedCommandV2 {
                expected_revision: revision,
                command,
            }),
            Err(Refusal::NotAuthorized(_)) | Err(Refusal::InvalidMessageEdit)
        ));
        assert_eq!(engine.revision(), revision);
        assert_eq!(engine.pending_events().count(), events);
    }
}

#[test]
fn revision_pages_are_bounded_bound_and_stale() {
    let (mut engine, id) = setup();
    edit(&mut engine, id, "second", 11);
    edit(&mut engine, id, "third", 12);
    let observer = ParticipantId::new("human");
    let room = RoomId::new("room");
    let first = engine
        .message_revision_page(&observer, &room, id, None, 2)
        .unwrap();
    assert_eq!(first.revisions.len(), 2);
    assert_eq!(first.revisions[0].number, 0);
    assert_eq!(first.revisions[1].number, 1);
    let cursor = first.next.as_ref().unwrap();
    assert_eq!(
        engine.message_revision_page(&ParticipantId::new("agent"), &room, id, Some(cursor), 2),
        Err(ObservationRefusal::ForeignCursor)
    );
    let revision = engine.revision();
    edit(&mut engine, id, "fourth", 13);
    assert_eq!(
        engine.message_revision_page(&observer, &room, id, Some(cursor), 2),
        Err(ObservationRefusal::Stale {
            actual_revision: engine.revision(),
        })
    );
    assert!(engine.revision() > revision);
}

#[test]
fn tombstones_scrub_revision_content_but_preserve_metadata_and_events() {
    let (mut engine, id) = setup();
    edit(&mut engine, id, "edited secret", 11);
    engine
        .apply(CommandV2::WithdrawMessage {
            room_id: "room".into(),
            actor: "agent".into(),
            message_id: id,
        })
        .unwrap();
    let page = engine
        .message_revision_page(
            &ParticipantId::new("human"),
            &RoomId::new("room"),
            id,
            None,
            MAX_MESSAGE_REVISIONS_V1,
        )
        .unwrap();
    assert_eq!(page.revisions.len(), 2);
    assert!(page.revisions.iter().all(|revision| {
        revision.body.is_empty()
            && revision.availability == MessageRevisionAvailabilityV1::Tombstoned
    }));
    assert_eq!(page.revisions[0].at, LogicalTime(10));
    assert_eq!(page.revisions[1].at, LogicalTime(11));
    assert!(
        engine
            .pending_events()
            .all(|envelope| match &envelope.event {
                DomainEventV2::MessageAccepted { message, .. } if message.id() == id => {
                    message.body().is_empty()
                }
                DomainEventV2::MessageEdited {
                    message_id, body, ..
                } if *message_id == id => body.is_empty(),
                _ => true,
            })
    );
    assert_eq!(
        engine.apply(CommandV2::EditMessage {
            room_id: "room".into(),
            actor: "agent".into(),
            message_id: id,
            at: LogicalTime(12),
            body: "resurrect".to_owned(),
        }),
        Err(Refusal::InvalidMessageEdit)
    );
}

#[test]
fn revision_history_has_a_fixed_atomic_ceiling() {
    let (mut engine, id) = setup();
    for number in 1..MAX_MESSAGE_REVISIONS_V1 {
        let body = format!("revision-{number}");
        let OutcomeV2::MessageEdited {
            message_revision, ..
        } = edit(&mut engine, id, &body, 10 + number as u64)
        else {
            panic!("edit acceptance expected");
        };
        assert_eq!(message_revision, number as u32);
    }
    let revision = engine.revision();
    let events = engine.pending_events().count();
    assert_eq!(
        engine.apply(CommandV2::EditMessage {
            room_id: "room".into(),
            actor: "agent".into(),
            message_id: id,
            at: LogicalTime(100),
            body: "one-too-many".to_owned(),
        }),
        Err(Refusal::MessageRevisionLimit)
    );
    assert_eq!(engine.revision(), revision);
    assert_eq!(engine.pending_events().count(), events);
    let message = engine.message_by_id(&RoomId::new("room"), id).unwrap();
    assert_eq!(message.revisions().len(), MAX_MESSAGE_REVISIONS_V1);
    assert_eq!(
        message.current_revision(),
        (MAX_MESSAGE_REVISIONS_V1 - 1) as u32
    );
}

#[test]
fn moderator_redaction_scrubs_every_revision_body() {
    let (mut engine, id) = setup();
    edit(&mut engine, id, "edited secret", 11);
    engine
        .apply(CommandV2::RedactMessage {
            room_id: "room".into(),
            actor: "human".into(),
            message_id: id,
            reason: ModerationReasonV1::new("policy"),
            causation: ModerationCausationV1::new([0x55; 16]),
        })
        .unwrap();
    let page = engine
        .message_revision_page(
            &ParticipantId::new("human"),
            &RoomId::new("room"),
            id,
            None,
            MAX_MESSAGE_REVISIONS_V1,
        )
        .unwrap();
    assert_eq!(page.revisions.len(), 2);
    assert!(page.revisions.iter().all(|revision| {
        revision.availability == MessageRevisionAvailabilityV1::Tombstoned
            && revision.body.is_empty()
    }));
    assert!(engine.pending_events().all(|event| match &event.event {
        DomainEventV2::MessageEdited {
            message_id, body, ..
        } if *message_id == id => {
            body.is_empty()
        }
        _ => true,
    }));
}
