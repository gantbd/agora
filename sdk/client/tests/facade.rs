use agora_sdk_client::{client, protocol, wire};
use std::convert::Infallible;

struct Port;

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
