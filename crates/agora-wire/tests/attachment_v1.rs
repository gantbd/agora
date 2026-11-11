use agora_protocol::v2::{
    AttachmentDescriptorV1, AttachmentIdV1, AttachmentIntegrityV1, AttachmentMessagePageV1,
    AttachmentMessageTargetV1, AttachmentMessageV1, AttachmentObjectKeyV1,
    AttachmentRoomChangePageV1, AttachmentStoreIdV1, CapabilityV2, ClientIdV2, CommandV2,
    IdentifiedMessageV1, IdentifiedRoomChangePageV1, IdentifiedRoomChangeV1, IdentifiedRoomEventV1,
    MessageAttachmentV1, MessageIdV1, OutcomeV2, PageCursorV2, ReplyV2, RequestIdV2, RequestV2,
    Sha256DigestV1,
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

fn decode_hex(source: &str) -> Vec<u8> {
    let compact = source.split_whitespace().collect::<String>();
    (0..compact.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&compact[index..index + 2], 16).unwrap())
        .collect()
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
    assert_eq!(
        bytes,
        decode_hex(include_str!("fixtures/attachment-speak-request-v2.hex"))
    );
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
    assert_eq!(
        bytes,
        decode_hex(include_str!("fixtures/attachment-history-reply-v2.hex"))
    );
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

#[test]
fn attachment_change_profile_and_capability_round_trip_canonically() {
    let limits = WireLimits::reference();
    for command in [
        CommandV2::OpenAttachmentChanges {
            room_id: "room".to_owned(),
            observed_revision: 4,
        },
        CommandV2::AttachmentChanges {
            room_id: "room".to_owned(),
            cursor: PageCursorV2(7),
            limit: 8,
        },
    ] {
        let request = WireRequestV2::Execute {
            client: ClientIdV2(1),
            request: RequestV2 {
                request_id: RequestIdV2(2),
                command,
            },
        };
        let encoded = encode_request_v2(&request, limits).unwrap();
        assert_eq!(decode_request_v2(&encoded, limits).unwrap(), request);
    }

    let identified_message = IdentifiedMessageV1 {
        message_id: MessageIdV1::new([5; 16]),
        sequence: 0,
        author: "agent".to_owned(),
        logical_time: 6,
        body: "body".to_owned(),
        addressed_to: vec![],
    };
    let attachment_message = AttachmentMessageV1 {
        message_id: identified_message.message_id,
        sequence: identified_message.sequence,
        author: identified_message.author.clone(),
        logical_time: identified_message.logical_time,
        body: identified_message.body.clone(),
        addressed_to: vec![],
        reply_to_root: None,
        topic_id: None,
        attachments: vec![MessageAttachmentV1 {
            attachment_id: AttachmentIdV1::new([1; 16]),
            descriptor: Some(descriptor()),
        }],
    };
    let reply = WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::AttachmentChanges(
        AttachmentRoomChangePageV1 {
            identified: IdentifiedRoomChangePageV1 {
                revision: 7,
                changes: vec![IdentifiedRoomChangeV1 {
                    event_sequence: 1,
                    revision: 7,
                    event: IdentifiedRoomEventV1::MessageAccepted(identified_message),
                }],
                next: PageCursorV2(8),
                has_more: false,
            },
            accepted_messages: vec![attachment_message],
        },
    )));
    let encoded = encode_reply_v2(&reply, limits).unwrap();
    assert_eq!(decode_reply_v2(&encoded, limits).unwrap(), reply);

    let capabilities = WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::Capabilities(vec![
        CapabilityV2::ImmutableAttachmentReferencesV1,
    ])));
    let encoded = encode_reply_v2(&capabilities, limits).unwrap();
    assert_eq!(decode_reply_v2(&encoded, limits).unwrap(), capabilities);
}
