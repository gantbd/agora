use agora_engine::{
    AdmissionPolicy, CommandV2 as EngineCommand, Engine, Participant, ParticipantKind,
};
use agora_protocol::v2::{
    AdmissionPolicyV2, ChangeGapV2, ClientIdV2, CommandV2, OutcomeV2, RefusalV2, ReplyV2,
    RequestIdV2, RequestV2, RoleV2, RoomEventV2,
};
use agora_service::v2::{ServiceLimitsV2, ServiceV2};

fn request(id: u64, command: CommandV2) -> RequestV2 {
    RequestV2 {
        request_id: RequestIdV2(id),
        command,
    }
}

fn participant_engine() -> Engine {
    let mut engine = Engine::new();
    for (id, name, kind) in [
        ("alice", "Alice", ParticipantKind::Human),
        ("bot", "Bot", ParticipantKind::Agent),
        ("carol", "Carol", ParticipantKind::Human),
        ("outsider", "Outsider", ParticipantKind::Agent),
    ] {
        engine
            .apply(EngineCommand::RegisterParticipant(Participant::new(
                id, name, kind,
            )))
            .unwrap();
    }
    engine
}

#[test]
fn v2_closes_the_participant_bound_room_lifecycle() {
    let mut service = ServiceV2::new(participant_engine(), ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    let bot = service.connect("bot").unwrap();
    let carol = service.connect("carol").unwrap();

    assert_eq!(
        service.execute(
            alice,
            request(
                0,
                CommandV2::CreateRoom {
                    room_id: "private".to_owned(),
                    name: "Private".to_owned(),
                    admission: AdmissionPolicyV2::InviteOnly,
                    expected_revision: 4,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::RoomCreated { revision: 5 })
    );
    assert_eq!(
        service.execute(
            bot,
            request(
                0,
                CommandV2::Join {
                    room_id: "private".to_owned(),
                    expected_revision: 5,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::NotAuthorized)
    );
    assert_eq!(service.engine().revision().0, 5);

    assert_eq!(
        service.execute(
            alice,
            request(
                1,
                CommandV2::Invite {
                    room_id: "private".to_owned(),
                    participant_id: "bot".to_owned(),
                    expected_revision: 5,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::InvitationRecorded { revision: 6 })
    );
    assert_eq!(
        service.execute(
            bot,
            request(
                1,
                CommandV2::Join {
                    room_id: "private".to_owned(),
                    expected_revision: 6,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::Joined { revision: 7 })
    );
    assert_eq!(
        service.execute(
            alice,
            request(
                2,
                CommandV2::SetRole {
                    room_id: "private".to_owned(),
                    participant_id: "bot".to_owned(),
                    role: RoleV2::Moderator,
                    expected_revision: 7,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::RoleChanged { revision: 8 })
    );
    assert_eq!(
        service.execute(
            bot,
            request(
                2,
                CommandV2::Invite {
                    room_id: "private".to_owned(),
                    participant_id: "carol".to_owned(),
                    expected_revision: 8,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::InvitationRecorded { revision: 9 })
    );
    assert_eq!(
        service.execute(
            carol,
            request(
                0,
                CommandV2::Join {
                    room_id: "private".to_owned(),
                    expected_revision: 9,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::Joined { revision: 10 })
    );
    assert_eq!(
        service.execute(
            bot,
            request(
                3,
                CommandV2::Speak {
                    room_id: "private".to_owned(),
                    expected_revision: 10,
                    logical_time: 12,
                    body: "hello".to_owned(),
                    addressed_to: vec!["alice".to_owned(), "carol".to_owned()],
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::MessageAccepted {
            revision: 11,
            sequence: 0,
        })
    );
    assert_eq!(
        service.execute(
            bot,
            request(
                4,
                CommandV2::Leave {
                    room_id: "private".to_owned(),
                    expected_revision: 11,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::Left { revision: 12 })
    );

    let ReplyV2::Accepted(OutcomeV2::Members(page)) = service.execute(
        alice,
        request(
            3,
            CommandV2::Members {
                room_id: "private".to_owned(),
                cursor: None,
                limit: 4,
            },
        ),
    ) else {
        panic!("member page should be accepted");
    };
    assert_eq!(
        page.members
            .iter()
            .map(|member| member.participant_id.as_str())
            .collect::<Vec<_>>(),
        ["alice", "carol"]
    );
}

#[test]
fn v2_pages_are_client_bound_one_shot_and_replay_safe() {
    let mut engine = participant_engine();
    for room in ["alpha", "beta", "gamma"] {
        engine
            .apply(EngineCommand::CreateRoom {
                room_id: room.into(),
                name: room.to_owned(),
                owner: "alice".into(),
                admission: AdmissionPolicy::Open,
            })
            .unwrap();
    }
    let mut service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    let bot = service.connect("bot").unwrap();
    let first_request = request(
        0,
        CommandV2::Rooms {
            cursor: None,
            limit: 2,
        },
    );
    let first_reply = service.execute(alice, first_request.clone());
    assert_eq!(service.execute(alice, first_request), first_reply);
    let ReplyV2::Accepted(OutcomeV2::Rooms(first)) = first_reply else {
        panic!("room page should be accepted");
    };
    let cursor = first.next.expect("first page should continue");
    assert_eq!(
        service.execute(
            bot,
            request(
                0,
                CommandV2::Rooms {
                    cursor: Some(cursor),
                    limit: 2,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::InvalidCursor)
    );
    let ReplyV2::Accepted(OutcomeV2::Rooms(second)) = service.execute(
        alice,
        request(
            1,
            CommandV2::Rooms {
                cursor: Some(cursor),
                limit: 2,
            },
        ),
    ) else {
        panic!("continuation should be accepted");
    };
    assert_eq!(second.rooms.len(), 1);
    assert!(second.next.is_none());
    assert_eq!(
        service.execute(
            alice,
            request(
                2,
                CommandV2::Rooms {
                    cursor: Some(cursor),
                    limit: 2,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::InvalidCursor)
    );
    assert_eq!(service.usage().replayed, 1);
}

#[test]
fn v2_cursor_staleness_and_capacity_are_explicit() {
    let mut engine = participant_engine();
    for room in ["alpha", "beta"] {
        engine
            .apply(EngineCommand::CreateRoom {
                room_id: room.into(),
                name: room.to_owned(),
                owner: "alice".into(),
                admission: AdmissionPolicy::Open,
            })
            .unwrap();
    }
    let mut service = ServiceV2::new(
        engine,
        ServiceLimitsV2 {
            max_retained_cursors: 1,
            ..ServiceLimitsV2::default()
        },
    );
    let alice = service.connect("alice").unwrap();
    let ReplyV2::Accepted(OutcomeV2::Rooms(first)) = service.execute(
        alice,
        request(
            0,
            CommandV2::Rooms {
                cursor: None,
                limit: 1,
            },
        ),
    ) else {
        panic!("room page should be accepted");
    };
    let cursor = first.next.unwrap();
    assert_eq!(
        service.execute(
            alice,
            request(
                1,
                CommandV2::Rooms {
                    cursor: None,
                    limit: 1,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::CursorLimit)
    );
    assert_eq!(
        service.execute(
            alice,
            request(
                2,
                CommandV2::Speak {
                    room_id: "alpha".to_owned(),
                    expected_revision: 6,
                    logical_time: 0,
                    body: "change".to_owned(),
                    addressed_to: vec![],
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::MessageAccepted {
            revision: 7,
            sequence: 0,
        })
    );
    assert_eq!(
        service.execute(
            alice,
            request(
                3,
                CommandV2::Rooms {
                    cursor: Some(cursor),
                    limit: 1,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::Stale { actual_revision: 7 })
    );
}

#[test]
fn v2_validation_and_request_conflict_precede_mutation() {
    let mut service = ServiceV2::new(
        participant_engine(),
        ServiceLimitsV2 {
            max_retained_requests: 1,
            ..ServiceLimitsV2::default()
        },
    );
    let alice = service.connect("alice").unwrap();
    assert_eq!(
        service.execute(
            alice,
            request(
                0,
                CommandV2::Rooms {
                    cursor: None,
                    limit: 0,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::InvalidInput)
    );
    let accepted = request(
        0,
        CommandV2::CreateRoom {
            room_id: "room".to_owned(),
            name: "Room".to_owned(),
            admission: AdmissionPolicyV2::Open,
            expected_revision: 4,
        },
    );
    assert_eq!(
        service.execute(alice, accepted.clone()),
        ReplyV2::Accepted(OutcomeV2::RoomCreated { revision: 5 })
    );
    assert_eq!(
        service.execute(
            alice,
            request(
                0,
                CommandV2::CreateRoom {
                    room_id: "changed".to_owned(),
                    name: "Changed".to_owned(),
                    admission: AdmissionPolicyV2::Open,
                    expected_revision: 5,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::RequestConflict)
    );
    assert_eq!(service.engine().revision().0, 5);
    assert_eq!(
        service.execute(alice, accepted),
        ReplyV2::Accepted(OutcomeV2::RoomCreated { revision: 5 })
    );
}

#[test]
fn v2_disconnect_reclaims_requests_and_cursors() {
    let mut engine = participant_engine();
    for room in ["alpha", "beta"] {
        engine
            .apply(EngineCommand::CreateRoom {
                room_id: room.into(),
                name: room.to_owned(),
                owner: "alice".into(),
                admission: AdmissionPolicy::Open,
            })
            .unwrap();
    }
    let mut service = ServiceV2::new(
        engine,
        ServiceLimitsV2 {
            max_clients: 1,
            max_retained_requests: 1,
            max_retained_cursors: 1,
            ..ServiceLimitsV2::default()
        },
    );
    let first = service.connect("alice").unwrap();
    assert_eq!(first, ClientIdV2(0));
    assert!(matches!(
        service.execute(
            first,
            request(
                0,
                CommandV2::Rooms {
                    cursor: None,
                    limit: 1,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::Rooms(_))
    ));
    assert_eq!(service.disconnect(first), Ok(true));
    assert_eq!(service.disconnect(first), Ok(false));
    let second = service.connect("alice").unwrap();
    assert_eq!(second, ClientIdV2(1));
    assert!(matches!(
        service.execute(
            second,
            request(
                0,
                CommandV2::Rooms {
                    cursor: None,
                    limit: 1,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::Rooms(_))
    ));
}

#[test]
fn v2_change_feed_opens_at_snapshot_and_advances_exclusively() {
    let mut service = ServiceV2::new(participant_engine(), ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    let outsider = service.connect("outsider").unwrap();
    assert_eq!(
        service.execute(
            alice,
            request(
                0,
                CommandV2::CreateRoom {
                    room_id: "room".to_owned(),
                    name: "Room".to_owned(),
                    admission: AdmissionPolicyV2::Open,
                    expected_revision: 4,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::RoomCreated { revision: 5 })
    );
    let ReplyV2::Accepted(OutcomeV2::Messages(snapshot)) = service.execute(
        alice,
        request(
            1,
            CommandV2::Messages {
                room_id: "room".to_owned(),
                cursor: None,
                limit: 4,
            },
        ),
    ) else {
        panic!("bounded snapshot expected");
    };
    let ReplyV2::Accepted(OutcomeV2::ChangesOpened { revision, cursor }) = service.execute(
        alice,
        request(
            2,
            CommandV2::OpenChanges {
                room_id: "room".to_owned(),
                observed_revision: snapshot.revision,
            },
        ),
    ) else {
        panic!("change position expected");
    };
    assert_eq!(revision, snapshot.revision);
    assert_eq!(
        service.execute(
            outsider,
            request(
                0,
                CommandV2::Changes {
                    room_id: "room".to_owned(),
                    cursor,
                    limit: 1,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::InvalidCursor)
    );
    assert_eq!(
        service.execute(
            alice,
            request(
                3,
                CommandV2::Speak {
                    room_id: "room".to_owned(),
                    expected_revision: 5,
                    logical_time: 1,
                    body: "change".to_owned(),
                    addressed_to: vec![],
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::MessageAccepted {
            revision: 6,
            sequence: 0,
        })
    );
    let change_request = request(
        4,
        CommandV2::Changes {
            room_id: "room".to_owned(),
            cursor,
            limit: 1,
        },
    );
    let reply = service.execute(alice, change_request.clone());
    let ReplyV2::Accepted(OutcomeV2::Changes(page)) = &reply else {
        panic!("change page expected");
    };
    assert_eq!(page.changes.len(), 1);
    assert!(!page.has_more);
    assert!(matches!(
        page.changes[0].event,
        RoomEventV2::MessageAccepted(_)
    ));
    assert_eq!(service.execute(alice, change_request), reply);
    assert_eq!(service.usage().replayed, 1);
}

#[test]
fn v2_change_feed_requires_resnapshot_after_leave_and_rejoin() {
    let mut service = ServiceV2::new(participant_engine(), ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    let bot = service.connect("bot").unwrap();
    assert!(matches!(
        service.execute(
            alice,
            request(
                0,
                CommandV2::CreateRoom {
                    room_id: "room".to_owned(),
                    name: "Room".to_owned(),
                    admission: AdmissionPolicyV2::Open,
                    expected_revision: 4,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::RoomCreated { revision: 5 })
    ));
    assert!(matches!(
        service.execute(
            bot,
            request(
                0,
                CommandV2::Join {
                    room_id: "room".to_owned(),
                    expected_revision: 5,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::Joined { revision: 6 })
    ));
    let ReplyV2::Accepted(OutcomeV2::ChangesOpened { cursor, .. }) = service.execute(
        bot,
        request(
            1,
            CommandV2::OpenChanges {
                room_id: "room".to_owned(),
                observed_revision: 6,
            },
        ),
    ) else {
        panic!("change position expected");
    };
    assert!(matches!(
        service.execute(
            bot,
            request(
                2,
                CommandV2::Leave {
                    room_id: "room".to_owned(),
                    expected_revision: 6,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::Left { revision: 7 })
    ));
    assert!(matches!(
        service.execute(
            alice,
            request(
                1,
                CommandV2::Speak {
                    room_id: "room".to_owned(),
                    expected_revision: 7,
                    logical_time: 1,
                    body: "while absent".to_owned(),
                    addressed_to: vec![],
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::MessageAccepted { revision: 8, .. })
    ));
    assert!(matches!(
        service.execute(
            bot,
            request(
                3,
                CommandV2::Join {
                    room_id: "room".to_owned(),
                    expected_revision: 8,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::Joined { revision: 9 })
    ));
    assert_eq!(
        service.execute(
            bot,
            request(
                4,
                CommandV2::Changes {
                    room_id: "room".to_owned(),
                    cursor,
                    limit: 8,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::ChangeGap {
            current_revision: 9,
            cause: ChangeGapV2::MembershipChanged,
        })
    );
}

#[test]
fn v2_moderation_is_actor_bound_replayable_and_distinguishes_kick_from_ban() {
    let mut service = ServiceV2::new(participant_engine(), ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    let bot = service.connect("bot").unwrap();
    assert_eq!(
        service.execute(
            alice,
            request(
                0,
                CommandV2::CreateRoom {
                    room_id: "room".to_owned(),
                    name: "Room".to_owned(),
                    admission: AdmissionPolicyV2::Open,
                    expected_revision: 4,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::RoomCreated { revision: 5 })
    );
    assert!(matches!(
        service.execute(
            bot,
            request(
                0,
                CommandV2::Join {
                    room_id: "room".to_owned(),
                    expected_revision: 5,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::Joined { revision: 6 })
    ));
    let ReplyV2::Accepted(OutcomeV2::ChangesOpened { cursor, .. }) = service.execute(
        alice,
        request(
            1,
            CommandV2::OpenChanges {
                room_id: "room".to_owned(),
                observed_revision: 6,
            },
        ),
    ) else {
        panic!("change position expected");
    };

    let moderation = |participant_id: &str, reason: &str, cause, revision| CommandV2::Kick {
        room_id: "room".to_owned(),
        participant_id: participant_id.to_owned(),
        reason: reason.to_owned(),
        causation: [cause; 16],
        expected_revision: revision,
    };
    assert_eq!(
        service.execute(alice, request(2, moderation("bot", "cooldown", 1, 6))),
        ReplyV2::Accepted(OutcomeV2::Kicked { revision: 7 })
    );
    assert_eq!(
        service.execute(
            bot,
            request(
                1,
                CommandV2::Join {
                    room_id: "room".to_owned(),
                    expected_revision: 7,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::Joined { revision: 8 })
    );
    assert_eq!(
        service.execute(
            alice,
            request(
                3,
                CommandV2::Ban {
                    room_id: "room".to_owned(),
                    participant_id: "bot".to_owned(),
                    reason: "policy".to_owned(),
                    causation: [2; 16],
                    expected_revision: 8,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::Banned { revision: 9 })
    );
    assert_eq!(
        service.execute(
            bot,
            request(
                2,
                CommandV2::Join {
                    room_id: "room".to_owned(),
                    expected_revision: 9,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::Banned)
    );
    let ReplyV2::Accepted(OutcomeV2::Bans(page)) = service.execute(
        alice,
        request(
            4,
            CommandV2::Bans {
                room_id: "room".to_owned(),
                cursor: None,
                limit: 1,
            },
        ),
    ) else {
        panic!("ban page expected");
    };
    assert_eq!(page.banned[0].participant_id, "bot");
    assert_eq!(
        page.banned[0].kind,
        agora_protocol::v2::ParticipantKindV2::Agent
    );
    assert_eq!(page.banned[0].prior_role, RoleV2::Member);
    assert_eq!(
        service.execute(
            alice,
            request(
                5,
                CommandV2::Unban {
                    room_id: "room".to_owned(),
                    participant_id: "bot".to_owned(),
                    reason: "appeal".to_owned(),
                    causation: [3; 16],
                    expected_revision: 9,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::Unbanned { revision: 10 })
    );
    assert_eq!(
        service.execute(
            bot,
            request(
                3,
                CommandV2::Join {
                    room_id: "room".to_owned(),
                    expected_revision: 10,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::Joined { revision: 11 })
    );

    let ReplyV2::Accepted(OutcomeV2::Changes(page)) = service.execute(
        alice,
        request(
            6,
            CommandV2::Changes {
                room_id: "room".to_owned(),
                cursor,
                limit: 8,
            },
        ),
    ) else {
        panic!("change page expected");
    };
    assert!(matches!(
        &page.changes[0].event,
        RoomEventV2::ParticipantKicked { actor_id, participant_id, reason, causation }
            if actor_id == "alice" && participant_id == "bot" && reason == "cooldown" && causation == &[1; 16]
    ));
    assert!(page.changes.iter().any(|change| matches!(
        &change.event,
        RoomEventV2::ParticipantBanned { actor_id, participant_id, reason, causation }
            if actor_id == "alice" && participant_id == "bot" && reason == "policy" && causation == &[2; 16]
    )));
    assert!(page.changes.iter().any(|change| matches!(
        &change.event,
        RoomEventV2::ParticipantUnbanned { actor_id, participant_id, reason, causation }
            if actor_id == "alice" && participant_id == "bot" && reason == "appeal" && causation == &[3; 16]
    )));
}

#[test]
fn v2_room_policy_change_is_owner_bound_and_feed_visible() {
    let mut service = ServiceV2::new(participant_engine(), ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    let bot = service.connect("bot").unwrap();
    assert_eq!(
        service.execute(
            alice,
            request(
                0,
                CommandV2::CreateRoom {
                    room_id: "room".to_owned(),
                    name: "Room".to_owned(),
                    admission: AdmissionPolicyV2::InviteOnly,
                    expected_revision: 4,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::RoomCreated { revision: 5 })
    );
    let ReplyV2::Accepted(OutcomeV2::ChangesOpened { cursor, .. }) = service.execute(
        alice,
        request(
            1,
            CommandV2::OpenChanges {
                room_id: "room".to_owned(),
                observed_revision: 5,
            },
        ),
    ) else {
        panic!("change position expected");
    };
    assert_eq!(
        service.execute(
            bot,
            request(
                0,
                CommandV2::SetAdmissionPolicy {
                    room_id: "room".to_owned(),
                    admission: AdmissionPolicyV2::Open,
                    expected_revision: 5,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::NotAuthorized)
    );
    assert_eq!(
        service.execute(
            alice,
            request(
                2,
                CommandV2::SetAdmissionPolicy {
                    room_id: "room".to_owned(),
                    admission: AdmissionPolicyV2::Open,
                    expected_revision: 5,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::RoomPolicyChanged { revision: 6 })
    );
    let ReplyV2::Accepted(OutcomeV2::Changes(page)) = service.execute(
        alice,
        request(
            3,
            CommandV2::Changes {
                room_id: "room".to_owned(),
                cursor,
                limit: 1,
            },
        ),
    ) else {
        panic!("change page expected");
    };
    assert!(matches!(
        &page.changes[0].event,
        RoomEventV2::RoomPolicyChanged { actor_id, previous: AdmissionPolicyV2::InviteOnly, admission: AdmissionPolicyV2::Open }
            if actor_id == "alice"
    ));
}
