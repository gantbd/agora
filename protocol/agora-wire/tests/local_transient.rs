use agora_wire::local_transient::{
    LocalReplayEpochV1, LocalRouteErrorV1, LocalRouteV1, decode_local_routed_frame_v1,
    decode_local_session_hello_v1, encode_local_routed_frame_v1, encode_local_session_hello_v1,
};

fn fixture(input: &str) -> Vec<u8> {
    let compact: String = input
        .chars()
        .filter(|value| !value.is_whitespace())
        .collect();
    compact
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn routed_frames_are_typed_bounded_exact_and_content_safe() {
    for route in [
        LocalRouteV1::Conversation,
        LocalRouteV1::Transient,
        LocalRouteV1::Search,
        LocalRouteV1::Scheduled,
        LocalRouteV1::Control,
    ] {
        let encoded = encode_local_routed_frame_v1(route, b"secret", 8).unwrap();
        let decoded = decode_local_routed_frame_v1(&encoded, 8).unwrap();
        assert_eq!(decoded.route(), route);
        assert_eq!(decoded.payload(), b"secret");
        assert!(!format!("{decoded:?}").contains("secret"));
    }
    assert_eq!(
        encode_local_routed_frame_v1(LocalRouteV1::Transient, b"", 8),
        Err(LocalRouteErrorV1::Empty)
    );
    assert_eq!(
        encode_local_routed_frame_v1(LocalRouteV1::Transient, b"12345678", 8),
        Err(LocalRouteErrorV1::Limit)
    );
    assert_eq!(
        decode_local_routed_frame_v1(&[9, 1], 8),
        Err(LocalRouteErrorV1::UnknownRoute)
    );
    assert_eq!(
        decode_local_routed_frame_v1(&[2], 8),
        Err(LocalRouteErrorV1::Empty)
    );
    assert_eq!(
        decode_local_routed_frame_v1(&[2, 1, 2], 2),
        Err(LocalRouteErrorV1::Limit)
    );
}

#[test]
fn session_hello_freezes_a_nonzero_opaque_replay_epoch() {
    assert!(LocalReplayEpochV1::new([0; 32]).is_none());
    let epoch = LocalReplayEpochV1::new([0xe7; 32]).unwrap();
    let encoded = encode_local_session_hello_v1(epoch);
    assert_eq!(
        encoded.as_slice(),
        fixture(include_str!("fixtures/local-session-hello-v1.hex"))
    );
    assert_eq!(decode_local_session_hello_v1(&encoded), Ok(epoch));
    assert!(!format!("{epoch:?}").contains("e7"));
    let mut changed = encoded;
    changed[0] ^= 1;
    assert_eq!(
        decode_local_session_hello_v1(&changed),
        Err(LocalRouteErrorV1::InvalidSessionHello)
    );
    assert_eq!(
        decode_local_session_hello_v1(&encoded[..39]),
        Err(LocalRouteErrorV1::InvalidSessionHello)
    );
    changed = encoded;
    changed[8..].fill(0);
    assert_eq!(
        decode_local_session_hello_v1(&changed),
        Err(LocalRouteErrorV1::InvalidSessionHello)
    );
}
