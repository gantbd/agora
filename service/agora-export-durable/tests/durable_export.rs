use agora_engine::retention::{
    ContentDigestProfileV1, ContentDigestV1, ExportMaterializationApplyV1, ExportProfileV1,
    ExportRecordV1, ExportRequestIdV1, ExportScopeBindingV1, RECORD_CLASSES_V1, RecordClassV1,
    RetentionFrontierV1, RetentionPolicyRevisionV1, RetentionRecordIdV1, materialize_export_v1,
};
use agora_export_durable::{
    DurableExportConfigV1, DurableExportEvidenceV1, DurableExportPublicationV1,
    DurableExportServiceV1, DurableExportStateV1, DurableExportSuppressionV1,
};
use agora_export_service::{
    ExportAdmissionV1, ExportCheckpointScopeV1, ExportRecoveryLimitsV1, ExportServiceLimitsV1,
    ExportServiceV1,
};
use keystone_storage::{
    AcknowledgementStrength, Checkpoint, CheckpointStore, CommitOutcome, CommitReceipt, Revision,
    StoreRefusal,
};
use std::cell::RefCell;
use std::rc::Rc;

fn digest(bytes: &[u8]) -> ContentDigestV1 {
    let mut output = [0_u8; 32];
    for (index, byte) in bytes.iter().enumerate() {
        let slot = index % output.len();
        output[slot] = output[slot].wrapping_add(*byte).wrapping_add(index as u8);
    }
    ContentDigestV1::new(output)
}

fn policy_revision() -> RetentionPolicyRevisionV1 {
    RetentionPolicyRevisionV1::new(3).unwrap()
}

fn materialization(
    request: u8,
    payload: &[u8],
) -> agora_engine::retention::ExportMaterializationV1 {
    let record = ExportRecordV1::new(
        RetentionRecordIdV1::new(RecordClassV1::RoomProjection, 1).unwrap(),
        payload.to_vec(),
        16,
    )
    .unwrap();
    let frontiers = RECORD_CLASSES_V1.map(|class| {
        RetentionFrontierV1::new(
            class,
            0,
            (class == RecordClassV1::RoomProjection).then_some(1),
        )
        .unwrap()
    });
    materialize_export_v1(
        ExportRequestIdV1::new([request; 16]).unwrap(),
        ExportProfileV1::RoomVisible,
        ExportScopeBindingV1::new([9; 32]).unwrap(),
        ContentDigestProfileV1::Sha256,
        8,
        policy_revision(),
        [true, false, false, false, false, false],
        frontiers,
        &[record],
        1,
        16,
        2,
        64,
        digest,
    )
    .unwrap()
}

fn admission() -> ExportAdmissionV1 {
    ExportAdmissionV1 {
        current_engine_revision: 8,
        current_policy_revision: policy_revision(),
    }
}

fn limits() -> ExportServiceLimitsV1 {
    ExportServiceLimitsV1 {
        max_exports: 4,
        max_chunks_per_export: 2,
        max_chunk_bytes: 64,
    }
}

fn config() -> DurableExportConfigV1 {
    DurableExportConfigV1::new(
        "export-custody",
        ExportCheckpointScopeV1::new([4; 16]).unwrap(),
        8_192,
        ExportRecoveryLimitsV1 {
            max_snapshot_bytes: 8_192,
            maximum_service_limits: limits(),
        },
    )
    .unwrap()
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum NextOutcome {
    #[default]
    Commit,
    Refuse,
    Indeterminate,
    Weak,
    InvalidReceipt,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct StoreState {
    checkpoint: Option<Checkpoint>,
    next: NextOutcome,
    offers: usize,
}

#[derive(Clone, Debug, Default)]
struct SharedStore(Rc<RefCell<StoreState>>);

impl SharedStore {
    fn set_next(&self, next: NextOutcome) {
        self.0.borrow_mut().next = next;
    }

    fn offers(&self) -> usize {
        self.0.borrow().offers
    }
}

impl CheckpointStore for SharedStore {
    type Error = ();

    fn load(&self, key: &str) -> Result<Option<Checkpoint>, Self::Error> {
        Ok((key == "export-custody")
            .then(|| self.0.borrow().checkpoint.clone())
            .flatten())
    }

    fn compare_exchange(
        &mut self,
        key: &str,
        expected: Option<Revision>,
        bytes: &[u8],
    ) -> Result<CommitOutcome, Self::Error> {
        let mut state = self.0.borrow_mut();
        state.offers += 1;
        if key != "export-custody" {
            return Ok(CommitOutcome::Refused(StoreRefusal::InvalidKey));
        }
        let actual = state.checkpoint.as_ref().map(|value| value.revision);
        if actual != expected {
            return Ok(CommitOutcome::Refused(StoreRefusal::RevisionConflict {
                expected,
                actual,
            }));
        }
        let next = state.next;
        state.next = NextOutcome::Commit;
        match next {
            NextOutcome::Refuse => Ok(CommitOutcome::Refused(StoreRefusal::ValueLimit)),
            NextOutcome::Indeterminate => Ok(CommitOutcome::Indeterminate),
            NextOutcome::Weak => Ok(CommitOutcome::Committed(CommitReceipt {
                revision: actual.map_or(Revision(0), |revision| Revision(revision.0 + 1)),
                mechanism: "test-weak-export-store",
                strength: AcknowledgementStrength::Volatile,
            })),
            NextOutcome::InvalidReceipt => Ok(CommitOutcome::Committed(CommitReceipt {
                revision: actual.map_or(Revision(1), |revision| Revision(revision.0 + 2)),
                mechanism: "test-invalid-export-store",
                strength: AcknowledgementStrength::Durable,
            })),
            NextOutcome::Commit => {
                let revision = actual.map_or(Revision(0), |revision| Revision(revision.0 + 1));
                state.checkpoint = Some(Checkpoint {
                    revision,
                    bytes: bytes.to_vec(),
                    strength: AcknowledgementStrength::Durable,
                });
                Ok(CommitOutcome::Committed(CommitReceipt {
                    revision,
                    mechanism: "test-export-store",
                    strength: AcknowledgementStrength::Durable,
                }))
            }
        }
    }
}

#[test]
fn commit_gate_replays_rolls_back_known_refusal_and_recovers() {
    let store = SharedStore::default();
    let mut durable = DurableExportServiceV1::initialize(
        ExportServiceV1::new(limits()).unwrap(),
        store.clone(),
        digest,
        config(),
    )
    .unwrap();
    assert_eq!(durable.store_revision(), Revision(0));
    let first = materialization(1, b"one");
    assert_eq!(
        durable.admit(admission(), first.clone()),
        DurableExportPublicationV1::Published {
            value: Ok(ExportMaterializationApplyV1::Materialized),
            evidence: DurableExportEvidenceV1::Committed {
                revision: Revision(1)
            },
        }
    );
    let offers = store.offers();
    assert_eq!(
        durable.admit(admission(), first.clone()),
        DurableExportPublicationV1::Published {
            value: Ok(ExportMaterializationApplyV1::Replay),
            evidence: DurableExportEvidenceV1::PreviouslyCommitted {
                revision: Revision(1)
            },
        }
    );
    assert_eq!(
        store.offers(),
        offers,
        "replay must not offer another checkpoint"
    );

    store.set_next(NextOutcome::Refuse);
    assert_eq!(
        durable.admit(admission(), materialization(2, b"two")),
        DurableExportPublicationV1::Suppressed(DurableExportSuppressionV1::StoreRefused(
            StoreRefusal::ValueLimit
        ))
    );
    assert_eq!(durable.state(), DurableExportStateV1::Active);
    assert_eq!(durable.service().unwrap().len(), 1);
    assert_eq!(durable.store_revision(), Revision(1));

    assert!(matches!(
        durable.admit(admission(), materialization(2, b"two")),
        DurableExportPublicationV1::Published {
            evidence: DurableExportEvidenceV1::Committed {
                revision: Revision(2)
            },
            ..
        }
    ));
    let recovered = DurableExportServiceV1::recover(store.clone(), digest, config()).unwrap();
    assert_eq!(recovered.store_revision(), Revision(2));
    assert_eq!(recovered.service().unwrap().len(), 2);
    assert_eq!(
        recovered
            .service()
            .unwrap()
            .materialization(first.manifest.request),
        Some(&first)
    );
}

#[test]
fn indeterminate_offer_fences_until_fresh_recovery() {
    let store = SharedStore::default();
    let mut durable = DurableExportServiceV1::initialize(
        ExportServiceV1::new(limits()).unwrap(),
        store.clone(),
        digest,
        config(),
    )
    .unwrap();
    store.set_next(NextOutcome::Indeterminate);
    assert_eq!(
        durable.admit(admission(), materialization(1, b"one")),
        DurableExportPublicationV1::Suppressed(DurableExportSuppressionV1::OutcomeUnknown)
    );
    assert_eq!(durable.state(), DurableExportStateV1::Fenced);
    assert!(durable.service().is_none());
    assert_eq!(
        durable.admit(admission(), materialization(2, b"two")),
        DurableExportPublicationV1::Suppressed(DurableExportSuppressionV1::Fenced)
    );
    let recovered = DurableExportServiceV1::recover(store, digest, config()).unwrap();
    assert_eq!(recovered.state(), DurableExportStateV1::Active);
    assert!(recovered.service().unwrap().is_empty());
}

#[test]
fn weak_and_invalid_receipts_never_publish_custody() {
    for (next, expected) in [
        (NextOutcome::Weak, DurableExportSuppressionV1::NotDurable),
        (
            NextOutcome::InvalidReceipt,
            DurableExportSuppressionV1::InvalidReceipt,
        ),
    ] {
        let store = SharedStore::default();
        let mut durable = DurableExportServiceV1::initialize(
            ExportServiceV1::new(limits()).unwrap(),
            store.clone(),
            digest,
            config(),
        )
        .unwrap();
        store.set_next(next);
        assert_eq!(
            durable.admit(admission(), materialization(1, b"one")),
            DurableExportPublicationV1::Suppressed(expected)
        );
        assert_eq!(durable.state(), DurableExportStateV1::Fenced);
        assert!(durable.service().is_none());
    }
}
