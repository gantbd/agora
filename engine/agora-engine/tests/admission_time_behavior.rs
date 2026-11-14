use agora_engine::persistence::RecoveryLimits;
use agora_engine::{AdmissionTimeRefusalV1, Engine, Limits};

fn recovery_limits() -> RecoveryLimits {
    RecoveryLimits {
        max_snapshot_bytes: 1024 * 1024,
        maximum_engine_limits: Limits::default(),
    }
}

#[test]
fn admission_time_is_monotone_single_current_and_content_safe() {
    let mut engine = Engine::new();
    let first = engine.advance_admission_time(10).unwrap();
    assert_eq!(format!("{first:?}"), "AdmissionTimeV1(<opaque>)");
    assert_eq!(engine.validate_admission_time(&first), Ok(()));

    let equal = engine.advance_admission_time(10).unwrap();
    assert_eq!(
        engine.validate_admission_time(&first),
        Err(AdmissionTimeRefusalV1::StaleObservation)
    );
    assert_eq!(engine.validate_admission_time(&equal), Ok(()));
    assert_eq!(
        engine.advance_admission_time(9),
        Err(AdmissionTimeRefusalV1::Regressed)
    );
    assert_eq!(engine.validate_admission_time(&equal), Ok(()));
}

#[test]
fn recovery_retains_floor_but_invalidates_old_observations() {
    let mut engine = Engine::new();
    let before_recovery = engine.advance_admission_time(42).unwrap();
    let bytes = engine.prepare_checkpoint(1024 * 1024).unwrap();
    let mut recovered = Engine::recover_checkpoint(bytes.bytes(), recovery_limits())
        .expect("recover admission-time floor");

    assert_eq!(
        recovered.validate_admission_time(&before_recovery),
        Err(AdmissionTimeRefusalV1::ForeignObservation)
    );
    assert_eq!(
        recovered.advance_admission_time(41),
        Err(AdmissionTimeRefusalV1::Regressed)
    );
    let current = recovered.advance_admission_time(42).unwrap();
    assert_eq!(recovered.validate_admission_time(&current), Ok(()));

    let canonical = recovered.prepare_checkpoint(1024 * 1024).unwrap();
    let again = Engine::recover_checkpoint(canonical.bytes(), recovery_limits())
        .expect("recover canonical admission-time checkpoint");
    assert_eq!(
        again.prepare_checkpoint(1024 * 1024).unwrap().bytes(),
        canonical.bytes()
    );
}

#[test]
fn observations_are_foreign_across_engines() {
    let mut left = Engine::new();
    let right = Engine::new();
    let time = left.advance_admission_time(7).unwrap();
    assert_eq!(
        right.validate_admission_time(&time),
        Err(AdmissionTimeRefusalV1::ForeignObservation)
    );
}
