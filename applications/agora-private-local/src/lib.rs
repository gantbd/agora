//! Standalone private-local Agora application composition.
//!
//! This application root owns deployment choices that do not belong in the
//! engine, daemon transport, or Workbench. The first closed boundary is
//! bounded create-time state plus public Aegis connection and narrow
//! same-session grant provisioning. Durable create/open and the process shell
//! compose here in later slices.

use aegis_client::{AuthorityClient, ServicePort};
use aegis_local_client::{ClientErrorV1, LimitsV1 as AegisWireLimitsV1, UnixPort};
use aegis_protocol::{
    ActionV1, EngineNamespaceV1, GrantHandleV1, OutcomeV1, RefusalV1, ReplyV1, ScopeV1,
    SessionHandleV1,
};
use agora_aegis_verifier::{AegisContextV1, AegisVerifierV1};
use agora_engine::{
    AdmissionPolicy, CommandV2, Engine, Limits, Participant, ParticipantId, ParticipantKind,
    Refusal, RoomId,
};
use agora_host::VerifiedSubjectV1;
use std::{path::Path, time::Duration};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapParticipantV1 {
    pub id: ParticipantId,
    pub display_name: String,
    pub kind: ParticipantKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapRoomV1 {
    pub id: RoomId,
    pub name: String,
    pub owner: ParticipantId,
    pub admission: AdmissionPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapV1 {
    pub principal: ParticipantId,
    pub participants: Vec<BootstrapParticipantV1>,
    pub rooms: Vec<BootstrapRoomV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootstrapStageV1 {
    Participant(usize),
    Room(usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BootstrapRefusalV1 {
    NoParticipants,
    ParticipantLimit,
    RoomLimit,
    PrincipalNotRegistered,
    Engine {
        stage: BootstrapStageV1,
        refusal: Refusal,
    },
}

impl BootstrapV1 {
    /// Builds the exact initial aggregate before any endpoint or durable owner
    /// is published. A refusal returns no partially initialized engine.
    pub fn build_engine(self, limits: Limits) -> Result<Engine, BootstrapRefusalV1> {
        if self.participants.is_empty() {
            return Err(BootstrapRefusalV1::NoParticipants);
        }
        if self.participants.len() > limits.max_participants {
            return Err(BootstrapRefusalV1::ParticipantLimit);
        }
        if self.rooms.len() > limits.max_rooms {
            return Err(BootstrapRefusalV1::RoomLimit);
        }
        if !self
            .participants
            .iter()
            .any(|participant| participant.id == self.principal)
        {
            return Err(BootstrapRefusalV1::PrincipalNotRegistered);
        }

        let mut engine = Engine::with_limits(limits);
        for (index, participant) in self.participants.into_iter().enumerate() {
            engine
                .apply(CommandV2::RegisterParticipant(Participant::new(
                    participant.id,
                    participant.display_name,
                    participant.kind,
                )))
                .map_err(|refusal| BootstrapRefusalV1::Engine {
                    stage: BootstrapStageV1::Participant(index),
                    refusal,
                })?;
        }
        for (index, room) in self.rooms.into_iter().enumerate() {
            engine
                .apply(CommandV2::CreateRoom {
                    room_id: room.id,
                    name: room.name,
                    owner: room.owner,
                    admission: room.admission,
                })
                .map_err(|refusal| BootstrapRefusalV1::Engine {
                    stage: BootstrapStageV1::Room(index),
                    refusal,
                })?;
        }
        Ok(engine)
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct AuthoritySelectionV1 {
    participant: ParticipantId,
    subject: [u8; 16],
    resource: [u8; 16],
}

impl AuthoritySelectionV1 {
    pub const fn new(participant: ParticipantId, subject: [u8; 16], resource: [u8; 16]) -> Self {
        Self {
            participant,
            subject,
            resource,
        }
    }
}

impl std::fmt::Debug for AuthoritySelectionV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuthoritySelectionV1")
            .field("participant_bytes", &self.participant.as_str().len())
            .field("subject", &"<redacted>")
            .field("resource", &"<redacted>")
            .finish()
    }
}

#[derive(Debug)]
pub enum AuthorityConnectErrorV1<E> {
    PrincipalMismatch,
    Connect(ClientErrorV1),
    Grant(E),
}

/// Deployment-owned policy for provisioning or selecting one grant on the
/// exact newly connected Aegis service session. Handles never cross sessions.
pub trait SessionGrantSelectorV1 {
    type Error;

    fn select(
        &mut self,
        session: SessionHandleV1,
        port: &mut UnixPort,
        resource: [u8; 16],
    ) -> Result<GrantHandleV1, Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgoraGrantProfileV1 {
    Conversation,
    ScheduledComposition,
}

impl AgoraGrantProfileV1 {
    const fn actions(self) -> u8 {
        match self {
            Self::Conversation => 1 << ActionV1::Submit as u8,
            Self::ScheduledComposition => {
                (1 << ActionV1::Submit as u8)
                    | (1 << ActionV1::Execute as u8)
                    | (1 << ActionV1::Observe as u8)
            }
        }
    }
}

/// Concrete private-local policy that issues one narrowly scoped Agora grant
/// on the exact newly connected Aegis session. The deployment must map that
/// connection to an Aegis administrator and explicitly supply epoch and time.
pub struct IssueAgoraGrantV1 {
    expected_epoch: u64,
    now: u64,
    expires_at: u64,
    profile: AgoraGrantProfileV1,
}

impl IssueAgoraGrantV1 {
    pub const fn new(
        expected_epoch: u64,
        now: u64,
        expires_at: u64,
        profile: AgoraGrantProfileV1,
    ) -> Self {
        Self {
            expected_epoch,
            now,
            expires_at,
            profile,
        }
    }

    fn issue<P: ServicePort>(
        &self,
        session: SessionHandleV1,
        port: P,
        resource: [u8; 16],
    ) -> Result<GrantHandleV1, IssueAgoraGrantErrorV1<P::Error>> {
        if self.expires_at <= self.now {
            return Err(IssueAgoraGrantErrorV1::InvalidLifetime);
        }
        let mut authority = AuthorityClient::new(session, port);
        let reply = authority
            .issue(
                self.expected_epoch,
                self.now,
                self.expires_at,
                vec![ScopeV1 {
                    engine: EngineNamespaceV1::Agora,
                    resource,
                    actions: self.profile.actions(),
                }],
            )
            .map_err(IssueAgoraGrantErrorV1::Transport)?;
        match reply {
            ReplyV1::Accepted(OutcomeV1::GrantIssued(summary))
                if summary.parent.is_none()
                    && summary.epoch == self.expected_epoch
                    && summary.expires_at == self.expires_at
                    && summary.scope_count == 1
                    && summary.delegation_depth == 0 =>
            {
                Ok(summary.handle)
            }
            ReplyV1::Refused(refusal) => Err(IssueAgoraGrantErrorV1::Refused(refusal)),
            ReplyV1::Accepted(_) => Err(IssueAgoraGrantErrorV1::InexactReply),
        }
    }
}

impl std::fmt::Debug for IssueAgoraGrantV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IssueAgoraGrantV1")
            .field("profile", &self.profile)
            .field("authority_inputs", &"<redacted>")
            .finish()
    }
}

pub enum IssueAgoraGrantErrorV1<E> {
    InvalidLifetime,
    Transport(E),
    Refused(RefusalV1),
    InexactReply,
}

impl<E> std::fmt::Debug for IssueAgoraGrantErrorV1<E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidLifetime => formatter.write_str("InvalidLifetime"),
            Self::Transport(_) => formatter.write_str("Transport(<redacted>)"),
            Self::Refused(refusal) => formatter.debug_tuple("Refused").field(refusal).finish(),
            Self::InexactReply => formatter.write_str("InexactReply"),
        }
    }
}

impl SessionGrantSelectorV1 for IssueAgoraGrantV1 {
    type Error = IssueAgoraGrantErrorV1<ClientErrorV1>;

    fn select(
        &mut self,
        session: SessionHandleV1,
        port: &mut UnixPort,
        resource: [u8; 16],
    ) -> Result<GrantHandleV1, Self::Error> {
        self.issue(session, port, resource)
    }
}

/// One live public Aegis connection plus the exact Agora context bound to its
/// server-selected session and deployment-selected session-local grant.
pub struct ConnectedAuthorityV1 {
    context: AegisContextV1,
    verifier: AegisVerifierV1<UnixPort>,
}

impl ConnectedAuthorityV1 {
    pub fn into_parts(self) -> (AegisContextV1, AegisVerifierV1<UnixPort>) {
        (self.context, self.verifier)
    }
}

impl std::fmt::Debug for ConnectedAuthorityV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ConnectedAuthorityV1")
            .field("context", &"<redacted>")
            .field("verifier", &"<redacted>")
            .finish()
    }
}

/// Connects only after the deployment-selected authority participant matches
/// the bootstrap principal. This function discovers no resource and owns no
/// grant policy. Provisioning/selection remains an explicit deployment input
/// and runs only on the exact server-selected session.
pub fn connect_authority_v1<G: SessionGrantSelectorV1>(
    socket: impl AsRef<Path>,
    limits: AegisWireLimitsV1,
    io_timeout: Duration,
    bootstrap_principal: &ParticipantId,
    selection: AuthoritySelectionV1,
    grant_selector: &mut G,
) -> Result<ConnectedAuthorityV1, AuthorityConnectErrorV1<G::Error>> {
    if &selection.participant != bootstrap_principal {
        return Err(AuthorityConnectErrorV1::PrincipalMismatch);
    }
    let (session, mut port) = aegis_local_client::connect(socket, limits, io_timeout)
        .map_err(AuthorityConnectErrorV1::Connect)?;
    let grant = grant_selector
        .select(session, &mut port, selection.resource)
        .map_err(AuthorityConnectErrorV1::Grant)?;
    let context = AegisContextV1::new(
        selection.participant.as_str(),
        VerifiedSubjectV1::from_verifier(selection.subject),
        session,
        grant,
    );
    Ok(ConnectedAuthorityV1 {
        context,
        verifier: AegisVerifierV1::new(port, selection.resource),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use aegis_protocol::{CommandV1, GrantSummaryV1};

    struct Port {
        reply: Result<ReplyV1, &'static str>,
        calls: Vec<(SessionHandleV1, CommandV1)>,
    }

    impl Port {
        fn returning(reply: ReplyV1) -> Self {
            Self {
                reply: Ok(reply),
                calls: Vec::new(),
            }
        }
    }

    impl ServicePort for &mut Port {
        type Error = &'static str;

        fn execute(
            &mut self,
            session: SessionHandleV1,
            command: CommandV1,
        ) -> Result<ReplyV1, Self::Error> {
            self.calls.push((session, command));
            self.reply.clone()
        }
    }

    fn issued(handle: u64, epoch: u64, expires_at: u64) -> ReplyV1 {
        ReplyV1::Accepted(OutcomeV1::GrantIssued(GrantSummaryV1 {
            handle: GrantHandleV1(handle),
            parent: None,
            epoch,
            expires_at,
            scope_count: 1,
            delegation_depth: 0,
        }))
    }

    #[test]
    fn conversation_issues_exact_session_resource_and_submit_scope() {
        let selector = IssueAgoraGrantV1::new(4, 10, 20, AgoraGrantProfileV1::Conversation);
        let mut port = Port::returning(issued(9, 4, 20));

        assert_eq!(
            selector
                .issue(SessionHandleV1(7), &mut port, [3; 16])
                .unwrap(),
            GrantHandleV1(9)
        );
        assert_eq!(port.calls.len(), 1);
        assert_eq!(port.calls[0].0, SessionHandleV1(7));
        assert_eq!(
            port.calls[0].1,
            CommandV1::Issue {
                expected_epoch: 4,
                now: 10,
                expires_at: 20,
                scopes: vec![ScopeV1 {
                    engine: EngineNamespaceV1::Agora,
                    resource: [3; 16],
                    actions: 1 << ActionV1::Submit as u8,
                }],
            }
        );
    }

    #[test]
    fn scheduled_profile_adds_only_execute_and_observe() {
        let selector = IssueAgoraGrantV1::new(4, 10, 20, AgoraGrantProfileV1::ScheduledComposition);
        let mut port = Port::returning(issued(9, 4, 20));

        selector
            .issue(SessionHandleV1(7), &mut port, [3; 16])
            .unwrap();
        let CommandV1::Issue { scopes, .. } = &port.calls[0].1 else {
            panic!("expected issue")
        };
        assert_eq!(
            scopes[0].actions,
            (1 << ActionV1::Submit as u8)
                | (1 << ActionV1::Execute as u8)
                | (1 << ActionV1::Observe as u8)
        );
    }

    #[test]
    fn invalid_lifetime_refuses_without_authority_io() {
        let selector = IssueAgoraGrantV1::new(4, 20, 20, AgoraGrantProfileV1::Conversation);
        let mut port = Port::returning(issued(9, 4, 20));

        assert!(matches!(
            selector.issue(SessionHandleV1(7), &mut port, [3; 16]),
            Err(IssueAgoraGrantErrorV1::InvalidLifetime)
        ));
        assert!(port.calls.is_empty());
    }

    #[test]
    fn refusal_transport_and_inexact_success_remain_distinct_and_redacted() {
        let selector = IssueAgoraGrantV1::new(4, 10, 20, AgoraGrantProfileV1::Conversation);
        let mut refused = Port::returning(ReplyV1::Refused(RefusalV1::NotAdministrator));
        assert!(matches!(
            selector.issue(SessionHandleV1(7), &mut refused, [3; 16]),
            Err(IssueAgoraGrantErrorV1::Refused(RefusalV1::NotAdministrator))
        ));

        let mut inexact = Port::returning(issued(9, 5, 20));
        assert!(matches!(
            selector.issue(SessionHandleV1(7), &mut inexact, [3; 16]),
            Err(IssueAgoraGrantErrorV1::InexactReply)
        ));

        let mut failed = Port {
            reply: Err("private transport detail"),
            calls: Vec::new(),
        };
        let error = selector
            .issue(SessionHandleV1(7), &mut failed, [3; 16])
            .unwrap_err();
        let rendered = format!("{error:?}");
        assert!(matches!(error, IssueAgoraGrantErrorV1::Transport(_)));
        assert!(!rendered.contains("private transport detail"));
    }
}
