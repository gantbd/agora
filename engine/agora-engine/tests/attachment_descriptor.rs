use agora_engine::{
    AttachmentDescriptorLimitsV1, AttachmentDescriptorRefusalV1, AttachmentDescriptorV1,
    AttachmentIdV1, AttachmentIntegrityV1, AttachmentObjectKeyV1, AttachmentStoreIdV1,
    Sha256DigestV1,
};

fn descriptor(
    declared_bytes: u64,
    media_type: &str,
    display_label: &str,
) -> AttachmentDescriptorV1 {
    AttachmentDescriptorV1::new(
        AttachmentIdV1::new([1; 16]),
        AttachmentStoreIdV1::new([2; 16]),
        AttachmentObjectKeyV1::new([3; 32]),
        declared_bytes,
        media_type,
        display_label,
        AttachmentIntegrityV1::Sha256(Sha256DigestV1::new([4; 32])),
    )
}

#[test]
fn bounded_descriptor_validation_is_closed_and_canonical() {
    let limits = AttachmentDescriptorLimitsV1 {
        max_declared_bytes: 8,
        max_media_type_bytes: 24,
        max_display_label_bytes: 8,
    };
    assert_eq!(
        descriptor(8, "text/plain", "notes.md").validate(limits),
        Ok(())
    );

    for (candidate, refusal) in [
        (
            descriptor(0, "text/plain", "notes.md"),
            AttachmentDescriptorRefusalV1::InvalidDeclaredLength,
        ),
        (
            descriptor(9, "text/plain", "notes.md"),
            AttachmentDescriptorRefusalV1::InvalidDeclaredLength,
        ),
        (
            descriptor(1, "Text/plain", "notes.md"),
            AttachmentDescriptorRefusalV1::InvalidMediaType,
        ),
        (
            descriptor(1, "text/plain; charset=utf-8", "notes.md"),
            AttachmentDescriptorRefusalV1::InvalidMediaType,
        ),
        (
            descriptor(1, "text//plain", "notes.md"),
            AttachmentDescriptorRefusalV1::InvalidMediaType,
        ),
        (
            descriptor(1, "text", "notes.md"),
            AttachmentDescriptorRefusalV1::InvalidMediaType,
        ),
        (
            descriptor(1, "text/plain", ""),
            AttachmentDescriptorRefusalV1::InvalidDisplayLabel,
        ),
        (
            descriptor(1, "text/plain", "too-long!"),
            AttachmentDescriptorRefusalV1::InvalidDisplayLabel,
        ),
        (
            descriptor(1, "text/plain", "bad\nname"),
            AttachmentDescriptorRefusalV1::InvalidDisplayLabel,
        ),
    ] {
        assert_eq!(candidate.validate(limits), Err(refusal));
    }
}

#[test]
fn descriptor_values_round_trip_without_becoming_authority() {
    let value = descriptor(7, "application/octet-stream", "payload");
    assert_eq!(value.attachment_id().into_bytes(), [1; 16]);
    assert_eq!(value.store_id().into_bytes(), [2; 16]);
    assert_eq!(value.object_key().into_bytes(), [3; 32]);
    assert_eq!(value.declared_bytes(), 7);
    assert_eq!(value.media_type(), "application/octet-stream");
    assert_eq!(value.display_label(), "payload");
    assert_eq!(
        value.integrity(),
        AttachmentIntegrityV1::Sha256(Sha256DigestV1::new([4; 32]))
    );
}

#[test]
fn descriptor_diagnostics_redact_every_external_reference_and_label() {
    let value = AttachmentDescriptorV1::new(
        AttachmentIdV1::new(*b"attach-secret-12"),
        AttachmentStoreIdV1::new(*b"store-secret-123"),
        AttachmentObjectKeyV1::new(*b"object-secret-123456789012345678"),
        7,
        "application/secret",
        "private-label",
        AttachmentIntegrityV1::Sha256(Sha256DigestV1::new(*b"digest-secret-123456789012345678")),
    );
    let debug = format!("{value:?}");
    for secret in [
        "attach-secret-12",
        "store-secret-123",
        "object-secret-123456789012345678",
        "application/secret",
        "private-label",
        "digest-secret-123456789012345678",
    ] {
        assert!(!debug.contains(secret));
    }
    assert!(debug.contains("declared_bytes: 7"));
    assert!(debug.contains("media_type_bytes: 18"));
    assert!(debug.contains("display_label_bytes: 13"));
}
