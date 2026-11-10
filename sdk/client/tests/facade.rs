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
    ) -> Result<protocol::v2::ReplyV2, Self::Error> {
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
}
