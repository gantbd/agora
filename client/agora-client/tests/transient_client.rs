use agora_client::transient::{
    TransientClientErrorV1, TransientClientRestoreErrorV1, TransientClientStateV1,
    TransientClientV1, TransientPortFailureV1, TransientPortV1, TransientReplayEpochV1,
};
use agora_protocol::transient::{
    TransientCommandV1, TransientRefusalV1, TransientReplyV1, TransientRequestIdV1,
    TransientRequestV1, TypingTargetV1,
};
use std::collections::VecDeque;

#[derive(Default)]
struct Port {
    replies: VecDeque<Result<TransientReplyV1, TransientPortFailureV1<&'static str>>>,
    requests: Vec<TransientRequestV1>,
}

impl TransientPortV1 for Port {
    type Error = &'static str;

    fn execute(
        &mut self,
        request: TransientRequestV1,
    ) -> Result<TransientReplyV1, TransientPortFailureV1<Self::Error>> {
        self.requests.push(request);
        self.replies.pop_front().unwrap()
    }
}

fn epoch(byte: u8) -> TransientReplayEpochV1 {
    TransientReplayEpochV1::new([byte; 32]).unwrap()
}

fn page(limit: u32) -> TransientCommandV1 {
    TransientCommandV1::PresencePage {
        room_id: "room".to_owned(),
        cursor: None,
        limit,
    }
}

fn refused() -> TransientReplyV1 {
    TransientReplyV1::Refused(TransientRefusalV1::NotMember)
}

#[test]
fn outcome_unknown_replays_only_the_exact_command_and_identity() {
    let port = Port {
        replies: [
            Err(TransientPortFailureV1::OutcomeUnknown("lost")),
            Ok(refused()),
        ]
        .into(),
        ..Port::default()
    };
    let mut client = TransientClientV1::new(epoch(1), port);
    assert_eq!(
        client.execute(page(2)),
        Err(TransientClientErrorV1::OutcomeUnknown("lost"))
    );
    assert_eq!(
        client.execute(page(3)),
        Err(TransientClientErrorV1::UnresolvedOutcome)
    );
    assert_eq!(client.execute(page(2)), Ok(refused()));
    let (port, state) = client.into_state();
    assert_eq!(port.requests[0], port.requests[1]);
    assert_eq!(port.requests[0].request_id, TransientRequestIdV1(0));
    assert!(state.pending.is_none());
    assert_eq!(state.next_request, 1);
}

#[test]
fn definitely_not_sent_does_not_create_uncertainty() {
    let port = Port {
        replies: [
            Err(TransientPortFailureV1::NotSent("closed")),
            Ok(refused()),
        ]
        .into(),
        ..Port::default()
    };
    let mut client = TransientClientV1::new(epoch(1), port);
    assert_eq!(
        client.execute(page(2)),
        Err(TransientClientErrorV1::NotSent("closed"))
    );
    assert!(client.pending_request().is_none());
    assert_eq!(client.execute(page(3)), Ok(refused()));
    let (port, _) = client.into_state();
    assert_eq!(port.requests[0].request_id, TransientRequestIdV1(0));
    assert_eq!(port.requests[1].request_id, TransientRequestIdV1(1));
}

#[test]
fn restoration_requires_the_same_epoch_and_exact_allocated_tail() {
    let pending = TransientRequestV1 {
        request_id: TransientRequestIdV1(4),
        command: page(2),
    };
    let state = TransientClientStateV1 {
        replay_epoch: epoch(1),
        next_request: 5,
        pending: Some(pending.clone()),
    };
    let restored = TransientClientV1::restore(epoch(1), state.clone(), Port::default()).unwrap();
    assert_eq!(restored.pending_request(), Some(&pending));
    assert!(matches!(
        TransientClientV1::restore(epoch(2), state.clone(), Port::default()),
        Err(TransientClientRestoreErrorV1::ReplayEpochChanged)
    ));
    let invalid = TransientClientStateV1 {
        next_request: 6,
        ..state
    };
    assert!(matches!(
        TransientClientV1::restore(epoch(1), invalid, Port::default()),
        Err(TransientClientRestoreErrorV1::InvalidPendingRequest)
    ));
}

#[test]
fn rebind_returns_old_uncertainty_and_starts_a_fresh_namespace() {
    let port = Port {
        replies: [
            Err(TransientPortFailureV1::OutcomeUnknown("lost")),
            Ok(refused()),
        ]
        .into(),
        ..Port::default()
    };
    let mut client = TransientClientV1::new(epoch(1), port);
    assert!(matches!(
        client.execute(page(2)),
        Err(TransientClientErrorV1::OutcomeUnknown("lost"))
    ));
    let (mut rebound, unresolved) = client.rebind(epoch(2));
    assert_eq!(unresolved.unwrap().request_id, TransientRequestIdV1(0));
    assert_eq!(rebound.execute(page(3)), Ok(refused()));
    let (port, state) = rebound.into_state();
    assert_eq!(port.requests[1].request_id, TransientRequestIdV1(0));
    assert_eq!(state.replay_epoch, epoch(2));
}

#[test]
fn epoch_and_state_diagnostics_are_content_safe() {
    assert!(TransientReplayEpochV1::new([0; 32]).is_none());
    let client = TransientClientV1::new(epoch(0x41), Port::default());
    let debug = format!("{:?}", client.state());
    assert!(!debug.contains("414141"));
    assert!(debug.contains("<opaque>"));
}

#[test]
fn typing_commands_share_the_exact_uncertainty_and_replay_laws() {
    let command = TransientCommandV1::PublishTyping {
        room_id: "room".to_owned(),
        target: TypingTargetV1::Room,
        lifetime: 5,
    };
    let port = Port {
        replies: [
            Err(TransientPortFailureV1::OutcomeUnknown("lost")),
            Ok(refused()),
        ]
        .into(),
        ..Port::default()
    };
    let mut client = TransientClientV1::new(epoch(3), port);
    assert_eq!(
        client.execute(command.clone()),
        Err(TransientClientErrorV1::OutcomeUnknown("lost"))
    );
    assert_eq!(
        client.execute(TransientCommandV1::ClearTyping {
            room_id: "room".to_owned(),
            target: TypingTargetV1::Room,
        }),
        Err(TransientClientErrorV1::UnresolvedOutcome)
    );
    assert_eq!(client.execute(command), Ok(refused()));
    let (port, state) = client.into_state();
    assert_eq!(port.requests[0], port.requests[1]);
    assert_eq!(port.requests[0].request_id, TransientRequestIdV1(0));
    assert!(state.pending.is_none());
}
