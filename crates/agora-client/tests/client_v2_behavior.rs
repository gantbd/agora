use agora_client::v2::{ClientV2, ServicePortV2};
use agora_protocol::v2::{ClientIdV2, CommandV2, RefusalV2, ReplyV2, RequestIdV2, RequestV2};

#[derive(Default)]
struct RecordingPortV2 {
    requests: Vec<(ClientIdV2, RequestV2)>,
}

impl ServicePortV2 for RecordingPortV2 {
    type Error = ();

    fn execute(&mut self, client: ClientIdV2, request: RequestV2) -> Result<ReplyV2, Self::Error> {
        self.requests.push((client, request));
        Ok(ReplyV2::Refused(RefusalV2::Unavailable))
    }
}

#[test]
fn v2_client_owns_an_independent_monotone_request_namespace() {
    let mut client = ClientV2::new(ClientIdV2(7), RecordingPortV2::default());
    for limit in [1, 2] {
        client
            .execute(CommandV2::Rooms {
                cursor: None,
                limit,
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
            (ClientIdV2(7), RequestIdV2(0)),
            (ClientIdV2(7), RequestIdV2(1)),
        ]
    );
}
