use agora_engine::export_source::{
    AuthoritativeChangeSourceRefusalV1, AuthoritativeChangeSourceV1,
};
use agora_engine::persistence::RecoveryLimits;
use agora_engine::retention::{
    AuthoritativeChangeRetentionRefusalV1, RecordClassV1, RetentionBoundV1,
    RetentionPolicyRevisionV1, RetentionPolicyV1, apply_authoritative_change_retention_v1,
};
use agora_engine::{CommandV2, Engine, Limits, Participant, ParticipantKind};

fn policy(engine: &Engine, bound: RetentionBoundV1) -> RetentionPolicyV1 {
    let mut bounds = [RetentionBoundV1::Disabled; 6];
    bounds[RecordClassV1::AuthoritativeChanges as usize - 1] = bound;
    RetentionPolicyV1::new(
        RetentionPolicyRevisionV1::new(19).unwrap(),
        engine.revision().0,
        bounds,
    )
}

fn engine_with_four_changes() -> Engine {
    let mut engine = Engine::new();
    for (id, kind) in [
        ("human-a", ParticipantKind::Human),
        ("agent-a", ParticipantKind::Agent),
        ("human-b", ParticipantKind::Human),
        ("agent-b", ParticipantKind::Agent),
    ] {
        engine
            .apply(CommandV2::RegisterParticipant(Participant::new(
                id, id, kind,
            )))
            .unwrap();
    }
    engine
}

#[test]
fn native_change_retention_applies_the_exact_bound_and_source_frontier() {
    let mut engine = engine_with_four_changes();
    let revision = engine.revision();
    let bounded_policy = policy(&engine, RetentionBoundV1::entries(2).unwrap());
    let applied = apply_authoritative_change_retention_v1(&mut engine, bounded_policy).unwrap();
    assert_eq!(engine.revision(), revision);
    assert_eq!(applied.source_engine_revision, revision.0);
    assert_eq!(applied.discarded_records, 2);
    assert_eq!(applied.retained_records, 2);
    assert_eq!(applied.frontier.discarded_through, 2);
    assert_eq!(applied.frontier.oldest_retained, Some(3));
    assert_eq!(engine.retained_changes().len(), 2);
    assert_eq!(engine.discarded_changes_through().unwrap().0, 1);

    let source = AuthoritativeChangeSourceV1::capture(
        &engine,
        policy(&engine, RetentionBoundV1::entries(2).unwrap()),
        2,
        64 * 1024,
    )
    .unwrap();
    assert_eq!(source.fragment().frontier(), applied.frontier);
    assert_eq!(source.fragment().records()[0].id.ordinal, 3);
    assert_eq!(source.fragment().records()[1].id.ordinal, 4);

    let replay_policy = policy(&engine, RetentionBoundV1::entries(2).unwrap());
    let replay = apply_authoritative_change_retention_v1(&mut engine, replay_policy).unwrap();
    assert_eq!(replay.discarded_records, 0);
    assert_eq!(replay.frontier, applied.frontier);

    let checkpoint = engine.prepare_checkpoint(4 * 1024 * 1024).unwrap();
    let recovered = Engine::recover_checkpoint(
        checkpoint.bytes(),
        RecoveryLimits {
            max_snapshot_bytes: 4 * 1024 * 1024,
            maximum_engine_limits: Limits::default(),
        },
    )
    .unwrap();
    assert_eq!(recovered.revision(), revision);
    assert_eq!(recovered.retained_changes().len(), 2);
    assert_eq!(recovered.discarded_changes_through().unwrap().0, 1);
}

#[test]
fn disabled_and_stale_application_are_exact_atomic_and_kind_neutral() {
    let mut engine = engine_with_four_changes();
    let before = engine.prepare_checkpoint(4 * 1024 * 1024).unwrap();
    let mut stale = policy(&engine, RetentionBoundV1::entries(1).unwrap());
    stale.effective_engine_revision -= 1;
    assert_eq!(
        apply_authoritative_change_retention_v1(&mut engine, stale),
        Err(AuthoritativeChangeRetentionRefusalV1::StalePolicy)
    );
    assert_eq!(
        engine.prepare_checkpoint(4 * 1024 * 1024).unwrap().bytes(),
        before.bytes()
    );

    let disabled = policy(&engine, RetentionBoundV1::Disabled);
    let applied = apply_authoritative_change_retention_v1(&mut engine, disabled).unwrap();
    assert_eq!(applied.discarded_records, 4);
    assert_eq!(applied.retained_records, 0);
    assert_eq!(applied.frontier.discarded_through, 4);
    assert_eq!(applied.frontier.oldest_retained, None);
    assert_eq!(engine.retained_changes().len(), 0);
    assert_eq!(engine.discarded_changes_through().unwrap().0, 3);
    assert_eq!(
        AuthoritativeChangeSourceV1::capture(
            &engine,
            policy(&engine, RetentionBoundV1::entries(1).unwrap()),
            1,
            64 * 1024,
        ),
        Err(AuthoritativeChangeSourceRefusalV1::NoRetainedChanges)
    );
}
