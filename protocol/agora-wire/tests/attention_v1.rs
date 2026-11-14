use agora_protocol::v2::{
    AttentionPreferenceV1, AttentionScopeV1, CapabilityV2, ClientIdV2, CommandV2, MessageIdV1,
    NotificationIntentV1, OutcomeV2, RefusalV2, ReplyV2, RequestIdV2, RequestV2, TopicIdV1,
};
use agora_wire::WireLimits;
use agora_wire::v2::{
    WireReplyV2, WireRequestV2, decode_reply_v2, decode_request_v2, encode_reply_v2,
    encode_request_v2,
};

fn limits() -> WireLimits {
    WireLimits::reference()
}

#[test]
fn attention_commands_outcomes_capability_and_refusal_are_closed() {
    let preference = AttentionPreferenceV1 {
        following: true,
        muted: true,
        notification_intent: NotificationIntentV1::AddressedOnly,
    };
    let commands = [
        CommandV2::AttentionPreference {
            room_id: "room".to_owned(),
            scope: AttentionScopeV1::Room,
        },
        CommandV2::SetAttentionPreference {
            room_id: "room".to_owned(),
            observed_revision: 9,
            scope: AttentionScopeV1::Thread(MessageIdV1::new([3; 16])),
            preference,
        },
        CommandV2::SetAttentionPreference {
            room_id: "room".to_owned(),
            observed_revision: 9,
            scope: AttentionScopeV1::Topic(TopicIdV1::new([4; 16])),
            preference: AttentionPreferenceV1::default(),
        },
    ];
    for (index, command) in commands.into_iter().enumerate() {
        let value = WireRequestV2::Execute {
            client: ClientIdV2(2),
            request: RequestV2 {
                request_id: RequestIdV2(index as u64 + 1),
                command,
            },
        };
        let encoded = encode_request_v2(&value, limits()).unwrap();
        assert_eq!(decode_request_v2(&encoded, limits()).unwrap(), value);
    }

    for outcome in [
        OutcomeV2::AttentionPreference(preference),
        OutcomeV2::AttentionPreferenceSet(preference),
        OutcomeV2::Capabilities(vec![CapabilityV2::PrivateAttentionPreferencesV1]),
    ] {
        let value = WireReplyV2::Executed(ReplyV2::Accepted(outcome));
        let encoded = encode_reply_v2(&value, limits()).unwrap();
        assert_eq!(decode_reply_v2(&encoded, limits()).unwrap(), value);
    }
    let refusal = WireReplyV2::Executed(ReplyV2::Refused(RefusalV2::InvalidAttentionScope));
    let encoded = encode_reply_v2(&refusal, limits()).unwrap();
    assert_eq!(decode_reply_v2(&encoded, limits()).unwrap(), refusal);
}

#[test]
fn attention_debug_redacts_preference_values() {
    let preference = AttentionPreferenceV1 {
        following: true,
        muted: false,
        notification_intent: NotificationIntentV1::AllActivity,
    };
    let debug = format!(
        "{:?}",
        CommandV2::SetAttentionPreference {
            room_id: "secret-room".to_owned(),
            observed_revision: 3,
            scope: AttentionScopeV1::Room,
            preference,
        }
    );
    assert!(!debug.contains("secret-room"));
    assert!(!debug.contains("AllActivity"));
    assert!(!format!("{preference:?}").contains("AllActivity"));
}
