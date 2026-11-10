use agora_engine::{
    AdmissionPolicy, CommandV2 as EngineCommand, Engine, Participant, ParticipantKind,
};
use agora_protocol::v2::{
    AdmissionPolicyV2, CapabilityV2, ChangeGapV2, ClientIdV2, CommandV2, IdentifiedRoomEventV1,
    MessageIdV1, OutcomeV2, RefusalV2, ReplyV2, RequestIdV2, RequestV2, RoleV2, RoomEventV2,
    ThreadLifecycleStateV1, ThreadedRoomEventV1,
};
use agora_service::v2::{ServiceLimitsV2, ServiceV2};

fn request(id: u64, command: CommandV2) -> RequestV2 {
    RequestV2 {
        request_id: RequestIdV2(id),
        command,
    }
}

#[test]
fn v2_one_level_threads_preserve_identity_pages_and_feed_relationships() {
    let mut engine = Engine::new();
    for (id, kind) in [
        ("alice", ParticipantKind::Human),
        ("bot", ParticipantKind::Agent),
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
    let mut service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    let bot = service.connect("bot").unwrap();
    let ReplyV2::Accepted(OutcomeV2::IdentifiedMessageAccepted {
        message_id: root_id,
        ..
    }) = service.execute(
        alice,
        request(
            1,
            CommandV2::SpeakIdentified {
                room_id: "room".to_owned(),
                expected_revision: 4,
                logical_time: 1,
                body: "root".to_owned(),
                addressed_to: vec![],
            },
        ),
    )
    else {
        panic!("root acceptance expected");
    };
    let ReplyV2::Accepted(OutcomeV2::ChangesOpened {
        cursor: threaded_cursor,
        ..
    }) = service.execute(
        alice,
        request(
            2,
            CommandV2::OpenThreadedChanges {
                room_id: "room".to_owned(),
                observed_revision: 5,
            },
        ),
    )
    else {
        panic!("threaded change position expected");
    };
    let reply_request = request(
        1,
        CommandV2::SpeakReply {
            room_id: "room".to_owned(),
            expected_revision: 5,
            root_message_id: root_id,
            logical_time: 2,
            body: "reply".to_owned(),
            addressed_to: vec!["alice".to_owned()],
        },
    );
    let accepted = service.execute(bot, reply_request.clone());
    let ReplyV2::Accepted(OutcomeV2::ReplyAccepted {
        message_id: reply_id,
        root_message_id,
        ..
    }) = &accepted
    else {
        panic!("reply acceptance expected");
    };
    let reply_id = *reply_id;
    assert_eq!(*root_message_id, root_id);
    assert_eq!(service.execute(bot, reply_request), accepted);

    let ReplyV2::Accepted(OutcomeV2::ThreadRoots(roots)) = service.execute(
        alice,
        request(
            3,
            CommandV2::ThreadRoots {
                room_id: "room".to_owned(),
                cursor: None,
                limit: 1,
            },
        ),
    ) else {
        panic!("root page expected");
    };
    assert_eq!(roots.messages[0].message_id, root_id);
    assert_eq!(roots.messages[0].reply_to_root, None);
    let ReplyV2::Accepted(OutcomeV2::ThreadReplies(replies)) = service.execute(
        alice,
        request(
            4,
            CommandV2::ThreadReplies {
                room_id: "room".to_owned(),
                root_message_id: root_id,
                cursor: None,
                limit: 1,
            },
        ),
    ) else {
        panic!("reply page expected");
    };
    assert_eq!(replies.messages[0].message_id, reply_id);
    assert_eq!(replies.messages[0].reply_to_root, Some(root_id));
    let ReplyV2::Accepted(OutcomeV2::ThreadedChanges(changes)) = service.execute(
        alice,
        request(
            5,
            CommandV2::ThreadedChanges {
                room_id: "room".to_owned(),
                cursor: threaded_cursor,
                limit: 1,
            },
        ),
    ) else {
        panic!("threaded change page expected");
    };
    assert!(matches!(
        &changes.changes[0].event,
        ThreadedRoomEventV1::MessageAccepted(message)
            if message.message_id == reply_id && message.reply_to_root == Some(root_id)
    ));
    assert_eq!(
        service.execute(
            bot,
            request(
                2,
                CommandV2::SpeakReply {
                    room_id: "room".to_owned(),
                    expected_revision: 6,
                    root_message_id: reply_id,
                    logical_time: 3,
                    body: "nested".to_owned(),
                    addressed_to: vec![],
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::InvalidInput)
    );

    let ReplyV2::Accepted(OutcomeV2::IdentifiedMessageAccepted {
        message_id: second_root_id,
        ..
    }) = service.execute(
        alice,
        request(
            6,
            CommandV2::SpeakIdentified {
                room_id: "room".to_owned(),
                expected_revision: 6,
                logical_time: 4,
                body: "second root".to_owned(),
                addressed_to: vec![],
            },
        ),
    )
    else {
        panic!("second root acceptance expected");
    };
    assert_ne!(second_root_id, root_id);
    assert!(matches!(
        service.execute(
            bot,
            request(
                3,
                CommandV2::SpeakReply {
                    room_id: "room".to_owned(),
                    expected_revision: 7,
                    root_message_id: root_id,
                    logical_time: 5,
                    body: "second reply".to_owned(),
                    addressed_to: vec![],
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::ReplyAccepted { .. })
    ));

    let ReplyV2::Accepted(OutcomeV2::ThreadRoots(root_page)) = service.execute(
        alice,
        request(
            7,
            CommandV2::ThreadRoots {
                room_id: "room".to_owned(),
                cursor: None,
                limit: 1,
            },
        ),
    ) else {
        panic!("bounded root page expected");
    };
    let root_cursor = root_page.next.expect("root continuation expected");
    let ReplyV2::Accepted(OutcomeV2::ThreadReplies(reply_page)) = service.execute(
        alice,
        request(
            8,
            CommandV2::ThreadReplies {
                room_id: "room".to_owned(),
                root_message_id: root_id,
                cursor: None,
                limit: 1,
            },
        ),
    ) else {
        panic!("bounded reply page expected");
    };
    let reply_cursor = reply_page.next.expect("reply continuation expected");
    assert_eq!(
        service.execute(
            alice,
            request(
                9,
                CommandV2::ThreadReplies {
                    room_id: "room".to_owned(),
                    root_message_id: root_id,
                    cursor: Some(root_cursor),
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
                10,
                CommandV2::ThreadRoots {
                    room_id: "room".to_owned(),
                    cursor: Some(reply_cursor),
                    limit: 1,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::InvalidCursor)
    );
    assert!(matches!(
        service.execute(
            alice,
            request(
                11,
                CommandV2::ThreadRoots {
                    room_id: "room".to_owned(),
                    cursor: Some(root_cursor),
                    limit: 1,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::ThreadRoots(_))
    ));

    let ReplyV2::Accepted(OutcomeV2::ChangesOpened {
        cursor: identified_cursor,
        ..
    }) = service.execute(
        alice,
        request(
            12,
            CommandV2::OpenIdentifiedChanges {
                room_id: "room".to_owned(),
                observed_revision: 8,
            },
        ),
    )
    else {
        panic!("identified change position expected");
    };
    let ReplyV2::Accepted(OutcomeV2::ChangesOpened {
        cursor: threaded_cursor,
        ..
    }) = service.execute(
        alice,
        request(
            13,
            CommandV2::OpenThreadedChanges {
                room_id: "room".to_owned(),
                observed_revision: 8,
            },
        ),
    )
    else {
        panic!("threaded change position expected");
    };
    assert_eq!(
        service.execute(
            alice,
            request(
                14,
                CommandV2::ThreadedChanges {
                    room_id: "room".to_owned(),
                    cursor: identified_cursor,
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
                15,
                CommandV2::IdentifiedChanges {
                    room_id: "room".to_owned(),
                    cursor: threaded_cursor,
                    limit: 1,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::InvalidCursor)
    );

    assert!(matches!(
        service.execute(
            alice,
            request(
                16,
                CommandV2::WithdrawMessage {
                    room_id: "room".to_owned(),
                    expected_revision: 8,
                    message_id: root_id,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::MessageWithdrawn { .. })
    ));
    let ReplyV2::Accepted(OutcomeV2::ReplyAccepted {
        message_id: post_withdrawal_reply_id,
        ..
    }) = service.execute(
        bot,
        request(
            4,
            CommandV2::SpeakReply {
                room_id: "room".to_owned(),
                expected_revision: 9,
                root_message_id: root_id,
                logical_time: 6,
                body: "reply after withdrawal".to_owned(),
                addressed_to: vec![],
            },
        ),
    )
    else {
        panic!("withdrawn retained root remains replyable");
    };
    assert!(matches!(
        service.execute(
            alice,
            request(
                17,
                CommandV2::RedactMessage {
                    room_id: "room".to_owned(),
                    expected_revision: 10,
                    message_id: post_withdrawal_reply_id,
                    reason: "policy".to_owned(),
                    causation: [0x77; 16],
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::MessageRedacted { .. })
    ));
    let ReplyV2::Accepted(OutcomeV2::ThreadReplies(replies)) = service.execute(
        alice,
        request(
            18,
            CommandV2::ThreadReplies {
                room_id: "room".to_owned(),
                root_message_id: root_id,
                cursor: None,
                limit: 10,
            },
        ),
    ) else {
        panic!("reply page expected after tombstones");
    };
    let redacted = replies
        .messages
        .iter()
        .find(|message| message.message_id == post_withdrawal_reply_id)
        .expect("redacted reply remains retained");
    assert_eq!(redacted.reply_to_root, Some(root_id));
    assert!(redacted.body.is_empty());
}

#[test]
fn v2_thread_lifecycle_is_authorized_observable_replayable_and_feed_visible() {
    let mut engine = participant_engine();
    engine
        .apply(EngineCommand::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
            owner: "alice".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    for participant in ["bot", "carol"] {
        engine
            .apply(EngineCommand::Join {
                room_id: "room".into(),
                participant: participant.into(),
            })
            .unwrap();
    }
    let mut service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    let bot = service.connect("bot").unwrap();
    let carol = service.connect("carol").unwrap();
    let ReplyV2::Accepted(OutcomeV2::IdentifiedMessageAccepted {
        message_id: root_id,
        ..
    }) = service.execute(
        bot,
        request(
            1,
            CommandV2::SpeakIdentified {
                room_id: "room".to_owned(),
                expected_revision: 7,
                logical_time: 1,
                body: "root".to_owned(),
                addressed_to: vec![],
            },
        ),
    )
    else {
        panic!("root acceptance expected");
    };
    let mut feed_cursors = Vec::new();
    for (request_id, command) in [
        (
            1,
            CommandV2::OpenChanges {
                room_id: "room".to_owned(),
                observed_revision: 8,
            },
        ),
        (
            2,
            CommandV2::OpenIdentifiedChanges {
                room_id: "room".to_owned(),
                observed_revision: 8,
            },
        ),
        (
            3,
            CommandV2::OpenThreadedChanges {
                room_id: "room".to_owned(),
                observed_revision: 8,
            },
        ),
    ] {
        let ReplyV2::Accepted(OutcomeV2::ChangesOpened { cursor, .. }) =
            service.execute(alice, request(request_id, command))
        else {
            panic!("change position expected");
        };
        feed_cursors.push(cursor);
    }

    assert_eq!(
        service.execute(
            carol,
            request(
                1,
                CommandV2::ArchiveThread {
                    room_id: "room".to_owned(),
                    expected_revision: 8,
                    root_message_id: root_id,
                    reason: "not mine".to_owned(),
                    causation: [1; 16],
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::NotAuthorized)
    );
    let archive = request(
        2,
        CommandV2::ArchiveThread {
            room_id: "room".to_owned(),
            expected_revision: 8,
            root_message_id: root_id,
            reason: "pause".to_owned(),
            causation: [2; 16],
        },
    );
    let accepted = service.execute(bot, archive.clone());
    assert_eq!(
        accepted,
        ReplyV2::Accepted(OutcomeV2::ThreadArchived {
            revision: 9,
            root_message_id: root_id,
        })
    );
    assert_eq!(service.execute(bot, archive), accepted);
    assert_eq!(
        service.execute(
            carol,
            request(
                2,
                CommandV2::SpeakReply {
                    room_id: "room".to_owned(),
                    expected_revision: 9,
                    root_message_id: root_id,
                    logical_time: 2,
                    body: "blocked".to_owned(),
                    addressed_to: vec![],
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::ThreadArchived)
    );
    let ReplyV2::Accepted(OutcomeV2::ThreadStatus(status)) = service.execute(
        alice,
        request(
            4,
            CommandV2::ThreadStatus {
                room_id: "room".to_owned(),
                root_message_id: root_id,
            },
        ),
    ) else {
        panic!("thread status expected");
    };
    assert_eq!(status.state, ThreadLifecycleStateV1::Archived);
    assert_eq!(status.root_message_id, root_id);

    for (request_id, cursor, profile) in [
        (5, feed_cursors[0], 0_u8),
        (6, feed_cursors[1], 1),
        (7, feed_cursors[2], 2),
    ] {
        let command = match profile {
            0 => CommandV2::Changes {
                room_id: "room".to_owned(),
                cursor,
                limit: 1,
            },
            1 => CommandV2::IdentifiedChanges {
                room_id: "room".to_owned(),
                cursor,
                limit: 1,
            },
            _ => CommandV2::ThreadedChanges {
                room_id: "room".to_owned(),
                cursor,
                limit: 1,
            },
        };
        let reply = service.execute(alice, request(request_id, command));
        let is_archive = match reply {
            ReplyV2::Accepted(OutcomeV2::Changes(page)) => matches!(
                &page.changes[0].event,
                RoomEventV2::ThreadLifecycleChanged {
                    root_message_id,
                    archived: true,
                    ..
                } if *root_message_id == root_id
            ),
            ReplyV2::Accepted(OutcomeV2::IdentifiedChanges(page)) => matches!(
                &page.changes[0].event,
                IdentifiedRoomEventV1::ThreadLifecycleChanged {
                    root_message_id,
                    archived: true,
                    ..
                } if *root_message_id == root_id
            ),
            ReplyV2::Accepted(OutcomeV2::ThreadedChanges(page)) => matches!(
                &page.changes[0].event,
                ThreadedRoomEventV1::ThreadLifecycleChanged {
                    root_message_id,
                    archived: true,
                    ..
                } if *root_message_id == root_id
            ),
            _ => false,
        };
        assert!(is_archive);
    }

    assert_eq!(
        service.execute(
            alice,
            request(
                8,
                CommandV2::ReopenThread {
                    room_id: "room".to_owned(),
                    expected_revision: 9,
                    root_message_id: root_id,
                    reason: "resume".to_owned(),
                    causation: [3; 16],
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::ThreadReopened {
            revision: 10,
            root_message_id: root_id,
        })
    );
    assert!(matches!(
        service.execute(
            carol,
            request(
                3,
                CommandV2::SpeakReply {
                    room_id: "room".to_owned(),
                    expected_revision: 10,
                    root_message_id: root_id,
                    logical_time: 3,
                    body: "allowed".to_owned(),
                    addressed_to: vec![],
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::ReplyAccepted { .. })
    ));
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
fn v2_author_withdrawal_tombstones_history_and_retained_change_content() {
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
    let ReplyV2::Accepted(OutcomeV2::IdentifiedMessageAccepted {
        message_id,
        sequence: 0,
        ..
    }) = service.execute(
        alice,
        request(
            2,
            CommandV2::SpeakIdentified {
                room_id: "room".to_owned(),
                expected_revision: 6,
                logical_time: 9,
                body: "withdraw me".to_owned(),
                addressed_to: vec!["bot".to_owned()],
            },
        ),
    )
    else {
        panic!("identified acceptance expected");
    };
    assert_eq!(
        service.execute(
            bot,
            request(
                1,
                CommandV2::WithdrawMessage {
                    room_id: "room".to_owned(),
                    expected_revision: 7,
                    message_id,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::NotAuthorized)
    );
    let withdrawal = request(
        3,
        CommandV2::WithdrawMessage {
            room_id: "room".to_owned(),
            expected_revision: 7,
            message_id,
        },
    );
    let reply = ReplyV2::Accepted(OutcomeV2::MessageWithdrawn {
        revision: 8,
        message_id,
        sequence: 0,
    });
    assert_eq!(service.execute(alice, withdrawal.clone()), reply);
    assert_eq!(service.execute(alice, withdrawal), reply);

    let ReplyV2::Accepted(OutcomeV2::IdentifiedMessages(history)) = service.execute(
        alice,
        request(
            4,
            CommandV2::IdentifiedMessages {
                room_id: "room".to_owned(),
                cursor: None,
                limit: 4,
            },
        ),
    ) else {
        panic!("identified history expected");
    };
    assert_eq!(history.messages[0].message_id, message_id);
    assert_eq!(history.messages[0].body, "");

    let ReplyV2::Accepted(OutcomeV2::Changes(changes)) = service.execute(
        alice,
        request(
            5,
            CommandV2::Changes {
                room_id: "room".to_owned(),
                cursor,
                limit: 4,
            },
        ),
    ) else {
        panic!("change page expected");
    };
    assert!(matches!(
        &changes.changes[0].event,
        RoomEventV2::MessageAccepted(message) if message.body.is_empty()
    ));
    assert!(matches!(
        &changes.changes[1].event,
        RoomEventV2::MessageWithdrawn {
            actor_id,
            message_id: id,
            sequence: 0,
        } if actor_id == "alice" && id == &message_id
    ));

    let _opaque_type_check: MessageIdV1 = message_id;
}

#[test]
fn v2_moderator_redaction_is_role_scoped_attributable_and_replayable() {
    let mut service = ServiceV2::new(participant_engine(), ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    let bot = service.connect("bot").unwrap();
    let carol = service.connect("carol").unwrap();
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
    for (client, expected_revision) in [(bot, 5), (carol, 6)] {
        assert!(matches!(
            service.execute(
                client,
                request(
                    0,
                    CommandV2::Join {
                        room_id: "room".to_owned(),
                        expected_revision,
                    },
                ),
            ),
            ReplyV2::Accepted(OutcomeV2::Joined { .. })
        ));
    }
    assert_eq!(
        service.execute(
            alice,
            request(
                1,
                CommandV2::SetRole {
                    room_id: "room".to_owned(),
                    participant_id: "bot".to_owned(),
                    role: RoleV2::Moderator,
                    expected_revision: 7,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::RoleChanged { revision: 8 })
    );
    let ReplyV2::Accepted(OutcomeV2::ChangesOpened { cursor, .. }) = service.execute(
        alice,
        request(
            2,
            CommandV2::OpenChanges {
                room_id: "room".to_owned(),
                observed_revision: 8,
            },
        ),
    ) else {
        panic!("change position expected");
    };
    let ReplyV2::Accepted(OutcomeV2::IdentifiedMessageAccepted { message_id, .. }) = service
        .execute(
            carol,
            request(
                1,
                CommandV2::SpeakIdentified {
                    room_id: "room".to_owned(),
                    expected_revision: 8,
                    logical_time: 3,
                    body: "moderate me".to_owned(),
                    addressed_to: vec![],
                },
            ),
        )
    else {
        panic!("identified acceptance expected");
    };
    let redaction = request(
        1,
        CommandV2::RedactMessage {
            room_id: "room".to_owned(),
            expected_revision: 9,
            message_id,
            reason: "room policy".to_owned(),
            causation: [0x33; 16],
        },
    );
    let reply = ReplyV2::Accepted(OutcomeV2::MessageRedacted {
        revision: 10,
        message_id,
        sequence: 0,
    });
    assert_eq!(service.execute(bot, redaction.clone()), reply);
    assert_eq!(service.execute(bot, redaction), reply);

    let ReplyV2::Accepted(OutcomeV2::Changes(changes)) = service.execute(
        alice,
        request(
            3,
            CommandV2::Changes {
                room_id: "room".to_owned(),
                cursor,
                limit: 4,
            },
        ),
    ) else {
        panic!("change page expected");
    };
    assert!(matches!(
        &changes.changes[0].event,
        RoomEventV2::MessageAccepted(message) if message.body.is_empty()
    ));
    assert!(matches!(
        &changes.changes[1].event,
        RoomEventV2::MessageRedacted {
            actor_id,
            author_id,
            message_id: id,
            sequence: 0,
            reason,
            causation,
        } if actor_id == "bot"
            && author_id == "carol"
            && id == &message_id
            && reason == "room policy"
            && causation == &[0x33; 16]
    ));
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
    let ReplyV2::Accepted(OutcomeV2::ChangesOpened {
        cursor: identified_cursor,
        ..
    }) = service.execute(
        bot,
        request(
            5,
            CommandV2::OpenIdentifiedChanges {
                room_id: "room".to_owned(),
                observed_revision: 6,
            },
        ),
    )
    else {
        panic!("identified change position expected");
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
    assert_eq!(
        service.execute(
            bot,
            request(
                6,
                CommandV2::IdentifiedChanges {
                    room_id: "room".to_owned(),
                    cursor: identified_cursor,
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
fn v2_reactions_are_actor_bound_replayable_aggregated_and_feed_visible() {
    let mut engine = participant_engine();
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
    let mut service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    let bot = service.connect("bot").unwrap();
    let ReplyV2::Accepted(OutcomeV2::IdentifiedMessageAccepted { message_id, .. }) = service
        .execute(
            alice,
            request(
                1,
                CommandV2::SpeakIdentified {
                    room_id: "room".to_owned(),
                    expected_revision: 6,
                    logical_time: 1,
                    body: "message".to_owned(),
                    addressed_to: vec![],
                },
            ),
        )
    else {
        panic!("identified message expected");
    };
    let mut cursors = Vec::new();
    for (id, command) in [
        (
            2,
            CommandV2::OpenChanges {
                room_id: "room".to_owned(),
                observed_revision: 7,
            },
        ),
        (
            3,
            CommandV2::OpenIdentifiedChanges {
                room_id: "room".to_owned(),
                observed_revision: 7,
            },
        ),
        (
            4,
            CommandV2::OpenThreadedChanges {
                room_id: "room".to_owned(),
                observed_revision: 7,
            },
        ),
    ] {
        let ReplyV2::Accepted(OutcomeV2::ChangesOpened { cursor, .. }) =
            service.execute(alice, request(id, command))
        else {
            panic!("change position expected");
        };
        cursors.push(cursor);
    }

    let add = request(
        1,
        CommandV2::AddReaction {
            room_id: "room".to_owned(),
            expected_revision: 7,
            message_id,
            key: "spark".to_owned(),
        },
    );
    let accepted = service.execute(bot, add.clone());
    assert_eq!(
        accepted,
        ReplyV2::Accepted(OutcomeV2::ReactionAdded {
            revision: 8,
            message_id,
        })
    );
    assert_eq!(service.execute(bot, add), accepted);

    let ReplyV2::Accepted(OutcomeV2::ReactionStatus(status)) = service.execute(
        bot,
        request(
            2,
            CommandV2::ReactionStatus {
                room_id: "room".to_owned(),
                message_id,
            },
        ),
    ) else {
        panic!("reaction status expected");
    };
    assert_eq!(status.reactions.len(), 1);
    assert_eq!(status.reactions[0].key, "spark");
    assert_eq!(status.reactions[0].count, 1);
    assert!(status.reactions[0].caller_reacted);

    for (id, cursor, profile) in [
        (5, cursors[0], 0_u8),
        (6, cursors[1], 1),
        (7, cursors[2], 2),
    ] {
        let command = match profile {
            0 => CommandV2::Changes {
                room_id: "room".to_owned(),
                cursor,
                limit: 1,
            },
            1 => CommandV2::IdentifiedChanges {
                room_id: "room".to_owned(),
                cursor,
                limit: 1,
            },
            _ => CommandV2::ThreadedChanges {
                room_id: "room".to_owned(),
                cursor,
                limit: 1,
            },
        };
        let reply = service.execute(alice, request(id, command));
        let is_reaction = match reply {
            ReplyV2::Accepted(OutcomeV2::Changes(page)) => matches!(
                &page.changes[0].event,
                RoomEventV2::ReactionChanged { actor_id, message_id: id, key, reacted: true }
                    if actor_id == "bot" && *id == message_id && key == "spark"
            ),
            ReplyV2::Accepted(OutcomeV2::IdentifiedChanges(page)) => matches!(
                &page.changes[0].event,
                IdentifiedRoomEventV1::ReactionChanged { actor_id, message_id: id, key, reacted: true }
                    if actor_id == "bot" && *id == message_id && key == "spark"
            ),
            ReplyV2::Accepted(OutcomeV2::ThreadedChanges(page)) => matches!(
                &page.changes[0].event,
                ThreadedRoomEventV1::ReactionChanged { actor_id, message_id: id, key, reacted: true }
                    if actor_id == "bot" && *id == message_id && key == "spark"
            ),
            _ => false,
        };
        assert!(is_reaction);
    }

    assert_eq!(
        service.execute(
            bot,
            request(
                3,
                CommandV2::RemoveReaction {
                    room_id: "room".to_owned(),
                    expected_revision: 8,
                    message_id,
                    key: "spark".to_owned(),
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::ReactionRemoved {
            revision: 9,
            message_id,
        })
    );
}

#[test]
fn v2_capability_discovery_is_closed_replayable_and_non_authorizing() {
    let mut engine = Engine::new();
    engine
        .apply(EngineCommand::RegisterParticipant(Participant::new(
            "alice",
            "Alice",
            ParticipantKind::Human,
        )))
        .unwrap();
    let mut service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    let request = request(1, CommandV2::Capabilities);
    let first = service.execute(alice, request.clone());
    let ReplyV2::Accepted(OutcomeV2::Capabilities(capabilities)) = &first else {
        panic!("capabilities expected");
    };
    assert_eq!(capabilities.len(), 13);
    assert_eq!(capabilities.last(), Some(&CapabilityV2::ReactionsV1));
    assert_eq!(service.execute(alice, request), first);
    assert_eq!(service.engine().revision().0, 1);
}

#[test]
fn v2_identified_messages_preserve_identity_without_reinterpreting_legacy_pages() {
    let mut engine = Engine::new();
    engine
        .apply(EngineCommand::RegisterParticipant(Participant::new(
            "alice",
            "Alice",
            ParticipantKind::Human,
        )))
        .unwrap();
    engine
        .apply(EngineCommand::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
            owner: "alice".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    let mut service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    let speak = request(
        1,
        CommandV2::SpeakIdentified {
            room_id: "room".to_owned(),
            expected_revision: 2,
            logical_time: 1,
            body: "first".to_owned(),
            addressed_to: vec![],
        },
    );
    let first = service.execute(alice, speak.clone());
    let ReplyV2::Accepted(OutcomeV2::IdentifiedMessageAccepted {
        message_id,
        sequence: 0,
        ..
    }) = &first
    else {
        panic!("identified acceptance expected");
    };
    assert_eq!(service.execute(alice, speak), first);
    assert!(matches!(
        service.execute(
            alice,
            request(
                2,
                CommandV2::SpeakIdentified {
                    room_id: "room".to_owned(),
                    expected_revision: 3,
                    logical_time: 2,
                    body: "second".to_owned(),
                    addressed_to: vec![],
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::IdentifiedMessageAccepted { sequence: 1, .. })
    ));
    let ReplyV2::Accepted(OutcomeV2::IdentifiedMessages(page)) = service.execute(
        alice,
        request(
            3,
            CommandV2::IdentifiedMessages {
                room_id: "room".to_owned(),
                cursor: None,
                limit: 1,
            },
        ),
    ) else {
        panic!("identified page expected");
    };
    assert_eq!(page.messages[0].message_id, *message_id);
    assert_eq!(page.messages[0].sequence, 0);
    assert_eq!(
        service.execute(
            alice,
            request(
                4,
                CommandV2::Messages {
                    room_id: "room".to_owned(),
                    cursor: page.next,
                    limit: 1,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::InvalidCursor)
    );
    let ReplyV2::Accepted(OutcomeV2::Messages(legacy)) = service.execute(
        alice,
        request(
            5,
            CommandV2::Messages {
                room_id: "room".to_owned(),
                cursor: None,
                limit: 1,
            },
        ),
    ) else {
        panic!("legacy page expected");
    };
    assert_eq!(legacy.messages[0].sequence, 0);
}

#[test]
fn v2_identified_change_feed_preserves_identity_and_isolates_cursor_profiles() {
    let mut engine = Engine::new();
    engine
        .apply(EngineCommand::RegisterParticipant(Participant::new(
            "alice",
            "Alice",
            ParticipantKind::Human,
        )))
        .unwrap();
    engine
        .apply(EngineCommand::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
            owner: "alice".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    let mut service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();

    let ReplyV2::Accepted(OutcomeV2::ChangesOpened {
        cursor: legacy_cursor,
        ..
    }) = service.execute(
        alice,
        request(
            1,
            CommandV2::OpenChanges {
                room_id: "room".to_owned(),
                observed_revision: 2,
            },
        ),
    )
    else {
        panic!("legacy change position expected");
    };
    let ReplyV2::Accepted(OutcomeV2::ChangesOpened {
        cursor: identified_cursor,
        ..
    }) = service.execute(
        alice,
        request(
            2,
            CommandV2::OpenIdentifiedChanges {
                room_id: "room".to_owned(),
                observed_revision: 2,
            },
        ),
    )
    else {
        panic!("identified change position expected");
    };

    assert_eq!(
        service.execute(
            alice,
            request(
                3,
                CommandV2::Changes {
                    room_id: "room".to_owned(),
                    cursor: identified_cursor,
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
                4,
                CommandV2::IdentifiedChanges {
                    room_id: "room".to_owned(),
                    cursor: legacy_cursor,
                    limit: 1,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::InvalidCursor)
    );

    let ReplyV2::Accepted(OutcomeV2::IdentifiedMessageAccepted {
        message_id,
        sequence: 0,
        ..
    }) = service.execute(
        alice,
        request(
            5,
            CommandV2::SpeakIdentified {
                room_id: "room".to_owned(),
                expected_revision: 2,
                logical_time: 7,
                body: "stable".to_owned(),
                addressed_to: vec![],
            },
        ),
    )
    else {
        panic!("identified acceptance expected");
    };

    let identified_request = request(
        6,
        CommandV2::IdentifiedChanges {
            room_id: "room".to_owned(),
            cursor: identified_cursor,
            limit: 1,
        },
    );
    let identified_reply = service.execute(alice, identified_request.clone());
    let ReplyV2::Accepted(OutcomeV2::IdentifiedChanges(page)) = &identified_reply else {
        panic!("identified change page expected");
    };
    assert!(matches!(
        &page.changes[0].event,
        IdentifiedRoomEventV1::MessageAccepted(message)
            if message.message_id == message_id && message.sequence == 0
    ));
    assert_eq!(service.execute(alice, identified_request), identified_reply);

    let ReplyV2::Accepted(OutcomeV2::Changes(legacy)) = service.execute(
        alice,
        request(
            7,
            CommandV2::Changes {
                room_id: "room".to_owned(),
                cursor: legacy_cursor,
                limit: 1,
            },
        ),
    ) else {
        panic!("legacy change page expected");
    };
    assert!(matches!(
        &legacy.changes[0].event,
        RoomEventV2::MessageAccepted(message) if message.sequence == 0
    ));

    let ReplyV2::Accepted(OutcomeV2::IdentifiedMessages(history)) = service.execute(
        alice,
        request(
            8,
            CommandV2::IdentifiedMessages {
                room_id: "room".to_owned(),
                cursor: None,
                limit: 1,
            },
        ),
    ) else {
        panic!("identified history expected");
    };
    assert_eq!(history.messages[0].message_id, message_id);
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
