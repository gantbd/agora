use agora_engine::{
    AdmissionPolicy, CommandV2, Engine, LogicalTime, Participant, ParticipantId, ParticipantKind,
    RoomId,
};

const IDS: [&str; 3] = ["a", "b", "c"];

fn execute(kinds: [ParticipantKind; 3], author: usize, recipients: u8) -> (u64, u64, Vec<String>) {
    let mut engine = Engine::new();
    for (id, kind) in IDS.into_iter().zip(kinds) {
        engine
            .apply(CommandV2::RegisterParticipant(Participant::new(
                id, id, kind,
            )))
            .unwrap();
    }
    engine
        .apply(CommandV2::CreateRoom {
            room_id: RoomId::from("room"),
            name: "Room".into(),
            owner: ParticipantId::from("a"),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    for id in ["b", "c"] {
        engine
            .apply(CommandV2::Join {
                room_id: RoomId::from("room"),
                participant: ParticipantId::from(id),
            })
            .unwrap();
    }
    let addressed_to = IDS
        .into_iter()
        .enumerate()
        .filter(|(index, _)| recipients & (1 << index) != 0)
        .map(|(_, id)| ParticipantId::from(id))
        .collect();
    engine
        .apply(CommandV2::SpeakAddressed {
            room_id: RoomId::from("room"),
            author: ParticipantId::from(IDS[author]),
            at: LogicalTime(7),
            body: "bounded model".into(),
            addressed_to,
        })
        .unwrap();
    let addressed = engine
        .message(&RoomId::from("room"), 0)
        .unwrap()
        .addressed_to()
        .iter()
        .map(|id| id.as_str().to_owned())
        .collect();
    (
        engine.revision().0,
        engine.pending_events().len() as u64,
        addressed,
    )
}

#[test]
fn participant_kind_is_non_authoritative_for_every_small_addressing_state() {
    let reference = [ParticipantKind::Human; 3];
    for kind_bits in 0_u8..8 {
        let kinds = std::array::from_fn(|index| {
            if kind_bits & (1 << index) == 0 {
                ParticipantKind::Human
            } else {
                ParticipantKind::Agent
            }
        });
        for author in 0..IDS.len() {
            for recipients in 0_u8..8 {
                assert_eq!(
                    execute(kinds, author, recipients),
                    execute(reference, author, recipients),
                    "kind_bits={kind_bits:03b}, author={author}, recipients={recipients:03b}"
                );
            }
        }
    }
}
