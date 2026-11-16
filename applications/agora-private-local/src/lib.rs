//! Standalone private-local Agora application composition.
//!
//! This application root owns deployment choices that do not belong in the
//! engine, daemon transport, or Workbench. The first closed boundary is
//! bounded create-time state plus public Aegis connection and same-session
//! grant selection. Durable create/open and the process shell compose here in
//! later slices.

use aegis_local_client::{ClientErrorV1, LimitsV1 as AegisWireLimitsV1, UnixPort};
use aegis_protocol::{GrantHandleV1, SessionHandleV1};
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
    ) -> Result<GrantHandleV1, Self::Error>;
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
        .select(session, &mut port)
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
