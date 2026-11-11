use agora_sdk_embedded::{engine, host, service};

struct Verifier;

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
