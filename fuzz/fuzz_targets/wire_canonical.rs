#![no_main]

use agora_wire::v2::{decode_reply_v2, decode_request_v2, encode_reply_v2, encode_request_v2};
use agora_wire::{
    WireLimits, decode_reply_v1, decode_request_v1, encode_reply_v1, encode_request_v1,
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
});
