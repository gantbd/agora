use agora_client::search::{
    SearchClientErrorV1, SearchClientRestoreErrorV1, SearchClientStateV1, SearchClientV1,
    SearchPortFailureV1, SearchPortV1, SearchReplayEpochV1,
};
use agora_protocol::search::{
    SearchCommandV1, SearchMatchProfileV1, SearchOfferV1, SearchRefusalV1, SearchReplyV1,
    SearchRequestIdV1, SearchRequestV1,
};
use agora_protocol::v2::ClientIdV2;
use std::collections::VecDeque;

#[derive(Default)]
struct Port {
    replies: VecDeque<Result<SearchReplyV1, SearchPortFailureV1<&'static str>>>,
    offers: Vec<SearchOfferV1>,
}

impl SearchPortV1 for Port {
    type Error = &'static str;

    fn execute(
        &mut self,
        offer: SearchOfferV1,
    ) -> Result<SearchReplyV1, SearchPortFailureV1<Self::Error>> {
        self.offers.push(offer);
        self.replies.pop_front().unwrap()
    }
}

fn epoch(byte: u8) -> SearchReplayEpochV1 {
    SearchReplayEpochV1::new([byte; 32]).unwrap()
}

fn command(limit: u32) -> SearchCommandV1 {
    SearchCommandV1 {
        room_id: "secret-room".to_owned(),
        profile: SearchMatchProfileV1::ExactUtf8Substring,
        term: "secret-term".to_owned(),
        cursor: None,
        limit,
    }
}

fn refused() -> SearchReplyV1 {
    SearchReplyV1::Refused(SearchRefusalV1::NotMember)
}

#[test]
fn outcome_unknown_replays_only_the_exact_client_request_and_command() {
    let port = Port {
        replies: [
            Err(SearchPortFailureV1::OutcomeUnknown("lost")),
            Ok(refused()),
        ]
        .into(),
        ..Port::default()
    };
    let mut client = SearchClientV1::new(epoch(1), ClientIdV2(7), port);
    assert_eq!(
        client.execute(command(2)),
        Err(SearchClientErrorV1::OutcomeUnknown("lost"))
    );
    assert_eq!(
        client.execute(command(3)),
        Err(SearchClientErrorV1::UnresolvedOutcome)
    );
    assert_eq!(client.execute(command(2)), Ok(refused()));
    let (port, state) = client.into_state();
    assert_eq!(port.offers[0], port.offers[1]);
    assert_eq!(port.offers[0].client, ClientIdV2(7));
    assert_eq!(port.offers[0].request.request_id, SearchRequestIdV1(0));
    assert!(state.pending.is_none());
    assert_eq!(state.next_request, 1);
}

#[test]
fn definitely_not_sent_consumes_identity_without_creating_uncertainty() {
    let port = Port {
        replies: [Err(SearchPortFailureV1::NotSent("closed")), Ok(refused())].into(),
        ..Port::default()
    };
    let mut client = SearchClientV1::new(epoch(1), ClientIdV2(7), port);
    assert_eq!(
        client.execute(command(2)),
        Err(SearchClientErrorV1::NotSent("closed"))
    );
    assert!(client.pending_request().is_none());
    assert_eq!(client.execute(command(3)), Ok(refused()));
    let (port, _) = client.into_state();
    assert_eq!(port.offers[0].request.request_id, SearchRequestIdV1(0));
    assert_eq!(port.offers[1].request.request_id, SearchRequestIdV1(1));
}

#[test]
fn restoration_requires_exact_epoch_client_and_allocated_tail() {
    let pending = SearchRequestV1 {
        request_id: SearchRequestIdV1(4),
        command: command(2),
    };
    let state = SearchClientStateV1 {
        replay_epoch: epoch(1),
        client: ClientIdV2(7),
        next_request: 5,
        pending: Some(pending.clone()),
    };
    let restored =
        SearchClientV1::restore(epoch(1), ClientIdV2(7), state.clone(), Port::default()).unwrap();
    assert_eq!(restored.pending_request(), Some(&pending));
    assert!(matches!(
        SearchClientV1::restore(epoch(2), ClientIdV2(7), state.clone(), Port::default()),
        Err(SearchClientRestoreErrorV1::ReplayEpochChanged)
    ));
    assert!(matches!(
        SearchClientV1::restore(epoch(1), ClientIdV2(8), state.clone(), Port::default()),
        Err(SearchClientRestoreErrorV1::ClientChanged)
    ));
    let invalid = SearchClientStateV1 {
        next_request: 6,
        ..state
    };
    assert!(matches!(
        SearchClientV1::restore(epoch(1), ClientIdV2(7), invalid, Port::default()),
        Err(SearchClientRestoreErrorV1::InvalidPendingRequest)
    ));
}

#[test]
fn rebind_returns_old_uncertainty_and_starts_a_fresh_namespace() {
    let port = Port {
        replies: [
            Err(SearchPortFailureV1::OutcomeUnknown("lost")),
            Ok(refused()),
        ]
        .into(),
        ..Port::default()
    };
    let mut client = SearchClientV1::new(epoch(1), ClientIdV2(7), port);
    assert!(matches!(
        client.execute(command(2)),
        Err(SearchClientErrorV1::OutcomeUnknown("lost"))
    ));
    let (mut rebound, unresolved) = client.rebind(epoch(2), ClientIdV2(8));
    assert_eq!(unresolved.unwrap().request_id, SearchRequestIdV1(0));
    assert_eq!(rebound.execute(command(3)), Ok(refused()));
    let (port, state) = rebound.into_state();
    assert_eq!(port.offers[1].client, ClientIdV2(8));
    assert_eq!(port.offers[1].request.request_id, SearchRequestIdV1(0));
    assert_eq!(state.replay_epoch, epoch(2));
}

#[test]
fn diagnostics_hide_epoch_room_term_and_pending_command() {
    assert!(SearchReplayEpochV1::new([0; 32]).is_none());
    let state = SearchClientStateV1 {
        replay_epoch: epoch(0x41),
        client: ClientIdV2(7),
        next_request: 1,
        pending: Some(SearchRequestV1 {
            request_id: SearchRequestIdV1(0),
            command: command(2),
        }),
    };
    let debug = format!("{state:?}");
    assert!(!debug.contains("414141"));
    assert!(!debug.contains("secret-room"));
    assert!(!debug.contains("secret-term"));
    assert!(debug.contains("<opaque>"));
}
