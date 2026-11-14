use agora_engine::{
    AdmissionPolicy, CommandV2 as EngineCommand, Engine, Limits, Participant, ParticipantKind,
};
use agora_protocol::v2::{
    AdmissionPolicyV2, AttentionPreferenceV1, AttentionScopeV1, CapabilityV2, ChangeGapV2,
    ClientIdV2, CommandV2, ConversationProfileV1, IdentifiedRoomEventV1, MessageIdV1,
    NotificationIntentV1, OutcomeV2, ReadScopeV1, RefusalV2, ReplyV2, RequestIdV2, RequestV2,
    RoleV2, RoomEventV2, StableReferenceAvailabilityV1, ThreadLifecycleStateV1,
    ThreadedRoomEventV1, TopicIdV1, TopicRoomEventV1,
};
use agora_service::v2::{ServiceLimitsV2, ServiceV2};

fn request(id: u64, command: CommandV2) -> RequestV2 {
    RequestV2 {
        request_id: RequestIdV2(id),
        command,
    }
}

#[test]
fn v2_attention_preferences_are_actor_bound_replayable_and_nonpublishing() {
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
    let revision = service.engine().revision().0;
    let events = service.engine().pending_events().count();
    let preference = AttentionPreferenceV1 {
        following: true,
        muted: true,
        notification_intent: NotificationIntentV1::AddressedOnly,
    };
    let set = request(
        1,
        CommandV2::SetAttentionPreference {
            room_id: "room".to_owned(),
            observed_revision: revision,
            scope: AttentionScopeV1::Room,
            preference,
        },
    );
    let accepted = ReplyV2::Accepted(OutcomeV2::AttentionPreferenceSet(preference));
    assert_eq!(service.execute(alice, set.clone()), accepted);
    assert_eq!(service.execute(alice, set), accepted);
    assert_eq!(service.engine().revision().0, revision);
    assert_eq!(service.engine().pending_events().count(), events);
    assert_eq!(
        service.execute(
            alice,
            request(
                2,
                CommandV2::AttentionPreference {
                    room_id: "room".to_owned(),
                    scope: AttentionScopeV1::Room,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::AttentionPreference(preference))
    );
    assert_eq!(
        service.execute(
            bot,
            request(
                1,
                CommandV2::AttentionPreference {
                    room_id: "room".to_owned(),
                    scope: AttentionScopeV1::Room,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::AttentionPreference(
            AttentionPreferenceV1::default()
        ))
    );
}

#[test]
fn v2_pins_are_shared_while_saved_references_are_private_and_nonpublishing() {
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
        revision,
        message_id,
        sequence,
    }) = service.execute(
        alice,
        request(
            1,
            CommandV2::SpeakIdentified {
                room_id: "room".to_owned(),
                expected_revision: 4,
                logical_time: 1,
                body: "payload stays in message retention".to_owned(),
                addressed_to: vec![],
            },
        ),
    )
    else {
        panic!("identified message expected");
    };
    let event_count = service.engine().pending_events().count();
    let save = request(
        2,
        CommandV2::SetSavedReference {
            room_id: "room".to_owned(),
            observed_revision: revision,
            message_id,
            saved: true,
        },
    );
    let saved = service.execute(alice, save.clone());
    assert_eq!(service.execute(alice, save), saved);
    assert!(matches!(
        saved,
        ReplyV2::Accepted(OutcomeV2::SavedReferenceSet {
            reference,
            saved: true,
        }) if reference.message_id == message_id
            && reference.original_sequence == sequence
            && reference.availability == StableReferenceAvailabilityV1::Available
    ));
    assert_eq!(service.engine().revision().0, revision);
    assert_eq!(service.engine().pending_events().count(), event_count);
    assert!(matches!(
        service.execute(
            alice,
            request(3, CommandV2::SavedReferences { room_id: "room".to_owned() }),
        ),
        ReplyV2::Accepted(OutcomeV2::SavedReferences(references))
            if references.len() == 1 && references[0].message_id == message_id
    ));
    assert_eq!(
        service.execute(
            bot,
            request(
                1,
                CommandV2::SavedReferences {
                    room_id: "room".to_owned()
                }
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::SavedReferences(vec![]))
    );

    assert!(matches!(
        service.execute(
            bot,
            request(
                2,
                CommandV2::SetPin {
                    room_id: "room".to_owned(),
                    expected_revision: revision,
                    message_id,
                    pinned: true,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::PinChanged {
            revision: pin_revision,
            message_id: pinned_message_id,
            original_sequence,
            pinned: true,
        }) if pin_revision == revision + 1
            && pinned_message_id == message_id
            && original_sequence == sequence
    ));
    assert!(matches!(
        service.execute(
            alice,
            request(4, CommandV2::Pins { room_id: "room".to_owned() }),
        ),
        ReplyV2::Accepted(OutcomeV2::Pins(pins))
            if pins.len() == 1 && pins[0].message_id == message_id && pins[0].pinned_by == "bot"
    ));
}

#[test]
fn v2_peer_conversations_bind_initiator_and_expose_profile_only_to_current_members() {
    let mut engine = Engine::new();
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
    let mut service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    let bot = service.connect("bot").unwrap();
    let outsider = service.connect("outsider").unwrap();

    let create = request(
        1,
        CommandV2::CreatePeerConversation {
            room_id: "direct".to_owned(),
            name: "Direct".to_owned(),
            profile: ConversationProfileV1::Direct,
            founding_participant_ids: vec!["alice".to_owned(), "bot".to_owned()],
            expected_revision: 3,
        },
    );
    assert_eq!(
        service.execute(alice, create.clone()),
        ReplyV2::Accepted(OutcomeV2::PeerConversationCreated {
            revision: 4,
            profile: ConversationProfileV1::Direct,
        })
    );
    assert_eq!(
        service.execute(alice, create),
        ReplyV2::Accepted(OutcomeV2::PeerConversationCreated {
            revision: 4,
            profile: ConversationProfileV1::Direct,
        })
    );

    assert!(matches!(
        service.execute(
            bot,
            request(
                1,
                CommandV2::ConversationProfile {
                    room_id: "direct".to_owned(),
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::ConversationProfile(status))
            if status.profile == ConversationProfileV1::Direct
                && status.founding_participant_ids == vec!["alice", "bot"]
    ));
    assert_eq!(
        service.execute(
            outsider,
            request(
                1,
                CommandV2::ConversationProfile {
                    room_id: "direct".to_owned(),
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::NotMember)
    );
    assert_eq!(
        service.execute(
            alice,
            request(
                2,
                CommandV2::Leave {
                    room_id: "direct".to_owned(),
                    expected_revision: 4,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::Left { revision: 5 })
    );
    assert_eq!(
        service.execute(
            alice,
            request(
                3,
                CommandV2::Join {
                    room_id: "direct".to_owned(),
                    expected_revision: 5,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::ImmutableConversationCohort)
    );
}

#[test]
fn v2_private_read_positions_are_actor_bound_replayable_and_nonpublishing() {
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
        message_id: first,
        revision,
        ..
    }) = service.execute(
        alice,
        request(
            1,
            CommandV2::SpeakIdentified {
                room_id: "room".to_owned(),
                expected_revision: 4,
                logical_time: 1,
                body: "first".to_owned(),
                addressed_to: vec![],
            },
        ),
    )
    else {
        panic!("identified message expected");
    };
    assert_eq!(
        service.execute(
            bot,
            request(
                1,
                CommandV2::ReadPosition {
                    room_id: "room".to_owned(),
                    scope: ReadScopeV1::Room,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::ReadPosition(None))
    );
    let events = service.engine().pending_events().count();
    let advance = request(
        2,
        CommandV2::AdvanceReadPosition {
            room_id: "room".to_owned(),
            observed_revision: revision,
            scope: ReadScopeV1::Room,
            target_message_id: first,
        },
    );
    let accepted = service.execute(alice, advance.clone());
    assert!(matches!(
        &accepted,
        ReplyV2::Accepted(OutcomeV2::ReadPositionAdvanced(position))
            if position.target_message_id == first
                && position.observed_revision == revision
                && position.scope == ReadScopeV1::Room
    ));
    assert_eq!(service.engine().revision().0, revision);
    assert_eq!(service.engine().pending_events().count(), events);
    assert_eq!(service.execute(alice, advance), accepted);
    assert_eq!(
        service.execute(
            bot,
            request(
                2,
                CommandV2::ReadPosition {
                    room_id: "room".to_owned(),
                    scope: ReadScopeV1::Room,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::ReadPosition(None))
    );
    assert!(matches!(
        service.execute(
            alice,
            request(
                3,
                CommandV2::ReadPosition {
                    room_id: "room".to_owned(),
                    scope: ReadScopeV1::Room,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::ReadPosition(Some(position)))
            if position.target_message_id == first
    ));

    let ReplyV2::Accepted(OutcomeV2::IdentifiedMessageAccepted {
        message_id: second,
        revision: second_revision,
        ..
    }) = service.execute(
        alice,
        request(
            4,
            CommandV2::SpeakIdentified {
                room_id: "room".to_owned(),
                expected_revision: revision,
                logical_time: 2,
                body: "second".to_owned(),
                addressed_to: vec![],
            },
        ),
    )
    else {
        panic!("second message expected");
    };
    assert!(matches!(
        service.execute(
            alice,
            request(
                5,
                CommandV2::AdvanceReadPosition {
                    room_id: "room".to_owned(),
                    observed_revision: second_revision,
                    scope: ReadScopeV1::Room,
                    target_message_id: second,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::ReadPositionAdvanced(position))
            if position.target_message_id == second
    ));
    assert_eq!(
        service.execute(
            alice,
            request(
                6,
                CommandV2::AdvanceReadPosition {
                    room_id: "room".to_owned(),
                    observed_revision: second_revision,
                    scope: ReadScopeV1::Room,
                    target_message_id: first,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::ReadPositionRegression)
    );
}

#[test]
fn v2_topics_are_kind_neutral_idempotent_bounded_and_relation_preserving() {
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

    let ReplyV2::Accepted(OutcomeV2::ChangesOpened {
        cursor: topic_change_cursor,
        ..
    }) = service.execute(
        alice,
        request(
            0,
            CommandV2::OpenTopicChanges {
                room_id: "room".to_owned(),
                observed_revision: 4,
            },
        ),
    )
    else {
        panic!("topic-aware change position expected");
    };
    let create = request(
        1,
        CommandV2::CreateTopic {
            room_id: "room".to_owned(),
            expected_revision: 4,
            name: "incidents".to_owned(),
        },
    );
    let accepted = service.execute(alice, create.clone());
    let ReplyV2::Accepted(OutcomeV2::TopicCreated {
        revision: 5,
        topic_id,
    }) = accepted
    else {
        panic!("topic creation expected");
    };
    assert_eq!(service.execute(alice, create), accepted);

    let ReplyV2::Accepted(OutcomeV2::TopicMessageAccepted {
        revision: 6,
        message_id,
        topic_id: accepted_topic,
        sequence: 0,
    }) = service.execute(
        bot,
        request(
            1,
            CommandV2::SpeakInTopic {
                room_id: "room".to_owned(),
                expected_revision: 5,
                topic_id,
                logical_time: 7,
                body: "agent evidence".to_owned(),
                addressed_to: vec!["alice".to_owned()],
            },
        ),
    )
    else {
        panic!("topic message acceptance expected");
    };
    assert_eq!(accepted_topic, topic_id);

    assert_eq!(
        service.execute(
            bot,
            request(
                4,
                CommandV2::RenameTopic {
                    room_id: "room".to_owned(),
                    expected_revision: 6,
                    topic_id,
                    name: "operations".to_owned(),
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::NotAuthorized)
    );
    let rename = request(
        4,
        CommandV2::RenameTopic {
            room_id: "room".to_owned(),
            expected_revision: 6,
            topic_id,
            name: "operations".to_owned(),
        },
    );
    let renamed = ReplyV2::Accepted(OutcomeV2::TopicRenamed {
        revision: 7,
        topic_id,
    });
    assert_eq!(service.execute(alice, rename.clone()), renamed);
    assert_eq!(service.execute(alice, rename), renamed);
    assert_eq!(
        service.execute(
            alice,
            request(
                4,
                CommandV2::RenameTopic {
                    room_id: "room".to_owned(),
                    expected_revision: 7,
                    topic_id,
                    name: "changed".to_owned(),
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::RequestConflict)
    );
    assert_eq!(
        service.execute(
            bot,
            request(
                5,
                CommandV2::ResolveTopic {
                    room_id: "room".to_owned(),
                    expected_revision: 7,
                    topic_id,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::NotAuthorized)
    );
    let resolve = request(
        5,
        CommandV2::ResolveTopic {
            room_id: "room".to_owned(),
            expected_revision: 7,
            topic_id,
        },
    );
    let resolved = ReplyV2::Accepted(OutcomeV2::TopicResolved {
        revision: 8,
        topic_id,
    });
    assert_eq!(service.execute(alice, resolve.clone()), resolved);
    assert_eq!(service.execute(alice, resolve), resolved);

    let ReplyV2::Accepted(OutcomeV2::TopicChanges(changes)) = service.execute(
        alice,
        request(
            3,
            CommandV2::TopicChanges {
                room_id: "room".to_owned(),
                cursor: topic_change_cursor,
                limit: 4,
            },
        ),
    ) else {
        panic!("topic-aware changes expected");
    };
    assert!(matches!(
        &changes.changes[0].event,
        agora_protocol::v2::TopicRoomEventV1::TopicCreated {
            topic_id: event_topic,
            ..
        } if *event_topic == topic_id
    ));
    assert!(matches!(
        &changes.changes[1].event,
        agora_protocol::v2::TopicRoomEventV1::MessageAccepted(message)
            if message.topic_id == Some(topic_id) && message.message_id == message_id
    ));
    assert!(matches!(
        &changes.changes[2].event,
        agora_protocol::v2::TopicRoomEventV1::TopicRenamed {
            actor_id,
            topic_id: event_topic,
            previous,
            name,
        } if actor_id == "alice"
            && *event_topic == topic_id
            && previous == "incidents"
            && name == "operations"
    ));
    assert!(matches!(
        &changes.changes[3].event,
        agora_protocol::v2::TopicRoomEventV1::TopicLifecycleChanged {
            actor_id,
            topic_id: event_topic,
            previous_resolved: false,
            resolved: true,
        } if actor_id == "alice" && *event_topic == topic_id
    ));

    let ReplyV2::Accepted(OutcomeV2::Topics(topics)) = service.execute(
        alice,
        request(
            2,
            CommandV2::Topics {
                room_id: "room".to_owned(),
                cursor: None,
                limit: 1,
            },
        ),
    ) else {
        panic!("topic page expected");
    };
    assert_eq!(topics.topics.len(), 1);
    assert_eq!(topics.topics[0].topic_id, topic_id);
    assert_eq!(topics.topics[0].name, "operations");
    assert_eq!(topics.topics[0].creator_id, "alice");
    assert!(matches!(
        service.execute(
            alice,
            request(
                7,
                CommandV2::TopicStatus {
                    room_id: "room".to_owned(),
                    topic_id,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::TopicStatus(status))
            if status.revision == 8 && status.topic_id == topic_id && status.resolved
    ));

    assert_eq!(
        service.execute(
            bot,
            request(
                6,
                CommandV2::SpeakInTopic {
                    room_id: "room".to_owned(),
                    expected_revision: 8,
                    topic_id,
                    logical_time: 8,
                    body: "blocked".to_owned(),
                    addressed_to: vec![],
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::TopicResolved)
    );
    assert_eq!(
        service.execute(
            alice,
            request(
                6,
                CommandV2::ReopenTopic {
                    room_id: "room".to_owned(),
                    expected_revision: 8,
                    topic_id,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::TopicReopened {
            revision: 9,
            topic_id,
        })
    );

    let ReplyV2::Accepted(OutcomeV2::TopicMessages(messages)) = service.execute(
        bot,
        request(
            2,
            CommandV2::TopicMessages {
                room_id: "room".to_owned(),
                topic_id,
                cursor: None,
                limit: 1,
            },
        ),
    ) else {
        panic!("topic messages expected");
    };
    assert_eq!(messages.messages.len(), 1);
    assert_eq!(messages.messages[0].message_id, message_id);
    assert_eq!(messages.messages[0].topic_id, Some(topic_id));
    assert_eq!(messages.messages[0].addressed_to, ["alice"]);

    assert_eq!(
        service.execute(
            bot,
            request(
                3,
                CommandV2::SpeakInTopic {
                    room_id: "room".to_owned(),
                    expected_revision: 9,
                    topic_id: TopicIdV1::new([0xff; 16]),
                    logical_time: 8,
                    body: "missing".to_owned(),
                    addressed_to: vec![],
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::UnknownTopic)
    );
    assert_eq!(
        service.execute(
            alice,
            request(
                8,
                CommandV2::TopicStatus {
                    room_id: "room".to_owned(),
                    topic_id: TopicIdV1::new([0xff; 16]),
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::UnknownTopic)
    );
    assert_eq!(service.engine().revision().0, 9);
}

#[test]
fn v2_topic_supersession_is_actor_bound_replayable_observable_and_feed_visible() {
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

    let ReplyV2::Accepted(OutcomeV2::TopicCreated {
        topic_id: source,
        revision: 5,
    }) = service.execute(
        alice,
        request(
            1,
            CommandV2::CreateTopic {
                room_id: "room".to_owned(),
                expected_revision: 4,
                name: "source".to_owned(),
            },
        ),
    )
    else {
        panic!("source topic expected");
    };
    let ReplyV2::Accepted(OutcomeV2::TopicCreated {
        topic_id: successor,
        revision: 6,
    }) = service.execute(
        alice,
        request(
            2,
            CommandV2::CreateTopic {
                room_id: "room".to_owned(),
                expected_revision: 5,
                name: "successor".to_owned(),
            },
        ),
    )
    else {
        panic!("successor topic expected");
    };
    assert_eq!(
        service.execute(
            bot,
            request(
                1,
                CommandV2::SupersedeTopic {
                    room_id: "room".to_owned(),
                    expected_revision: 6,
                    topic_id: source,
                    successor_topic_id: successor,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::NotAuthorized)
    );
    assert_eq!(
        service.execute(
            alice,
            request(
                3,
                CommandV2::SupersedeTopic {
                    room_id: "room".to_owned(),
                    expected_revision: 6,
                    topic_id: source,
                    successor_topic_id: successor,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::TopicMustBeResolved)
    );
    assert!(matches!(
        service.execute(
            alice,
            request(
                4,
                CommandV2::ResolveTopic {
                    room_id: "room".to_owned(),
                    expected_revision: 6,
                    topic_id: source,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::TopicResolved { revision: 7, .. })
    ));
    let ReplyV2::Accepted(OutcomeV2::ChangesOpened { cursor, .. }) = service.execute(
        alice,
        request(
            5,
            CommandV2::OpenTopicChanges {
                room_id: "room".to_owned(),
                observed_revision: 7,
            },
        ),
    ) else {
        panic!("change cursor expected");
    };
    let supersede = request(
        6,
        CommandV2::SupersedeTopic {
            room_id: "room".to_owned(),
            expected_revision: 7,
            topic_id: source,
            successor_topic_id: successor,
        },
    );
    let accepted = ReplyV2::Accepted(OutcomeV2::TopicSuperseded {
        revision: 8,
        topic_id: source,
        successor_topic_id: successor,
    });
    assert_eq!(service.execute(alice, supersede.clone()), accepted);
    assert_eq!(service.execute(alice, supersede), accepted);
    assert!(matches!(
        service.execute(
            alice,
            request(
                7,
                CommandV2::TopicSupersessionStatus {
                    room_id: "room".to_owned(),
                    topic_id: source,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::TopicSupersessionStatus(status))
            if status.revision == 8 && status.successor_topic_id == Some(successor)
    ));
    let ReplyV2::Accepted(OutcomeV2::TopicChanges(page)) = service.execute(
        alice,
        request(
            8,
            CommandV2::TopicChanges {
                room_id: "room".to_owned(),
                cursor,
                limit: 1,
            },
        ),
    ) else {
        panic!("supersession feed event expected");
    };
    assert!(matches!(
        &page.changes[0].event,
        agora_protocol::v2::TopicRoomEventV1::TopicSupersessionChanged {
            actor_id,
            topic_id,
            previous_successor_topic_id: None,
            successor_topic_id: Some(event_successor),
        } if actor_id == "alice" && *topic_id == source && *event_successor == successor
    ));
    assert_eq!(
        service.execute(
            alice,
            request(
                9,
                CommandV2::ReopenTopic {
                    room_id: "room".to_owned(),
                    expected_revision: 8,
                    topic_id: source,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::InvalidInput)
    );
    assert!(matches!(
        service.execute(
            alice,
            request(
                10,
                CommandV2::ClearTopicSupersession {
                    room_id: "room".to_owned(),
                    expected_revision: 8,
                    topic_id: source,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::TopicSupersessionCleared { revision: 9, .. })
    ));
}

#[test]
fn v2_topic_aliases_are_actor_bound_replayable_collision_safe_and_visible_in_every_feed() {
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

    assert!(matches!(
        service.execute(
            alice,
            request(
                1,
                CommandV2::SetRole {
                    room_id: "room".to_owned(),
                    participant_id: "bot".to_owned(),
                    role: RoleV2::Moderator,
                    expected_revision: 4,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::RoleChanged { revision: 5 })
    ));
    let ReplyV2::Accepted(OutcomeV2::TopicCreated {
        topic_id,
        revision: 6,
    }) = service.execute(
        alice,
        request(
            2,
            CommandV2::CreateTopic {
                room_id: "room".to_owned(),
                expected_revision: 5,
                name: "operations".to_owned(),
            },
        ),
    )
    else {
        panic!("topic expected");
    };
    let ReplyV2::Accepted(OutcomeV2::TopicCreated {
        topic_id: other_topic_id,
        revision: 7,
    }) = service.execute(
        alice,
        request(
            3,
            CommandV2::CreateTopic {
                room_id: "room".to_owned(),
                expected_revision: 6,
                name: "current".to_owned(),
            },
        ),
    )
    else {
        panic!("other topic expected");
    };

    let ReplyV2::Accepted(OutcomeV2::ChangesOpened {
        cursor: room_cursor,
        ..
    }) = service.execute(
        alice,
        request(
            4,
            CommandV2::OpenChanges {
                room_id: "room".to_owned(),
                observed_revision: 7,
            },
        ),
    )
    else {
        panic!("room cursor expected");
    };
    let ReplyV2::Accepted(OutcomeV2::ChangesOpened {
        cursor: identified_cursor,
        ..
    }) = service.execute(
        alice,
        request(
            5,
            CommandV2::OpenIdentifiedChanges {
                room_id: "room".to_owned(),
                observed_revision: 7,
            },
        ),
    )
    else {
        panic!("identified cursor expected");
    };
    let ReplyV2::Accepted(OutcomeV2::ChangesOpened {
        cursor: threaded_cursor,
        ..
    }) = service.execute(
        alice,
        request(
            6,
            CommandV2::OpenThreadedChanges {
                room_id: "room".to_owned(),
                observed_revision: 7,
            },
        ),
    )
    else {
        panic!("threaded cursor expected");
    };
    let ReplyV2::Accepted(OutcomeV2::ChangesOpened {
        cursor: topic_cursor,
        ..
    }) = service.execute(
        alice,
        request(
            7,
            CommandV2::OpenTopicChanges {
                room_id: "room".to_owned(),
                observed_revision: 7,
            },
        ),
    )
    else {
        panic!("topic cursor expected");
    };

    let add = request(
        1,
        CommandV2::AddTopicAlias {
            room_id: "room".to_owned(),
            expected_revision: 7,
            topic_id,
            alias: "incidents".to_owned(),
        },
    );
    let accepted = ReplyV2::Accepted(OutcomeV2::TopicAliasAdded {
        revision: 8,
        topic_id,
        alias: "incidents".to_owned(),
    });
    assert_eq!(service.execute(bot, add.clone()), accepted);
    assert_eq!(service.execute(bot, add), accepted);
    assert_eq!(
        service.execute(
            bot,
            request(
                1,
                CommandV2::AddTopicAlias {
                    room_id: "room".to_owned(),
                    expected_revision: 8,
                    topic_id,
                    alias: "conflict".to_owned(),
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::RequestConflict)
    );
    assert!(matches!(
        service.execute(
            bot,
            request(
                2,
                CommandV2::TopicAliasStatus {
                    room_id: "room".to_owned(),
                    topic_id,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::TopicAliasStatus(status))
            if status.revision == 8 && status.aliases == vec!["incidents"]
    ));

    for (request_id, command) in [
        (
            8,
            CommandV2::CreateTopic {
                room_id: "room".to_owned(),
                expected_revision: 8,
                name: "incidents".to_owned(),
            },
        ),
        (
            9,
            CommandV2::RenameTopic {
                room_id: "room".to_owned(),
                expected_revision: 8,
                topic_id: other_topic_id,
                name: "incidents".to_owned(),
            },
        ),
        (
            10,
            CommandV2::AddTopicAlias {
                room_id: "room".to_owned(),
                expected_revision: 8,
                topic_id,
                alias: "current".to_owned(),
            },
        ),
    ] {
        assert_eq!(
            service.execute(alice, request(request_id, command)),
            ReplyV2::Refused(RefusalV2::InvalidInput)
        );
    }
    assert_eq!(
        service.execute(
            alice,
            request(
                11,
                CommandV2::RemoveTopicAlias {
                    room_id: "room".to_owned(),
                    expected_revision: 8,
                    topic_id,
                    alias: "missing".to_owned(),
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::UnknownTopicAlias)
    );

    let ReplyV2::Accepted(OutcomeV2::Changes(page)) = service.execute(
        alice,
        request(
            12,
            CommandV2::Changes {
                room_id: "room".to_owned(),
                cursor: room_cursor,
                limit: 1,
            },
        ),
    ) else {
        panic!("room alias event expected");
    };
    assert!(matches!(
        &page.changes[0].event,
        RoomEventV2::TopicAliasChanged { actor_id, topic_id: id, alias, added }
            if actor_id == "bot" && *id == topic_id && alias == "incidents" && *added
    ));
    let ReplyV2::Accepted(OutcomeV2::IdentifiedChanges(page)) = service.execute(
        alice,
        request(
            13,
            CommandV2::IdentifiedChanges {
                room_id: "room".to_owned(),
                cursor: identified_cursor,
                limit: 1,
            },
        ),
    ) else {
        panic!("identified alias event expected");
    };
    assert!(matches!(
        &page.changes[0].event,
        IdentifiedRoomEventV1::TopicAliasChanged { actor_id, topic_id: id, alias, added }
            if actor_id == "bot" && *id == topic_id && alias == "incidents" && *added
    ));
    let ReplyV2::Accepted(OutcomeV2::ThreadedChanges(page)) = service.execute(
        alice,
        request(
            14,
            CommandV2::ThreadedChanges {
                room_id: "room".to_owned(),
                cursor: threaded_cursor,
                limit: 1,
            },
        ),
    ) else {
        panic!("threaded alias event expected");
    };
    assert!(matches!(
        &page.changes[0].event,
        ThreadedRoomEventV1::TopicAliasChanged { actor_id, topic_id: id, alias, added }
            if actor_id == "bot" && *id == topic_id && alias == "incidents" && *added
    ));
    let ReplyV2::Accepted(OutcomeV2::TopicChanges(page)) = service.execute(
        alice,
        request(
            15,
            CommandV2::TopicChanges {
                room_id: "room".to_owned(),
                cursor: topic_cursor,
                limit: 1,
            },
        ),
    ) else {
        panic!("topic alias event expected");
    };
    assert!(matches!(
        &page.changes[0].event,
        TopicRoomEventV1::TopicAliasChanged { actor_id, topic_id: id, alias, added }
            if actor_id == "bot" && *id == topic_id && alias == "incidents" && *added
    ));
}

#[test]
fn v2_topic_tags_are_actor_bound_replayable_and_visible_in_every_feed() {
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
    assert!(matches!(
        service.execute(
            alice,
            request(
                1,
                CommandV2::SetRole {
                    room_id: "room".to_owned(),
                    participant_id: "bot".to_owned(),
                    role: RoleV2::Moderator,
                    expected_revision: 4,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::RoleChanged { revision: 5 })
    ));
    let ReplyV2::Accepted(OutcomeV2::TopicCreated {
        topic_id,
        revision: 6,
    }) = service.execute(
        alice,
        request(
            2,
            CommandV2::CreateTopic {
                room_id: "room".to_owned(),
                expected_revision: 5,
                name: "operations".to_owned(),
            },
        ),
    )
    else {
        panic!("topic expected");
    };

    let mut cursors = Vec::new();
    for (id, command) in [
        (
            3,
            CommandV2::OpenChanges {
                room_id: "room".to_owned(),
                observed_revision: 6,
            },
        ),
        (
            4,
            CommandV2::OpenIdentifiedChanges {
                room_id: "room".to_owned(),
                observed_revision: 6,
            },
        ),
        (
            5,
            CommandV2::OpenThreadedChanges {
                room_id: "room".to_owned(),
                observed_revision: 6,
            },
        ),
        (
            6,
            CommandV2::OpenTopicChanges {
                room_id: "room".to_owned(),
                observed_revision: 6,
            },
        ),
    ] {
        let ReplyV2::Accepted(OutcomeV2::ChangesOpened { cursor, .. }) =
            service.execute(alice, request(id, command))
        else {
            panic!("change cursor expected");
        };
        cursors.push(cursor);
    }

    let add = request(
        1,
        CommandV2::AddTopicTag {
            room_id: "room".to_owned(),
            expected_revision: 6,
            topic_id,
            tag: "urgent".to_owned(),
        },
    );
    let accepted = ReplyV2::Accepted(OutcomeV2::TopicTagAdded {
        revision: 7,
        topic_id,
        tag: "urgent".to_owned(),
    });
    assert_eq!(service.execute(bot, add.clone()), accepted);
    assert_eq!(service.execute(bot, add), accepted);
    assert_eq!(
        service.execute(
            bot,
            request(
                1,
                CommandV2::AddTopicTag {
                    room_id: "room".to_owned(),
                    expected_revision: 7,
                    topic_id,
                    tag: "conflict".to_owned(),
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::RequestConflict)
    );
    assert!(matches!(
        service.execute(
            alice,
            request(
                7,
                CommandV2::TopicTagStatus {
                    room_id: "room".to_owned(),
                    topic_id,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::TopicTagStatus(status))
            if status.revision == 7 && status.tags == vec!["urgent"]
    ));
    assert_eq!(
        service.execute(
            alice,
            request(
                8,
                CommandV2::AddTopicTag {
                    room_id: "room".to_owned(),
                    expected_revision: 7,
                    topic_id,
                    tag: "urgent".to_owned(),
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::DuplicateTopicTag)
    );
    assert_eq!(
        service.execute(
            alice,
            request(
                9,
                CommandV2::RemoveTopicTag {
                    room_id: "room".to_owned(),
                    expected_revision: 7,
                    topic_id,
                    tag: "missing".to_owned(),
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::UnknownTopicTag)
    );

    let replies = [
        service.execute(
            alice,
            request(
                10,
                CommandV2::Changes {
                    room_id: "room".to_owned(),
                    cursor: cursors[0],
                    limit: 1,
                },
            ),
        ),
        service.execute(
            alice,
            request(
                11,
                CommandV2::IdentifiedChanges {
                    room_id: "room".to_owned(),
                    cursor: cursors[1],
                    limit: 1,
                },
            ),
        ),
        service.execute(
            alice,
            request(
                12,
                CommandV2::ThreadedChanges {
                    room_id: "room".to_owned(),
                    cursor: cursors[2],
                    limit: 1,
                },
            ),
        ),
        service.execute(
            alice,
            request(
                13,
                CommandV2::TopicChanges {
                    room_id: "room".to_owned(),
                    cursor: cursors[3],
                    limit: 1,
                },
            ),
        ),
    ];
    assert!(matches!(
        &replies[0],
        ReplyV2::Accepted(OutcomeV2::Changes(page))
            if matches!(&page.changes[0].event, RoomEventV2::TopicTagChanged { actor_id, topic_id: id, tag, added } if actor_id == "bot" && *id == topic_id && tag == "urgent" && *added)
    ));
    assert!(matches!(
        &replies[1],
        ReplyV2::Accepted(OutcomeV2::IdentifiedChanges(page))
            if matches!(&page.changes[0].event, IdentifiedRoomEventV1::TopicTagChanged { actor_id, topic_id: id, tag, added } if actor_id == "bot" && *id == topic_id && tag == "urgent" && *added)
    ));
    assert!(matches!(
        &replies[2],
        ReplyV2::Accepted(OutcomeV2::ThreadedChanges(page))
            if matches!(&page.changes[0].event, ThreadedRoomEventV1::TopicTagChanged { actor_id, topic_id: id, tag, added } if actor_id == "bot" && *id == topic_id && tag == "urgent" && *added)
    ));
    assert!(matches!(
        &replies[3],
        ReplyV2::Accepted(OutcomeV2::TopicChanges(page))
            if matches!(&page.changes[0].event, TopicRoomEventV1::TopicTagChanged { actor_id, topic_id: id, tag, added } if actor_id == "bot" && *id == topic_id && tag == "urgent" && *added)
    ));
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
    participant_engine_with_limits(Limits::default())
}

fn participant_engine_with_limits(limits: Limits) -> Engine {
    let mut engine = Engine::with_limits(limits);
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
fn v2_change_feed_reports_independent_journal_retention() {
    let engine = participant_engine_with_limits(Limits {
        max_pending_events: 16,
        max_retained_changes: 1,
        ..Limits::default()
    });
    let mut service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
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
    assert!(matches!(
        service.execute(
            alice,
            request(
                2,
                CommandV2::Speak {
                    room_id: "room".to_owned(),
                    expected_revision: 5,
                    logical_time: 1,
                    body: "will be evicted from pull history".to_owned(),
                    addressed_to: vec![],
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::MessageAccepted { revision: 6, .. })
    ));
    assert!(matches!(
        service.execute(
            alice,
            request(
                3,
                CommandV2::CreateRoom {
                    room_id: "other".to_owned(),
                    name: "Other".to_owned(),
                    admission: AdmissionPolicyV2::Open,
                    expected_revision: 6,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::RoomCreated { revision: 7 })
    ));
    assert_eq!(
        service.execute(
            alice,
            request(
                4,
                CommandV2::Changes {
                    room_id: "room".to_owned(),
                    cursor,
                    limit: 1,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::ChangeGap {
            current_revision: 7,
            cause: ChangeGapV2::ChangeRetention {
                discarded_through: 5,
            },
        })
    );
    assert_eq!(service.engine().pending_events().count(), 7);
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
fn v2_message_revisions_are_author_bound_paged_and_visible_in_every_feed() {
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
    let ReplyV2::Accepted(OutcomeV2::IdentifiedMessageAccepted { message_id, .. }) = service
        .execute(
            bot,
            request(
                1,
                CommandV2::SpeakIdentified {
                    room_id: "room".to_owned(),
                    expected_revision: 4,
                    logical_time: 10,
                    body: "original".to_owned(),
                    addressed_to: vec!["alice".to_owned()],
                },
            ),
        )
    else {
        panic!("identified message expected");
    };
    assert!(matches!(
        service.execute(
            alice,
            request(
                1,
                CommandV2::AddReaction {
                    room_id: "room".to_owned(),
                    expected_revision: 5,
                    message_id,
                    key: "spark".to_owned(),
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::ReactionAdded { revision: 6, .. })
    ));

    let mut cursors = Vec::new();
    for (id, command) in [
        (
            2,
            CommandV2::OpenChanges {
                room_id: "room".to_owned(),
                observed_revision: 6,
            },
        ),
        (
            3,
            CommandV2::OpenIdentifiedChanges {
                room_id: "room".to_owned(),
                observed_revision: 6,
            },
        ),
        (
            4,
            CommandV2::OpenThreadedChanges {
                room_id: "room".to_owned(),
                observed_revision: 6,
            },
        ),
    ] {
        let ReplyV2::Accepted(OutcomeV2::ChangesOpened { cursor, .. }) =
            service.execute(alice, request(id, command))
        else {
            panic!("change cursor expected");
        };
        cursors.push(cursor);
    }

    assert_eq!(
        service.execute(
            alice,
            request(
                5,
                CommandV2::EditMessage {
                    room_id: "room".to_owned(),
                    expected_revision: 6,
                    message_id,
                    logical_time: 20,
                    body: "foreign edit".to_owned(),
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::NotAuthorized)
    );
    let edit = request(
        2,
        CommandV2::EditMessage {
            room_id: "room".to_owned(),
            expected_revision: 6,
            message_id,
            logical_time: 20,
            body: "revised".to_owned(),
        },
    );
    let edited = service.execute(bot, edit.clone());
    assert_eq!(
        edited,
        ReplyV2::Accepted(OutcomeV2::MessageEdited {
            revision: 7,
            message_id,
            message_revision: 1,
        })
    );
    assert_eq!(service.execute(bot, edit), edited);

    let ReplyV2::Accepted(OutcomeV2::IdentifiedMessages(messages)) = service.execute(
        alice,
        request(
            6,
            CommandV2::IdentifiedMessages {
                room_id: "room".to_owned(),
                cursor: None,
                limit: 4,
            },
        ),
    ) else {
        panic!("message page expected");
    };
    assert_eq!(messages.messages[0].body, "revised");
    assert_eq!(messages.messages[0].addressed_to, ["alice"]);

    let ReplyV2::Accepted(OutcomeV2::MessageRevisions(first)) = service.execute(
        alice,
        request(
            7,
            CommandV2::MessageRevisions {
                room_id: "room".to_owned(),
                message_id,
                cursor: None,
                limit: 1,
            },
        ),
    ) else {
        panic!("revision page expected");
    };
    assert_eq!(first.revisions.len(), 1);
    assert_eq!(first.revisions[0].number, 0);
    assert_eq!(first.revisions[0].body, "original");
    let next = first.next.unwrap();
    assert_eq!(
        service.execute(
            bot,
            request(
                3,
                CommandV2::MessageRevisions {
                    room_id: "room".to_owned(),
                    message_id,
                    cursor: Some(next),
                    limit: 1
                }
            )
        ),
        ReplyV2::Refused(RefusalV2::InvalidCursor)
    );
    let ReplyV2::Accepted(OutcomeV2::MessageRevisions(second)) = service.execute(
        alice,
        request(
            8,
            CommandV2::MessageRevisions {
                room_id: "room".to_owned(),
                message_id,
                cursor: Some(next),
                limit: 1,
            },
        ),
    ) else {
        panic!("revision continuation expected");
    };
    assert_eq!(second.revisions[0].number, 1);
    assert_eq!(second.revisions[0].editor_id, "bot");
    assert_eq!(second.revisions[0].body, "revised");
    assert!(second.next.is_none());

    let ReplyV2::Accepted(OutcomeV2::ReactionStatus(status)) = service.execute(
        alice,
        request(
            9,
            CommandV2::ReactionStatus {
                room_id: "room".to_owned(),
                message_id,
            },
        ),
    ) else {
        panic!("reaction status expected");
    };
    assert_eq!(status.reactions[0].key, "spark");

    for (id, cursor, profile) in [
        (10, cursors[0], 0_u8),
        (11, cursors[1], 1),
        (12, cursors[2], 2),
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
        let visible = match service.execute(alice, request(id, command)) {
            ReplyV2::Accepted(OutcomeV2::Changes(page)) => {
                matches!(&page.changes[0].event, RoomEventV2::MessageEdited { actor_id, message_id: id, message_revision: 1, logical_time: 20, body } if actor_id == "bot" && *id == message_id && body == "revised")
            }
            ReplyV2::Accepted(OutcomeV2::IdentifiedChanges(page)) => {
                matches!(&page.changes[0].event, IdentifiedRoomEventV1::MessageEdited { actor_id, message_id: id, message_revision: 1, logical_time: 20, body } if actor_id == "bot" && *id == message_id && body == "revised")
            }
            ReplyV2::Accepted(OutcomeV2::ThreadedChanges(page)) => {
                matches!(&page.changes[0].event, ThreadedRoomEventV1::MessageEdited { actor_id, message_id: id, message_revision: 1, logical_time: 20, body } if actor_id == "bot" && *id == message_id && body == "revised")
            }
            _ => false,
        };
        assert!(visible);
    }
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
    assert_eq!(capabilities.len(), 28);
    assert_eq!(
        capabilities.last(),
        Some(&CapabilityV2::PrivateSavedReferencesV1)
    );
    assert!(capabilities.contains(&CapabilityV2::SharedPinsV1));
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
