use agora_sdk_client::{client, protocol, wire};
use std::convert::Infallible;

struct Port;

struct PortV2;

impl client::ServicePort for Port {
    type Error = Infallible;

    fn execute(
        &mut self,
        _client: protocol::ClientIdV1,
        _request: protocol::RequestV1,
    ) -> Result<protocol::ReplyV1, Self::Error> {
        Ok(protocol::ReplyV1::Refused(protocol::RefusalV1::UnknownRoom))
    }
}

impl client::v2::ServicePortV2 for PortV2 {
    type Error = Infallible;

    fn execute(
        &mut self,
        _client: protocol::v2::ClientIdV2,
        _request: protocol::v2::RequestV2,
    ) -> Result<protocol::v2::ReplyV2, client::v2::PortFailureV2<Self::Error>> {
        Ok(protocol::v2::ReplyV2::Refused(
            protocol::v2::RefusalV2::UnknownRoom,
        ))
    }
}

#[test]
fn facade_alone_constructs_protocol_client_and_wire_values() {
    let mut client = client::Client::new(protocol::ClientIdV1(7), Port);
    let reply = client
        .execute(protocol::CommandV1::Snapshot {
            room_id: "room".into(),
        })
        .unwrap();
    assert_eq!(
        reply,
        protocol::ReplyV1::Refused(protocol::RefusalV1::UnknownRoom)
    );

    let request = wire::WireRequestV1::Connect {
        participant_id: "participant".into(),
    };
    let limits = wire::WireLimits::reference();
    let encoded = wire::encode_request_v1(&request, limits).unwrap();
    assert_eq!(wire::decode_request_v1(&encoded, limits).unwrap(), request);
}

#[test]
fn facade_exposes_the_canonical_local_handshake_without_transport_policy() {
    use wire::local_handshake::{
        ServerIncarnationV1, decode_client_hello_v1, decode_server_hello_v1,
        encode_client_hello_v1, encode_server_hello_v1,
    };

    let client = encode_client_hello_v1();
    assert_eq!(decode_client_hello_v1(&client), Ok(()));

    let incarnation = ServerIncarnationV1::new([0x5a; 16]);
    let server = encode_server_hello_v1(incarnation, 8_192, 1_024).unwrap();
    let hello = decode_server_hello_v1(&server).unwrap();
    assert_eq!(hello.server_incarnation(), incarnation);
    assert_eq!(hello.max_frame_bytes(), 8_192);
    assert_eq!(hello.max_frames_after_connect(), 1_024);
    assert!(hello.supports_capability_discovery());
}

#[test]
fn facade_alone_constructs_v2_client_and_canonical_wire_values() {
    let mut client = client::v2::ClientV2::new(protocol::v2::ClientIdV2(7), PortV2);
    let reply = client
        .execute(protocol::v2::CommandV2::Rooms {
            cursor: None,
            limit: 2,
        })
        .unwrap();
    assert_eq!(
        reply,
        protocol::v2::ReplyV2::Refused(protocol::v2::RefusalV2::UnknownRoom)
    );

    let request = wire::v2::WireRequestV2::Connect {
        participant_id: "participant".into(),
    };
    let limits = wire::WireLimits::reference();
    let encoded = wire::v2::encode_request_v2(&request, limits).unwrap();
    assert_eq!(
        wire::v2::decode_request_v2(&encoded, limits).unwrap(),
        request
    );

    let identified = wire::v2::WireRequestV2::Execute {
        client: protocol::v2::ClientIdV2(7),
        request: protocol::v2::RequestV2 {
            request_id: protocol::v2::RequestIdV2(9),
            command: protocol::v2::CommandV2::IdentifiedMessages {
                room_id: "room".to_owned(),
                cursor: None,
                limit: 2,
            },
        },
    };
    let encoded = wire::v2::encode_request_v2(&identified, limits).unwrap();
    assert_eq!(
        wire::v2::decode_request_v2(&encoded, limits).unwrap(),
        identified
    );

    let withdrawal = wire::v2::WireRequestV2::Execute {
        client: protocol::v2::ClientIdV2(7),
        request: protocol::v2::RequestV2 {
            request_id: protocol::v2::RequestIdV2(10),
            command: protocol::v2::CommandV2::WithdrawMessage {
                room_id: "room".to_owned(),
                expected_revision: 9,
                message_id: protocol::v2::MessageIdV1::new([0x44; 16]),
            },
        },
    };
    let encoded = wire::v2::encode_request_v2(&withdrawal, limits).unwrap();
    assert_eq!(
        wire::v2::decode_request_v2(&encoded, limits).unwrap(),
        withdrawal
    );

    let redaction = wire::v2::WireRequestV2::Execute {
        client: protocol::v2::ClientIdV2(7),
        request: protocol::v2::RequestV2 {
            request_id: protocol::v2::RequestIdV2(11),
            command: protocol::v2::CommandV2::RedactMessage {
                room_id: "room".to_owned(),
                expected_revision: 10,
                message_id: protocol::v2::MessageIdV1::new([0x45; 16]),
                reason: "policy".to_owned(),
                causation: [0x46; 16],
            },
        },
    };
    let encoded = wire::v2::encode_request_v2(&redaction, limits).unwrap();
    assert_eq!(
        wire::v2::decode_request_v2(&encoded, limits).unwrap(),
        redaction
    );

    for command in [
        protocol::v2::CommandV2::OpenIdentifiedChanges {
            room_id: "room".to_owned(),
            observed_revision: 10,
        },
        protocol::v2::CommandV2::IdentifiedChanges {
            room_id: "room".to_owned(),
            cursor: protocol::v2::PageCursorV2(12),
            limit: 2,
        },
        protocol::v2::CommandV2::SpeakReply {
            room_id: "room".to_owned(),
            expected_revision: 10,
            root_message_id: protocol::v2::MessageIdV1::new([0x50; 16]),
            logical_time: 11,
            body: "reply".to_owned(),
            addressed_to: vec!["participant".to_owned()],
        },
        protocol::v2::CommandV2::ThreadReplies {
            room_id: "room".to_owned(),
            root_message_id: protocol::v2::MessageIdV1::new([0x50; 16]),
            cursor: Some(protocol::v2::PageCursorV2(13)),
            limit: 2,
        },
        protocol::v2::CommandV2::OpenThreadedChanges {
            room_id: "room".to_owned(),
            observed_revision: 10,
        },
        protocol::v2::CommandV2::ThreadedChanges {
            room_id: "room".to_owned(),
            cursor: protocol::v2::PageCursorV2(14),
            limit: 2,
        },
        protocol::v2::CommandV2::ArchiveThread {
            room_id: "room".to_owned(),
            expected_revision: 15,
            root_message_id: protocol::v2::MessageIdV1::new([0x50; 16]),
            reason: "pause".to_owned(),
            causation: [0x51; 16],
        },
        protocol::v2::CommandV2::ReopenThread {
            room_id: "room".to_owned(),
            expected_revision: 16,
            root_message_id: protocol::v2::MessageIdV1::new([0x50; 16]),
            reason: "resume".to_owned(),
            causation: [0x52; 16],
        },
        protocol::v2::CommandV2::ThreadStatus {
            room_id: "room".to_owned(),
            root_message_id: protocol::v2::MessageIdV1::new([0x50; 16]),
        },
        protocol::v2::CommandV2::AddReaction {
            room_id: "room".to_owned(),
            expected_revision: 17,
            message_id: protocol::v2::MessageIdV1::new([0x53; 16]),
            key: "spark".to_owned(),
        },
        protocol::v2::CommandV2::RemoveReaction {
            room_id: "room".to_owned(),
            expected_revision: 18,
            message_id: protocol::v2::MessageIdV1::new([0x53; 16]),
            key: "spark".to_owned(),
        },
        protocol::v2::CommandV2::ReactionStatus {
            room_id: "room".to_owned(),
            message_id: protocol::v2::MessageIdV1::new([0x53; 16]),
        },
        protocol::v2::CommandV2::EditMessage {
            room_id: "room".to_owned(),
            expected_revision: 19,
            message_id: protocol::v2::MessageIdV1::new([0x54; 16]),
            logical_time: 20,
            body: "revised".to_owned(),
        },
        protocol::v2::CommandV2::MessageRevisions {
            room_id: "room".to_owned(),
            message_id: protocol::v2::MessageIdV1::new([0x54; 16]),
            cursor: Some(protocol::v2::PageCursorV2(20)),
            limit: 2,
        },
        protocol::v2::CommandV2::CreateTopic {
            room_id: "room".to_owned(),
            expected_revision: 20,
            name: "incidents".to_owned(),
        },
        protocol::v2::CommandV2::SpeakInTopic {
            room_id: "room".to_owned(),
            expected_revision: 21,
            topic_id: protocol::v2::TopicIdV1::new([0x55; 16]),
            logical_time: 22,
            body: "evidence".to_owned(),
            addressed_to: vec!["participant".to_owned()],
        },
        protocol::v2::CommandV2::Topics {
            room_id: "room".to_owned(),
            cursor: Some(protocol::v2::PageCursorV2(21)),
            limit: 2,
        },
        protocol::v2::CommandV2::TopicMessages {
            room_id: "room".to_owned(),
            topic_id: protocol::v2::TopicIdV1::new([0x55; 16]),
            cursor: Some(protocol::v2::PageCursorV2(22)),
            limit: 2,
        },
        protocol::v2::CommandV2::OpenTopicChanges {
            room_id: "room".to_owned(),
            observed_revision: 22,
        },
        protocol::v2::CommandV2::TopicChanges {
            room_id: "room".to_owned(),
            cursor: protocol::v2::PageCursorV2(23),
            limit: 2,
        },
        protocol::v2::CommandV2::ReadPosition {
            room_id: "room".to_owned(),
            scope: protocol::v2::ReadScopeV1::Thread(protocol::v2::MessageIdV1::new([0x56; 16])),
        },
        protocol::v2::CommandV2::AdvanceReadPosition {
            room_id: "room".to_owned(),
            observed_revision: 23,
            scope: protocol::v2::ReadScopeV1::Topic(protocol::v2::TopicIdV1::new([0x57; 16])),
            target_message_id: protocol::v2::MessageIdV1::new([0x58; 16]),
        },
    ] {
        let request = wire::v2::WireRequestV2::Execute {
            client: protocol::v2::ClientIdV2(7),
            request: protocol::v2::RequestV2 {
                request_id: protocol::v2::RequestIdV2(12),
                command,
            },
        };
        let encoded = wire::v2::encode_request_v2(&request, limits).unwrap();
        assert_eq!(
            wire::v2::decode_request_v2(&encoded, limits).unwrap(),
            request
        );
    }

    let gap = wire::v2::WireReplyV2::Executed(protocol::v2::ReplyV2::Accepted(
        protocol::v2::OutcomeV2::ChangeGap {
            current_revision: 24,
            cause: protocol::v2::ChangeGapV2::ChangeRetention {
                discarded_through: 20,
            },
        },
    ));
    let encoded = wire::v2::encode_reply_v2(&gap, limits).unwrap();
    assert_eq!(wire::v2::decode_reply_v2(&encoded, limits).unwrap(), gap);
}
