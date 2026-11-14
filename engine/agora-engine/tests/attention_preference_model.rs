use agora_engine::{
    AdmissionPolicy, AttentionPreferenceV1, AttentionScopeV1, CommandV2, Engine,
    NotificationIntentV1, Participant, ParticipantId, ParticipantKind, RoomId,
};

fn preference(index: usize) -> AttentionPreferenceV1 {
    match index {
        0 => AttentionPreferenceV1::default(),
        1 => AttentionPreferenceV1::new(true, false, NotificationIntentV1::Default),
        2 => AttentionPreferenceV1::new(false, true, NotificationIntentV1::AddressedOnly),
        3 => AttentionPreferenceV1::new(true, true, NotificationIntentV1::AllActivity),
        _ => unreachable!(),
    }
}

fn engine(actor_kind: ParticipantKind) -> Engine {
    let mut engine = Engine::new();
    for (id, kind) in [("owner", ParticipantKind::Human), ("actor", actor_kind)] {
        engine
            .apply(CommandV2::RegisterParticipant(Participant::new(
                id, id, kind,
            )))
            .unwrap();
    }
    engine
        .apply(CommandV2::CreateRoom {
            room_id: "room".into(),
            name: "Room".to_owned(),
            owner: "owner".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(CommandV2::Join {
            room_id: "room".into(),
            participant: "actor".into(),
        })
        .unwrap();
    engine
}

#[test]
fn all_three_step_preference_schedules_are_kind_neutral_and_last_write_exact() {
    for first in 0..4 {
        for second in 0..4 {
            for third in 0..4 {
                let schedule = [first, second, third];
                let mut observations = Vec::new();
                for kind in [ParticipantKind::Human, ParticipantKind::Agent] {
                    let mut engine = engine(kind);
                    let revision = engine.revision();
                    let event_count = engine.pending_events().count();
                    for value in schedule.map(preference) {
                        assert_eq!(
                            engine
                                .set_attention_preference(
                                    &ParticipantId::new("actor"),
                                    &RoomId::new("room"),
                                    AttentionScopeV1::Room,
                                    revision,
                                    value,
                                )
                                .unwrap(),
                            value
                        );
                        assert_eq!(engine.revision(), revision);
                        assert_eq!(engine.pending_events().count(), event_count);
                    }
                    observations.push(
                        engine
                            .attention_preference(
                                &ParticipantId::new("actor"),
                                &RoomId::new("room"),
                                AttentionScopeV1::Room,
                            )
                            .unwrap(),
                    );
                }
                assert_eq!(observations, [preference(third), preference(third)]);
            }
        }
    }
}
