use agora_protocol::v2::{
    CapabilityV2, ClientIdV2, CommandV2, MessageIdV1, MessageVellumArtifactReferenceV1, OutcomeV2,
    ReplyV2, RequestIdV2, RequestV2, VellumArtifactMessagePageV1, VellumArtifactMessageTargetV1,
    VellumArtifactMessageV1, VellumArtifactReferenceIdV1, VellumArtifactReferenceV1,
    VellumDocumentTargetV1,
};
use agora_wire::v2::{
    WireReplyV2, WireRequestV2, decode_reply_v2, decode_request_v2, encode_reply_v2,
    encode_request_v2,
};
use agora_wire::{WireDecodeError, WireEncodeError, WireLimits};

fn reference(id: u8, document: u64) -> VellumArtifactReferenceV1 {
    VellumArtifactReferenceV1 {
        reference_id: VellumArtifactReferenceIdV1::new([id; 16]),
        target: VellumDocumentTargetV1 {
            workspace_id: 7,
            document_id: document,
            source_revision: 11,
        },
    }
}

fn fixture(source: &str) -> Vec<u8> {
    let compact = source.split_whitespace().collect::<String>();
    (0..compact.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&compact[index..index + 2], 16).unwrap())
        .collect()
}

#[test]
fn vellum_artifact_admission_and_history_round_trip_without_disclosing_targets() {
    let limits = WireLimits::reference();
    let request = WireRequestV2::Execute {
        client: ClientIdV2(3),
        request: RequestV2 {
            request_id: RequestIdV2(5),
            command: CommandV2::SpeakWithVellumArtifacts {
                room_id: "room".to_owned(),
                expected_revision: 8,
                target: VellumArtifactMessageTargetV1::Room,
                logical_time: 13,
                body: "discuss".to_owned(),
                addressed_to: vec!["agent".to_owned()],
                references: vec![reference(1, 9), reference(2, 10)],
            },
        },
    };
    let encoded = encode_request_v2(&request, limits).unwrap();
    assert_eq!(
        encoded,
        fixture(include_str!(
            "fixtures/vellum-artifact-speak-request-v2.hex"
        ))
    );
    assert_eq!(decode_request_v2(&encoded, limits).unwrap(), request);
    let debug = format!("{request:?}");
    assert!(!debug.contains("workspace_id"));
    assert!(!debug.contains("document_id"));

    let reply = WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::VellumArtifactMessages(
        VellumArtifactMessagePageV1 {
            revision: 9,
            room_id: "room".to_owned(),
            messages: vec![VellumArtifactMessageV1 {
                message_id: MessageIdV1::new([4; 16]),
                sequence: 0,
                author: "agent".to_owned(),
                logical_time: 13,
                body: "discuss".to_owned(),
                addressed_to: vec![],
                reply_to_root: None,
                topic_id: None,
                references: vec![MessageVellumArtifactReferenceV1 {
                    reference_id: VellumArtifactReferenceIdV1::new([1; 16]),
                    target: Some(reference(1, 9).target),
                }],
            }],
            oldest_retained_sequence: 0,
            next_message_sequence: 1,
            next: None,
        },
    )));
    let encoded = encode_reply_v2(&reply, limits).unwrap();
    assert_eq!(
        encoded,
        fixture(include_str!(
            "fixtures/vellum-artifact-history-reply-v2.hex"
        ))
    );
    assert_eq!(decode_reply_v2(&encoded, limits).unwrap(), reply);
    assert!(!format!("{reply:?}").contains("document_id"));
}

#[test]
fn vellum_artifact_wire_rejects_zero_and_duplicate_targets() {
    let limits = WireLimits::reference();
    let command = |references| WireRequestV2::Execute {
        client: ClientIdV2(1),
        request: RequestV2 {
            request_id: RequestIdV2(1),
            command: CommandV2::SpeakWithVellumArtifacts {
                room_id: "room".to_owned(),
                expected_revision: 0,
                target: VellumArtifactMessageTargetV1::Room,
                logical_time: 1,
                body: "body".to_owned(),
                addressed_to: vec![],
                references,
            },
        },
    };
    let zero = VellumArtifactReferenceV1 {
        reference_id: VellumArtifactReferenceIdV1::new([1; 16]),
        target: VellumDocumentTargetV1 {
            workspace_id: 0,
            document_id: 1,
            source_revision: 0,
        },
    };
    assert_eq!(
        encode_request_v2(&command(vec![zero]), limits),
        Err(WireEncodeError::Invalid)
    );
    assert_eq!(
        encode_request_v2(&command(vec![reference(1, 9), reference(2, 9)]), limits),
        Err(WireEncodeError::Invalid)
    );

    let mut encoded = encode_request_v2(&command(vec![reference(1, 9)]), limits).unwrap();
    let workspace = 7_u64.to_be_bytes();
    let offset = encoded
        .windows(workspace.len())
        .rposition(|window| window == workspace)
        .unwrap();
    encoded[offset..offset + workspace.len()].fill(0);
    assert_eq!(
        decode_request_v2(&encoded, limits),
        Err(WireDecodeError::Malformed)
    );
}

#[test]
fn vellum_artifact_capability_and_acceptance_are_closed_and_bounded() {
    let limits = WireLimits::reference();
    for outcome in [
        OutcomeV2::Capabilities(vec![CapabilityV2::VellumArtifactReferencesV1]),
        OutcomeV2::VellumArtifactMessageAccepted {
            revision: 3,
            message_id: MessageIdV1::new([8; 16]),
            sequence: 2,
            reference_count: 1,
        },
    ] {
        let reply = WireReplyV2::Executed(ReplyV2::Accepted(outcome));
        let encoded = encode_reply_v2(&reply, limits).unwrap();
        assert_eq!(decode_reply_v2(&encoded, limits).unwrap(), reply);
    }
    let invalid = WireReplyV2::Executed(ReplyV2::Accepted(
        OutcomeV2::VellumArtifactMessageAccepted {
            revision: 3,
            message_id: MessageIdV1::new([8; 16]),
            sequence: 2,
            reference_count: 0,
        },
    ));
    assert_eq!(
        encode_reply_v2(&invalid, limits),
        Err(WireEncodeError::Invalid)
    );
}
