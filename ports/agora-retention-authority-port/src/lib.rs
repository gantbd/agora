//! Injected administrative authority for Agora retention and exact export.
//!
//! The port decides current authority only. It performs no retention transition,
//! export materialization, store publication, reclamation, or compliance policy.

use agora_engine::retention::{
    ExportProfileV1, ExportRequestIdV1, ExportScopeBindingV1, RetentionPolicyRevisionV1,
    RetentionPolicyV1,
};
use std::fmt;

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AdministrativePrincipalBindingV1([u8; 32]);

impl AdministrativePrincipalBindingV1 {
    pub fn new(bytes: [u8; 32]) -> Option<Self> {
        (bytes != [0; 32]).then_some(Self(bytes))
    }

    pub fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Debug for AdministrativePrincipalBindingV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AdministrativePrincipalBindingV1(<redacted>)")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PolicyAuthorityRequestV1 {
    pub principal: AdministrativePrincipalBindingV1,
    pub observed_engine_revision: u64,
    pub current_policy_revision: Option<RetentionPolicyRevisionV1>,
    pub proposed_policy: RetentionPolicyV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExportAuthorityRequestV1 {
    pub principal: AdministrativePrincipalBindingV1,
    pub request: ExportRequestIdV1,
    pub profile: ExportProfileV1,
    pub scope: ExportScopeBindingV1,
    pub source_engine_revision: u64,
    pub policy_revision: RetentionPolicyRevisionV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorityDecisionV1 {
    Authorized,
    Denied,
    Stale,
    Unavailable,
}

/// A configured verifier. Callers provide observations, never proof of
/// authority; implementations revalidate current policy for every call.
pub trait RetentionAuthorityPortV1 {
    fn authorize_policy(&mut self, request: PolicyAuthorityRequestV1) -> AuthorityDecisionV1;

    fn authorize_export(&mut self, request: ExportAuthorityRequestV1) -> AuthorityDecisionV1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use agora_engine::retention::RetentionBoundV1;

    #[test]
    fn inputs_bind_exact_revision_scope_and_redact_principal() {
        let principal = AdministrativePrincipalBindingV1::new([1; 32]).unwrap();
        let revision = RetentionPolicyRevisionV1::new(3).unwrap();
        let policy =
            RetentionPolicyV1::new(revision, 8, [RetentionBoundV1::entries(4).unwrap(); 6]);
        let policy_request = PolicyAuthorityRequestV1 {
            principal,
            observed_engine_revision: 8,
            current_policy_revision: Some(RetentionPolicyRevisionV1::new(2).unwrap()),
            proposed_policy: policy,
        };
        let export_request = ExportAuthorityRequestV1 {
            principal,
            request: ExportRequestIdV1::new([2; 16]).unwrap(),
            profile: ExportProfileV1::Administrative,
            scope: ExportScopeBindingV1::new([3; 32]).unwrap(),
            source_engine_revision: 8,
            policy_revision: revision,
        };
        let diagnostic = format!("{policy_request:?} {export_request:?}");
        assert!(!diagnostic.contains("[1, 1"));
        assert!(AdministrativePrincipalBindingV1::new([0; 32]).is_none());
    }
}
