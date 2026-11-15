use std::collections::{BTreeMap, BTreeSet};

use agora_engine::retention::{
    ContentDigestProfileV1, ContentDigestV1, ExportMaterializationV1, ExportProfileV1,
    ExportRecordV1, ExportRequestIdV1, ExportScopeBindingV1, RECORD_CLASSES_V1, RecordClassV1,
    RetentionBoundV1, RetentionFrontierV1, RetentionPolicyRevisionV1, RetentionPolicyV1,
    RetentionRecordIdV1, RetentionRecordV1, materialize_export_v1, plan_retention_v1,
};

const MAX_RECORDS: usize = 12;
const MAX_DEPENDENCIES: usize = 2;

fn id(class: RecordClassV1, ordinal: u64) -> RetentionRecordIdV1 {
    RetentionRecordIdV1::new(class, ordinal).unwrap()
}

fn policy(bounds: [RetentionBoundV1; 6]) -> RetentionPolicyV1 {
    RetentionPolicyV1::new(RetentionPolicyRevisionV1::new(1).unwrap(), 7, bounds)
}

fn records_with_dependencies(
    dependencies: &BTreeMap<RetentionRecordIdV1, Vec<RetentionRecordIdV1>>,
) -> Vec<RetentionRecordV1> {
    RECORD_CLASSES_V1
        .into_iter()
        .flat_map(|class| [id(class, 1), id(class, 2)])
        .map(|record_id| {
            RetentionRecordV1::new(
                record_id,
                dependencies.get(&record_id).cloned().unwrap_or_default(),
                MAX_DEPENDENCIES,
            )
            .unwrap()
        })
        .collect()
}

fn reference_retained(
    records: &[RetentionRecordV1],
    bounds: [RetentionBoundV1; 6],
) -> BTreeSet<RetentionRecordIdV1> {
    let mut retained = BTreeSet::new();
    for class in RECORD_CLASSES_V1 {
        let maximum = bounds[class as usize - 1].maximum().unwrap_or(0);
        retained.extend(
            records
                .iter()
                .rev()
                .filter(|record| record.id.class == class)
                .take(maximum)
                .map(|record| record.id),
        );
    }

    loop {
        let prior_len = retained.len();
        for record in records {
            if retained.contains(&record.id) {
                retained.extend(record.dependencies().iter().copied());
            }
        }
        if retained.len() == prior_len {
            return retained;
        }
    }
}

fn assert_matches_reference(records: &[RetentionRecordV1], bounds: [RetentionBoundV1; 6]) {
    let expected = reference_retained(records, bounds);
    let first =
        plan_retention_v1(policy(bounds), 9, records, MAX_RECORDS, MAX_DEPENDENCIES).unwrap();
    let second =
        plan_retention_v1(policy(bounds), 9, records, MAX_RECORDS, MAX_DEPENDENCIES).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        first.retained(),
        expected.iter().copied().collect::<Vec<_>>()
    );
    assert_eq!(
        first.discarded(),
        records
            .iter()
            .map(|record| record.id)
            .filter(|record_id| !expected.contains(record_id))
            .collect::<Vec<_>>()
    );

    for (class_index, class) in RECORD_CLASSES_V1.into_iter().enumerate() {
        let retained_for_class = expected
            .iter()
            .filter(|record_id| record_id.class == class)
            .copied()
            .collect::<Vec<_>>();
        let discarded_for_class = records
            .iter()
            .filter(|record| record.id.class == class && !expected.contains(&record.id))
            .map(|record| record.id)
            .collect::<Vec<_>>();
        assert_eq!(
            first.transition.discarded_records[class_index],
            discarded_for_class.len() as u64
        );
        assert_eq!(
            first.transition.frontiers[class_index],
            RetentionFrontierV1::new(
                class,
                discarded_for_class
                    .iter()
                    .take_while(|record_id| {
                        retained_for_class
                            .first()
                            .is_none_or(|oldest| record_id.ordinal < oldest.ordinal)
                    })
                    .map(|record_id| record_id.ordinal)
                    .last()
                    .unwrap_or(0),
                retained_for_class
                    .first()
                    .map(|record_id| record_id.ordinal),
            )
            .unwrap()
        );
    }
}

#[test]
fn every_small_complete_policy_matches_an_independent_reference_model() {
    let records = records_with_dependencies(&BTreeMap::new());
    for encoded in 0_u16..3_u16.pow(6) {
        let mut cursor = encoded;
        let bounds = RECORD_CLASSES_V1.map(|_| {
            let bound = match cursor % 3 {
                0 => RetentionBoundV1::Disabled,
                1 => RetentionBoundV1::entries(1).unwrap(),
                2 => RetentionBoundV1::entries(2).unwrap(),
                _ => unreachable!(),
            };
            cursor /= 3;
            bound
        });
        assert_matches_reference(&records, bounds);
    }
}

#[test]
fn every_cross_record_dependency_edge_and_chain_matches_the_reference_closure() {
    let every_id = RECORD_CLASSES_V1
        .into_iter()
        .flat_map(|class| [id(class, 1), id(class, 2)])
        .collect::<Vec<_>>();

    for root in every_id
        .iter()
        .copied()
        .filter(|record_id| record_id.ordinal == 2)
    {
        for middle in every_id
            .iter()
            .copied()
            .filter(|record_id| *record_id != root)
        {
            for leaf in every_id
                .iter()
                .copied()
                .filter(|record_id| *record_id != root && *record_id != middle)
            {
                let mut dependencies = BTreeMap::new();
                dependencies.insert(root, vec![middle]);
                dependencies.insert(middle, vec![leaf]);
                let records = records_with_dependencies(&dependencies);
                let bounds = RECORD_CLASSES_V1.map(|class| {
                    if class == root.class {
                        RetentionBoundV1::entries(1).unwrap()
                    } else {
                        RetentionBoundV1::Disabled
                    }
                });
                assert_matches_reference(&records, bounds);
            }
        }
    }
}

fn additive_digest(bytes: &[u8]) -> ContentDigestV1 {
    let sum = bytes
        .iter()
        .fold(0_u8, |state, byte| state.wrapping_add(*byte));
    ContentDigestV1::new([sum; 32])
}

#[test]
fn every_single_bit_chunk_mutation_is_rejected() {
    let records = [
        ExportRecordV1::new(
            id(RecordClassV1::RoomProjection, 1),
            b"room projection".to_vec(),
            32,
        )
        .unwrap(),
        ExportRecordV1::new(
            id(RecordClassV1::FederationState, 1),
            b"federation copy".to_vec(),
            32,
        )
        .unwrap(),
    ];
    let frontiers = RECORD_CLASSES_V1.map(|class| {
        RetentionFrontierV1::new(
            class,
            0,
            matches!(
                class,
                RecordClassV1::RoomProjection | RecordClassV1::FederationState
            )
            .then_some(1),
        )
        .unwrap()
    });
    let included = RECORD_CLASSES_V1.map(|class| {
        matches!(
            class,
            RecordClassV1::RoomProjection | RecordClassV1::FederationState
        )
    });
    let materialization = materialize_export_v1(
        ExportRequestIdV1::new([1; 16]).unwrap(),
        ExportProfileV1::Administrative,
        ExportScopeBindingV1::new([2; 32]).unwrap(),
        ContentDigestProfileV1::Sha256,
        9,
        RetentionPolicyRevisionV1::new(1).unwrap(),
        included,
        frontiers,
        &records,
        32,
        32,
        8,
        64,
        additive_digest,
    )
    .unwrap();

    for chunk_index in 0..materialization.chunks().len() {
        for byte_index in 0..materialization.chunks()[chunk_index].bytes().len() {
            for bit in 0..8 {
                let mut chunks = materialization.chunks().to_vec();
                let mut bytes = chunks[chunk_index].bytes().to_vec();
                bytes[byte_index] ^= 1 << bit;
                let Some(mutated) = agora_engine::retention::ExportChunkPayloadV1::new(
                    chunk_index as u32,
                    bytes,
                    64,
                ) else {
                    continue;
                };
                chunks[chunk_index] = mutated;
                assert!(
                    ExportMaterializationV1::recover(
                        materialization.manifest.clone(),
                        chunks,
                        8,
                        64,
                        additive_digest,
                    )
                    .is_err()
                );
            }
        }
    }
}
