use agora_engine::export_source::RoomProjectionSourceV1;
use agora_engine::retention::{
    RecordClassV1, RetentionBoundV1, RetentionPolicyRevisionV1, RetentionPolicyV1,
    RoomProjectionRetentionRefusalV1, apply_room_projection_retention_v1,
};
use agora_engine::{
    AdmissionPolicy, CommandV2, Engine, LogicalTime, Participant, ParticipantKind, RoomId,
};

const MAX_RECORD_BYTES: usize = 4 * 1024 * 1024;

fn policy(engine: &Engine, bound: RetentionBoundV1) -> RetentionPolicyV1 {
    let mut bounds = [RetentionBoundV1::Disabled; 6];
    bounds[RecordClassV1::RoomProjection as usize - 1] = bound;
    RetentionPolicyV1::new(
        RetentionPolicyRevisionV1::new(23).unwrap(),
        engine.revision().0,
        bounds,
    )
}

fn symmetric_engine() -> Engine {
    let mut engine = Engine::new();
    for (id, kind) in [
        ("human", ParticipantKind::Human),
        ("agent", ParticipantKind::Agent),
    ] {
        engine
            .apply(CommandV2::RegisterParticipant(Participant::new(
                id, id, kind,
            )))
            .unwrap();
    }
    let room_id = RoomId::from("room");
    engine
        .apply(CommandV2::CreateRoom {
            room_id: room_id.clone(),
            name: "Room".to_owned(),
            owner: "human".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(CommandV2::Join {
            room_id: room_id.clone(),
            participant: "agent".into(),
        })
        .unwrap();
    engine
        .apply(CommandV2::Speak {
            room_id,
            author: "agent".into(),
            at: LogicalTime(1),
            body: "public".to_owned(),
        })
        .unwrap();
    engine
}

#[test]
fn positive_bound_retains_only_the_live_generation_without_mutation() {
    let engine = symmetric_engine();
    let before = engine.prepare_checkpoint(MAX_RECORD_BYTES).unwrap();
    let exact = policy(&engine, RetentionBoundV1::entries(1).unwrap());
    let applied = apply_room_projection_retention_v1(&engine, exact).unwrap();
    let source = RoomProjectionSourceV1::capture(&engine, exact, MAX_RECORD_BYTES).unwrap();

    assert_eq!(applied.source_engine_revision, engine.revision().0);
    assert_eq!(applied.retained_records, 1);
    assert_eq!(applied.discarded_records, 0);
    assert_eq!(
        applied.current_generation,
        source.fragment().records()[0].id
    );
    assert_eq!(applied.frontier, source.fragment().frontier());

    let wider = apply_room_projection_retention_v1(
        &engine,
        policy(&engine, RetentionBoundV1::entries(8).unwrap()),
    )
    .unwrap();
    assert_eq!(wider.current_generation, applied.current_generation);
    assert_eq!(wider.retained_records, 1);
    assert_eq!(wider.discarded_records, 0);
    assert_eq!(
        engine.prepare_checkpoint(MAX_RECORD_BYTES).unwrap().bytes(),
        before.bytes()
    );
}

#[test]
fn disabled_and_future_policy_never_delete_live_authoritative_state() {
    let engine = symmetric_engine();
    let before = engine.prepare_checkpoint(MAX_RECORD_BYTES).unwrap();

    assert_eq!(
        apply_room_projection_retention_v1(&engine, policy(&engine, RetentionBoundV1::Disabled),),
        Err(RoomProjectionRetentionRefusalV1::ClassDisabled)
    );
    let mut future = policy(&engine, RetentionBoundV1::entries(1).unwrap());
    future.effective_engine_revision += 1;
    assert_eq!(
        apply_room_projection_retention_v1(&engine, future),
        Err(RoomProjectionRetentionRefusalV1::StalePolicy)
    );
    assert_eq!(
        engine.prepare_checkpoint(MAX_RECORD_BYTES).unwrap().bytes(),
        before.bytes()
    );
}
