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
