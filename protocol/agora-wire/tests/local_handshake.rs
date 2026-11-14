use agora_wire::local_handshake::{
    HandshakeErrorV1, ServerIncarnationV1, decode_client_hello_v1,
    decode_collaboration_client_hello_v1, decode_collaboration_server_hello_v1,
    decode_search_client_hello_v1, decode_search_server_hello_v1, decode_server_hello_v1,
    encode_client_hello_v1, encode_collaboration_client_hello_v1,
    encode_collaboration_server_hello_v1, encode_search_client_hello_v1,
    encode_search_server_hello_v1, encode_server_hello_v1,
};

fn fixture(input: &str) -> Vec<u8> {
    let compact: String = input
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    compact
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            u8::from_str_radix(std::str::from_utf8(pair).expect("ASCII fixture"), 16)
                .expect("hex fixture")
        })
        .collect()
}

#[test]
fn search_handshake_is_additive_closed_and_content_safe() {
    let client = encode_search_client_hello_v1();
    assert_eq!(
        client.as_slice(),
        fixture(include_str!("fixtures/local-search-client-hello-v1.hex"))
    );
    assert_eq!(decode_search_client_hello_v1(&client), Ok(()));
    assert_eq!(
        decode_collaboration_client_hello_v1(&client),
        Err(HandshakeErrorV1::Magic)
    );
    let mut changed = client;
    changed[13] = 2;
    assert_eq!(
        decode_search_client_hello_v1(&changed),
        Err(HandshakeErrorV1::Profile)
    );

    let incarnation = ServerIncarnationV1::new([0xa5; 16]);
    let server = encode_search_server_hello_v1(incarnation, 8_192, 8).unwrap();
    assert_eq!(
        server.as_slice(),
        fixture(include_str!("fixtures/local-search-server-hello-v1.hex"))
    );
    let decoded = decode_search_server_hello_v1(&server).unwrap();
    assert_eq!(decoded.max_frame_bytes(), 8_192);
    assert_eq!(decoded.max_frames_after_connect(), 8);
    assert_eq!(decoded.server_incarnation(), incarnation);
    assert!(decoded.supports_capability_discovery());
    assert!(decoded.supports_derived_search());
    assert!(!format!("{decoded:?}").contains("a5"));
    assert_eq!(
        decode_collaboration_server_hello_v1(&server),
        Err(HandshakeErrorV1::Magic)
    );
    assert_eq!(
        decode_search_server_hello_v1(&server[..41]),
        Err(HandshakeErrorV1::Length)
    );
    for index in [9, 11, 13, 41] {
        changed = encode_search_client_hello_v1();
        if index < changed.len() {
            changed[index] ^= 1;
            assert!(decode_search_client_hello_v1(&changed).is_err());
        }
    }
    let mut changed_server = server;
    changed_server[22..38].fill(0);
    assert_eq!(
        decode_search_server_hello_v1(&changed_server),
        Err(HandshakeErrorV1::Limits)
    );
}

#[test]
fn collaboration_handshake_adds_transient_without_reinterpreting_profile_one() {
    let client = encode_collaboration_client_hello_v1();
    assert_eq!(
        client.as_slice(),
        fixture(include_str!(
            "fixtures/local-collaboration-client-hello-v1.hex"
        ))
    );
    assert_eq!(decode_collaboration_client_hello_v1(&client), Ok(()));
    assert_eq!(
        decode_client_hello_v1(&client),
        Err(HandshakeErrorV1::Length)
    );
    let mut changed = client;
    changed[13] = 2;
    assert_eq!(
        decode_collaboration_client_hello_v1(&changed),
        Err(HandshakeErrorV1::Profile)
    );

    let incarnation = ServerIncarnationV1::new([0xa5; 16]);
    let server = encode_collaboration_server_hello_v1(incarnation, 8_192, 8).unwrap();
    assert_eq!(
        server.as_slice(),
        fixture(include_str!(
            "fixtures/local-collaboration-server-hello-v1.hex"
        ))
    );
    let decoded = decode_collaboration_server_hello_v1(&server).unwrap();
    assert_eq!(decoded.max_frame_bytes(), 8_192);
    assert_eq!(decoded.max_frames_after_connect(), 8);
    assert_eq!(decoded.server_incarnation(), incarnation);
    assert!(decoded.supports_capability_discovery());
    assert!(decoded.supports_transient_collaboration());
    assert!(!format!("{decoded:?}").contains("a5"));
    assert_eq!(
        decode_server_hello_v1(&server),
        Err(HandshakeErrorV1::Length)
    );
    assert_eq!(
        decode_collaboration_server_hello_v1(&server[..41]),
        Err(HandshakeErrorV1::Length)
    );
    let mut changed = server;
    changed[0] ^= 1;
    assert_eq!(
        decode_collaboration_server_hello_v1(&changed),
        Err(HandshakeErrorV1::Magic)
    );
    for index in [9, 11, 13, 41] {
        changed = server;
        changed[index] ^= 1;
        assert_eq!(
            decode_collaboration_server_hello_v1(&changed),
            Err(HandshakeErrorV1::Profile)
        );
    }
    for range in [14..18, 18..22, 22..38] {
        changed = server;
        changed[range].fill(0);
        assert_eq!(
            decode_collaboration_server_hello_v1(&changed),
            Err(HandshakeErrorV1::Limits)
        );
    }
    assert_eq!(
        encode_collaboration_server_hello_v1(ServerIncarnationV1::new([0; 16]), 8_192, 8),
        Err(HandshakeErrorV1::Limits)
    );
}

#[test]
fn handshake_profile_is_canonical_closed_bounded_and_content_safe() {
    let client = encode_client_hello_v1();
    assert_eq!(
        client.as_slice(),
        fixture(include_str!("fixtures/local-client-hello-v1.hex"))
    );
    assert_eq!(decode_client_hello_v1(&client), Ok(()));
    assert_eq!(
        decode_client_hello_v1(&client[..11]),
        Err(HandshakeErrorV1::Length)
    );
    let mut changed = client;
    changed[0] ^= 1;
    assert_eq!(
        decode_client_hello_v1(&changed),
        Err(HandshakeErrorV1::Magic)
    );
    changed = client;
    changed[11] = 3;
    assert_eq!(
        decode_client_hello_v1(&changed),
        Err(HandshakeErrorV1::Profile)
    );

    let incarnation = ServerIncarnationV1::new([0xa5; 16]);
    let server = encode_server_hello_v1(incarnation, 8_192, 8).unwrap();
    assert_eq!(
        server.as_slice(),
        fixture(include_str!("fixtures/local-server-hello-v1.hex"))
    );
    let decoded = decode_server_hello_v1(&server).unwrap();
    assert_eq!(decoded.max_frame_bytes(), 8_192);
    assert_eq!(decoded.max_frames_after_connect(), 8);
    assert_eq!(decoded.server_incarnation(), incarnation);
    assert!(decoded.supports_capability_discovery());
    assert!(!format!("{decoded:?}").contains("a5"));
    assert!(!format!("{incarnation:?}").contains("a5"));

    assert_eq!(
        decode_server_hello_v1(&server[..39]),
        Err(HandshakeErrorV1::Length)
    );
    let mut changed = server;
    changed[0] ^= 1;
    assert_eq!(
        decode_server_hello_v1(&changed),
        Err(HandshakeErrorV1::Magic)
    );
    changed = server;
    changed[39] = 2;
    assert_eq!(
        decode_server_hello_v1(&changed),
        Err(HandshakeErrorV1::Profile)
    );
    changed = server;
    changed[12..16].fill(0);
    assert_eq!(
        decode_server_hello_v1(&changed),
        Err(HandshakeErrorV1::Limits)
    );
    changed = server;
    changed[20..36].fill(0);
    assert_eq!(
        decode_server_hello_v1(&changed),
        Err(HandshakeErrorV1::Limits)
    );
    assert_eq!(
        encode_server_hello_v1(ServerIncarnationV1::new([0; 16]), 8_192, 8),
        Err(HandshakeErrorV1::Limits)
    );
}
