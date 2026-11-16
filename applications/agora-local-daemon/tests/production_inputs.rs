#![cfg(unix)]

use aegis_protocol::{GrantHandleV1, SessionHandleV1};
use agora_aegis_verifier::AegisContextV1;
use agora_host::{HostTimeV1, VerifiedSubjectV1};
use agora_local_daemon::{
    AuthenticatorV1, HostClockV1, PrivateEndpointAuthenticatorV1, SystemHostClockV1,
};
use std::os::unix::net::UnixStream;

fn context() -> AegisContextV1 {
    AegisContextV1::new(
        "operator",
        VerifiedSubjectV1::from_verifier([7; 16]),
        SessionHandleV1(11),
        GrantHandleV1(13),
    )
}

#[test]
fn private_endpoint_authenticator_binds_every_admitted_connection_to_one_context() {
    let expected = context();
    let mut authenticator = PrivateEndpointAuthenticatorV1::new(expected.clone());
    let (first, _first_peer) = UnixStream::pair().unwrap();
    let (second, _second_peer) = UnixStream::pair().unwrap();
    assert_eq!(authenticator.authenticate(&first), Ok(expected.clone()));
    assert_eq!(authenticator.authenticate(&second), Ok(expected));
    let debug = format!("{authenticator:?}");
    assert!(!debug.contains("operator"));
    assert!(!debug.contains("11"));
    assert!(!debug.contains("13"));
}

#[test]
fn system_clock_starts_at_epoch_milliseconds_and_never_regresses() {
    let mut clock = SystemHostClockV1::try_new().unwrap();
    let initial = clock.last();
    let first = clock.now();
    let second = clock.now();
    assert!(initial <= first);
    assert!(first <= second);
    assert!(matches!(second, HostTimeV1(value) if value > 0));
    assert!(format!("{clock:?}").contains("SystemHostClockV1"));
}
