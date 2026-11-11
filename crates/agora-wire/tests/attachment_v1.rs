use agora_protocol::v2::{
    AttachmentDescriptorV1, AttachmentIdV1, AttachmentIntegrityV1, AttachmentMessagePageV1,
    AttachmentMessageTargetV1, AttachmentMessageV1, AttachmentObjectKeyV1, AttachmentStoreIdV1,
    ClientIdV2, CommandV2, MessageAttachmentV1, MessageIdV1, OutcomeV2, ReplyV2, RequestIdV2,
    RequestV2, Sha256DigestV1,
};
use agora_wire::v2::{
    WireReplyV2, WireRequestV2, decode_reply_v2, decode_request_v2, encode_reply_v2,
    encode_request_v2,
};
use agora_wire::{WireDecodeError, WireEncodeError, WireLimits};

fn descriptor() -> AttachmentDescriptorV1 {
    AttachmentDescriptorV1 {
        attachment_id: AttachmentIdV1::new([1; 16]),
        store_id: AttachmentStoreIdV1::new([2; 16]),
        object_key: AttachmentObjectKeyV1::new([3; 32]),
        declared_bytes: 42,
        media_type: "text/plain".to_owned(),
        display_label: "report.txt".to_owned(),
        integrity: AttachmentIntegrityV1::Sha256(Sha256DigestV1::new([4; 32])),
    }
}

#[test]
fn attachment_command_and_history_are_canonical_closed_and_content_safe() {
    let limits = WireLimits::reference();
    let command = WireRequestV2::Execute {
        client: ClientIdV2(7),
        request: RequestV2 {
            request_id: RequestIdV2(8),
            command: CommandV2::SpeakWithAttachments {
                room_id: "room".to_owned(),
                expected_revision: 9,
                target: AttachmentMessageTargetV1::Room,
                logical_time: 10,
                body: "body".to_owned(),
                addressed_to: vec!["alice".to_owned()],
                attachments: vec![descriptor()],
            },
        },
    };
    let bytes = encode_request_v2(&command, limits).unwrap();
    assert_eq!(decode_request_v2(&bytes, limits).unwrap(), command);
    assert!(!format!("{command:?}").contains("report.txt"));

    let relation = MessageAttachmentV1 {
        attachment_id: AttachmentIdV1::new([1; 16]),
        descriptor: Some(descriptor()),
    };
    let reply = WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::AttachmentMessages(
        AttachmentMessagePageV1 {
            revision: 10,
            room_id: "room".to_owned(),
            messages: vec![AttachmentMessageV1 {
                message_id: MessageIdV1::new([5; 16]),
                sequence: 0,
                author: "alice".to_owned(),
                logical_time: 10,
                body: "body".to_owned(),
                addressed_to: vec![],
                reply_to_root: None,
                topic_id: None,
                attachments: vec![relation],
            }],
            oldest_retained_sequence: 0,
            next_message_sequence: 1,
            next: None,
        },
    )));
    let bytes = encode_reply_v2(&reply, limits).unwrap();
    assert_eq!(decode_reply_v2(&bytes, limits).unwrap(), reply);
}

#[test]
fn attachment_wire_rejects_noncanonical_descriptor_shapes() {
    let limits = WireLimits::reference();
    let mut invalid = descriptor();
    invalid.media_type = "Text/Plain".to_owned();
    let request = WireRequestV2::Execute {
        client: ClientIdV2(1),
        request: RequestV2 {
            request_id: RequestIdV2(1),
            command: CommandV2::SpeakWithAttachments {
                room_id: "room".to_owned(),
                expected_revision: 0,
                target: AttachmentMessageTargetV1::Room,
                logical_time: 1,
                body: "body".to_owned(),
                addressed_to: vec![],
                attachments: vec![invalid],
            },
        },
    };
    assert_eq!(
        encode_request_v2(&request, limits),
        Err(WireEncodeError::Invalid)
    );

    let mut canonical = descriptor();
    canonical.media_type = "text/plain".to_owned();
    let request = WireRequestV2::Execute {
        client: ClientIdV2(1),
        request: RequestV2 {
            request_id: RequestIdV2(1),
            command: CommandV2::SpeakWithAttachments {
                room_id: "room".to_owned(),
                expected_revision: 0,
                target: AttachmentMessageTargetV1::Room,
                logical_time: 1,
                body: "body".to_owned(),
                addressed_to: vec![],
                attachments: vec![canonical],
            },
        },
    };
    let mut bytes = encode_request_v2(&request, limits).unwrap();
    let offset = bytes
        .windows(10)
        .position(|window| window == b"text/plain")
        .unwrap();
    bytes[offset] = b'T';
    assert_eq!(
        decode_request_v2(&bytes, limits),
        Err(WireDecodeError::Malformed)
    );
}
