use agora_protocol::{CommandV1, MessageV1};

#[test]
fn debug_output_reports_shape_without_conversation_content() {
    let command = CommandV1::Speak {
        room_id: "sensitive-room".to_owned(),
        expected_revision: 2,
        logical_time: 3,
        body: "private message".to_owned(),
        addressed_to: vec!["private-recipient".to_owned()],
    };
    let message = MessageV1 {
        sequence: 0,
        author: "private-author".to_owned(),
        logical_time: 3,
        body: "private message".to_owned(),
        addressed_to: vec!["private-recipient".to_owned()],
    };

    let rendered = format!("{command:?} {message:?}");
    for secret in [
        "sensitive-room",
        "private message",
        "private-recipient",
        "private-author",
    ] {
        assert!(!rendered.contains(secret));
    }
    assert!(rendered.contains("body_bytes: 15"));
}
