use agora_engine::retention::{
    ContentDigestProfileV1, ExportMaterializationRefusalV1, ExportProfileV1, ExportRecordV1,
    ExportRequestIdV1, ExportScopeBindingV1, RECORD_CLASSES_V1, RetentionFrontierV1,
    RetentionPolicyRevisionV1, RetentionRecordIdV1, materialize_export_v1,
};
use agora_sha256::{PROFILE_V1, digest_v1};

fn expected(hex: &str) -> [u8; 32] {
    assert_eq!(hex.len(), 64);
    let mut output = [0_u8; 32];
    for (index, destination) in output.iter_mut().enumerate() {
        *destination = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap();
    }
    output
}

#[test]
fn standard_vectors_cover_empty_single_multi_and_many_block_inputs() {
    let vectors = [
        (
            Vec::new(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            b"abc".to_vec(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq".to_vec(),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
        ),
        (
            vec![b'a'; 1_000_000],
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0",
        ),
    ];
    for (input, digest) in vectors {
        assert_eq!(digest_v1(&input).as_bytes(), expected(digest));
    }
    assert_eq!(PROFILE_V1, ContentDigestProfileV1::Sha256);
}

#[test]
fn boundary_lengths_are_distinct_and_repeatable() {
    let lengths = [55, 56, 63, 64, 65, 119, 120, 127, 128, 129];
    let digests: Vec<_> = lengths
        .map(|length| digest_v1(&vec![0x5a; length]))
        .into_iter()
        .collect();
    assert!(digests.windows(2).all(|pair| pair[0] != pair[1]));
    for length in lengths {
        assert_eq!(
            digest_v1(&vec![0x5a; length]),
            digest_v1(&vec![0x5a; length])
        );
    }
}

#[test]
fn agora_materialization_and_recovery_use_the_named_provider_exactly() {
    let included = [true, false, false, false, false, false];
    let frontiers = RECORD_CLASSES_V1.map(|class| {
        RetentionFrontierV1::new(class, 0, (class == RECORD_CLASSES_V1[0]).then_some(1)).unwrap()
    });
    let records = [ExportRecordV1::new(
        RetentionRecordIdV1::new(RECORD_CLASSES_V1[0], 1).unwrap(),
        b"authoritative export bytes".to_vec(),
        128,
    )
    .unwrap()];
    let materialization = materialize_export_v1(
        ExportRequestIdV1::new([1; 16]).unwrap(),
        ExportProfileV1::Administrative,
        ExportScopeBindingV1::new([2; 32]).unwrap(),
        PROFILE_V1,
        7,
        RetentionPolicyRevisionV1::new(3).unwrap(),
        included,
        frontiers,
        &records,
        8,
        128,
        8,
        128,
        digest_v1,
    )
    .unwrap();
    assert_eq!(
        materialization.manifest.chunks()[0].digest,
        digest_v1(materialization.chunks()[0].bytes())
    );
    assert_eq!(
        agora_engine::retention::ExportMaterializationV1::recover(
            materialization.manifest.clone(),
            materialization.chunks().to_vec(),
            8,
            128,
            digest_v1,
        ),
        Ok(materialization.clone())
    );

    let mut changed = materialization.chunks()[0].bytes().to_vec();
    *changed.last_mut().unwrap() ^= 1;
    let changed = agora_engine::retention::ExportChunkPayloadV1::new(0, changed, 128).unwrap();
    assert_eq!(
        agora_engine::retention::ExportMaterializationV1::recover(
            materialization.manifest,
            vec![changed],
            8,
            128,
            digest_v1,
        ),
        Err(ExportMaterializationRefusalV1::DigestMismatch)
    );
}

#[test]
fn diagnostics_do_not_disclose_digest_bytes() {
    let digest = digest_v1(b"sensitive export payload");
    let rendered = format!("{digest:?}");
    assert_eq!(rendered, "ContentDigestV1(<digest>)");
    assert!(!rendered.contains("sensitive"));
}
