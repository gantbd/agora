#![no_main]

use agora_engine::{Engine, Limits, persistence::RecoveryLimits};
use agora_host::v2::HostV2;
use agora_host::v2::persistence::{HostCheckpointScopeV1, HostRecoveryLimitsV2};
use agora_host::{HostLimits, HostTimeV1, PrincipalVerifier, VerifiedSubjectV1};
use agora_service::v2::persistence::{ServiceCheckpointScopeV1, ServiceRecoveryLimitsV2};
use agora_service::v2::{ServiceLimitsV2, ServiceV2};
use libfuzzer_sys::fuzz_target;

const MAX_CHECKPOINT: usize = 2 * 1_024 * 1_024;
const SERVICE_SCOPE: ServiceCheckpointScopeV1 = ServiceCheckpointScopeV1::new(*b"agora-test-scope");
const HOST_SCOPE: HostCheckpointScopeV1 = HostCheckpointScopeV1::new(*b"host-test-scope!");

struct InertVerifier;

impl PrincipalVerifier for InertVerifier {
    type Context = ();
    type Error = ();

    fn authenticate(
        &mut self,
        _claimed_participant: &str,
        _context: &Self::Context,
        _now: HostTimeV1,
    ) -> Result<VerifiedSubjectV1, Self::Error> {
        Err(())
    }

    fn revalidate(
        &mut self,
        _subject: VerifiedSubjectV1,
        _participant: &str,
        _context: &Self::Context,
        _now: HostTimeV1,
    ) -> Result<(), Self::Error> {
        Err(())
    }
}

fn service_bounds() -> ServiceRecoveryLimitsV2 {
    ServiceRecoveryLimitsV2 {
        max_snapshot_bytes: MAX_CHECKPOINT,
        max_engine_snapshot_bytes: MAX_CHECKPOINT,
        maximum_service_limits: ServiceLimitsV2::default(),
        maximum_engine_limits: Limits::default(),
    }
}

fuzz_target!(|input: &[u8]| {
    if input.is_empty() || input.len() > MAX_CHECKPOINT {
        return;
    }

    if let Ok(engine) = Engine::recover_checkpoint(
        input,
        RecoveryLimits {
            max_snapshot_bytes: MAX_CHECKPOINT,
            maximum_engine_limits: Limits::default(),
        },
    ) {
        let encoded = engine
            .prepare_checkpoint(MAX_CHECKPOINT)
            .expect("recovered engine must encode");
        assert_eq!(
            encoded.bytes(),
            input,
            "accepted engine checkpoint was not canonical"
        );
    }

    if let Ok(service) = ServiceV2::recover_checkpoint(input, SERVICE_SCOPE, service_bounds()) {
        let encoded = service
            .prepare_checkpoint(SERVICE_SCOPE, MAX_CHECKPOINT)
            .expect("recovered service must encode");
        let current =
            ServiceV2::recover_checkpoint(encoded.bytes(), SERVICE_SCOPE, service_bounds()).expect(
                "accepted service checkpoint must migrate to a recoverable canonical record",
            );
        let stable = current
            .prepare_checkpoint(SERVICE_SCOPE, MAX_CHECKPOINT)
            .expect("migrated service checkpoint must encode");
        assert_eq!(
            stable.bytes(),
            encoded.bytes(),
            "accepted service checkpoint did not stabilize after migration"
        );
    }

    let host_bounds = HostRecoveryLimitsV2 {
        max_snapshot_bytes: MAX_CHECKPOINT,
        max_service_snapshot_bytes: MAX_CHECKPOINT,
        maximum_host_limits: HostLimits::default(),
        service: service_bounds(),
    };
    if let Ok(host) = HostV2::recover_checkpoint(input, HOST_SCOPE, host_bounds, InertVerifier) {
        let encoded = host
            .prepare_checkpoint(HOST_SCOPE, MAX_CHECKPOINT)
            .expect("recovered host must encode");
        let current =
            HostV2::recover_checkpoint(encoded.bytes(), HOST_SCOPE, host_bounds, InertVerifier)
                .expect("accepted host checkpoint must migrate to a recoverable canonical record");
        let stable = current
            .prepare_checkpoint(HOST_SCOPE, MAX_CHECKPOINT)
            .expect("migrated host checkpoint must encode");
        assert_eq!(
            stable.bytes(),
            encoded.bytes(),
            "accepted host checkpoint did not stabilize after migration"
        );
    }
});
