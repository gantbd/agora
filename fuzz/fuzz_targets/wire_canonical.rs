#![no_main]

use agora_wire::v2::{decode_reply_v2, decode_request_v2, encode_reply_v2, encode_request_v2};
use agora_wire::{
    WireLimits, decode_reply_v1, decode_request_v1, encode_reply_v1, encode_request_v1,
};
use agora_wire::local_handshake::{
    decode_client_hello_v1, decode_collaboration_client_hello_v1,
    decode_collaboration_server_hello_v1, decode_server_hello_v1, encode_client_hello_v1,
    encode_collaboration_client_hello_v1, encode_collaboration_server_hello_v1,
    encode_search_client_hello_v1, encode_search_server_hello_v1, encode_server_hello_v1,
    decode_search_client_hello_v1, decode_search_server_hello_v1,
};
use agora_wire::local_transient::{
    decode_local_routed_frame_v1, decode_local_session_hello_v1, encode_local_routed_frame_v1,
    encode_local_session_hello_v1,
};
use agora_wire::search::{
    decode_search_offer_v1, decode_search_reply_v1, encode_search_offer_v1,
    encode_search_reply_v1,
};
use libfuzzer_sys::fuzz_target;

const MAX_INPUT_BYTES: usize = 64 * 1_024;

fuzz_target!(|input: &[u8]| {
    if input.is_empty() || input.len() > MAX_INPUT_BYTES {
        return;
    }
    let limits = WireLimits {
        max_frame_bytes: MAX_INPUT_BYTES,
        ..WireLimits::reference()
    };

    if let Ok(value) = decode_request_v1(input, limits) {
        let encoded = encode_request_v1(&value, limits).expect("decoded V1 request must encode");
        assert_eq!(encoded, input, "accepted V1 request was not canonical");
        assert_eq!(decode_request_v1(&encoded, limits), Ok(value));
    }
    if let Ok(value) = decode_reply_v1(input, limits) {
        let encoded = encode_reply_v1(&value, limits).expect("decoded V1 reply must encode");
        assert_eq!(encoded, input, "accepted V1 reply was not canonical");
        assert_eq!(decode_reply_v1(&encoded, limits), Ok(value));
    }
    if let Ok(value) = decode_request_v2(input, limits) {
        let encoded = encode_request_v2(&value, limits).expect("decoded V2 request must encode");
        assert_eq!(encoded, input, "accepted V2 request was not canonical");
        assert_eq!(decode_request_v2(&encoded, limits), Ok(value));
    }
    if let Ok(value) = decode_reply_v2(input, limits) {
        let encoded = encode_reply_v2(&value, limits).expect("decoded V2 reply must encode");
        assert_eq!(encoded, input, "accepted V2 reply was not canonical");
        assert_eq!(decode_reply_v2(&encoded, limits), Ok(value));
    }
    if let Ok(value) = decode_search_offer_v1(input, limits) {
        let encoded =
            encode_search_offer_v1(&value, limits).expect("decoded search offer must encode");
        assert_eq!(encoded, input, "accepted search offer was not canonical");
        assert_eq!(decode_search_offer_v1(&encoded, limits), Ok(value));
    }
    if let Ok(value) = decode_search_reply_v1(input, limits) {
        let encoded =
            encode_search_reply_v1(&value, limits).expect("decoded search reply must encode");
        assert_eq!(encoded, input, "accepted search reply was not canonical");
        assert_eq!(decode_search_reply_v1(&encoded, limits), Ok(value));
    }
    if decode_client_hello_v1(input).is_ok() {
        assert_eq!(encode_client_hello_v1().as_slice(), input);
    }
    if let Ok(value) = decode_server_hello_v1(input) {
        let encoded = encode_server_hello_v1(
            value.server_incarnation(),
            value.max_frame_bytes(),
            value.max_frames_after_connect(),
        )
        .expect("decoded server hello must encode");
        assert_eq!(encoded.as_slice(), input, "accepted server hello was not canonical");
        assert_eq!(decode_server_hello_v1(&encoded), Ok(value));
    }
    if decode_collaboration_client_hello_v1(input).is_ok() {
        assert_eq!(encode_collaboration_client_hello_v1().as_slice(), input);
    }
    if let Ok(value) = decode_collaboration_server_hello_v1(input) {
        let encoded = encode_collaboration_server_hello_v1(
            value.server_incarnation(),
            value.max_frame_bytes(),
            value.max_frames_after_connect(),
        )
        .expect("decoded collaboration server hello must encode");
        assert_eq!(
            encoded.as_slice(),
            input,
            "accepted collaboration server hello was not canonical"
        );
        assert_eq!(decode_collaboration_server_hello_v1(&encoded), Ok(value));
    }
    if decode_search_client_hello_v1(input).is_ok() {
        assert_eq!(encode_search_client_hello_v1().as_slice(), input);
    }
    if let Ok(value) = decode_search_server_hello_v1(input) {
        let encoded = encode_search_server_hello_v1(
            value.server_incarnation(),
            value.max_frame_bytes(),
            value.max_frames_after_connect(),
        )
        .expect("decoded search server hello must encode");
        assert_eq!(encoded.as_slice(), input, "accepted search server hello was not canonical");
        assert_eq!(decode_search_server_hello_v1(&encoded), Ok(value));
    }
    if let Ok(value) = decode_local_session_hello_v1(input) {
        let encoded = encode_local_session_hello_v1(value);
        assert_eq!(
            encoded.as_slice(),
            input,
            "accepted local session hello was not canonical"
        );
        assert_eq!(decode_local_session_hello_v1(&encoded), Ok(value));
    }
    if let Ok(value) = decode_local_routed_frame_v1(input, MAX_INPUT_BYTES) {
        let encoded = encode_local_routed_frame_v1(value.route(), value.payload(), MAX_INPUT_BYTES)
            .expect("decoded local route must encode");
        assert_eq!(
            encoded, input,
            "accepted local routed frame was not canonical"
        );
        let reparsed = decode_local_routed_frame_v1(&encoded, MAX_INPUT_BYTES)
            .expect("encoded local route must decode");
        assert_eq!(reparsed.route(), value.route());
        assert_eq!(reparsed.payload(), value.payload());
    }
});
