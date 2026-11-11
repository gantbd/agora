#![no_main]

use agora_local_daemon::{decode_outer_frame_prefix_v1, encode_outer_frame_v1};
use libfuzzer_sys::fuzz_target;

const MAX_INPUT_BYTES: usize = 64 * 1_024;
const MAX_FRAME_BYTES: usize = MAX_INPUT_BYTES - 4;

fuzz_target!(|input: &[u8]| {
    if input.len() > MAX_INPUT_BYTES {
        return;
    }
    if let Ok(decoded) = decode_outer_frame_prefix_v1(input, MAX_FRAME_BYTES) {
        let encoded = encode_outer_frame_v1(decoded.payload(), MAX_FRAME_BYTES)
            .expect("decoded outer payload must encode");
        assert_eq!(
            encoded,
            input[..encoded.len()],
            "accepted outer frame prefix was not canonical"
        );
        assert_eq!(
            decoded.remainder(),
            &input[encoded.len()..],
            "outer frame decoder consumed beyond one prefix"
        );
        let reparsed = decode_outer_frame_prefix_v1(&encoded, MAX_FRAME_BYTES)
            .expect("encoded outer frame must decode");
        assert_eq!(reparsed.payload(), decoded.payload());
        assert!(reparsed.remainder().is_empty());
    }
});
