use agora_durable::{
    DurableConfigV2, DurableEvidenceV2, DurableOwnerStateV2, DurablePublicationV2,
    DurableServiceV2, DurableStartErrorV2, DurableSuppressionV2,
};
use agora_engine::{CommandV2 as EngineCommand, Engine, Limits, Participant, ParticipantKind};
use agora_protocol::v2::{
    AdmissionPolicyV2, CommandV2, OutcomeV2, RefusalV2, ReplyV2, RequestIdV2, RequestV2,
};
use agora_service::v2::{
    ServiceLimitsV2, ServiceV2,
    persistence::{ServiceCheckpointScopeV1, ServiceRecoveryLimitsV2},
};
use keystone_storage::{
    AcknowledgementStrength, Checkpoint, CheckpointStore, CommitOutcome, CommitReceipt, Revision,
    StoreRefusal,
};
use std::{cell::RefCell, rc::Rc};

const MAX: usize = 2 * 1_024 * 1_024;
const SCOPE: ServiceCheckpointScopeV1 = ServiceCheckpointScopeV1::new(*b"durable-test-v1!");

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum NextCommit {
    #[default]
    Normal,
    Refuse,
    IndeterminateBefore,
    IndeterminateAfter,
    Weak,
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
