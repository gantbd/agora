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
