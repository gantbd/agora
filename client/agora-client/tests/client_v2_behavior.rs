use agora_client::v2::{
    ClientErrorV2, ClientRestoreErrorV2, ClientStateV2, ClientV2, PortFailureV2, ServicePortV2,
};
use agora_protocol::v2::{
    AttachmentDescriptorV1, AttachmentIdV1, AttachmentIntegrityV1, AttachmentMessageTargetV1,
    AttachmentObjectKeyV1, AttachmentStoreIdV1, AttentionPreferenceV1, AttentionScopeV1,
    ClientIdV2, CommandV2, MessageIdV1, NotificationIntentV1, PollIdV1, RefusalV2, ReplyV2,
    RequestIdV2, RequestV2, Sha256DigestV1, TopicCollectionFilterV1, TopicCollectionLifecycleV1,
    VellumArtifactMessageTargetV1, VellumArtifactReferenceIdV1, VellumArtifactReferenceV1,
    VellumDocumentTargetV1,
};
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

fn attachment_command(label: &str) -> CommandV2 {
    CommandV2::SpeakWithAttachments {
        room_id: "room".to_owned(),
        expected_revision: 4,
        target: AttachmentMessageTargetV1::Room,
        logical_time: 5,
        body: "body".to_owned(),
        addressed_to: vec![],
        attachments: vec![AttachmentDescriptorV1 {
            attachment_id: AttachmentIdV1::new([1; 16]),
            store_id: AttachmentStoreIdV1::new([2; 16]),
            object_key: AttachmentObjectKeyV1::new([3; 32]),
            declared_bytes: 4,
            media_type: "text/plain".to_owned(),
            display_label: label.to_owned(),
            integrity: AttachmentIntegrityV1::Sha256(Sha256DigestV1::new([4; 32])),
        }],
    }
}

fn vellum_artifact_command(source_revision: u64) -> CommandV2 {
    CommandV2::SpeakWithVellumArtifacts {
        room_id: "room".to_owned(),
        expected_revision: 4,
        target: VellumArtifactMessageTargetV1::Room,
        logical_time: 5,
        body: "body".to_owned(),
        addressed_to: vec![],
        references: vec![VellumArtifactReferenceV1 {
            reference_id: VellumArtifactReferenceIdV1::new([1; 16]),
            target: VellumDocumentTargetV1 {
                workspace_id: 2,
                document_id: 3,
                source_revision,
            },
        }],
    }
}

fn attention_command(following: bool) -> CommandV2 {
    CommandV2::SetAttentionPreference {
        room_id: "room".to_owned(),
        observed_revision: 4,
        scope: AttentionScopeV1::Room,
        preference: AttentionPreferenceV1 {
            following,
            muted: true,
            notification_intent: NotificationIntentV1::AddressedOnly,
        },
    }
}

fn save_command(saved: bool) -> CommandV2 {
    CommandV2::SetSavedReference {
        room_id: "room".to_owned(),
        observed_revision: 4,
        message_id: MessageIdV1::new([8; 16]),
        saved,
    }
}

fn poll_ballot_command(selections: Vec<u8>) -> CommandV2 {
    CommandV2::SetPollBallot {
        room_id: "room".to_owned(),
        expected_revision: 4,
        poll_id: PollIdV1::new([9; 16]),
        selections,
    }
}

fn topic_collection_command(tags: Vec<String>) -> CommandV2 {
    CommandV2::TopicCollection {
        room_id: "room".to_owned(),
        filter: TopicCollectionFilterV1 {
            lifecycle: TopicCollectionLifecycleV1::Active,
            tags,
        },
        cursor: None,
        limit: 8,
    }
}

fn report_command(reason: &str) -> CommandV2 {
    CommandV2::SubmitMessageReport {
        room_id: "room".to_owned(),
        observed_revision: 4,
        message_id: MessageIdV1::new([8; 16]),
        reason: reason.to_owned(),
        causation: [7; 16],
    }
}

#[test]
fn private_report_uncertainty_replays_only_the_exact_allegation() {
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
        client.execute(report_command("review")),
        Err(ClientErrorV2::OutcomeUnknown("lost"))
    );
    assert_eq!(
        client.execute(report_command("changed")),
        Err(ClientErrorV2::UnresolvedOutcome)
    );
    assert_eq!(
        client.execute(report_command("review")),
        Ok(ReplyV2::Refused(RefusalV2::Unavailable))
    );
    let (port, pending) = client.into_parts();
    assert!(pending.is_none());
    assert_eq!(port.requests[0], port.requests[1]);
}

#[test]
fn topic_collection_uncertainty_replays_only_the_exact_filter() {
    let port = RecordingPortV2 {
        replies: [
            Err(PortFailureV2::OutcomeUnknown("lost")),
            Ok(ReplyV2::Refused(RefusalV2::Unavailable)),
        ]
        .into(),
        ..Default::default()
    };
    let mut client = ClientV2::new(ClientIdV2(7), port);
    let exact = topic_collection_command(vec!["blue".to_owned(), "urgent".to_owned()]);
    assert_eq!(
        client.execute(exact.clone()),
        Err(ClientErrorV2::OutcomeUnknown("lost"))
    );
    assert_eq!(
        client.execute(topic_collection_command(vec!["blue".to_owned()])),
        Err(ClientErrorV2::UnresolvedOutcome)
    );
    assert_eq!(
        client.execute(exact),
        Ok(ReplyV2::Refused(RefusalV2::Unavailable))
    );
    let (port, pending) = client.into_parts();
    assert!(pending.is_none());
    assert_eq!(port.requests[0], port.requests[1]);
}

#[test]
fn poll_uncertainty_replays_only_the_exact_private_ballot() {
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
        client.execute(poll_ballot_command(vec![0])),
        Err(ClientErrorV2::OutcomeUnknown("lost"))
    );
    assert_eq!(
        client.execute(poll_ballot_command(vec![1])),
        Err(ClientErrorV2::UnresolvedOutcome)
    );
    assert_eq!(
        client.execute(poll_ballot_command(vec![0])),
        Ok(ReplyV2::Refused(RefusalV2::Unavailable))
    );
    assert!(client.pending_request().is_none());
}

#[test]
fn attention_uncertainty_replays_only_the_exact_private_preference() {
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
        client.execute(attention_command(true)),
        Err(ClientErrorV2::OutcomeUnknown("lost"))
    );
    assert_eq!(
        client.execute(attention_command(false)),
        Err(ClientErrorV2::UnresolvedOutcome)
    );
    assert_eq!(
        client.execute(attention_command(true)),
        Ok(ReplyV2::Refused(RefusalV2::Unavailable))
    );
    let (port, pending) = client.into_parts();
    assert!(pending.is_none());
    assert_eq!(port.requests[0], port.requests[1]);
}

#[test]
fn private_save_uncertainty_replays_only_the_exact_relation_change() {
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
        client.execute(save_command(true)),
        Err(ClientErrorV2::OutcomeUnknown("lost"))
    );
    assert_eq!(
        client.execute(save_command(false)),
        Err(ClientErrorV2::UnresolvedOutcome)
    );
    assert_eq!(
        client.execute(save_command(true)),
        Ok(ReplyV2::Refused(RefusalV2::Unavailable))
    );
    let (port, pending) = client.into_parts();
    assert!(pending.is_none());
    assert_eq!(port.requests[0], port.requests[1]);
}

#[test]
fn attachment_uncertainty_replays_only_the_exact_descriptor_set() {
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
        client.execute(attachment_command("first")),
        Err(ClientErrorV2::OutcomeUnknown("lost"))
    );
    assert_eq!(
        client.execute(attachment_command("changed")),
        Err(ClientErrorV2::UnresolvedOutcome)
    );
    assert_eq!(
        client.execute(attachment_command("first")),
        Ok(ReplyV2::Refused(RefusalV2::Unavailable))
    );
    let (port, pending) = client.into_parts();
    assert!(pending.is_none());
    assert_eq!(port.requests[0], port.requests[1]);
}

#[test]
fn vellum_artifact_uncertainty_replays_only_the_exact_revision_set() {
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
        client.execute(vellum_artifact_command(4)),
        Err(ClientErrorV2::OutcomeUnknown("lost"))
    );
    assert_eq!(
        client.execute(vellum_artifact_command(5)),
        Err(ClientErrorV2::UnresolvedOutcome)
    );
    assert_eq!(
        client.execute(vellum_artifact_command(4)),
        Ok(ReplyV2::Refused(RefusalV2::Unavailable))
    );
    let (port, pending) = client.into_parts();
    assert!(pending.is_none());
    assert_eq!(port.requests[0], port.requests[1]);
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

#[test]
fn exported_uncertainty_restores_the_exact_namespace_and_request() {
    let port = RecordingPortV2 {
        replies: [Err(PortFailureV2::OutcomeUnknown("lost"))].into(),
        ..Default::default()
    };
    let mut client = ClientV2::new(ClientIdV2(7), port);
    assert_eq!(
        client.execute(rooms(2)),
        Err(ClientErrorV2::OutcomeUnknown("lost"))
    );
    let (_, state) = client.into_state();
    assert_eq!(state.next_request, 1);
    assert_eq!(state.pending.as_ref().unwrap().request_id, RequestIdV2(0));

    let port = RecordingPortV2 {
        replies: [Ok(ReplyV2::Refused(RefusalV2::Unavailable))].into(),
        ..Default::default()
    };
    let mut restored = ClientV2::restore(state, port).unwrap();
    assert_eq!(
        restored.execute(rooms(3)),
        Err(ClientErrorV2::UnresolvedOutcome)
    );
    assert_eq!(
        restored.execute(rooms(2)),
        Ok(ReplyV2::Refused(RefusalV2::Unavailable))
    );
    let (port, state) = restored.into_state();
    assert!(state.pending.is_none());
    assert_eq!(state.next_request, 1);
    assert_eq!(port.requests[0].1.request_id, RequestIdV2(0));
}

#[test]
fn restore_refuses_a_pending_request_outside_the_exact_allocated_tail() {
    let invalid = ClientStateV2 {
        client: ClientIdV2(7),
        next_request: 2,
        pending: Some(RequestV2 {
            request_id: RequestIdV2(0),
            command: rooms(2),
        }),
    };
    assert!(matches!(
        ClientV2::restore(invalid, RecordingPortV2::default()),
        Err(ClientRestoreErrorV2::InvalidPendingRequest)
    ));
}
