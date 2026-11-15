use agora_wire::local_handshake::ServerIncarnationV1;
use agora_wire::remote_handshake::*;

#[test]
fn current_client_and_server_hellos_are_frozen_and_canonical() {
    let client = encode_client_hello_v1(RemoteClientHelloV1::current());
    assert_eq!(client, hex("41474f5241524331000100010002000200000001"));
    assert_eq!(
        decode_client_hello_v1(&client).unwrap(),
        RemoteClientHelloV1::current()
    );
    let server = negotiate_server_hello_v1(
        RemoteClientHelloV1::current(),
        ProfileRangeV1::new(1, 1).unwrap(),
        ProfileRangeV1::new(2, 2).unwrap(),
        4_096,
        32,
        8,
        ServerIncarnationV1::new([7; 16]),
    )
    .unwrap();
    let encoded = encode_server_hello_v1(server).unwrap();
    assert_eq!(
        encoded,
        hex(
            "41474f5241525331000100020000100000000020000000080707070707070707070707070707070700000001"
        )
    );
    assert_eq!(decode_server_hello_v1(&encoded).unwrap(), server);
    assert!(!format!("{server:?}").contains("070707"));
}

#[test]
fn range_selection_is_highest_common_and_fail_closed() {
    let client = RemoteClientHelloV1 {
        handshake_profiles: ProfileRangeV1::new(1, 3).unwrap(),
        conversation_wire_profiles: ProfileRangeV1::new(2, 5).unwrap(),
    };
    let selected = negotiate_server_hello_v1(
        client,
        ProfileRangeV1::new(2, 4).unwrap(),
        ProfileRangeV1::new(3, 4).unwrap(),
        1,
        1,
        1,
        ServerIncarnationV1::new([1; 16]),
    )
    .unwrap();
    assert_eq!(selected.handshake_profile, 3);
    assert_eq!(selected.conversation_wire_profile, 4);
    assert_eq!(
        negotiate_server_hello_v1(
            RemoteClientHelloV1::current(),
            ProfileRangeV1::new(2, 3).unwrap(),
            ProfileRangeV1::new(2, 2).unwrap(),
            1,
            1,
            1,
            ServerIncarnationV1::new([1; 16]),
        ),
        Err(RemoteHandshakeErrorV1::Profile)
    );
}

#[test]
fn malformed_limits_ranges_versions_and_framing_refuse() {
    assert_eq!(
        ProfileRangeV1::new(0, 1),
        Err(RemoteHandshakeErrorV1::Profile)
    );
    let client = encode_client_hello_v1(RemoteClientHelloV1::current());
    for length in 0..client.len() {
        assert_eq!(
            decode_client_hello_v1(&client[..length]),
            Err(RemoteHandshakeErrorV1::Length)
        );
    }
    let mut changed = client;
    changed[0] ^= 1;
    assert_eq!(
        decode_client_hello_v1(&changed),
        Err(RemoteHandshakeErrorV1::Magic)
    );
    let server = RemoteServerHelloV1 {
        handshake_profile: 1,
        conversation_wire_profile: 2,
        max_frame_bytes: 0,
        max_requests: 1,
        max_pending_writes: 1,
        server_incarnation: ServerIncarnationV1::new([1; 16]),
    };
    assert_eq!(
        encode_server_hello_v1(server),
        Err(RemoteHandshakeErrorV1::Limits)
    );
}

fn hex<const N: usize>(value: &str) -> [u8; N] {
    assert_eq!(value.len(), N * 2);
    let mut output = [0; N];
    for (index, byte) in output.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).unwrap();
    }
    output
}
