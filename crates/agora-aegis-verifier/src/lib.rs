//! A narrow Aegis authorization adapter for Agora's trusted host verifier port.
//!
//! The adapter authorizes `Submit` on one configured Agora resource through an
//! Aegis public service session and opaque grant handle. This admits use of an
//! Agora host session; Agora room membership and roles still authorize each
//! conversation operation. Credentials, grant issuance, and transport remain
//! outside this crate.

use aegis_client::ServicePort;
use aegis_protocol::{
    ActionV1, CommandV1, EngineNamespaceV1, GrantHandleV1, OutcomeV1, ReplyV1, SessionHandleV1,
};
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
