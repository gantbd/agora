use agora_engine::retention::{ContentDigestV1, ReclamationStageV1};
use agora_storage_directory::generational::{
    GenerationalDirectoryCheckpointStoreV1, GenerationalDirectoryErrorV1,
    GenerationalDirectoryLimitsV1, GenerationalReclamationApplyV1,
    GenerationalReclamationRefusalV1,
};
use keystone_storage::{CheckpointStore, CommitOutcome, Revision, StoreRefusal};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temporary_path(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "agora-generational-{label}-{}-{nonce}",
        std::process::id()
    ))
}

fn limits() -> GenerationalDirectoryLimitsV1 {
    GenerationalDirectoryLimitsV1 {
        max_key_bytes: 32,
        max_value_bytes: 1_024,
        max_live_generations: 3,
        max_reclamation_receipts: 4,
        max_generations_per_reclamation: 2,
    }
}

fn digest(bytes: &[u8]) -> ContentDigestV1 {
    let mut output = [0_u8; 32];
    for (index, byte) in bytes.iter().enumerate() {
        output[index % 32] = output[index % 32]
            .wrapping_add(*byte)
            .wrapping_add(index as u8);
    }
    ContentDigestV1::new(output)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn commit(
    store: &mut GenerationalDirectoryCheckpointStoreV1,
    expected: Option<Revision>,
    bytes: &[u8],
) -> Revision {
    match store.compare_exchange("owner", expected, bytes).unwrap() {
        CommitOutcome::Committed(receipt) => receipt.revision,
        outcome => panic!("unexpected commit outcome: {outcome:?}"),
    }
}

#[test]
fn reclamation_requires_exact_reopen_then_unlinks_and_replays_across_restart() {
    let path = temporary_path("reclaim");
    let mut store = GenerationalDirectoryCheckpointStoreV1::create_new(&path, limits()).unwrap();
    assert_eq!(commit(&mut store, None, b"zero"), Revision(0));
    assert_eq!(commit(&mut store, Some(Revision(0)), b"one"), Revision(1));
    assert_eq!(commit(&mut store, Some(Revision(1)), b"two"), Revision(2));
    assert_eq!(store.live_generation_count(), 3);
    assert_eq!(
        store.prepare_reclamation(11, 2, digest),
        Err(GenerationalReclamationRefusalV1::NotReopened)
    );
    assert_eq!(
        store
            .compare_exchange("owner", Some(Revision(2)), b"three")
            .unwrap(),
        CommitOutcome::Refused(StoreRefusal::EntryLimit)
    );
    drop(store);

    let mut reopened =
        GenerationalDirectoryCheckpointStoreV1::open_existing(&path, limits()).unwrap();
    assert_eq!(
        reopened.load("owner").unwrap().unwrap().revision,
        Revision(2)
    );
    let plan = reopened.prepare_reclamation(11, 2, digest).unwrap();
    assert_eq!(plan.source_engine_revision, 11);
    assert_eq!(plan.source_store_revision, 1);
    assert_eq!(plan.replacement_store_revision, 2);
    assert_eq!(plan.obsolete_generations().len(), 2);
    let outcome = reopened.reclaim(plan.clone(), digest).unwrap();
    assert_eq!(outcome.apply, GenerationalReclamationApplyV1::Reclaimed);
    assert_eq!(outcome.evidence().len(), 2);
    assert!(outcome.evidence().iter().all(|evidence| {
        evidence.stage == ReclamationStageV1::GenerationUnlinked
            && !evidence.proves_secure_erasure()
    }));
    assert_eq!(reopened.live_generation_count(), 1);
    assert_eq!(
        hex(&fs::read(path.join(".reclamation")).unwrap()),
        include_str!("fixtures/generational-reclamation-v1.hex").trim()
    );
    assert_eq!(
        reopened.reclaim(plan.clone(), digest).unwrap().apply,
        GenerationalReclamationApplyV1::Replay
    );
    drop(reopened);

    let mut reopened =
        GenerationalDirectoryCheckpointStoreV1::open_existing(&path, limits()).unwrap();
    assert_eq!(
        reopened.reclaim(plan, digest).unwrap().apply,
        GenerationalReclamationApplyV1::Replay
    );
    assert_eq!(
        commit(&mut reopened, Some(Revision(2)), b"three"),
        Revision(3)
    );
    assert_eq!(
        reopened.prepare_reclamation(12, 1, digest),
        Err(GenerationalReclamationRefusalV1::NotReopened)
    );
    drop(reopened);

    let mut reopened =
        GenerationalDirectoryCheckpointStoreV1::open_existing(&path, limits()).unwrap();
    assert_eq!(
        reopened.load("owner").unwrap().unwrap().bytes,
        b"three".to_vec()
    );
    let next = reopened.prepare_reclamation(12, 1, digest).unwrap();
    assert_eq!(next.source_store_revision, 2);
    assert_eq!(
        reopened.reclaim(next, digest).unwrap().apply,
        GenerationalReclamationApplyV1::Reclaimed
    );
    drop(reopened);
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn resource_replacement_and_reclamation_journal_corruption_fail_closed() {
    let path = temporary_path("corrupt");
    let mut store = GenerationalDirectoryCheckpointStoreV1::create_new(&path, limits()).unwrap();
    commit(&mut store, None, b"zero");
    commit(&mut store, Some(Revision(0)), b"one");
    commit(&mut store, Some(Revision(1)), b"two");
    drop(store);
    let mut store = GenerationalDirectoryCheckpointStoreV1::open_existing(&path, limits()).unwrap();
    let plan = store.prepare_reclamation(1, 2, digest).unwrap();
    store.reclaim(plan, digest).unwrap();
    drop(store);

    let journal = path.join(".reclamation");
    let mut bytes = fs::read(&journal).unwrap();
    bytes[9] ^= 1;
    fs::write(&journal, bytes).unwrap();
    assert!(matches!(
        GenerationalDirectoryCheckpointStoreV1::open_existing(&path, limits()),
        Err(GenerationalDirectoryErrorV1::Corrupt)
    ));
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn limits_and_live_resource_drift_are_sticky_fail_closed() {
    let path = temporary_path("drift");
    assert!(matches!(
        GenerationalDirectoryCheckpointStoreV1::create_new(
            &path,
            GenerationalDirectoryLimitsV1 {
                max_live_generations: 2,
                ..limits()
            }
        ),
        Err(GenerationalDirectoryErrorV1::InvalidLimits)
    ));
    let mut store = GenerationalDirectoryCheckpointStoreV1::create_new(&path, limits()).unwrap();
    commit(&mut store, None, b"zero");
    fs::write(
        path.join("generation-00000000000000000000.bin"),
        b"replaced",
    )
    .unwrap();
    assert_eq!(
        store.load("owner"),
        Err(GenerationalDirectoryErrorV1::LostResource)
    );
    assert_eq!(
        store.load("owner"),
        Err(GenerationalDirectoryErrorV1::Fenced)
    );
    drop(store);
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn every_single_bit_reclamation_journal_mutation_fails_closed() {
    let path = temporary_path("journal-mutations");
    let mut store = GenerationalDirectoryCheckpointStoreV1::create_new(&path, limits()).unwrap();
    commit(&mut store, None, b"zero");
    commit(&mut store, Some(Revision(0)), b"one");
    commit(&mut store, Some(Revision(1)), b"two");
    drop(store);

    let mut store = GenerationalDirectoryCheckpointStoreV1::open_existing(&path, limits()).unwrap();
    let plan = store.prepare_reclamation(7, 2, digest).unwrap();
    store.reclaim(plan, digest).unwrap();
    drop(store);

    let journal = path.join(".reclamation");
    let canonical = fs::read(&journal).unwrap();
    for byte_index in 0..canonical.len() {
        for bit in 0..8 {
            let mut mutated = canonical.clone();
            mutated[byte_index] ^= 1 << bit;
            fs::write(&journal, mutated).unwrap();
            assert!(
                GenerationalDirectoryCheckpointStoreV1::open_existing(&path, limits()).is_err(),
                "accepted journal mutation at byte {byte_index}, bit {bit}"
            );
        }
    }
    fs::write(&journal, &canonical).unwrap();
    assert!(GenerationalDirectoryCheckpointStoreV1::open_existing(&path, limits()).is_ok());
    fs::remove_dir_all(path).unwrap();
}
