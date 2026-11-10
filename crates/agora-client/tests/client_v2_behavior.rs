use agora_client::v2::{ClientErrorV2, ClientV2, PortFailureV2, ServicePortV2};
use agora_protocol::v2::{ClientIdV2, CommandV2, RefusalV2, ReplyV2, RequestIdV2, RequestV2};
use std::collections::VecDeque;

#[derive(Default)]
struct RecordingPortV2 {
    requests: Vec<(ClientIdV2, RequestV2)>,
    replies: VecDeque<Result<ReplyV2, PortFailureV2<&'static str>>>,
}

impl ServicePortV2 for RecordingPortV2 {
    type Error = &'static str;

    fn execute(
        &mut self,
        client: ClientIdV2,
        request: RequestV2,
    ) -> Result<ReplyV2, PortFailureV2<Self::Error>> {
        self.requests.push((client, request));
        self.replies
            .pop_front()
            .unwrap_or(Ok(ReplyV2::Refused(RefusalV2::Unavailable)))
    }
}

fn rooms(limit: u32) -> CommandV2 {
    CommandV2::Rooms {
        cursor: None,
        limit,
    }
}

#[test]
fn v2_client_owns_an_independent_monotone_request_namespace() {
    let mut client = ClientV2::new(ClientIdV2(7), RecordingPortV2::default());
    for limit in [1, 2] {
        client.execute(rooms(limit)).unwrap();
    }
    let (port, pending) = client.into_parts();
    assert!(pending.is_none());
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

#[test]
fn outcome_unknown_replays_only_the_equal_command_under_the_exact_identity() {
    let port = RecordingPortV2 {
        replies: [
            Err(PortFailureV2::OutcomeUnknown("lost")),
            Ok(ReplyV2::Refused(RefusalV2::Unavailable)),
        ]
        .into(),
        ..Default::default()
    };
    let mut client = ClientV2::new(ClientIdV2(7), port);
    assert_eq!(
        client.execute(rooms(2)),
        Err(ClientErrorV2::OutcomeUnknown("lost"))
    );
    assert_eq!(client.pending_request().unwrap().request_id, RequestIdV2(0));
    assert_eq!(
        client.execute(rooms(3)),
        Err(ClientErrorV2::UnresolvedOutcome)
    );
    assert_eq!(
        client.execute(rooms(2)),
        Ok(ReplyV2::Refused(RefusalV2::Unavailable))
    );
    let (port, pending) = client.into_parts();
    assert!(pending.is_none());
    assert_eq!(port.requests.len(), 2);
    assert_eq!(port.requests[0], port.requests[1]);
}

#[test]
fn not_sent_does_not_create_or_clear_uncertainty() {
    let port = RecordingPortV2 {
        replies: [
            Err(PortFailureV2::NotSent("offline")),
            Err(PortFailureV2::OutcomeUnknown("lost")),
            Err(PortFailureV2::NotSent("offline")),
        ]
        .into(),
        ..Default::default()
    };
    let mut client = ClientV2::new(ClientIdV2(7), port);
    assert_eq!(
        client.execute(rooms(2)),
        Err(ClientErrorV2::NotSent("offline"))
    );
    assert!(client.pending_request().is_none());
    assert_eq!(
        client.execute(rooms(2)),
        Err(ClientErrorV2::OutcomeUnknown("lost"))
    );
    assert_eq!(
        client.execute(rooms(2)),
        Err(ClientErrorV2::NotSent("offline"))
    );
    assert_eq!(client.pending_request().unwrap().request_id, RequestIdV2(1));
}

#[test]
fn abandonment_is_explicit_and_returns_the_unresolved_request() {
    let port = RecordingPortV2 {
        replies: [Err(PortFailureV2::OutcomeUnknown("lost"))].into(),
        ..Default::default()
    };
    let mut client = ClientV2::new(ClientIdV2(7), port);
    let _ = client.execute(rooms(2));
    assert_eq!(
        client.abandon_uncertainty().unwrap().request_id,
        RequestIdV2(0)
    );
    assert!(client.pending_request().is_none());
}
