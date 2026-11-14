use agora_durable::{
    DurableConfigV2, DurableEvidenceV2, DurableHostConfigV2, DurableHostPublicationV2,
    DurableHostStartErrorV2, DurableHostSuppressionV2, DurableHostV2, DurableOwnerStateV2,
    DurablePublicationV2, DurableServiceV2, DurableStartErrorV2, DurableSuppressionV2,
    DurableTransientHostV1, OperatorRestoreAuthorityV1, OperatorRestoreAuthorizationV1,
    OperatorRestoreRequestV1,
};
use agora_engine::{
    AdmissionPolicy, AttentionScopeV1 as EngineAttentionScopeV1, CommandV2 as EngineCommand,
    Engine, Limits, LogicalTime, Participant, ParticipantId, ParticipantKind,
    ReadScopeV1 as EngineReadScopeV1, RoomId,
};
use agora_host::transient::{TransientHostBuildRefusalV1, TransientHostV1};
use agora_host::v2::HostV2;
use agora_host::v2::persistence::{HostCheckpointScopeV1, HostRecoveryLimitsV2};
use agora_host::{HostLimits, HostRefusal, HostTimeV1, PrincipalVerifier, VerifiedSubjectV1};
use agora_protocol::transient::{
    PresenceStateV1, PresenceViewStateV1, TransientCommandV1, TransientOutcomeV1, TransientReplyV1,
    TransientRequestIdV1, TransientRequestV1,
};
use agora_protocol::v2::{
    AdmissionPolicyV2, AttentionPreferenceV1, AttentionScopeV1, CommandV2, MessageIdV1,
    MessageReportDispositionV1, NotificationIntentV1, OutcomeV2, ReadScopeV1, RefusalV2, ReplyV2,
    RequestIdV2, RequestV2, TopicCollectionFilterV1, TopicCollectionLifecycleV1,
    VellumArtifactMessageTargetV1, VellumArtifactReferenceIdV1, VellumArtifactReferenceV1,
    VellumDocumentTargetV1,
};
use agora_service::transient::TransientLimitsV1;
use agora_service::typing::TypingLimitsV1;
use agora_service::v2::{
    ServiceLimitsV2, ServiceV2,
    persistence::{ServiceCheckpointScopeV1, ServiceRecoveryLimitsV2},
};
use agora_wire::WireLimits;
use agora_wire::transient::{decode_transient_reply_v1, encode_transient_request_v1};
use agora_wire::v2::{WireReplyV2, WireRequestV2, decode_reply_v2, encode_request_v2};
use keystone_storage::{
    AcknowledgementStrength, Checkpoint, CheckpointStore, CommitOutcome, CommitReceipt, Revision,
    StoreRefusal,
};
use std::{cell::RefCell, rc::Rc};

const MAX: usize = 2 * 1_024 * 1_024;
const SCOPE: ServiceCheckpointScopeV1 = ServiceCheckpointScopeV1::new(*b"durable-test-v1!");
const HOST_SCOPE: HostCheckpointScopeV1 = HostCheckpointScopeV1::new(*b"durable-host-v1!");

#[derive(Clone)]
struct Context {
    active: bool,
}

struct Verifier;

#[derive(Default)]
struct RestoreAuthority {
    deny: bool,
    requests: Vec<(String, HostCheckpointScopeV1, Revision)>,
}

impl OperatorRestoreAuthorityV1 for RestoreAuthority {
    type Context = ();
    type Error = ();

    fn authorize(
        &mut self,
        request: &OperatorRestoreRequestV1<'_>,
        _context: &Self::Context,
    ) -> Result<(), Self::Error> {
        self.requests.push((
            request.key().to_owned(),
            request.scope(),
            request.revision(),
        ));
        if self.deny { Err(()) } else { Ok(()) }
    }
}

impl PrincipalVerifier for Verifier {
    type Context = Context;
    type Error = ();

    fn authenticate(
        &mut self,
        claimed_participant: &str,
        context: &Self::Context,
        _now: HostTimeV1,
    ) -> Result<VerifiedSubjectV1, Self::Error> {
        if context.active && claimed_participant == "alice" {
            Ok(VerifiedSubjectV1::from_verifier([7; 16]))
        } else {
            Err(())
        }
    }

    fn revalidate(
        &mut self,
        subject: VerifiedSubjectV1,
        participant: &str,
        context: &Self::Context,
        _now: HostTimeV1,
    ) -> Result<(), Self::Error> {
        if context.active
            && participant == "alice"
            && subject == VerifiedSubjectV1::from_verifier([7; 16])
        {
            Ok(())
        } else {
            Err(())
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum NextCommit {
    #[default]
    Normal,
    Refuse,
    IndeterminateBefore,
    IndeterminateAfter,
    Weak,
    WrongRevision,
    Error,
}

#[derive(Default)]
struct StoreState {
    checkpoint: Option<Checkpoint>,
    next: NextCommit,
    commit_calls: usize,
}

#[derive(Clone, Default)]
struct TestStore(Rc<RefCell<StoreState>>);

impl TestStore {
    fn next(&self, next: NextCommit) {
        self.0.borrow_mut().next = next;
    }

    fn commit_calls(&self) -> usize {
        self.0.borrow().commit_calls
    }
}

impl CheckpointStore for TestStore {
    type Error = ();

    fn load(&self, _key: &str) -> Result<Option<Checkpoint>, Self::Error> {
        Ok(self.0.borrow().checkpoint.clone())
    }

    fn compare_exchange(
        &mut self,
        _key: &str,
        expected: Option<Revision>,
        bytes: &[u8],
    ) -> Result<CommitOutcome, Self::Error> {
        let mut state = self.0.borrow_mut();
        state.commit_calls += 1;
        let actual = state
            .checkpoint
            .as_ref()
            .map(|checkpoint| checkpoint.revision);
        if actual != expected {
            return Ok(CommitOutcome::Refused(StoreRefusal::RevisionConflict {
                expected,
                actual,
            }));
        }
        let revision = Revision(actual.map_or(0, |revision| revision.0 + 1));
        let next = std::mem::take(&mut state.next);
        match next {
            NextCommit::Refuse => Ok(CommitOutcome::Refused(StoreRefusal::ValueLimit)),
            NextCommit::IndeterminateBefore => Ok(CommitOutcome::Indeterminate),
            NextCommit::Error => Err(()),
            NextCommit::WrongRevision => Ok(CommitOutcome::Committed(CommitReceipt {
                revision: Revision(revision.0 + 1),
                mechanism: "test-store-v1",
                strength: AcknowledgementStrength::Durable,
            })),
            NextCommit::Normal | NextCommit::IndeterminateAfter | NextCommit::Weak => {
                let strength = if next == NextCommit::Weak {
                    AcknowledgementStrength::Volatile
                } else {
                    AcknowledgementStrength::Durable
                };
                state.checkpoint = Some(Checkpoint {
                    revision,
                    bytes: bytes.to_vec(),
                    strength,
                });
                if next == NextCommit::IndeterminateAfter {
                    Ok(CommitOutcome::Indeterminate)
                } else {
                    Ok(CommitOutcome::Committed(CommitReceipt {
                        revision,
                        mechanism: "test-store-v1",
                        strength,
                    }))
                }
            }
        }
    }
}

fn config() -> DurableConfigV2 {
    DurableConfigV2::new(
        "agora",
        SCOPE,
        MAX,
        ServiceRecoveryLimitsV2 {
            max_snapshot_bytes: MAX,
            max_engine_snapshot_bytes: MAX,
            maximum_service_limits: ServiceLimitsV2::default(),
            maximum_engine_limits: Limits::default(),
        },
    )
}

fn host_limits() -> HostLimits {
    HostLimits {
        max_sessions: 4,
        max_retired_sessions: 4,
        wire: WireLimits {
            max_frame_bytes: 8_192,
            max_text_bytes: 256,
            max_fields: 512,
            max_list_items: 64,
        },
    }
}

fn host_config() -> DurableHostConfigV2 {
    DurableHostConfigV2::new(
        "agora-host",
        HOST_SCOPE,
        MAX,
        HostRecoveryLimitsV2 {
            max_snapshot_bytes: MAX,
            max_service_snapshot_bytes: MAX,
            maximum_host_limits: host_limits(),
            service: ServiceRecoveryLimitsV2 {
                max_snapshot_bytes: MAX,
                max_engine_snapshot_bytes: MAX,
                maximum_service_limits: ServiceLimitsV2::default(),
                maximum_engine_limits: Limits::default(),
            },
        },
    )
}

fn connect_frame() -> Vec<u8> {
    encode_request_v2(
        &WireRequestV2::Connect {
            participant_id: "alice".to_owned(),
        },
        host_limits().wire,
    )
    .unwrap()
}

fn create_frame(client: agora_protocol::v2::ClientIdV2) -> Vec<u8> {
    encode_request_v2(
        &WireRequestV2::Execute {
            client,
            request: create_request("room"),
        },
        host_limits().wire,
    )
    .unwrap()
}

fn service() -> ServiceV2 {
    let mut engine = Engine::new();
    engine
        .apply(EngineCommand::RegisterParticipant(Participant::new(
            "alice",
            "Alice",
            ParticipantKind::Human,
        )))
        .unwrap();
    ServiceV2::new(engine, ServiceLimitsV2::default())
}

fn create_request(name: &str) -> RequestV2 {
    RequestV2 {
        request_id: RequestIdV2(7),
        command: CommandV2::CreateRoom {
            room_id: name.to_owned(),
            name: name.to_owned(),
            admission: AdmissionPolicyV2::Open,
            expected_revision: 1,
        },
    }
}

fn initialized() -> (
    DurableServiceV2<TestStore>,
    TestStore,
    agora_protocol::v2::ClientIdV2,
) {
    let store = TestStore::default();
    let mut durable = DurableServiceV2::initialize(service(), store.clone(), config()).unwrap();
    let DurablePublicationV2::Published {
        value: Ok(client),
        evidence: DurableEvidenceV2::Committed {
            revision: Revision(1),
        },
    } = durable.connect("alice")
    else {
        panic!("connect must be durable");
    };
    (durable, store, client)
}

#[test]
fn committed_lost_reply_recovers_as_exact_replay_without_duplicate_event() {
    let (mut durable, store, client) = initialized();
    let request = create_request("room");
    let DurablePublicationV2::Published {
        value: ReplyV2::Accepted(OutcomeV2::RoomCreated { revision: 2 }),
        evidence: DurableEvidenceV2::Committed {
            revision: Revision(2),
        },
    } = durable.execute(client, request.clone())
    else {
        panic!("mutation must commit before publication");
    };
    let pending = durable.service().unwrap().engine().pending_events().len();
    assert_eq!(store.commit_calls(), 3);
    drop(durable.into_store());

    let mut recovered = DurableServiceV2::recover(store.clone(), config()).unwrap();
    assert_eq!(recovered.service().unwrap().engine().revision().0, 2);
    assert_eq!(
        recovered.service().unwrap().engine().pending_events().len(),
        pending
    );
    assert_eq!(
        recovered.execute(client, request.clone()),
        DurablePublicationV2::Published {
            value: ReplyV2::Accepted(OutcomeV2::RoomCreated { revision: 2 }),
            evidence: DurableEvidenceV2::PreviouslyCommitted {
                revision: Revision(2)
            }
        }
    );
    assert_eq!(store.commit_calls(), 3, "replay must not touch storage");

    let mut changed = request;
    let CommandV2::CreateRoom { name, .. } = &mut changed.command else {
        unreachable!()
    };
    *name = "changed".to_owned();
    assert_eq!(
        recovered.execute(client, changed),
        DurablePublicationV2::Published {
            value: ReplyV2::Refused(RefusalV2::RequestConflict),
            evidence: DurableEvidenceV2::PreviouslyCommitted {
                revision: Revision(2)
            }
        }
    );
    assert_eq!(store.commit_calls(), 3);
}

#[test]
fn committed_poll_and_private_ballot_recover_with_exact_receipts() {
    let (mut durable, store, client) = initialized();
    let create_room = create_request("room");
    assert!(matches!(
        durable.execute(client, create_room),
        DurablePublicationV2::Published {
            value: ReplyV2::Accepted(OutcomeV2::RoomCreated { revision: 2 }),
            ..
        }
    ));
    let create_poll = RequestV2 {
        request_id: RequestIdV2(8),
        command: CommandV2::CreatePoll {
            room_id: "room".to_owned(),
            expected_revision: 2,
            prompt: "Ship?".to_owned(),
            options: vec!["yes".to_owned(), "no".to_owned()],
            eligible_participant_ids: vec!["alice".to_owned()],
            max_choices: 1,
        },
    };
    let DurablePublicationV2::Published {
        value:
            ReplyV2::Accepted(OutcomeV2::PollCreated {
                revision: 3,
                poll_id,
            }),
        ..
    } = durable.execute(client, create_poll.clone())
    else {
        panic!("poll creation must commit")
    };
    let vote = RequestV2 {
        request_id: RequestIdV2(9),
        command: CommandV2::SetPollBallot {
            room_id: "room".to_owned(),
            expected_revision: 3,
            poll_id,
            selections: vec![0],
        },
    };
    assert!(matches!(
        durable.execute(client, vote.clone()),
        DurablePublicationV2::Published {
            value: ReplyV2::Accepted(OutcomeV2::PollBallotChanged {
                revision: 4,
                total_ballots: 1,
                ..
            }),
            ..
        }
    ));
    drop(durable.into_store());

    let mut recovered = DurableServiceV2::recover(store, config()).unwrap();
    assert_eq!(
        recovered.execute(client, vote),
        DurablePublicationV2::Published {
            value: ReplyV2::Accepted(OutcomeV2::PollBallotChanged {
                revision: 4,
                poll_id,
                totals: vec![1, 0],
                total_ballots: 1,
            }),
            evidence: DurableEvidenceV2::PreviouslyCommitted {
                revision: Revision(4),
            },
        }
    );
    assert!(matches!(
        recovered.execute(
            client,
            RequestV2 {
                request_id: RequestIdV2(10),
                command: CommandV2::PollBallot { room_id: "room".to_owned(), poll_id },
            }
        ),
        DurablePublicationV2::Published {
            value: ReplyV2::Accepted(OutcomeV2::PollBallot(ballot)),
            ..
        } if ballot.selections == vec![0]
    ));
}

#[test]
fn committed_topic_collection_receipt_recovers_exact_metadata() {
    let (mut durable, store, client) = initialized();
    assert!(matches!(
        durable.execute(client, create_request("room")),
        DurablePublicationV2::Published {
            value: ReplyV2::Accepted(OutcomeV2::RoomCreated { revision: 2 }),
            ..
        }
    ));
    let DurablePublicationV2::Published {
        value: ReplyV2::Accepted(OutcomeV2::TopicCreated { topic_id, .. }),
        ..
    } = durable.execute(
        client,
        RequestV2 {
            request_id: RequestIdV2(8),
            command: CommandV2::CreateTopic {
                room_id: "room".to_owned(),
                expected_revision: 2,
                name: "work".to_owned(),
            },
        },
    )
    else {
        panic!("topic creation must commit")
    };
    assert!(matches!(
        durable.execute(
            client,
            RequestV2 {
                request_id: RequestIdV2(9),
                command: CommandV2::AddTopicTag {
                    room_id: "room".to_owned(),
                    expected_revision: 3,
                    topic_id,
                    tag: "urgent".to_owned(),
                },
            },
        ),
        DurablePublicationV2::Published {
            value: ReplyV2::Accepted(OutcomeV2::TopicTagAdded { revision: 4, .. }),
            ..
        }
    ));
    let observe = RequestV2 {
        request_id: RequestIdV2(10),
        command: CommandV2::TopicCollection {
            room_id: "room".to_owned(),
            filter: TopicCollectionFilterV1 {
                lifecycle: TopicCollectionLifecycleV1::Active,
                tags: vec!["urgent".to_owned()],
            },
            cursor: None,
            limit: 8,
        },
    };
    assert!(matches!(
        durable.execute(client, observe.clone()),
        DurablePublicationV2::Published {
            value: ReplyV2::Accepted(OutcomeV2::TopicCollection(page)),
            ..
        } if page.topics.len() == 1
            && page.topics[0].topic_id == topic_id
            && page.topics[0].tags == vec!["urgent"]
    ));
    drop(durable.into_store());

    let mut recovered = DurableServiceV2::recover(store, config()).unwrap();
    assert!(matches!(
        recovered.execute(client, observe),
        DurablePublicationV2::Published {
            value: ReplyV2::Accepted(OutcomeV2::TopicCollection(page)),
            evidence: DurableEvidenceV2::PreviouslyCommitted { .. },
        } if page.topics.len() == 1
            && page.topics[0].topic_id == topic_id
            && page.topics[0].tags == vec!["urgent"]
    ));
}

#[test]
fn committed_private_report_recovers_with_exact_receipt_and_visibility() {
    let (mut durable, store, client) = initialized();
    assert!(matches!(
        durable.execute(client, create_request("room")),
        DurablePublicationV2::Published {
            value: ReplyV2::Accepted(OutcomeV2::RoomCreated { revision: 2 }),
            ..
        }
    ));
    let DurablePublicationV2::Published {
        value:
            ReplyV2::Accepted(OutcomeV2::IdentifiedMessageAccepted {
                revision: 3,
                message_id,
                ..
            }),
        ..
    } = durable.execute(
        client,
        RequestV2 {
            request_id: RequestIdV2(8),
            command: CommandV2::SpeakIdentified {
                room_id: "room".to_owned(),
                expected_revision: 2,
                logical_time: 3,
                body: "target".to_owned(),
                addressed_to: Vec::new(),
            },
        },
    )
    else {
        panic!("message must commit")
    };
    let report_request = RequestV2 {
        request_id: RequestIdV2(9),
        command: CommandV2::SubmitMessageReport {
            room_id: "room".to_owned(),
            observed_revision: 3,
            message_id,
            reason: "review".to_owned(),
            causation: [9; 16],
        },
    };
    let published = durable.execute(client, report_request.clone());
    assert!(matches!(
        &published,
        DurablePublicationV2::Published {
            value: ReplyV2::Accepted(OutcomeV2::MessageReportSubmitted(report)),
            ..
        } if report.message_id == message_id
            && report.disposition == MessageReportDispositionV1::Pending
    ));
    drop(durable.into_store());

    let mut recovered = DurableServiceV2::recover(store, config()).unwrap();
    assert!(matches!(
        recovered.execute(client, report_request),
        DurablePublicationV2::Published {
            value: ReplyV2::Accepted(OutcomeV2::MessageReportSubmitted(report)),
            evidence: DurableEvidenceV2::PreviouslyCommitted { .. },
        } if report.message_id == message_id
            && report.disposition == MessageReportDispositionV1::Pending
    ));
    assert!(matches!(
        recovered.execute(
            client,
            RequestV2 {
                request_id: RequestIdV2(10),
                command: CommandV2::MessageReports {
                    room_id: "room".to_owned(),
                },
            },
        ),
        DurablePublicationV2::Published {
            value: ReplyV2::Accepted(OutcomeV2::MessageReports(reports)),
            ..
        } if reports.len() == 1 && reports[0].message_id == message_id
    ));
}

#[test]
fn vellum_artifact_admission_commits_as_one_unit_and_recovers_exact_replay() {
    let (mut durable, store, client) = initialized();
    assert!(matches!(
        durable.execute(client, create_request("room")),
        DurablePublicationV2::Published {
            value: ReplyV2::Accepted(OutcomeV2::RoomCreated { revision: 2 }),
            evidence: DurableEvidenceV2::Committed {
                revision: Revision(2)
            },
        }
    ));
    let request = RequestV2 {
        request_id: RequestIdV2(8),
        command: CommandV2::SpeakWithVellumArtifacts {
            room_id: "room".to_owned(),
            expected_revision: 2,
            target: VellumArtifactMessageTargetV1::Room,
            logical_time: 3,
            body: "artifact".to_owned(),
            addressed_to: vec![],
            references: vec![VellumArtifactReferenceV1 {
                reference_id: VellumArtifactReferenceIdV1::new([1; 16]),
                target: VellumDocumentTargetV1 {
                    workspace_id: 2,
                    document_id: 3,
                    source_revision: 4,
                },
            }],
        },
    };
    let published = durable.execute(client, request.clone());
    assert!(matches!(
        published,
        DurablePublicationV2::Published {
            value: ReplyV2::Accepted(OutcomeV2::VellumArtifactMessageAccepted { revision: 3, .. }),
            evidence: DurableEvidenceV2::Committed {
                revision: Revision(3)
            },
        }
    ));
    let pending = durable.service().unwrap().engine().pending_events().len();
    assert_eq!(store.commit_calls(), 4);
    drop(durable.into_store());

    let mut recovered = DurableServiceV2::recover(store.clone(), config()).unwrap();
    assert_eq!(recovered.service().unwrap().engine().revision().0, 3);
    assert_eq!(
        recovered.service().unwrap().engine().pending_events().len(),
        pending
    );
    assert!(matches!(
        recovered.execute(client, request),
        DurablePublicationV2::Published {
            value: ReplyV2::Accepted(OutcomeV2::VellumArtifactMessageAccepted { revision: 3, .. }),
            evidence: DurableEvidenceV2::PreviouslyCommitted {
                revision: Revision(3)
            },
        }
    ));
    assert_eq!(store.commit_calls(), 4, "exact replay must not recommit");
}

#[test]
fn private_read_position_commits_before_reply_and_recovers_as_exact_replay() {
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
            at: LogicalTime(3),
            body: "durable target".to_owned(),
        })
        .unwrap()
    else {
        panic!("message expected");
    };
    let target = engine.message(&RoomId::new("room"), sequence).unwrap().id();
    let observed_revision = engine.revision().0;
    let pending = engine.pending_events().len();
    let store = TestStore::default();
    let service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let mut durable = DurableServiceV2::initialize(service, store.clone(), config()).unwrap();
    let DurablePublicationV2::Published {
        value: Ok(client), ..
    } = durable.connect("alice")
    else {
        panic!("connect must commit");
    };
    let advance = RequestV2 {
        request_id: RequestIdV2(41),
        command: CommandV2::AdvanceReadPosition {
            room_id: "room".to_owned(),
            observed_revision,
            scope: ReadScopeV1::Room,
            target_message_id: MessageIdV1::new(target.into_bytes()),
        },
    };
    let DurablePublicationV2::Published {
        value: accepted @ ReplyV2::Accepted(OutcomeV2::ReadPositionAdvanced(_)),
        evidence: DurableEvidenceV2::Committed { revision },
    } = durable.execute(client, advance.clone())
    else {
        panic!("private marker must commit before publication");
    };
    assert_eq!(store.commit_calls(), 3);
    assert_eq!(
        durable.service().unwrap().engine().revision().0,
        observed_revision
    );
    assert_eq!(
        durable.service().unwrap().engine().pending_events().len(),
        pending
    );
    drop(durable.into_store());

    let mut recovered = DurableServiceV2::recover(store.clone(), config()).unwrap();
    assert_eq!(
        recovered
            .service()
            .unwrap()
            .engine()
            .read_position(
                &ParticipantId::new("alice"),
                &RoomId::new("room"),
                &EngineReadScopeV1::Room,
            )
            .unwrap()
            .expect("private marker must recover")
            .target_message_id(),
        target
    );
    assert_eq!(
        recovered.execute(client, advance),
        DurablePublicationV2::Published {
            value: accepted,
            evidence: DurableEvidenceV2::PreviouslyCommitted { revision },
        }
    );
    assert_eq!(store.commit_calls(), 3, "exact replay must not recommit");
}

#[test]
fn private_attention_commits_before_reply_and_recovers_as_exact_replay() {
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
    let store = TestStore::default();
    let service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let mut durable = DurableServiceV2::initialize(service, store.clone(), config()).unwrap();
    let DurablePublicationV2::Published {
        value: Ok(client), ..
    } = durable.connect("alice")
    else {
        panic!("connect must commit");
    };
    let preference = AttentionPreferenceV1 {
        following: true,
        muted: true,
        notification_intent: NotificationIntentV1::AddressedOnly,
    };
    let set = RequestV2 {
        request_id: RequestIdV2(42),
        command: CommandV2::SetAttentionPreference {
            room_id: "room".to_owned(),
            observed_revision,
            scope: AttentionScopeV1::Room,
            preference,
        },
    };
    let DurablePublicationV2::Published {
        value: accepted @ ReplyV2::Accepted(OutcomeV2::AttentionPreferenceSet(_)),
        evidence: DurableEvidenceV2::Committed { revision },
    } = durable.execute(client, set.clone())
    else {
        panic!("private attention must commit before publication");
    };
    assert_eq!(store.commit_calls(), 3);
    assert_eq!(
        durable.service().unwrap().engine().revision().0,
        observed_revision
    );
    assert_eq!(
        durable.service().unwrap().engine().pending_events().len(),
        pending
    );
    drop(durable.into_store());

    let mut recovered = DurableServiceV2::recover(store.clone(), config()).unwrap();
    assert!(
        recovered
            .service()
            .unwrap()
            .engine()
            .attention_preference(
                &ParticipantId::new("alice"),
                &RoomId::new("room"),
                EngineAttentionScopeV1::Room,
            )
            .unwrap()
            .following()
    );
    assert_eq!(
        recovered.execute(client, set),
        DurablePublicationV2::Published {
            value: accepted,
            evidence: DurableEvidenceV2::PreviouslyCommitted { revision },
        }
    );
    assert_eq!(store.commit_calls(), 3, "exact replay must not recommit");
}

#[test]
fn shared_pin_and_private_save_commit_before_reply_and_recover_as_exact_replay() {
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
            at: agora_engine::LogicalTime(1),
            body: "body".to_owned(),
        })
        .unwrap()
    else {
        panic!("message accepted expected");
    };
    let message_id = engine.message(&RoomId::new("room"), sequence).unwrap().id();
    let revision = engine.revision().0;
    let store = TestStore::default();
    let service = ServiceV2::new(engine, ServiceLimitsV2::default());
    let mut durable = DurableServiceV2::initialize(service, store.clone(), config()).unwrap();
    let DurablePublicationV2::Published {
        value: Ok(client), ..
    } = durable.connect("alice")
    else {
        panic!("connect must commit");
    };
    let save = RequestV2 {
        request_id: RequestIdV2(51),
        command: CommandV2::SetSavedReference {
            room_id: "room".to_owned(),
            observed_revision: revision,
            message_id: MessageIdV1::new(message_id.into_bytes()),
            saved: true,
        },
    };
    let DurablePublicationV2::Published {
        value: save_reply @ ReplyV2::Accepted(OutcomeV2::SavedReferenceSet { .. }),
        evidence:
            DurableEvidenceV2::Committed {
                revision: save_store_revision,
            },
    } = durable.execute(client, save.clone())
    else {
        panic!("private save must commit before publication");
    };
    let pin = RequestV2 {
        request_id: RequestIdV2(52),
        command: CommandV2::SetPin {
            room_id: "room".to_owned(),
            expected_revision: revision,
            message_id: MessageIdV1::new(message_id.into_bytes()),
            pinned: true,
        },
    };
    let DurablePublicationV2::Published {
        value: pin_reply @ ReplyV2::Accepted(OutcomeV2::PinChanged { .. }),
        evidence: DurableEvidenceV2::Committed {
            revision: pin_store_revision,
        },
    } = durable.execute(client, pin.clone())
    else {
        panic!("shared pin must commit before publication");
    };
    assert!(pin_store_revision > save_store_revision);
    assert_eq!(store.commit_calls(), 4);
    drop(durable.into_store());

    let mut recovered = DurableServiceV2::recover(store.clone(), config()).unwrap();
    let engine = recovered.service().unwrap().engine();
    assert!(
        engine
            .saved_reference(
                &ParticipantId::new("alice"),
                &RoomId::new("room"),
                message_id,
            )
            .unwrap()
            .is_some()
    );
    assert_eq!(
        engine
            .pinned_references(&ParticipantId::new("alice"), &RoomId::new("room"))
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        recovered.execute(client, save),
        DurablePublicationV2::Published {
            value: save_reply,
            evidence: DurableEvidenceV2::PreviouslyCommitted {
                revision: pin_store_revision,
            },
        }
    );
    assert_eq!(
        recovered.execute(client, pin),
        DurablePublicationV2::Published {
            value: pin_reply,
            evidence: DurableEvidenceV2::PreviouslyCommitted {
                revision: pin_store_revision,
            },
        }
    );
    assert_eq!(store.commit_calls(), 4, "exact replay must not recommit");
}

#[test]
fn known_store_refusal_rolls_back_and_same_request_can_commit_later() {
    let (mut durable, store, client) = initialized();
    store.next(NextCommit::Refuse);
    assert_eq!(
        durable.execute(client, create_request("room")),
        DurablePublicationV2::Suppressed(DurableSuppressionV2::StoreRefused(
            StoreRefusal::ValueLimit
        ))
    );
    assert_eq!(durable.state(), DurableOwnerStateV2::Active);
    assert_eq!(durable.service().unwrap().engine().revision().0, 1);
    assert_eq!(durable.store_revision(), Revision(1));

    assert_eq!(
        durable.execute(client, create_request("room")),
        DurablePublicationV2::Published {
            value: ReplyV2::Accepted(OutcomeV2::RoomCreated { revision: 2 }),
            evidence: DurableEvidenceV2::Committed {
                revision: Revision(2)
            }
        }
    );
}

#[test]
fn indeterminate_after_commit_fences_then_recovery_resolves_the_same_request() {
    let (mut durable, store, client) = initialized();
    store.next(NextCommit::IndeterminateAfter);
    assert_eq!(
        durable.execute(client, create_request("room")),
        DurablePublicationV2::Suppressed(DurableSuppressionV2::OutcomeUnknown)
    );
    assert_eq!(durable.state(), DurableOwnerStateV2::Fenced);
    assert!(durable.service().is_none());
    assert_eq!(
        durable.execute(client, create_request("room")),
        DurablePublicationV2::Suppressed(DurableSuppressionV2::Fenced)
    );
    drop(durable.into_store());

    let mut recovered = DurableServiceV2::recover(store, config()).unwrap();
    assert_eq!(
        recovered.execute(client, create_request("room")),
        DurablePublicationV2::Published {
            value: ReplyV2::Accepted(OutcomeV2::RoomCreated { revision: 2 }),
            evidence: DurableEvidenceV2::PreviouslyCommitted {
                revision: Revision(2)
            }
        }
    );
}

#[test]
fn indeterminate_before_commit_recovers_old_state_and_exact_retry_is_safe() {
    let (mut durable, store, client) = initialized();
    store.next(NextCommit::IndeterminateBefore);
    assert_eq!(
        durable.execute(client, create_request("room")),
        DurablePublicationV2::Suppressed(DurableSuppressionV2::OutcomeUnknown)
    );
    drop(durable.into_store());

    let mut recovered = DurableServiceV2::recover(store, config()).unwrap();
    assert_eq!(recovered.service().unwrap().engine().revision().0, 1);
    assert!(matches!(
        recovered.execute(client, create_request("room")),
        DurablePublicationV2::Published {
            value: ReplyV2::Accepted(OutcomeV2::RoomCreated { revision: 2 }),
            evidence: DurableEvidenceV2::Committed { .. }
        }
    ));
}

#[test]
fn weak_acknowledgement_or_store_error_never_publishes_and_fences() {
    for next in [NextCommit::Weak, NextCommit::Error] {
        let (mut durable, store, client) = initialized();
        store.next(next);
        let publication = durable.execute(client, create_request("room"));
        assert!(matches!(
            publication,
            DurablePublicationV2::Suppressed(
                DurableSuppressionV2::NotDurable | DurableSuppressionV2::OutcomeUnknown
            )
        ));
        assert_eq!(durable.state(), DurableOwnerStateV2::Fenced);
        drop(durable.into_store());
        if next == NextCommit::Weak {
            assert_eq!(
                DurableServiceV2::recover(store, config()).unwrap_err(),
                DurableStartErrorV2::NotDurable
            );
        }
    }
}

#[test]
fn debug_output_contains_no_checkpoint_or_store_payload() {
    let (durable, _, _) = initialized();
    let debug = format!("{durable:?}");
    assert!(debug.contains("DurableServiceV2"));
    assert!(!debug.contains("Alice"));
}

#[test]
fn authenticated_host_commits_session_service_and_reply_as_one_recoverable_unit() {
    let store = TestStore::default();
    let host = HostV2::new(service(), Verifier, host_limits());
    let mut durable = DurableHostV2::initialize(host, store.clone(), host_config()).unwrap();
    let context = Context { active: true };
    let DurableHostPublicationV2::Published {
        value: Ok(opened),
        evidence: DurableEvidenceV2::Committed {
            revision: Revision(1),
        },
    } = durable.open(&connect_frame(), &context, HostTimeV1(1))
    else {
        panic!("authenticated session must commit before publication");
    };
    let session = opened.session().unwrap();
    let WireReplyV2::Connected(client) =
        decode_reply_v2(opened.frame(), host_limits().wire).unwrap()
    else {
        panic!("connect reply expected");
    };

    store.next(NextCommit::IndeterminateAfter);
    assert_eq!(
        durable.handle(session, &create_frame(client), &context, HostTimeV1(2)),
        DurableHostPublicationV2::Suppressed(DurableHostSuppressionV2::OutcomeUnknown)
    );
    assert_eq!(durable.state(), DurableOwnerStateV2::Fenced);
    let observation = durable.observation();
    assert_eq!(observation.offers, 2);
    assert_eq!(observation.committed, 1);
    assert_eq!(observation.outcome_unknown, 1);
    drop(durable.into_store());

    let mut recovered = DurableHostV2::recover(store.clone(), host_config(), Verifier).unwrap();
    let DurableHostPublicationV2::Published {
        value: Ok(frame),
        evidence,
    } = recovered.handle(session, &create_frame(client), &context, HostTimeV1(2))
    else {
        panic!("same authenticated request must resolve after recovery");
    };
    assert_eq!(
        evidence,
        DurableEvidenceV2::PreviouslyCommitted {
            revision: Revision(2)
        }
    );
    assert_eq!(
        decode_reply_v2(&frame, host_limits().wire).unwrap(),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::RoomCreated { revision: 2 }))
    );
    assert_eq!(store.commit_calls(), 3, "exact same-time replay is inert");
    let observation = recovered.observation();
    assert_eq!(observation.state, DurableOwnerStateV2::Active);
    assert_eq!(observation.store_revision, 2);
    assert_eq!(observation.offers, 1);
    assert_eq!(observation.previously_committed, 1);
    assert_eq!(observation.committed, 0);
    let diagnostic = format!("{observation:?}");
    assert!(!diagnostic.contains("Alice"));
    assert!(!diagnostic.contains("room"));
}

#[test]
fn operator_restore_commits_session_invalidation_before_activation() {
    let store = TestStore::default();
    let host = HostV2::new(service(), Verifier, host_limits());
    let mut durable = DurableHostV2::initialize(host, store.clone(), host_config()).unwrap();
    let context = Context { active: true };
    let DurableHostPublicationV2::Published {
        value: Ok(opened), ..
    } = durable.open(&connect_frame(), &context, HostTimeV1(1))
    else {
        panic!("connect expected");
    };
    let old_session = opened.session().unwrap();
    let WireReplyV2::Connected(old_client) =
        decode_reply_v2(opened.frame(), host_limits().wire).unwrap()
    else {
        panic!("connected frame expected");
    };
    assert!(matches!(
        durable.handle(
            old_session,
            &create_frame(old_client),
            &context,
            HostTimeV1(2),
        ),
        DurableHostPublicationV2::Published {
            value: Ok(_),
            evidence: DurableEvidenceV2::Committed {
                revision: Revision(2)
            }
        }
    ));
    drop(durable.into_store());

    let mut authority = RestoreAuthority::default();
    let restored = DurableHostV2::recover_after_restore(
        store.clone(),
        host_config(),
        Verifier,
        OperatorRestoreAuthorizationV1::new(&mut authority, &()),
    )
    .unwrap();
    assert_eq!(
        authority.requests,
        vec![("agora-host".to_owned(), HOST_SCOPE, Revision(2))]
    );
    assert_eq!(restored.observation().store_revision, 3);
    assert_eq!(restored.host().unwrap().service().active_client_count(), 0);
    assert_eq!(restored.host().unwrap().service().engine().revision().0, 2);
    assert_eq!(
        store.commit_calls(),
        4,
        "restore invalidation is one commit"
    );
    drop(restored.into_store());

    let mut restarted = DurableHostV2::recover(store.clone(), host_config(), Verifier).unwrap();
    assert_eq!(restarted.host().unwrap().service().active_client_count(), 0);
    assert_eq!(
        restarted.handle(
            old_session,
            &create_frame(old_client),
            &context,
            HostTimeV1(3),
        ),
        DurableHostPublicationV2::Published {
            value: Err(HostRefusal::Unavailable),
            evidence: DurableEvidenceV2::PreviouslyCommitted {
                revision: Revision(3)
            }
        }
    );
    assert_eq!(store.commit_calls(), 4, "invalid old session is inert");

    let DurableHostPublicationV2::Published {
        value: Ok(opened),
        evidence: DurableEvidenceV2::Committed {
            revision: Revision(4),
        },
    } = restarted.open(&connect_frame(), &context, HostTimeV1(3))
    else {
        panic!("fresh post-restore connection expected");
    };
    let WireReplyV2::Connected(new_client) =
        decode_reply_v2(opened.frame(), host_limits().wire).unwrap()
    else {
        panic!("connected frame expected");
    };
    assert_ne!(opened.session().unwrap(), old_session);
    assert_eq!(new_client.0, 1, "client identity frontier must not rewind");
}

#[test]
fn operator_restore_never_activates_without_exact_durable_invalidation() {
    for (next, expected) in [
        (
            NextCommit::Refuse,
            DurableHostStartErrorV2::StoreRefused(StoreRefusal::ValueLimit),
        ),
        (
            NextCommit::IndeterminateBefore,
            DurableHostStartErrorV2::OutcomeUnknown,
        ),
        (
            NextCommit::IndeterminateAfter,
            DurableHostStartErrorV2::OutcomeUnknown,
        ),
        (NextCommit::Weak, DurableHostStartErrorV2::NotDurable),
        (
            NextCommit::WrongRevision,
            DurableHostStartErrorV2::InvalidReceipt,
        ),
        (NextCommit::Error, DurableHostStartErrorV2::StoreFault),
    ] {
        let store = TestStore::default();
        let host = HostV2::new(service(), Verifier, host_limits());
        let mut durable = DurableHostV2::initialize(host, store.clone(), host_config()).unwrap();
        let context = Context { active: true };
        assert!(matches!(
            durable.open(&connect_frame(), &context, HostTimeV1(1)),
            DurableHostPublicationV2::Published { value: Ok(_), .. }
        ));
        drop(durable.into_store());
        store.next(next);

        let mut authority = RestoreAuthority::default();
        assert_eq!(
            DurableHostV2::recover_after_restore(
                store,
                host_config(),
                Verifier,
                OperatorRestoreAuthorizationV1::new(&mut authority, &()),
            )
            .unwrap_err(),
            expected,
            "restore boundary must fail closed for {next:?}"
        );
    }

    let store = TestStore::default();
    let host = HostV2::new(service(), Verifier, host_limits());
    let mut durable = DurableHostV2::initialize(host, store.clone(), host_config()).unwrap();
    let context = Context { active: true };
    assert!(matches!(
        durable.open(&connect_frame(), &context, HostTimeV1(1)),
        DurableHostPublicationV2::Published { value: Ok(_), .. }
    ));
    drop(durable.into_store());
    let foreign = DurableHostConfigV2::new(
        "agora-host",
        HostCheckpointScopeV1::new(*b"foreign-host-v1!"),
        MAX,
        HostRecoveryLimitsV2 {
            max_snapshot_bytes: MAX,
            max_service_snapshot_bytes: MAX,
            maximum_host_limits: host_limits(),
            service: ServiceRecoveryLimitsV2 {
                max_snapshot_bytes: MAX,
                max_engine_snapshot_bytes: MAX,
                maximum_service_limits: ServiceLimitsV2::default(),
                maximum_engine_limits: Limits::default(),
            },
        },
    );
    let mut authority = RestoreAuthority::default();
    assert_eq!(
        DurableHostV2::recover_after_restore(
            store.clone(),
            foreign,
            Verifier,
            OperatorRestoreAuthorizationV1::new(&mut authority, &()),
        )
        .unwrap_err(),
        DurableHostStartErrorV2::Checkpoint(
            agora_host::v2::persistence::HostCheckpointErrorV2::Scope
        )
    );
    assert_eq!(store.commit_calls(), 2, "checkpoint refusal is precommit");
    assert!(
        authority.requests.is_empty(),
        "invalid checkpoint is pre-authority"
    );
}

#[test]
fn operator_restore_authority_denial_is_precommit_and_non_enumerating() {
    let store = TestStore::default();
    let host = HostV2::new(service(), Verifier, host_limits());
    let durable = DurableHostV2::initialize(host, store.clone(), host_config()).unwrap();
    drop(durable.into_store());
    let before = store.0.borrow().checkpoint.clone();
    let mut authority = RestoreAuthority {
        deny: true,
        ..RestoreAuthority::default()
    };

    assert_eq!(
        DurableHostV2::recover_after_restore(
            store.clone(),
            host_config(),
            Verifier,
            OperatorRestoreAuthorizationV1::new(&mut authority, &()),
        )
        .unwrap_err(),
        DurableHostStartErrorV2::Unauthorized
    );
    assert_eq!(authority.requests.len(), 1);
    assert_eq!(
        store.commit_calls(),
        1,
        "denial cannot reach store mutation"
    );
    assert_eq!(store.0.borrow().checkpoint, before);
}

#[test]
fn transient_operator_restore_validates_configuration_before_commit() {
    let store = TestStore::default();
    let host = HostV2::new(service(), Verifier, host_limits());
    let mut durable = DurableHostV2::initialize(host, store.clone(), host_config()).unwrap();
    let context = Context { active: true };
    assert!(matches!(
        durable.open(&connect_frame(), &context, HostTimeV1(1)),
        DurableHostPublicationV2::Published { value: Ok(_), .. }
    ));
    drop(durable.into_store());
    assert_eq!(store.commit_calls(), 2);

    assert!(matches!(
        DurableTransientHostV1::recover_after_restore(
            store.clone(),
            host_config(),
            Verifier,
            OperatorRestoreAuthorizationV1::new(&mut RestoreAuthority::default(), &()),
            TransientLimitsV1::default(),
            TypingLimitsV1::default(),
            0,
        ),
        Err(agora_durable::DurableTransientHostStartErrorV1::Transient(
            TransientHostBuildRefusalV1::InvalidCursorAge
        ))
    ));
    assert_eq!(
        store.commit_calls(),
        2,
        "invalid transient configuration must not mutate restored storage"
    );

    let mut authority = RestoreAuthority::default();
    let restored = DurableTransientHostV1::recover_after_restore(
        store.clone(),
        host_config(),
        Verifier,
        OperatorRestoreAuthorizationV1::new(&mut authority, &()),
        TransientLimitsV1::default(),
        TypingLimitsV1::default(),
        10,
    )
    .unwrap();
    assert_eq!(restored.observation().store_revision, 2);
    assert_eq!(
        restored
            .host()
            .unwrap()
            .host()
            .service()
            .active_client_count(),
        0
    );
    assert_eq!(store.commit_calls(), 3);
}

#[test]
fn host_store_refusal_rolls_back_both_session_and_service_namespace() {
    let store = TestStore::default();
    let host = HostV2::new(service(), Verifier, host_limits());
    let mut durable = DurableHostV2::initialize(host, store.clone(), host_config()).unwrap();
    let context = Context { active: true };
    store.next(NextCommit::Refuse);
    assert!(matches!(
        durable.open(&connect_frame(), &context, HostTimeV1(1)),
        DurableHostPublicationV2::Suppressed(DurableHostSuppressionV2::StoreRefused(
            StoreRefusal::ValueLimit
        ))
    ));
    let DurableHostPublicationV2::Published {
        value: Ok(opened), ..
    } = durable.open(&connect_frame(), &context, HostTimeV1(1))
    else {
        panic!("retry after known refusal must commit");
    };
    let WireReplyV2::Connected(client) =
        decode_reply_v2(opened.frame(), host_limits().wire).unwrap()
    else {
        panic!("connect reply expected");
    };
    assert_eq!(client.0, 0, "rolled back client namespace must not skip");
}

#[test]
fn durable_transient_refusal_restores_host_and_never_leaks_a_signal() {
    let store = TestStore::default();
    let transient = TransientHostV1::new_with_typing(
        HostV2::new(service(), Verifier, host_limits()),
        TransientLimitsV1::default(),
        TypingLimitsV1::default(),
        10,
    )
    .unwrap();
    let mut durable =
        DurableTransientHostV1::initialize(transient, store.clone(), host_config()).unwrap();
    let context = Context { active: true };
    let DurableHostPublicationV2::Published {
        value: Ok(opened), ..
    } = durable.open(&connect_frame(), &context, HostTimeV1(1))
    else {
        panic!("durable transient connect expected");
    };
    let session = opened.session().unwrap();
    let WireReplyV2::Connected(client) =
        decode_reply_v2(opened.frame(), host_limits().wire).unwrap()
    else {
        panic!("connected frame expected");
    };
    assert!(matches!(
        durable.handle(session, &create_frame(client), &context, HostTimeV1(2)),
        DurableHostPublicationV2::Published {
            value: Ok(_),
            evidence: DurableEvidenceV2::Committed { .. },
        }
    ));

    let request = |request_id, command| {
        encode_transient_request_v1(
            &TransientRequestV1 {
                request_id: TransientRequestIdV1(request_id),
                command,
            },
            host_limits().wire,
        )
        .unwrap()
    };
    store.next(NextCommit::Refuse);
    let publish = request(
        0,
        TransientCommandV1::PublishPresence {
            room_id: "room".to_owned(),
            state: PresenceStateV1::Available,
            lifetime: 20,
        },
    );
    assert!(matches!(
        durable.handle_transient(session, &publish, &context, HostTimeV1(3)),
        DurableHostPublicationV2::Suppressed(DurableHostSuppressionV2::StoreRefused(
            StoreRefusal::ValueLimit
        ))
    ));
    assert_eq!(durable.state(), DurableOwnerStateV2::Active);

    let page = request(
        0,
        TransientCommandV1::PresencePage {
            room_id: "room".to_owned(),
            cursor: None,
            limit: 8,
        },
    );
    let DurableHostPublicationV2::Published {
        value: Ok(reply),
        evidence: DurableEvidenceV2::Committed { .. },
    } = durable.handle_transient(session, &page, &context, HostTimeV1(4))
    else {
        panic!("post-refusal observation expected");
    };
    let TransientReplyV1::Accepted(TransientOutcomeV1::PresencePage(page)) =
        decode_transient_reply_v1(&reply, host_limits().wire).unwrap()
    else {
        panic!("presence page expected");
    };
    assert_eq!(page.entries.len(), 1);
    assert_eq!(page.entries[0].state, PresenceViewStateV1::Unknown);

    let publish = request(
        1,
        TransientCommandV1::PublishPresence {
            room_id: "room".to_owned(),
            state: PresenceStateV1::Available,
            lifetime: 20,
        },
    );
    assert!(matches!(
        durable.handle_transient(session, &publish, &context, HostTimeV1(5)),
        DurableHostPublicationV2::Published {
            value: Ok(_),
            evidence: DurableEvidenceV2::Committed { .. },
        }
    ));
    store.next(NextCommit::IndeterminateAfter);
    let page = request(
        2,
        TransientCommandV1::PresencePage {
            room_id: "room".to_owned(),
            cursor: None,
            limit: 8,
        },
    );
    assert_eq!(
        durable.handle_transient(session, &page, &context, HostTimeV1(6)),
        DurableHostPublicationV2::Suppressed(DurableHostSuppressionV2::OutcomeUnknown)
    );
    assert_eq!(durable.state(), DurableOwnerStateV2::Fenced);
    assert!(durable.host().is_none());
    let observation = durable.observation();
    assert_eq!(observation.state, DurableOwnerStateV2::Fenced);
    assert_eq!(observation.store_revision, 4);
    assert_eq!(observation.offers, 6);
    assert_eq!(observation.committed, 4);
    assert_eq!(observation.previously_committed, 0);
    assert_eq!(observation.store_refused, 1);
    assert_eq!(observation.outcome_unknown, 1);
    assert_eq!(observation.checkpoint_suppressed, 0);
    assert_eq!(observation.not_durable, 0);
    assert_eq!(observation.invalid_receipt, 0);
    assert_eq!(observation.fenced, 0);
    assert!(!observation.saturated);
    let diagnostic = format!("{durable:?}");
    assert!(diagnostic.contains("presence_signal_count: 0"));
    assert!(!diagnostic.contains("alice"));
    assert!(store.commit_calls() >= 7);
}

#[test]
fn durable_revocation_cleanup_is_committed_before_unavailable_is_visible() {
    let store = TestStore::default();
    let host = HostV2::new(service(), Verifier, host_limits());
    let mut durable = DurableHostV2::initialize(host, store.clone(), host_config()).unwrap();
    let active = Context { active: true };
    let DurableHostPublicationV2::Published {
        value: Ok(opened), ..
    } = durable.open(&connect_frame(), &active, HostTimeV1(1))
    else {
        panic!("connect expected");
    };
    let session = opened.session().unwrap();
    let WireReplyV2::Connected(client) =
        decode_reply_v2(opened.frame(), host_limits().wire).unwrap()
    else {
        panic!("connect reply expected");
    };
    let revoked = Context { active: false };
    assert_eq!(
        durable.handle(session, &create_frame(client), &revoked, HostTimeV1(2)),
        DurableHostPublicationV2::Published {
            value: Err(HostRefusal::Unavailable),
            evidence: DurableEvidenceV2::Committed {
                revision: Revision(2)
            }
        }
    );
    drop(durable.into_store());
    let recovered = DurableHostV2::recover(store, host_config(), Verifier).unwrap();
    assert!(recovered.host().unwrap().service().active_client_count() == 0);
}
