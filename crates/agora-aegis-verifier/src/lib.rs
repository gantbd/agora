//! A narrow Aegis authorization adapter for Agora's trusted host verifier port.
//!
//! The adapter authorizes `Submit` on one configured Agora resource through an
//! Aegis public service session and opaque grant handle. This admits use of an
//! Agora host session; Agora room membership and roles still authorize each
//! conversation operation. Credentials, grant issuance, and transport remain
//! outside this crate.

use aegis_client::ServicePort;
use aegis_protocol::{
    ActionV1, CommandV1, DecisionSummaryV1, EngineNamespaceV1, GrantHandleV1, OutcomeV1, ReplyV1,
    SessionHandleV1,
};
use agora_durable::{OperatorRestoreAuthorityV1, OperatorRestoreRequestV1};
use agora_host::v2::persistence::HostCheckpointScopeV1;
use agora_host::{HostTimeV1, PrincipalVerifier, VerifiedSubjectV1};

#[derive(Clone, Eq, PartialEq)]
pub struct AegisContextV1 {
    participant_id: String,
    subject: VerifiedSubjectV1,
    session: SessionHandleV1,
    grant: GrantHandleV1,
}

impl AegisContextV1 {
    pub fn new(
        participant_id: impl Into<String>,
        subject: VerifiedSubjectV1,
        session: SessionHandleV1,
        grant: GrantHandleV1,
    ) -> Self {
        Self {
            participant_id: participant_id.into(),
            subject,
            session,
            grant,
        }
    }
}

impl std::fmt::Debug for AegisContextV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AegisContextV1")
            .field("participant_id_bytes", &self.participant_id.len())
            .field("subject", &self.subject)
            .field("session", &"<redacted>")
            .field("grant", &"<redacted>")
            .finish()
    }
}

/// Aegis credentials plus the trusted time used for one operator restore
/// authorization decision.
#[derive(Clone, Eq, PartialEq)]
pub struct AegisOperatorRestoreContextV1 {
    authority: AegisContextV1,
    now: HostTimeV1,
}

impl AegisOperatorRestoreContextV1 {
    pub const fn new(authority: AegisContextV1, now: HostTimeV1) -> Self {
        Self { authority, now }
    }
}

impl std::fmt::Debug for AegisOperatorRestoreContextV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AegisOperatorRestoreContextV1")
            .field("authority", &self.authority)
            .field("now", &self.now)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AegisVerifierErrorV1 {
    Unavailable,
}

pub struct AegisVerifierV1<P> {
    port: P,
    resource: [u8; 16],
}

impl<P> AegisVerifierV1<P> {
    pub const fn new(port: P, resource: [u8; 16]) -> Self {
        Self { port, resource }
    }

    pub const fn port(&self) -> &P {
        &self.port
    }

    pub fn port_mut(&mut self) -> &mut P {
        &mut self.port
    }

    pub fn into_port(self) -> P {
        self.port
    }
}

impl<P> std::fmt::Debug for AegisVerifierV1<P> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AegisVerifierV1")
            .field("port", &"<redacted>")
            .field("resource", &"<redacted>")
            .finish()
    }
}

impl<P: ServicePort> AegisVerifierV1<P> {
    fn authorize(
        &mut self,
        context: &AegisContextV1,
        now: HostTimeV1,
    ) -> Result<(), AegisVerifierErrorV1> {
        let reply = self
            .port
            .execute(
                context.session,
                CommandV1::Authorize {
                    grant: context.grant,
                    now: now.0,
                    engine: EngineNamespaceV1::Agora,
                    resource: self.resource,
                    action: ActionV1::Submit,
                },
            )
            .map_err(|_| AegisVerifierErrorV1::Unavailable)?;
        match reply {
            ReplyV1::Accepted(OutcomeV1::Authorized(decision))
                if decision.engine == EngineNamespaceV1::Agora
                    && decision.action == ActionV1::Submit
                    && decision.expires_at > now.0 =>
            {
                Ok(())
            }
            _ => Err(AegisVerifierErrorV1::Unavailable),
        }
    }
}

impl<P: ServicePort> PrincipalVerifier for AegisVerifierV1<P> {
    type Context = AegisContextV1;
    type Error = AegisVerifierErrorV1;

    fn authenticate(
        &mut self,
        claimed_participant: &str,
        context: &Self::Context,
        now: HostTimeV1,
    ) -> Result<VerifiedSubjectV1, Self::Error> {
        if claimed_participant != context.participant_id {
            return Err(AegisVerifierErrorV1::Unavailable);
        }
        self.authorize(context, now)?;
        Ok(context.subject)
    }

    fn revalidate(
        &mut self,
        subject: VerifiedSubjectV1,
        participant: &str,
        context: &Self::Context,
        now: HostTimeV1,
    ) -> Result<(), Self::Error> {
        if subject != context.subject || participant != context.participant_id {
            return Err(AegisVerifierErrorV1::Unavailable);
        }
        self.authorize(context, now)
    }
}

/// Aegis-backed authority for sanitizing one exact durable Host target after
/// an external operator restore.
///
/// Aegis decides `Administer` on the configured resource. The adapter also
/// rejects a mismatched Agora key or checkpoint scope before contacting Aegis;
/// Agora's durable boundary binds the accepted decision to the loaded revision
/// with its subsequent exact compare-and-exchange.
pub struct AegisOperatorRestoreAuthorityV1<P> {
    port: P,
    resource: [u8; 16],
    key: String,
    scope: HostCheckpointScopeV1,
    evidence: Option<AegisOperatorRestoreEvidenceV1>,
}

/// Copyable cross-engine evidence that one Aegis decision authorized
/// sanitation of one loaded Agora revision.
///
/// This is correlation evidence, not evidence of an atomic Aegis/Agora
/// transaction. The durable Agora CAS remains the activation boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AegisOperatorRestoreEvidenceV1 {
    pub decision: DecisionSummaryV1,
    pub agora_revision: u64,
}

impl<P> AegisOperatorRestoreAuthorityV1<P> {
    pub fn new(
        port: P,
        resource: [u8; 16],
        key: impl Into<String>,
        scope: HostCheckpointScopeV1,
    ) -> Self {
        Self {
            port,
            resource,
            key: key.into(),
            scope,
            evidence: None,
        }
    }

    pub const fn port(&self) -> &P {
        &self.port
    }

    pub fn port_mut(&mut self) -> &mut P {
        &mut self.port
    }

    pub fn into_port(self) -> P {
        self.port
    }

    pub const fn evidence(&self) -> Option<AegisOperatorRestoreEvidenceV1> {
        self.evidence
    }

    pub fn take_evidence(&mut self) -> Option<AegisOperatorRestoreEvidenceV1> {
        self.evidence.take()
    }
}

impl<P> std::fmt::Debug for AegisOperatorRestoreAuthorityV1<P> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AegisOperatorRestoreAuthorityV1")
            .field("port", &"<redacted>")
            .field("resource", &"<redacted>")
            .field("key_bytes", &self.key.len())
            .field("scope", &self.scope)
            .finish()
    }
}

impl<P: ServicePort> OperatorRestoreAuthorityV1 for AegisOperatorRestoreAuthorityV1<P> {
    type Context = AegisOperatorRestoreContextV1;
    type Error = AegisVerifierErrorV1;

    fn authorize(
        &mut self,
        request: &OperatorRestoreRequestV1<'_>,
        context: &Self::Context,
    ) -> Result<(), Self::Error> {
        self.evidence = None;
        if request.key() != self.key || request.scope() != self.scope {
            return Err(AegisVerifierErrorV1::Unavailable);
        }
        let reply = self
            .port
            .execute(
                context.authority.session,
                CommandV1::Authorize {
                    grant: context.authority.grant,
                    now: context.now.0,
                    engine: EngineNamespaceV1::Agora,
                    resource: self.resource,
                    action: ActionV1::Administer,
                },
            )
            .map_err(|_| AegisVerifierErrorV1::Unavailable)?;
        match reply {
            ReplyV1::Accepted(OutcomeV1::Authorized(decision))
                if decision.engine == EngineNamespaceV1::Agora
                    && decision.action == ActionV1::Administer
                    && decision.expires_at > context.now.0 =>
            {
                self.evidence = Some(AegisOperatorRestoreEvidenceV1 {
                    decision,
                    agora_revision: request.revision().0,
                });
                Ok(())
            }
            _ => Err(AegisVerifierErrorV1::Unavailable),
        }
    }
}
