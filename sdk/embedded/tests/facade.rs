use agora_sdk_embedded::{engine, host, service};

struct Verifier;

struct SearchPort;

impl service::search::SearchPortV1 for SearchPort {
    fn search(
        &mut self,
        _query: service::search::port::SearchQueryV1,
    ) -> Result<service::search::port::SearchObservationV1, service::search::port::SearchRefusalV1>
    {
        Ok(service::search::port::SearchObservationV1::Unavailable)
    }

    fn retire_cursor(&mut self, _cursor: service::search::port::SearchCursorV1) -> bool {
        true
    }
}

impl host::PrincipalVerifier for Verifier {
    type Context = ();
    type Error = ();

    fn authenticate(
        &mut self,
        _claimed_participant: &str,
        _context: &Self::Context,
        _now: host::HostTimeV1,
    ) -> Result<host::VerifiedSubjectV1, Self::Error> {
        Ok(host::VerifiedSubjectV1::from_verifier([1; 16]))
    }

    fn revalidate(
        &mut self,
        _subject: host::VerifiedSubjectV1,
        _participant: &str,
        _context: &Self::Context,
        _now: host::HostTimeV1,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[test]
fn facade_alone_constructs_the_embedded_stack() {
    let engine = engine::Engine::new();
    let service = service::Service::new(engine, service::ServiceLimits::default());
    let host = host::Host::new(service, Verifier, host::HostLimits::default());

    assert_eq!(host.service().usage(), service::Usage::default());
}

#[test]
fn facade_alone_constructs_the_embedded_v2_stack() {
    let engine = engine::Engine::new();
    let service = service::v2::ServiceV2::new(engine, service::v2::ServiceLimitsV2::default());
    let host = host::v2::HostV2::new(service, Verifier, host::HostLimits::default());

    assert_eq!(host.service().usage(), service::v2::UsageV2::default());
    assert_eq!(
        engine::ReadScopeV1::Room,
        engine::ReadScopeV1::Room,
        "private read-position substrate is reachable through the embedded facade"
    );
    let source = engine::TopicId::from_bytes([0x51; 16]);
    let successor = engine::TopicId::from_bytes([0x52; 16]);
    let command = engine::CommandV2::SetTopicSuccessor {
        room_id: "room".into(),
        actor: "owner".into(),
        topic_id: source,
        successor_topic_id: Some(successor),
    };
    assert!(matches!(
        command,
        engine::CommandV2::SetTopicSuccessor { .. }
    ));
    let descriptor = engine::AttachmentDescriptorV1::new(
        engine::AttachmentIdV1::new([1; 16]),
        engine::AttachmentStoreIdV1::new([2; 16]),
        engine::AttachmentObjectKeyV1::new([3; 32]),
        4,
        "text/plain",
        "note.txt",
        engine::AttachmentIntegrityV1::Sha256(engine::Sha256DigestV1::new([4; 32])),
    );
    assert_eq!(descriptor.declared_bytes(), 4);
    let reference = engine::VellumArtifactReferenceV1::new(
        engine::VellumArtifactReferenceIdV1::new([5; 16]),
        engine::VellumDocumentTargetV1::new(6, 7, 8).unwrap(),
    );
    assert_eq!(reference.target().source_revision(), 8);
}

#[test]
fn facade_names_authoritative_conversation_profiles() {
    let command = engine::CommandV2::CreatePeerConversation {
        room_id: "direct".into(),
        name: "Direct".to_owned(),
        initiator: "alice".into(),
        profile: engine::ConversationProfileV1::Direct,
        founding_members: vec!["alice".into(), "bot".into()],
    };
    assert!(matches!(
        command,
        engine::CommandV2::CreatePeerConversation {
            profile: engine::ConversationProfileV1::Direct,
            ..
        }
    ));
}

#[test]
fn facade_names_private_attention_engine_values() {
    let preference = engine::AttentionPreferenceV1::new(
        true,
        false,
        engine::NotificationIntentV1::AddressedOnly,
    );
    assert!(preference.following());
    assert!(!preference.muted());
    assert_eq!(
        preference.notification_intent(),
        engine::NotificationIntentV1::AddressedOnly
    );
    assert_eq!(
        engine::AttentionScopeV1::Room,
        engine::AttentionScopeV1::Room
    );
}

#[test]
fn facade_names_shared_pin_and_private_save_engine_values() {
    let message_id = engine::MessageId::from_bytes([9; 16]);
    let command = engine::CommandV2::SetPin {
        room_id: "room".into(),
        actor: "alice".into(),
        message_id,
        pinned: true,
    };
    assert!(matches!(
        command,
        engine::CommandV2::SetPin { pinned: true, .. }
    ));
    assert_eq!(engine::MAX_ROOM_PINS_V1, 256);
    assert_eq!(engine::MAX_PRIVATE_SAVES_V1, 4_096);
    let _: Option<engine::SavedReferenceV1> = None;
    let _: Option<engine::PinnedReferenceV1> = None;
}

#[test]
fn facade_names_bounded_poll_engine_values() {
    let poll_id = engine::PollId::from_bytes([7; 16]);
    let command = engine::CommandV2::SetPollBallot {
        room_id: "room".into(),
        actor: "alice".into(),
        poll_id,
        selections: vec![0],
    };
    assert!(matches!(command, engine::CommandV2::SetPollBallot { .. }));
    assert_eq!(engine::MAX_POLLS_PER_ROOM_V1, 256);
    assert_eq!(engine::MAX_POLL_OPTIONS_V1, 32);
}

#[test]
fn facade_names_topic_collection_engine_values() {
    let filter = engine::TopicCollectionFilterV1 {
        lifecycle: engine::TopicCollectionLifecycleV1::Active,
        tags: vec!["blue".to_owned()],
    };
    assert_eq!(filter.tags.len(), 1);
    let _: Option<engine::TopicCollectionPageV1> = None;
}

#[test]
fn facade_constructs_the_additive_transient_host_profile() {
    let engine = engine::Engine::new();
    let service = service::v2::ServiceV2::new(engine, service::v2::ServiceLimitsV2::default());
    let host = host::v2::HostV2::new(service, Verifier, host::HostLimits::default());
    let transient = host::transient::TransientHostV1::new_with_typing(
        host,
        service::transient::TransientLimitsV1::default(),
        service::typing::TypingLimitsV1::default(),
        5,
    )
    .unwrap();

    assert_eq!(
        transient.host().service().usage(),
        service::v2::UsageV2::default()
    );
}

#[test]
fn facade_constructs_the_participant_bound_search_host_profile() {
    let engine = engine::Engine::new();
    let service = service::v2::ServiceV2::new(engine, service::v2::ServiceLimitsV2::default());
    let host = host::v2::HostV2::new(service, Verifier, host::HostLimits::default());
    let search = host::search::SearchHostV1::new(
        host,
        SearchPort,
        service::search::SearchServiceLimitsV1::default(),
    );

    assert_eq!(
        search.host().service().usage(),
        service::v2::UsageV2::default()
    );
    assert_eq!(
        search.search_usage(),
        service::search::SearchServiceUsageV1::default()
    );
}
