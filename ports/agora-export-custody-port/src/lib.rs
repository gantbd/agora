//! Effect-free port for admitting one authorized materialization into separate
//! export custody.
//!
//! The port carries current Agora revision observations and the exact prepared
//! materialization. It does not authorize, project source records, implement a
//! digest, select storage, or define participant protocol.

pub use agora_engine::retention::ExportMaterializationV1;
use agora_engine::retention::RetentionPolicyRevisionV1;
use std::fmt;

#[derive(Clone, Eq, PartialEq)]
pub struct ExportCustodyRequestV1 {
    pub current_engine_revision: u64,
    pub current_policy_revision: RetentionPolicyRevisionV1,
    materialization: ExportMaterializationV1,
}

impl ExportCustodyRequestV1 {
    pub fn new(
        current_engine_revision: u64,
        current_policy_revision: RetentionPolicyRevisionV1,
        materialization: ExportMaterializationV1,
    ) -> Self {
        Self {
            current_engine_revision,
            current_policy_revision,
            materialization,
        }
    }

    pub const fn materialization(&self) -> &ExportMaterializationV1 {
        &self.materialization
    }

    pub fn into_materialization(self) -> ExportMaterializationV1 {
        self.materialization
    }
}

impl fmt::Debug for ExportCustodyRequestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExportCustodyRequestV1")
            .field("current_engine_revision", &self.current_engine_revision)
            .field("current_policy_revision", &self.current_policy_revision)
            .field("request", &self.materialization.manifest.request)
            .field("profile", &self.materialization.manifest.profile)
            .field("scope", &self.materialization.manifest.scope)
            .field("chunk_count", &self.materialization.chunks().len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportCustodyApplyV1 {
    Materialized,
    Replay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportCustodyDurabilityV1 {
    PreviouslyCommitted { store_revision: u64 },
    Committed { store_revision: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExportCustodyAcceptedV1 {
    pub apply: ExportCustodyApplyV1,
    pub durability: ExportCustodyDurabilityV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportCustodyRefusalV1 {
    Invalid,
    Stale,
    Conflict,
    Capacity,
    DefinitelyNotCommitted,
    OutcomeUnknown,
    Fenced,
    Unavailable,
}

pub trait ExportCustodyPortV1 {
    fn admit(
        &mut self,
        request: ExportCustodyRequestV1,
    ) -> Result<ExportCustodyAcceptedV1, ExportCustodyRefusalV1>;
}
