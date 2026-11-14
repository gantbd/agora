use agora_client::{Client, ServicePort};
use agora_protocol::{ClientIdV1, CommandV1, RefusalV1, ReplyV1, RequestIdV1, RequestV1};

#[derive(Default)]
struct RecordingPort {
    requests: Vec<(ClientIdV1, RequestV1)>,
}

impl ServicePort for RecordingPort {
    type Error = ();

    fn execute(&mut self, client: ClientIdV1, request: RequestV1) -> Result<ReplyV1, Self::Error> {
        self.requests.push((client, request));
        Ok(ReplyV1::Refused(RefusalV1::Unavailable))
    }
}

#[test]
fn client_owns_a_monotone_request_namespace() {
    let mut client = Client::new(ClientIdV1(7), RecordingPort::default());
    for room in ["one", "two"] {
        client
            .execute(CommandV1::Snapshot {
                room_id: room.to_owned(),
            })
            .unwrap();
    }
    let port = client.into_port();
    assert_eq!(
        port.requests
            .iter()
            .map(|(client, request)| (*client, request.request_id))
            .collect::<Vec<_>>(),
        [
            (ClientIdV1(7), RequestIdV1(0)),
            (ClientIdV1(7), RequestIdV1(1)),
        ]
    );
}
