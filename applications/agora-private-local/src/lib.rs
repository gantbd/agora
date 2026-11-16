//! Standalone private-local Agora application composition.
//!
//! This application root owns deployment choices that do not belong in the
//! engine, daemon transport, or Workbench. The first closed boundary is
//! bounded create-time state. Durable create/open, Aegis selection, and the
//! process shell compose here in later slices.

use agora_engine::{
    AdmissionPolicy, CommandV2, Engine, Limits, Participant, ParticipantId, ParticipantKind,
    Refusal, RoomId,
};

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
