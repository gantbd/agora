use agora_engine::persistence::RecoveryLimits;
use agora_engine::{
    AdmissionPolicy, CommandV2, Engine, EventSequence, LogicalTime, ModerationCausationV1,
    ModerationReasonV1, Participant, ParticipantKind, PublicationCommandV2, PublicationKeyV2,
};

fn fixture_engine() -> Engine {
    let mut engine = Engine::new();
    for (id, name, kind) in [
        ("human", "Human", ParticipantKind::Human),
        ("agent", "Agent", ParticipantKind::Agent),
    ] {
        engine
            .apply(CommandV2::RegisterParticipant(Participant::new(
                id, name, kind,
            )))
            .unwrap();
    }
    engine
        .apply(CommandV2::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
            owner: "human".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(CommandV2::Join {
            room_id: "room".into(),
            participant: "agent".into(),
        })
        .unwrap();
    engine
        .apply(CommandV2::SpeakAddressed {
            room_id: "room".into(),
            author: "human".into(),
            at: LogicalTime(17),
            body: "hello".to_owned(),
            addressed_to: vec!["agent".into()],
        })
        .unwrap();
    engine
        .publish(PublicationCommandV2::new(
            engine.revision(),
            PublicationKeyV2::new([0x2a; 16]),
            "room".into(),
            "agent".into(),
            LogicalTime(18),
            "reply",
        ))
        .unwrap();
    engine.acknowledge_events(EventSequence(2)).unwrap();
    engine
        .apply(CommandV2::Ban {
            room_id: "room".into(),
            actor: "human".into(),
            participant: "agent".into(),
            reason: ModerationReasonV1::new("policy"),
            causation: ModerationCausationV1::new([0x5a; 16]),
        })
        .unwrap();
    engine
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

fn unhex(value: &str) -> Vec<u8> {
    fn digit(value: u8) -> u8 {
        match value {
            b'0'..=b'9' => value - b'0',
            b'a'..=b'f' => value - b'a' + 10,
            _ => panic!("fixture contains non-canonical hex"),
        }
    }
    let value = value.trim().as_bytes();
    assert_eq!(value.len() % 2, 0, "fixture has a partial byte");
    value
        .chunks_exact(2)
        .map(|pair| (digit(pair[0]) << 4) | digit(pair[1]))
        .collect()
}

#[test]
fn canonical_v3_checkpoint_is_immutable_and_recovers() {
    let prepared = fixture_engine().prepare_checkpoint(65_536).unwrap();
    let expected_hex = include_str!("fixtures/checkpoint-v3.hex").trim();
    assert_eq!(hex(prepared.bytes()), expected_hex);

    let recovered = Engine::recover_checkpoint(
        &unhex(expected_hex),
        RecoveryLimits {
            max_snapshot_bytes: 65_536,
            maximum_engine_limits: Default::default(),
        },
    )
    .unwrap();
    assert_eq!(
        recovered.prepare_checkpoint(65_536).unwrap().bytes(),
        prepared.bytes()
    );
}
