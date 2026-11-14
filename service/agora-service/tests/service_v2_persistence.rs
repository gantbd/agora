use agora_engine::{
    AdmissionPolicy, CommandV2 as EngineCommand, Engine, Limits, Participant, ParticipantId,
    ParticipantKind, ReadScopeV1 as EngineReadScopeV1, RoomId,
};
use agora_protocol::v2::{
    AttentionPreferenceV1, AttentionScopeV1, ClientIdV2, CommandV2, ConversationProfileV1,
    IdentifiedRoomEventV1, MessageIdV1, MessageRevisionAvailabilityV1, NotificationIntentV1,
    OutcomeV2, ReadScopeV1, RefusalV2, ReplyV2, RequestIdV2, RequestV2,
    StableReferenceAvailabilityV1, ThreadLifecycleStateV1, ThreadedRoomEventV1,
};
use agora_service::v2::{
    ServiceLimitsV2, ServiceV2,
    persistence::{ServiceCheckpointErrorV2, ServiceCheckpointScopeV1, ServiceRecoveryLimitsV2},
};

const MAX_CHECKPOINT: usize = 2 * 1_024 * 1_024;
const SCOPE: ServiceCheckpointScopeV1 = ServiceCheckpointScopeV1::new(*b"agora-test-scope");

fn bounds() -> ServiceRecoveryLimitsV2 {
    ServiceRecoveryLimitsV2 {
        max_snapshot_bytes: MAX_CHECKPOINT,
        max_engine_snapshot_bytes: MAX_CHECKPOINT,
        maximum_service_limits: ServiceLimitsV2::default(),
        maximum_engine_limits: Limits::default(),
    }
}

fn request(request_id: u64, command: CommandV2) -> RequestV2 {
    RequestV2 {
        request_id: RequestIdV2(request_id),
        command,
    }
}

#[test]
fn peer_conversation_profile_cohort_and_exact_receipts_survive_recovery() {
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
    let mut service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    let bot = service.connect("bot").unwrap();
    let create = request(
        1,
        CommandV2::CreatePeerConversation {
            room_id: "direct".to_owned(),
            name: "Direct".to_owned(),
            profile: ConversationProfileV1::Direct,
            founding_participant_ids: vec!["alice".to_owned(), "bot".to_owned()],
            expected_revision: 2,
        },
    );
    let created = service.execute(alice, create.clone());
    assert!(matches!(
        created,
        ReplyV2::Accepted(OutcomeV2::PeerConversationCreated {
            revision: 3,
            profile: ConversationProfileV1::Direct
        })
    ));
    let profile = request(
        1,
        CommandV2::ConversationProfile {
            room_id: "direct".to_owned(),
        },
    );
    let observed = service.execute(bot, profile.clone());
    service.execute(
        alice,
        request(
            2,
            CommandV2::Leave {
                room_id: "direct".to_owned(),
                expected_revision: 3,
            },
        ),
    );

    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    assert_eq!(checkpoint.bytes()[8], 26);
    let mut disguised_as_legacy = checkpoint.bytes().to_vec();
    disguised_as_legacy[8] = 24;
    assert_eq!(
        ServiceV2::recover_checkpoint(&disguised_as_legacy, SCOPE, bounds()).unwrap_err(),
        ServiceCheckpointErrorV2::Profile
    );
    let mut recovered = ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, bounds()).unwrap();
    assert_eq!(recovered.execute(alice, create), created);
    assert_eq!(recovered.execute(bot, profile), observed);
    assert_eq!(
        recovered.execute(
            alice,
            request(
                3,
                CommandV2::Join {
                    room_id: "direct".to_owned(),
                    expected_revision: 4,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::ImmutableConversationCohort)
    );
}

fn next_mutation_bit(state: &mut u64, byte_len: usize) -> usize {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    (*state as usize) % (byte_len * 8)
}

fn decode_hex(source: &str) -> Vec<u8> {
    let compact = source.split_whitespace().collect::<String>();
    (0..compact.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&compact[index..index + 2], 16).unwrap())
        .collect()
}

#[test]
fn legacy_service_checkpoint_migrates_to_the_current_canonical_envelope() {
    let legacy = decode_hex(include_str!("fixtures/service-checkpoint-v24.hex"));
    let recovered = ServiceV2::recover_checkpoint(&legacy, SCOPE, bounds()).unwrap();
    let migrated = recovered.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    assert_eq!(migrated.bytes()[8], 26);
    assert_ne!(migrated.bytes(), legacy);
    assert!(ServiceV2::recover_checkpoint(migrated.bytes(), SCOPE, bounds()).is_ok());
}

fn populated() -> (
    ServiceV2,
    ClientIdV2,
    ReplyV2,
    agora_protocol::v2::PageCursorV2,
) {
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
    let agora_engine::OutcomeV2::MessageAccepted { sequence, .. } = engine
        .apply(EngineCommand::Speak {
            room_id: "alpha".into(),
            author: "alice".into(),
            at: agora_engine::LogicalTime(8),
            body: "seed marker".to_owned(),
        })
        .unwrap()
    else {
        panic!("seed message expected");
    };
    let marker_target = engine
        .message(&RoomId::new("alpha"), sequence)
        .unwrap()
        .id();
    engine
        .advance_read_position(
            &ParticipantId::new("alice"),
            &RoomId::new("alpha"),
            EngineReadScopeV1::Room,
            engine.revision(),
            marker_target,
        )
        .unwrap();
    let mut service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    let _bot = service.connect("bot").unwrap();
    let mutation = service.execute(
        alice,
        request(
            10,
            CommandV2::Speak {
                room_id: "alpha".to_owned(),
                expected_revision: 6,
                logical_time: 9,
                body: "checkpoint me".to_owned(),
                addressed_to: vec![],
            },
        ),
    );
    let message_id = MessageIdV1::new(
        service
            .engine()
            .message(&RoomId::new("alpha"), 0)
            .unwrap()
            .id()
            .into_bytes(),
    );
    assert!(matches!(
        service.execute(
            alice,
            request(
                99,
                CommandV2::EditMessage {
                    room_id: "alpha".to_owned(),
                    expected_revision: 7,
                    message_id,
                    logical_time: 10,
                    body: "checkpoint revised".to_owned(),
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::MessageEdited { revision: 8, .. })
    ));
    let ReplyV2::Accepted(OutcomeV2::TopicCreated { topic_id, .. }) = service.execute(
        alice,
        request(
            100,
            CommandV2::CreateTopic {
                room_id: "alpha".to_owned(),
                expected_revision: 8,
                name: "incidents".to_owned(),
            },
        ),
    ) else {
        panic!("topic creation expected");
    };
    let ReplyV2::Accepted(OutcomeV2::TopicCreated {
        topic_id: successor_topic_id,
        revision: 10,
    }) = service.execute(
        alice,
        request(
            101,
            CommandV2::CreateTopic {
                room_id: "alpha".to_owned(),
                expected_revision: 9,
                name: "alerts".to_owned(),
            },
        ),
    )
    else {
        panic!("successor topic creation expected");
    };
    for (request_id, expected_revision, logical_time, body) in
        [(102, 10, 11, "topic one"), (103, 11, 12, "topic two")]
    {
        assert!(matches!(
            service.execute(
                alice,
                request(
                    request_id,
                    CommandV2::SpeakInTopic {
                        room_id: "alpha".to_owned(),
                        expected_revision,
                        topic_id,
                        logical_time,
                        body: body.to_owned(),
                        addressed_to: vec![],
                    },
                ),
            ),
            ReplyV2::Accepted(OutcomeV2::TopicMessageAccepted { .. })
        ));
    }
    assert!(matches!(
        service.execute(
            alice,
            request(
                107,
                CommandV2::RenameTopic {
                    room_id: "alpha".to_owned(),
                    expected_revision: 12,
                    topic_id,
                    name: "operations".to_owned(),
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::TopicRenamed { revision: 13, .. })
    ));
    assert!(matches!(
        service.execute(
            alice,
            request(
                108,
                CommandV2::ResolveTopic {
                    room_id: "alpha".to_owned(),
                    expected_revision: 13,
                    topic_id,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::TopicResolved { revision: 14, .. })
    ));
    assert!(matches!(
        service.execute(
            alice,
            request(
                109,
                CommandV2::SupersedeTopic {
                    room_id: "alpha".to_owned(),
                    expected_revision: 14,
                    topic_id,
                    successor_topic_id,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::TopicSuperseded { revision: 15, .. })
    ));
    assert!(matches!(
        service.execute(
            alice,
            request(
                110,
                CommandV2::AddTopicAlias {
                    room_id: "alpha".to_owned(),
                    expected_revision: 15,
                    topic_id,
                    alias: "incidents".to_owned(),
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::TopicAliasAdded { revision: 16, .. })
    ));
    assert!(matches!(
        service.execute(
            alice,
            request(
                111,
                CommandV2::AddTopicTag {
                    room_id: "alpha".to_owned(),
                    expected_revision: 16,
                    topic_id,
                    tag: "urgent".to_owned(),
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::TopicTagAdded { revision: 17, .. })
    ));
    let ReplyV2::Accepted(OutcomeV2::Topics(topic_page)) = service.execute(
        alice,
        request(
            104,
            CommandV2::Topics {
                room_id: "alpha".to_owned(),
                cursor: None,
                limit: 1,
            },
        ),
    ) else {
        panic!("topic page expected");
    };
    assert!(topic_page.next.is_some());
    let ReplyV2::Accepted(OutcomeV2::TopicMessages(topic_messages)) = service.execute(
        alice,
        request(
            105,
            CommandV2::TopicMessages {
                room_id: "alpha".to_owned(),
                topic_id,
                cursor: None,
                limit: 1,
            },
        ),
    ) else {
        panic!("topic message page expected");
    };
    assert!(topic_messages.next.is_some());
    assert!(matches!(
        service.execute(
            alice,
            request(
                106,
                CommandV2::OpenTopicChanges {
                    room_id: "alpha".to_owned(),
                    observed_revision: 17,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::ChangesOpened { revision: 17, .. })
    ));
    let ReplyV2::Accepted(OutcomeV2::Rooms(page)) = service.execute(
        alice,
        request(
            11,
            CommandV2::Rooms {
                cursor: None,
                limit: 2,
            },
        ),
    ) else {
        panic!("room page should be accepted");
    };
    (service, alice, mutation, page.next.unwrap())
}

fn topic_id_from_populated(service: &ServiceV2) -> agora_engine::TopicId {
    service
        .engine()
        .topic_page(&"alice".into(), &"alpha".into(), None, 1)
        .unwrap()
        .topics[0]
        .id()
}

#[test]
fn complete_service_state_round_trips_and_continues_exactly() {
    let (service, alice, mutation, cursor) = populated();
    let revision = service.engine().revision();
    let pending = service
        .engine()
        .pending_events()
        .cloned()
        .collect::<Vec<_>>();
    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let mut fixture = decode_hex(include_str!("fixtures/service-checkpoint-v24.hex"));
    fixture[8] = 26;
    assert_eq!(checkpoint.bytes(), fixture);
    let mut recovered = ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, bounds()).unwrap();

    assert_eq!(recovered.engine().revision(), revision);
    assert!(
        recovered
            .engine()
            .topic(&RoomId::new("alpha"), topic_id_from_populated(&recovered))
            .unwrap()
            .is_resolved()
    );
    assert!(
        recovered
            .engine()
            .topic_supersession_status(
                &ParticipantId::new("alice"),
                &RoomId::new("alpha"),
                topic_id_from_populated(&recovered),
            )
            .unwrap()
            .successor_topic_id
            .is_some()
    );
    assert_eq!(
        recovered
            .engine()
            .topic_alias_status(
                &ParticipantId::new("alice"),
                &RoomId::new("alpha"),
                topic_id_from_populated(&recovered),
            )
            .unwrap()
            .aliases,
        vec!["incidents".to_owned()]
    );
    assert_eq!(
        recovered
            .engine()
            .topic_tag_status(
                &ParticipantId::new("alice"),
                &RoomId::new("alpha"),
                topic_id_from_populated(&recovered),
            )
            .unwrap()
            .tags,
        vec!["urgent".to_owned()]
    );
    assert!(
        recovered
            .engine()
            .read_position(
                &ParticipantId::new("alice"),
                &RoomId::new("alpha"),
                &EngineReadScopeV1::Room,
            )
            .unwrap()
            .is_some()
    );
    assert_eq!(
        recovered
            .engine()
            .pending_events()
            .cloned()
            .collect::<Vec<_>>(),
        pending
    );
    assert_eq!(
        recovered
            .prepare_checkpoint(SCOPE, MAX_CHECKPOINT)
            .unwrap()
            .bytes(),
        checkpoint.bytes()
    );

    assert_eq!(
        recovered.execute(
            alice,
            request(
                10,
                CommandV2::Speak {
                    room_id: "alpha".to_owned(),
                    expected_revision: 6,
                    logical_time: 9,
                    body: "checkpoint me".to_owned(),
                    addressed_to: vec![],
                },
            ),
        ),
        mutation
    );
    assert_eq!(recovered.engine().revision(), revision);
    assert_eq!(
        recovered.execute(
            alice,
            request(
                10,
                CommandV2::Speak {
                    room_id: "alpha".to_owned(),
                    expected_revision: 6,
                    logical_time: 9,
                    body: "changed".to_owned(),
                    addressed_to: vec![],
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::RequestConflict)
    );

    let ReplyV2::Accepted(OutcomeV2::Rooms(page)) = recovered.execute(
        alice,
        request(
            12,
            CommandV2::Rooms {
                cursor: Some(cursor),
                limit: 2,
            },
        ),
    ) else {
        panic!("recovered cursor should continue");
    };
    assert_eq!(page.rooms.len(), 1);
    assert!(page.next.is_none());
}

#[test]
fn recovery_is_bounded_scope_bound_and_fail_closed() {
    let (service, _, _, _) = populated();
    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let bytes = checkpoint.bytes();

    assert_eq!(
        ServiceV2::recover_checkpoint(
            bytes,
            ServiceCheckpointScopeV1::new(*b"foreign-scope-v1"),
            bounds(),
        )
        .unwrap_err(),
        ServiceCheckpointErrorV2::Scope
    );
    let mut too_small = bounds();
    too_small.max_snapshot_bytes = bytes.len() - 1;
    assert_eq!(
        ServiceV2::recover_checkpoint(bytes, SCOPE, too_small).unwrap_err(),
        ServiceCheckpointErrorV2::Limit
    );

    for end in 0..bytes.len() {
        assert!(ServiceV2::recover_checkpoint(&bytes[..end], SCOPE, bounds()).is_err());
    }
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert_eq!(
        ServiceV2::recover_checkpoint(&trailing, SCOPE, bounds()).unwrap_err(),
        ServiceCheckpointErrorV2::Trailing
    );
    let mut future = bytes.to_vec();
    future[8] = 28;
    assert_eq!(
        ServiceV2::recover_checkpoint(&future, SCOPE, bounds()).unwrap_err(),
        ServiceCheckpointErrorV2::Profile
    );
    assert_eq!(format!("{checkpoint:?}"), "ServiceCheckpointV2(<redacted>)");
}

#[test]
fn fixed_seed_service_checkpoint_mutations_refuse_or_remain_canonical() {
    let (service, _, _, _) = populated();
    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let mut seed = 0x5e12_71ce_d15c_a11eu64;

    for sample in 0..512 {
        let bit = next_mutation_bit(&mut seed, checkpoint.bytes().len());
        let mut candidate = checkpoint.bytes().to_vec();
        candidate[bit / 8] ^= 1 << (bit % 8);
        if let Ok(recovered) = ServiceV2::recover_checkpoint(&candidate, SCOPE, bounds()) {
            assert_eq!(
                recovered
                    .prepare_checkpoint(SCOPE, MAX_CHECKPOINT)
                    .unwrap()
                    .bytes(),
                candidate,
                "accepted mutation must remain canonical at sample {sample} bit {bit}"
            );
        }
    }
}

#[test]
fn any_accepted_mutation_after_recovery_advances_from_the_recovered_revision() {
    let (service, alice, _, _) = populated();
    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let mut recovered = ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, bounds()).unwrap();
    let before = recovered.engine().revision().0;

    assert_eq!(
        recovered.execute(
            alice,
            request(
                13,
                CommandV2::CreateRoom {
                    room_id: "delta".to_owned(),
                    name: "Delta".to_owned(),
                    admission: agora_protocol::v2::AdmissionPolicyV2::Open,
                    expected_revision: before,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::RoomCreated {
            revision: before + 1
        })
    );
}

#[test]
fn moderation_state_and_receipt_survive_recovery() {
    let mut engine = Engine::new();
    for id in ["alice", "bot"] {
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
    let mut service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    let bot = service.connect("bot").unwrap();
    let ban = request(
        1,
        CommandV2::Ban {
            room_id: "room".to_owned(),
            participant_id: "bot".to_owned(),
            reason: "policy".to_owned(),
            causation: [7; 16],
            expected_revision: 3,
        },
    );
    let accepted = service.execute(alice, ban.clone());
    assert_eq!(
        accepted,
        ReplyV2::Accepted(OutcomeV2::Banned { revision: 4 })
    );

    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let mut recovered = ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, bounds()).unwrap();
    assert_eq!(recovered.execute(alice, ban), accepted);
    assert_eq!(
        recovered.execute(
            bot,
            request(
                1,
                CommandV2::Join {
                    room_id: "room".to_owned(),
                    expected_revision: 4,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::Banned)
    );
}

#[test]
fn room_policy_event_and_receipt_survive_recovery() {
    let mut engine = Engine::new();
    for id in ["alice", "bot"] {
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
    let mut service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    let bot = service.connect("bot").unwrap();
    let policy = request(
        1,
        CommandV2::SetAdmissionPolicy {
            room_id: "room".to_owned(),
            admission: agora_protocol::v2::AdmissionPolicyV2::InviteOnly,
            expected_revision: 3,
        },
    );
    let accepted = service.execute(alice, policy.clone());
    assert_eq!(
        accepted,
        ReplyV2::Accepted(OutcomeV2::RoomPolicyChanged { revision: 4 })
    );

    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let mut recovered = ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, bounds()).unwrap();
    assert_eq!(recovered.execute(alice, policy), accepted);
    assert_eq!(
        recovered.execute(
            bot,
            request(
                1,
                CommandV2::Join {
                    room_id: "room".to_owned(),
                    expected_revision: 4,
                },
            ),
        ),
        ReplyV2::Refused(RefusalV2::NotAuthorized)
    );
}

#[test]
fn ban_page_cursor_survives_recovery_without_widening_authority() {
    let mut engine = Engine::new();
    for (id, kind) in [
        ("alice", ParticipantKind::Human),
        ("bot", ParticipantKind::Agent),
        ("carol", ParticipantKind::Human),
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
    let mut service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    for (request_id, subject, revision) in [(1, "bot", 4), (2, "carol", 5)] {
        assert!(matches!(
            service.execute(
                alice,
                request(
                    request_id,
                    CommandV2::Ban {
                        room_id: "room".to_owned(),
                        participant_id: subject.to_owned(),
                        reason: "policy".to_owned(),
                        causation: [request_id as u8; 16],
                        expected_revision: revision,
                    },
                ),
            ),
            ReplyV2::Accepted(OutcomeV2::Banned { .. })
        ));
    }
    let ReplyV2::Accepted(OutcomeV2::Bans(first)) = service.execute(
        alice,
        request(
            3,
            CommandV2::Bans {
                room_id: "room".to_owned(),
                cursor: None,
                limit: 1,
            },
        ),
    ) else {
        panic!("first ban page expected");
    };
    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let mut recovered = ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, bounds()).unwrap();
    let ReplyV2::Accepted(OutcomeV2::Bans(second)) = recovered.execute(
        alice,
        request(
            4,
            CommandV2::Bans {
                room_id: "room".to_owned(),
                cursor: first.next,
                limit: 1,
            },
        ),
    ) else {
        panic!("continued ban page expected");
    };
    assert_eq!(second.banned.len(), 1);
    assert_eq!(second.banned[0].participant_id, "carol");
    assert!(second.next.is_none());
}

#[test]
fn identified_message_receipt_and_cursor_survive_recovery_exactly() {
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
    let first_request = request(
        1,
        CommandV2::SpeakIdentified {
            room_id: "room".to_owned(),
            expected_revision: 2,
            logical_time: 1,
            body: "first".to_owned(),
            addressed_to: vec![],
        },
    );
    let first_reply = service.execute(alice, first_request.clone());
    assert!(matches!(
        &first_reply,
        ReplyV2::Accepted(OutcomeV2::IdentifiedMessageAccepted { .. })
    ));
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
        ReplyV2::Accepted(OutcomeV2::IdentifiedMessageAccepted { .. })
    ));
    let ReplyV2::Accepted(OutcomeV2::IdentifiedMessages(first_page)) = service.execute(
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
    let first_id = first_page.messages[0].message_id;
    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let mut recovered = ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, bounds()).unwrap();
    assert_eq!(recovered.execute(alice, first_request), first_reply);
    let ReplyV2::Accepted(OutcomeV2::IdentifiedMessages(second_page)) = recovered.execute(
        alice,
        request(
            4,
            CommandV2::IdentifiedMessages {
                room_id: "room".to_owned(),
                cursor: first_page.next,
                limit: 1,
            },
        ),
    ) else {
        panic!("continued identified page expected");
    };
    assert_ne!(second_page.messages[0].message_id, first_id);
    assert_eq!(second_page.messages[0].sequence, 1);
    assert!(second_page.next.is_none());
}

#[test]
fn identified_change_cursor_and_message_identity_survive_recovery_exactly() {
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
    let open = request(
        1,
        CommandV2::OpenIdentifiedChanges {
            room_id: "room".to_owned(),
            observed_revision: 2,
        },
    );
    let opened = service.execute(alice, open.clone());
    let ReplyV2::Accepted(OutcomeV2::ChangesOpened { cursor, .. }) = &opened else {
        panic!("identified change position expected");
    };
    let cursor = *cursor;

    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let mut recovered = ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, bounds()).unwrap();
    assert_eq!(recovered.execute(alice, open), opened);

    let speak = request(
        2,
        CommandV2::SpeakIdentified {
            room_id: "room".to_owned(),
            expected_revision: 2,
            logical_time: 3,
            body: "after recovery".to_owned(),
            addressed_to: vec![],
        },
    );
    let receipt = recovered.execute(alice, speak.clone());
    let ReplyV2::Accepted(OutcomeV2::IdentifiedMessageAccepted { message_id, .. }) = &receipt
    else {
        panic!("identified receipt expected");
    };
    let message_id = *message_id;
    assert_eq!(recovered.execute(alice, speak), receipt);

    let ReplyV2::Accepted(OutcomeV2::IdentifiedChanges(page)) = recovered.execute(
        alice,
        request(
            3,
            CommandV2::IdentifiedChanges {
                room_id: "room".to_owned(),
                cursor,
                limit: 1,
            },
        ),
    ) else {
        panic!("identified change page expected");
    };
    assert!(matches!(
        &page.changes[0].event,
        IdentifiedRoomEventV1::MessageAccepted(message) if message.message_id == message_id
    ));
    let ReplyV2::Accepted(OutcomeV2::IdentifiedMessages(history)) = recovered.execute(
        alice,
        request(
            4,
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
fn thread_reply_page_feed_and_receipt_survive_recovery_exactly() {
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
    let ReplyV2::Accepted(OutcomeV2::IdentifiedMessageAccepted {
        message_id: root_id,
        ..
    }) = service.execute(
        alice,
        request(
            1,
            CommandV2::SpeakIdentified {
                room_id: "room".to_owned(),
                expected_revision: 2,
                logical_time: 1,
                body: "root".to_owned(),
                addressed_to: vec![],
            },
        ),
    )
    else {
        panic!("root expected");
    };
    let ReplyV2::Accepted(OutcomeV2::ChangesOpened {
        cursor: feed_cursor,
        ..
    }) = service.execute(
        alice,
        request(
            2,
            CommandV2::OpenThreadedChanges {
                room_id: "room".to_owned(),
                observed_revision: 3,
            },
        ),
    )
    else {
        panic!("threaded feed expected");
    };
    let first_request = request(
        3,
        CommandV2::SpeakReply {
            room_id: "room".to_owned(),
            expected_revision: 3,
            root_message_id: root_id,
            logical_time: 2,
            body: "one".to_owned(),
            addressed_to: vec![],
        },
    );
    let first_reply = service.execute(alice, first_request.clone());
    let ReplyV2::Accepted(OutcomeV2::ReplyAccepted {
        message_id: first_id,
        ..
    }) = &first_reply
    else {
        panic!("first reply expected");
    };
    let first_id = *first_id;
    assert!(matches!(
        service.execute(
            alice,
            request(
                4,
                CommandV2::SpeakReply {
                    room_id: "room".to_owned(),
                    expected_revision: 4,
                    root_message_id: root_id,
                    logical_time: 3,
                    body: "two".to_owned(),
                    addressed_to: vec![],
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::ReplyAccepted { .. })
    ));
    let ReplyV2::Accepted(OutcomeV2::ThreadReplies(first_page)) = service.execute(
        alice,
        request(
            5,
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
    let continuation = first_page.next.unwrap();
    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let mut recovered = ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, bounds()).unwrap();
    assert_eq!(recovered.execute(alice, first_request), first_reply);
    let ReplyV2::Accepted(OutcomeV2::ThreadReplies(second_page)) = recovered.execute(
        alice,
        request(
            6,
            CommandV2::ThreadReplies {
                room_id: "room".to_owned(),
                root_message_id: root_id,
                cursor: Some(continuation),
                limit: 1,
            },
        ),
    ) else {
        panic!("continued reply page expected");
    };
    assert_ne!(second_page.messages[0].message_id, first_id);
    assert_eq!(second_page.messages[0].reply_to_root, Some(root_id));
    let ReplyV2::Accepted(OutcomeV2::ThreadedChanges(changes)) = recovered.execute(
        alice,
        request(
            7,
            CommandV2::ThreadedChanges {
                room_id: "room".to_owned(),
                cursor: feed_cursor,
                limit: 2,
            },
        ),
    ) else {
        panic!("threaded changes expected");
    };
    assert!(matches!(
        &changes.changes[0].event,
        ThreadedRoomEventV1::MessageAccepted(message)
            if message.message_id == first_id && message.reply_to_root == Some(root_id)
    ));
}

#[test]
fn thread_lifecycle_status_feed_and_receipt_survive_recovery_exactly() {
    let mut engine = Engine::new();
    for id in ["alice", "bot"] {
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
        bot,
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
    let ReplyV2::Accepted(OutcomeV2::ChangesOpened { cursor, .. }) = service.execute(
        alice,
        request(
            1,
            CommandV2::OpenThreadedChanges {
                room_id: "room".to_owned(),
                observed_revision: 5,
            },
        ),
    ) else {
        panic!("threaded position expected");
    };
    let archive = request(
        2,
        CommandV2::ArchiveThread {
            room_id: "room".to_owned(),
            expected_revision: 5,
            root_message_id: root_id,
            reason: "pause".to_owned(),
            causation: [0x71; 16],
        },
    );
    let archive_reply = service.execute(bot, archive.clone());
    assert!(matches!(
        archive_reply,
        ReplyV2::Accepted(OutcomeV2::ThreadArchived { revision: 6, .. })
    ));

    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let mut recovered = ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, bounds()).unwrap();
    assert_eq!(recovered.execute(bot, archive), archive_reply);
    let ReplyV2::Accepted(OutcomeV2::ThreadStatus(status)) = recovered.execute(
        alice,
        request(
            2,
            CommandV2::ThreadStatus {
                room_id: "room".to_owned(),
                root_message_id: root_id,
            },
        ),
    ) else {
        panic!("recovered status expected");
    };
    assert_eq!(status.state, ThreadLifecycleStateV1::Archived);
    let ReplyV2::Accepted(OutcomeV2::ThreadedChanges(page)) = recovered.execute(
        alice,
        request(
            3,
            CommandV2::ThreadedChanges {
                room_id: "room".to_owned(),
                cursor,
                limit: 1,
            },
        ),
    ) else {
        panic!("recovered lifecycle change expected");
    };
    assert!(matches!(
        &page.changes[0].event,
        ThreadedRoomEventV1::ThreadLifecycleChanged {
            root_message_id,
            archived: true,
            ..
        } if *root_message_id == root_id
    ));
    assert_eq!(
        recovered.execute(
            alice,
            request(
                4,
                CommandV2::ReopenThread {
                    room_id: "room".to_owned(),
                    expected_revision: 6,
                    root_message_id: root_id,
                    reason: "resume".to_owned(),
                    causation: [0x72; 16],
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::ThreadReopened {
            revision: 7,
            root_message_id: root_id,
        })
    );
}

#[test]
fn reaction_status_event_and_receipt_survive_recovery_exactly() {
    let mut engine = Engine::new();
    for id in ["alice", "bot"] {
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
                    expected_revision: 4,
                    logical_time: 1,
                    body: "message".to_owned(),
                    addressed_to: vec![],
                },
            ),
        )
    else {
        panic!("identified message expected");
    };
    let ReplyV2::Accepted(OutcomeV2::ChangesOpened { cursor, .. }) = service.execute(
        alice,
        request(
            2,
            CommandV2::OpenIdentifiedChanges {
                room_id: "room".to_owned(),
                observed_revision: 5,
            },
        ),
    ) else {
        panic!("change position expected");
    };
    let add = request(
        1,
        CommandV2::AddReaction {
            room_id: "room".to_owned(),
            expected_revision: 5,
            message_id,
            key: "spark".to_owned(),
        },
    );
    let reply = service.execute(bot, add.clone());
    assert!(matches!(
        reply,
        ReplyV2::Accepted(OutcomeV2::ReactionAdded { revision: 6, .. })
    ));

    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let mut recovered = ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, bounds()).unwrap();
    assert_eq!(recovered.execute(bot, add), reply);
    let ReplyV2::Accepted(OutcomeV2::ReactionStatus(status)) = recovered.execute(
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
    assert!(status.reactions[0].caller_reacted);
    let ReplyV2::Accepted(OutcomeV2::IdentifiedChanges(page)) = recovered.execute(
        alice,
        request(
            3,
            CommandV2::IdentifiedChanges {
                room_id: "room".to_owned(),
                cursor,
                limit: 1,
            },
        ),
    ) else {
        panic!("reaction change expected");
    };
    assert!(matches!(
        &page.changes[0].event,
        IdentifiedRoomEventV1::ReactionChanged {
            actor_id,
            message_id: id,
            key,
            reacted: true,
        } if actor_id == "bot" && *id == message_id && key == "spark"
    ));
    assert_eq!(
        recovered.execute(
            bot,
            request(
                3,
                CommandV2::RemoveReaction {
                    room_id: "room".to_owned(),
                    expected_revision: 6,
                    message_id,
                    key: "spark".to_owned(),
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::ReactionRemoved {
            revision: 7,
            message_id,
        })
    );
}

#[test]
fn message_revision_cursor_event_and_receipt_survive_recovery_exactly() {
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
    let ReplyV2::Accepted(OutcomeV2::ChangesOpened {
        cursor: feed_cursor,
        ..
    }) = service.execute(
        alice,
        request(
            2,
            CommandV2::OpenIdentifiedChanges {
                room_id: "room".to_owned(),
                observed_revision: 5,
            },
        ),
    )
    else {
        panic!("feed cursor expected");
    };
    let edit = request(
        2,
        CommandV2::EditMessage {
            room_id: "room".to_owned(),
            expected_revision: 5,
            message_id,
            logical_time: 20,
            body: "revised".to_owned(),
        },
    );
    let edit_reply = service.execute(bot, edit.clone());
    assert_eq!(
        edit_reply,
        ReplyV2::Accepted(OutcomeV2::MessageEdited {
            revision: 6,
            message_id,
            message_revision: 1,
        })
    );
    let ReplyV2::Accepted(OutcomeV2::MessageRevisions(first)) = service.execute(
        alice,
        request(
            3,
            CommandV2::MessageRevisions {
                room_id: "room".to_owned(),
                message_id,
                cursor: None,
                limit: 1,
            },
        ),
    ) else {
        panic!("first revision page expected");
    };
    let revision_cursor = first.next.unwrap();

    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let mut recovered = ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, bounds()).unwrap();
    assert_eq!(recovered.execute(bot, edit), edit_reply);
    assert_eq!(recovered.engine().revision().0, 6);

    let ReplyV2::Accepted(OutcomeV2::MessageRevisions(second)) = recovered.execute(
        alice,
        request(
            4,
            CommandV2::MessageRevisions {
                room_id: "room".to_owned(),
                message_id,
                cursor: Some(revision_cursor),
                limit: 1,
            },
        ),
    ) else {
        panic!("revision continuation expected");
    };
    assert_eq!(second.revisions[0].number, 1);
    assert_eq!(second.revisions[0].body, "revised");
    let ReplyV2::Accepted(OutcomeV2::IdentifiedChanges(changes)) = recovered.execute(
        alice,
        request(
            5,
            CommandV2::IdentifiedChanges {
                room_id: "room".to_owned(),
                cursor: feed_cursor,
                limit: 1,
            },
        ),
    ) else {
        panic!("edit change expected");
    };
    assert!(matches!(&changes.changes[0].event,
        IdentifiedRoomEventV1::MessageEdited { actor_id, message_id: id, message_revision: 1, logical_time: 20, body }
        if actor_id == "bot" && *id == message_id && body == "revised"
    ));

    assert!(matches!(
        recovered.execute(
            bot,
            request(
                3,
                CommandV2::WithdrawMessage {
                    room_id: "room".to_owned(),
                    expected_revision: 6,
                    message_id,
                }
            )
        ),
        ReplyV2::Accepted(OutcomeV2::MessageWithdrawn { revision: 7, .. })
    ));
    let ReplyV2::Accepted(OutcomeV2::MessageRevisions(tombstoned)) = recovered.execute(
        alice,
        request(
            6,
            CommandV2::MessageRevisions {
                room_id: "room".to_owned(),
                message_id,
                cursor: None,
                limit: 4,
            },
        ),
    ) else {
        panic!("tombstoned revisions expected");
    };
    assert_eq!(tombstoned.revisions.len(), 2);
    assert!(
        tombstoned
            .revisions
            .iter()
            .all(
                |revision| revision.availability == MessageRevisionAvailabilityV1::Tombstoned
                    && revision.body.is_empty()
            )
    );
}

#[test]
fn author_withdrawal_tombstone_and_exact_receipt_survive_recovery() {
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
    let ReplyV2::Accepted(OutcomeV2::IdentifiedMessageAccepted { message_id, .. }) = service
        .execute(
            alice,
            request(
                1,
                CommandV2::SpeakIdentified {
                    room_id: "room".to_owned(),
                    expected_revision: 2,
                    logical_time: 1,
                    body: "withdrawn after recovery".to_owned(),
                    addressed_to: vec![],
                },
            ),
        )
    else {
        panic!("identified acceptance expected");
    };
    let withdrawal = request(
        2,
        CommandV2::WithdrawMessage {
            room_id: "room".to_owned(),
            expected_revision: 3,
            message_id,
        },
    );
    let reply = service.execute(alice, withdrawal.clone());
    assert!(matches!(
        reply,
        ReplyV2::Accepted(OutcomeV2::MessageWithdrawn { revision: 4, .. })
    ));

    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let mut recovered = ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, bounds()).unwrap();
    assert_eq!(recovered.execute(alice, withdrawal), reply);
    let ReplyV2::Accepted(OutcomeV2::IdentifiedMessages(page)) = recovered.execute(
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
    assert_eq!(page.messages[0].message_id, message_id);
    assert!(page.messages[0].body.is_empty());
}

#[test]
fn moderator_redaction_evidence_and_tombstone_survive_recovery() {
    let mut engine = Engine::new();
    for id in ["owner", "member"] {
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
            owner: "owner".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(EngineCommand::Join {
            room_id: "room".into(),
            participant: "member".into(),
        })
        .unwrap();
    let mut service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let owner = service.connect("owner").unwrap();
    let member = service.connect("member").unwrap();
    let ReplyV2::Accepted(OutcomeV2::IdentifiedMessageAccepted { message_id, .. }) = service
        .execute(
            member,
            request(
                1,
                CommandV2::SpeakIdentified {
                    room_id: "room".to_owned(),
                    expected_revision: 4,
                    logical_time: 1,
                    body: "redacted after recovery".to_owned(),
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
            expected_revision: 5,
            message_id,
            reason: "policy".to_owned(),
            causation: [0x44; 16],
        },
    );
    let reply = service.execute(owner, redaction.clone());
    assert!(matches!(
        reply,
        ReplyV2::Accepted(OutcomeV2::MessageRedacted { revision: 6, .. })
    ));

    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let mut recovered = ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, bounds()).unwrap();
    assert_eq!(recovered.execute(owner, redaction), reply);
    let ReplyV2::Accepted(OutcomeV2::IdentifiedMessages(page)) = recovered.execute(
        owner,
        request(
            2,
            CommandV2::IdentifiedMessages {
                room_id: "room".to_owned(),
                cursor: None,
                limit: 1,
            },
        ),
    ) else {
        panic!("identified page expected");
    };
    assert_eq!(page.messages[0].message_id, message_id);
    assert!(page.messages[0].body.is_empty());
}

#[test]
fn recovered_change_position_continues_without_snapshot_gap() {
    let (mut service, alice, _, _) = populated();
    let revision = service.engine().revision().0;
    let ReplyV2::Accepted(OutcomeV2::ChangesOpened { cursor, .. }) = service.execute(
        alice,
        request(
            20,
            CommandV2::OpenChanges {
                room_id: "alpha".to_owned(),
                observed_revision: revision,
            },
        ),
    ) else {
        panic!("change position expected");
    };
    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let mut recovered = ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, bounds()).unwrap();
    assert_eq!(
        recovered.execute(
            alice,
            request(
                21,
                CommandV2::Speak {
                    room_id: "alpha".to_owned(),
                    expected_revision: revision,
                    logical_time: 10,
                    body: "after recovery".to_owned(),
                    addressed_to: vec![],
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::MessageAccepted {
            revision: revision + 1,
            sequence: 4,
        })
    );
    let ReplyV2::Accepted(OutcomeV2::Changes(page)) = recovered.execute(
        alice,
        request(
            22,
            CommandV2::Changes {
                room_id: "alpha".to_owned(),
                cursor,
                limit: 4,
            },
        ),
    ) else {
        panic!("recovered position must continue");
    };
    assert_eq!(page.changes.len(), 1);
    assert_eq!(page.changes[0].revision, revision + 1);
}

#[test]
fn topic_change_position_continues_after_recovery_with_topic_identity() {
    let mut engine = Engine::new();
    engine
        .apply(EngineCommand::RegisterParticipant(Participant::new(
            "alice",
            "alice",
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
    let ReplyV2::Accepted(OutcomeV2::ChangesOpened { cursor, .. }) = service.execute(
        alice,
        request(
            1,
            CommandV2::OpenTopicChanges {
                room_id: "room".to_owned(),
                observed_revision: 2,
            },
        ),
    ) else {
        panic!("topic change position expected");
    };
    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let mut recovered = ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, bounds()).unwrap();
    let ReplyV2::Accepted(OutcomeV2::TopicCreated { topic_id, .. }) = recovered.execute(
        alice,
        request(
            2,
            CommandV2::CreateTopic {
                room_id: "room".to_owned(),
                expected_revision: 2,
                name: "incidents".to_owned(),
            },
        ),
    ) else {
        panic!("topic creation expected");
    };
    let rename = request(
        4,
        CommandV2::RenameTopic {
            room_id: "room".to_owned(),
            expected_revision: 3,
            topic_id,
            name: "operations".to_owned(),
        },
    );
    let renamed = recovered.execute(alice, rename.clone());
    assert_eq!(
        renamed,
        ReplyV2::Accepted(OutcomeV2::TopicRenamed {
            revision: 4,
            topic_id,
        })
    );
    let renamed_checkpoint = recovered.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let mut recovered =
        ServiceV2::recover_checkpoint(renamed_checkpoint.bytes(), SCOPE, bounds()).unwrap();
    assert_eq!(recovered.execute(alice, rename), renamed);

    let ReplyV2::Accepted(OutcomeV2::TopicChanges(page)) = recovered.execute(
        alice,
        request(
            3,
            CommandV2::TopicChanges {
                room_id: "room".to_owned(),
                cursor,
                limit: 2,
            },
        ),
    ) else {
        panic!("recovered topic feed expected");
    };
    assert!(matches!(
        &page.changes[0].event,
        agora_protocol::v2::TopicRoomEventV1::TopicCreated {
            topic_id: event_topic,
            ..
        } if *event_topic == topic_id
    ));
    assert!(matches!(
        &page.changes[1].event,
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
    let ReplyV2::Accepted(OutcomeV2::Topics(topics)) = recovered.execute(
        alice,
        request(
            5,
            CommandV2::Topics {
                room_id: "room".to_owned(),
                cursor: None,
                limit: 1,
            },
        ),
    ) else {
        panic!("recovered topic page expected");
    };
    assert_eq!(topics.topics[0].topic_id, topic_id);
    assert_eq!(topics.topics[0].name, "operations");
}

#[test]
fn private_read_position_and_exact_receipt_survive_recovery() {
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
    let agora_engine::OutcomeV2::MessageAccepted { sequence, .. } = engine
        .apply(EngineCommand::Speak {
            room_id: "room".into(),
            author: "alice".into(),
            at: agora_engine::LogicalTime(3),
            body: "checkpoint target".to_owned(),
        })
        .unwrap()
    else {
        panic!("message expected");
    };
    let message_id = MessageIdV1::new(
        engine
            .message(&RoomId::new("room"), sequence)
            .unwrap()
            .id()
            .into_bytes(),
    );
    let observed_revision = engine.revision().0;
    let pending = engine.pending_events().len();
    let mut service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    let advance = request(
        1,
        CommandV2::AdvanceReadPosition {
            room_id: "room".to_owned(),
            observed_revision,
            scope: ReadScopeV1::Room,
            target_message_id: message_id,
        },
    );
    let accepted = service.execute(alice, advance.clone());
    assert!(matches!(
        accepted,
        ReplyV2::Accepted(OutcomeV2::ReadPositionAdvanced(_))
    ));
    assert_eq!(service.engine().revision().0, observed_revision);
    assert_eq!(service.engine().pending_events().len(), pending);

    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let mut recovered = ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, bounds()).unwrap();
    assert_eq!(recovered.execute(alice, advance), accepted);
    assert_eq!(
        recovered.execute(
            alice,
            request(
                2,
                CommandV2::ReadPosition {
                    room_id: "room".to_owned(),
                    scope: ReadScopeV1::Room,
                },
            ),
        ),
        ReplyV2::Accepted(OutcomeV2::ReadPosition(Some(
            agora_protocol::v2::ReadPositionV1 {
                room_id: "room".to_owned(),
                scope: ReadScopeV1::Room,
                target_message_id: message_id,
                target_sequence: sequence,
                observed_revision,
            }
        )))
    );
}

#[test]
fn private_attention_preference_and_exact_receipt_survive_recovery() {
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
    let observed_revision = engine.revision().0;
    let pending = engine.pending_events().len();
    let mut service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let alice = service.connect("alice").unwrap();
    let preference = AttentionPreferenceV1 {
        following: true,
        muted: true,
        notification_intent: NotificationIntentV1::AddressedOnly,
    };
    let set = request(
        1,
        CommandV2::SetAttentionPreference {
            room_id: "room".to_owned(),
            observed_revision,
            scope: AttentionScopeV1::Room,
            preference,
        },
    );
    let accepted = service.execute(alice, set.clone());
    assert_eq!(
        accepted,
        ReplyV2::Accepted(OutcomeV2::AttentionPreferenceSet(preference))
    );
    assert_eq!(service.engine().revision().0, observed_revision);
    assert_eq!(service.engine().pending_events().len(), pending);

    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let mut recovered = ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, bounds()).unwrap();
    assert_eq!(
        recovered
            .prepare_checkpoint(SCOPE, MAX_CHECKPOINT)
            .unwrap()
            .bytes(),
        checkpoint.bytes()
    );
    assert_eq!(recovered.execute(alice, set), accepted);
    assert_eq!(
        recovered.execute(
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
}

#[test]
fn shared_pin_private_save_and_exact_receipts_survive_recovery() {
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
                expected_revision: 2,
                logical_time: 1,
                body: "body".to_owned(),
                addressed_to: vec![],
            },
        ),
    )
    else {
        panic!("identified message expected");
    };
    let save_request = request(
        2,
        CommandV2::SetSavedReference {
            room_id: "room".to_owned(),
            observed_revision: revision,
            message_id,
            saved: true,
        },
    );
    let save_reply = service.execute(alice, save_request.clone());
    let pin_request = request(
        3,
        CommandV2::SetPin {
            room_id: "room".to_owned(),
            expected_revision: revision,
            message_id,
            pinned: true,
        },
    );
    let pin_reply = service.execute(alice, pin_request.clone());

    let checkpoint = service.prepare_checkpoint(SCOPE, MAX_CHECKPOINT).unwrap();
    let mut recovered = ServiceV2::recover_checkpoint(checkpoint.bytes(), SCOPE, bounds()).unwrap();
    assert_eq!(
        recovered
            .prepare_checkpoint(SCOPE, MAX_CHECKPOINT)
            .unwrap()
            .bytes(),
        checkpoint.bytes()
    );
    assert_eq!(recovered.execute(alice, save_request), save_reply);
    assert_eq!(recovered.execute(alice, pin_request), pin_reply);
    assert!(matches!(
        recovered.execute(
            alice,
            request(4, CommandV2::SavedReferences { room_id: "room".to_owned() }),
        ),
        ReplyV2::Accepted(OutcomeV2::SavedReferences(references))
            if references.len() == 1
                && references[0].message_id == message_id
                && references[0].original_sequence == sequence
                && references[0].availability == StableReferenceAvailabilityV1::Available
    ));
    assert!(matches!(
        recovered.execute(
            alice,
            request(5, CommandV2::Pins { room_id: "room".to_owned() }),
        ),
        ReplyV2::Accepted(OutcomeV2::Pins(pins))
            if pins.len() == 1
                && pins[0].message_id == message_id
                && pins[0].original_sequence == sequence
                && pins[0].pinned_by == "alice"
    ));
}
