use agora_engine::persistence::{CursorRecoveryLimitsV1, RecoveryLimits};
use agora_engine::{
    AdmissionPolicy, CommandV2, DomainEventV2, Engine, EventSequence, Limits, LogicalTime,
    ObservationRefusal, Participant, ParticipantId, ParticipantKind, Revision, RoomChangeGap,
    RoomChangeResult, RoomId,
};

fn engine_with_change_limit(max_retained_changes: usize) -> Engine {
    let mut engine = Engine::with_limits(Limits {
        max_rooms: 4,
        max_members_per_room: 4,
        max_pending_events: 64,
        max_retained_changes,
        ..Limits::default()
    });
    for (id, kind) in [
        ("alice", ParticipantKind::Human),
        ("bot", ParticipantKind::Agent),
        ("outsider", ParticipantKind::Human),
    ] {
        engine
            .apply(CommandV2::RegisterParticipant(Participant::new(
                id, id, kind,
            )))
            .unwrap();
    }
    for room in ["alpha", "beta"] {
        engine
            .apply(CommandV2::CreateRoom {
                room_id: room.into(),
                name: room.to_owned(),
                owner: "alice".into(),
                admission: AdmissionPolicy::Open,
            })
            .unwrap();
    }
    engine
}

fn engine() -> Engine {
    engine_with_change_limit(32)
}

fn speak(engine: &mut Engine, room: &str, body: &str) {
    engine
        .apply(CommandV2::Speak {
            room_id: room.into(),
            author: "alice".into(),
            at: LogicalTime(engine.revision().0),
            body: body.to_owned(),
        })
        .unwrap();
}

#[test]
fn feed_is_exclusive_replayable_room_filtered_and_bounded() {
    let mut engine = engine();
    let alice = ParticipantId::from("alice");
    let alpha = RoomId::from("alpha");
    let position = engine
        .room_change_position(&alice, &alpha, engine.revision())
        .unwrap();
    let position_checkpoint = position.prepare_checkpoint(1_024).unwrap();
    let cursor_bounds = CursorRecoveryLimitsV1 {
        max_snapshot_bytes: 1_024,
        max_participant_id_bytes: 128,
        max_room_id_bytes: 128,
    };
    let position_after_checkpoint = agora_engine::RoomChangePosition::recover_checkpoint(
        position_checkpoint.bytes(),
        cursor_bounds,
    )
    .unwrap();
    assert_eq!(position_after_checkpoint, position);
    for end in 0..position_checkpoint.bytes().len() {
        assert!(
            agora_engine::RoomChangePosition::recover_checkpoint(
                &position_checkpoint.bytes()[..end],
                cursor_bounds,
            )
            .is_err()
        );
    }

    engine
        .apply(CommandV2::Invite {
            room_id: alpha.clone(),
            actor: alice.clone(),
            participant: "bot".into(),
        })
        .unwrap();
    speak(&mut engine, "beta", "not visible in alpha");
    engine
        .apply(CommandV2::Join {
            room_id: alpha.clone(),
            participant: "bot".into(),
        })
        .unwrap();

    let RoomChangeResult::Page(first) = engine
        .room_change_page(&alice, &alpha, &position, 1)
        .unwrap()
    else {
        panic!("retained prefix must be available");
    };
    assert_eq!(first.changes.len(), 1);
    assert!(first.has_more);
    assert!(matches!(
        first.changes[0].event,
        DomainEventV2::InvitationRecorded { .. }
    ));
    assert_eq!(
        engine
            .room_change_page(&alice, &alpha, &position, 1)
            .unwrap(),
        RoomChangeResult::Page(first.clone()),
        "reusing an exclusive position must replay the same retained page"
    );

    let RoomChangeResult::Page(second) = engine
        .room_change_page(&alice, &alpha, &first.next, 1)
        .unwrap()
    else {
        panic!("second page expected");
    };
    assert_eq!(second.changes.len(), 1);
    assert!(!second.has_more);
    assert!(matches!(
        second.changes[0].event,
        DomainEventV2::ParticipantJoined { .. }
    ));
    assert!(first.changes[0].sequence < second.changes[0].sequence);

    let RoomChangeResult::Page(at_edge) = engine
        .room_change_page(&alice, &alpha, &second.next, 1)
        .unwrap()
    else {
        panic!("edge page expected");
    };
    assert!(at_edge.changes.is_empty());
    assert!(!at_edge.has_more);
    let debug = format!("{first:?}");
    assert!(!debug.contains("alice"));
    assert!(!debug.contains("alpha"));
    assert!(!debug.contains("bot"));
}

#[test]
fn delivery_acknowledgement_is_independent_and_change_eviction_is_an_explicit_gap() {
    let mut engine = engine_with_change_limit(1);
    let alice = ParticipantId::from("alice");
    let alpha = RoomId::from("alpha");
    let position = engine
        .room_change_position(&alice, &alpha, engine.revision())
        .unwrap();
    speak(&mut engine, "alpha", "change");
    let acknowledged = engine.pending_events().last().unwrap().sequence;
    engine.acknowledge_events(acknowledged).unwrap();
    assert_eq!(engine.pending_events().len(), 0);
    let RoomChangeResult::Page(retained) = engine
        .room_change_page(&alice, &alpha, &position, 1)
        .unwrap()
    else {
        panic!("delivery acknowledgement must not consume pull history");
    };
    assert_eq!(retained.changes.len(), 1);

    let checkpoint = engine.prepare_checkpoint(1_000_000).unwrap();
    engine = Engine::recover_checkpoint(
        checkpoint.bytes(),
        RecoveryLimits {
            max_snapshot_bytes: 1_000_000,
            maximum_engine_limits: Limits::default(),
        },
    )
    .unwrap();
    assert_eq!(engine.pending_events().len(), 0);
    assert_eq!(engine.retained_changes().len(), 1);
    assert!(matches!(
        engine
            .room_change_page(&alice, &alpha, &position, 1)
            .unwrap(),
        RoomChangeResult::Page(page) if page.changes.len() == 1
    ));

    speak(&mut engine, "beta", "evicts the retained alpha change");
    let discarded = engine.discarded_changes_through().unwrap();
    assert_eq!(
        engine
            .room_change_page(&alice, &alpha, &position, 1)
            .unwrap(),
        RoomChangeResult::Gap {
            cause: RoomChangeGap::ChangeRetention {
                discarded_through: discarded
            },
            current_revision: engine.revision(),
        }
    );

    assert_eq!(
        engine.room_change_position(&alice, &alpha, Revision(engine.revision().0 - 1)),
        Err(ObservationRefusal::Stale {
            actual_revision: engine.revision()
        })
    );
    let resnapshot = engine
        .room_change_position(&alice, &alpha, engine.revision())
        .unwrap();
    assert!(matches!(
        engine
            .room_change_page(&alice, &alpha, &resnapshot, 1)
            .unwrap(),
        RoomChangeResult::Page(page) if page.changes.is_empty()
    ));
}

#[test]
fn old_position_cannot_cross_a_leave_and_rejoin_visibility_epoch() {
    let mut engine = engine();
    let alice = ParticipantId::from("alice");
    let bot = ParticipantId::from("bot");
    let alpha = RoomId::from("alpha");
    engine
        .apply(CommandV2::Join {
            room_id: alpha.clone(),
            participant: bot.clone(),
        })
        .unwrap();
    let position = engine
        .room_change_position(&bot, &alpha, engine.revision())
        .unwrap();
    engine
        .apply(CommandV2::Leave {
            room_id: alpha.clone(),
            participant: bot.clone(),
        })
        .unwrap();
    speak(&mut engine, "alpha", "not visible while absent");
    engine
        .apply(CommandV2::Join {
            room_id: alpha.clone(),
            participant: bot.clone(),
        })
        .unwrap();

    assert_eq!(
        engine.room_change_page(&bot, &alpha, &position, 8).unwrap(),
        RoomChangeResult::Gap {
            cause: RoomChangeGap::MembershipChanged,
            current_revision: engine.revision(),
        }
    );
    let current = engine
        .room_change_position(&bot, &alpha, engine.revision())
        .unwrap();
    assert!(matches!(
        engine.room_change_page(&bot, &alpha, &current, 8).unwrap(),
        RoomChangeResult::Page(page) if page.changes.is_empty()
    ));
    assert!(
        engine
            .room_change_position(&alice, &alpha, engine.revision())
            .is_ok()
    );
}

#[test]
fn positions_are_membership_bound_and_survive_engine_recovery() {
    let mut engine = engine();
    let alice = ParticipantId::from("alice");
    let outsider = ParticipantId::from("outsider");
    let alpha = RoomId::from("alpha");
    let beta = RoomId::from("beta");
    let position = engine
        .room_change_position(&alice, &alpha, engine.revision())
        .unwrap();
    let foreign = engine
        .room_change_position(&alice, &beta, engine.revision())
        .unwrap();
    assert_eq!(
        engine.room_change_page(&alice, &alpha, &foreign, 1),
        Err(ObservationRefusal::ForeignCursor)
    );
    assert_eq!(
        engine.room_change_page(&outsider, &alpha, &position, 1),
        Err(ObservationRefusal::NotMember)
    );
    assert_eq!(
        engine.room_change_page(&alice, &alpha, &position, 0),
        Err(ObservationRefusal::InvalidLimit)
    );

    speak(&mut engine, "alpha", "recoverable");
    let expected_sequence = engine.pending_events().last().unwrap().sequence;
    let checkpoint = engine.prepare_checkpoint(1_000_000).unwrap();
    let recovered = Engine::recover_checkpoint(
        checkpoint.bytes(),
        RecoveryLimits {
            max_snapshot_bytes: 1_000_000,
            maximum_engine_limits: Limits::default(),
        },
    )
    .unwrap();
    let RoomChangeResult::Page(page) = recovered
        .room_change_page(&alice, &alpha, &position, 4)
        .unwrap()
    else {
        panic!("position must remain meaningful after canonical recovery");
    };
    assert_eq!(page.changes.len(), 1);
    assert_eq!(page.changes[0].sequence, expected_sequence);
    assert_eq!(page.next.revision(), recovered.revision());
    assert_ne!(expected_sequence, EventSequence(0));
}
