//! Serialized volatile service profile for Agora.

use agora_engine::{
    CommandV2 as EngineCommand, Engine, ParticipantId, ParticipantKind, Refusal as EngineRefusal,
    Revision, RevisionedCommandV2, Role, RoomId,
};
use agora_protocol::{
    ClientIdV1, CommandV1, MemberV1, MessageV1, OutcomeV1, ParticipantKindV1, RefusalV1, ReplyV1,
    RequestIdV1, RequestV1, RoleV1, RoomSnapshotV1,
};
use std::collections::{BTreeMap, VecDeque};

pub mod v2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceLimits {
    pub max_clients: usize,
    pub max_retired_clients: usize,
    pub max_retained_requests: usize,
    pub max_participant_id_bytes: usize,
    pub max_room_id_bytes: usize,
    pub max_message_body_bytes: usize,
    pub max_addressed_participants: usize,
    pub max_snapshot_members: usize,
    pub max_snapshot_messages: usize,
    pub max_snapshot_bytes: usize,
}

impl Default for ServiceLimits {
    fn default() -> Self {
        Self {
            max_clients: 64,
            max_retired_clients: 256,
            max_retained_requests: 4_096,
            max_participant_id_bytes: 128,
            max_room_id_bytes: 128,
            max_message_body_bytes: 16 * 1_024,
            max_addressed_participants: 64,
            max_snapshot_members: 256,
            max_snapshot_messages: 1_024,
            max_snapshot_bytes: 4 * 1_024 * 1_024,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Usage {
    pub connect_offers: u64,
    pub connected: u64,
    pub request_offers: u64,
    pub accepted: u64,
    pub refused: u64,
    pub replayed: u64,
    pub conflicts: u64,
    pub disconnected: u64,
    pub overflowed: bool,
}

fn count(value: &mut u64, overflowed: &mut bool) {
    let old = *value;
    *value = value.saturating_add(1);
    *overflowed |= *value == old && old == u64::MAX;
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RetainedRequest {
    request: RequestV1,
    reply: ReplyV1,
}

#[derive(Debug)]
pub struct Service {
    engine: Engine,
    limits: ServiceLimits,
    clients: BTreeMap<ClientIdV1, ParticipantId>,
    retired_clients: VecDeque<ClientIdV1>,
    retained: BTreeMap<(ClientIdV1, RequestIdV1), RetainedRequest>,
    next_client: u64,
    usage: Usage,
}

impl Service {
    pub fn new(engine: Engine, limits: ServiceLimits) -> Self {
        Self {
            engine,
            limits,
            clients: BTreeMap::new(),
            retired_clients: VecDeque::new(),
            retained: BTreeMap::new(),
            next_client: 0,
            usage: Usage::default(),
        }
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn usage(&self) -> Usage {
        self.usage
    }

    pub fn connect(&mut self, participant: &str) -> Result<ClientIdV1, RefusalV1> {
        count(&mut self.usage.connect_offers, &mut self.usage.overflowed);
        if participant.is_empty() || participant.len() > self.limits.max_participant_id_bytes {
            return Err(RefusalV1::InvalidInput);
        }
        if self.clients.len() >= self.limits.max_clients {
            return Err(RefusalV1::ClientLimit);
        }
        let participant_id = ParticipantId::new(participant);
        if self.engine.participant(&participant_id).is_none() {
            return Err(RefusalV1::UnknownParticipant);
        }
        let id = ClientIdV1(self.next_client);
        self.next_client = self
            .next_client
            .checked_add(1)
            .ok_or(RefusalV1::Exhausted)?;
        self.clients.insert(id, participant_id);
        count(&mut self.usage.connected, &mut self.usage.overflowed);
        Ok(id)
    }

    pub fn disconnect(&mut self, client: ClientIdV1) -> Result<bool, RefusalV1> {
        if self.retired_clients.contains(&client) {
            return Ok(false);
        }
        if !self.clients.contains_key(&client) {
            return Err(RefusalV1::UnknownClient);
        }
        self.clients.remove(&client);
        if self.limits.max_retired_clients > 0 {
            if self.retired_clients.len() >= self.limits.max_retired_clients {
                self.retired_clients.pop_front();
            }
            self.retired_clients.push_back(client);
        }
        self.retained.retain(|(owner, _), _| *owner != client);
        count(&mut self.usage.disconnected, &mut self.usage.overflowed);
        Ok(true)
    }

    pub fn execute(&mut self, client: ClientIdV1, request: RequestV1) -> ReplyV1 {
        count(&mut self.usage.request_offers, &mut self.usage.overflowed);
        let Some(participant) = self.clients.get(&client).cloned() else {
            count(&mut self.usage.refused, &mut self.usage.overflowed);
            return ReplyV1::Refused(RefusalV1::UnknownClient);
        };
        let key = (client, request.request_id);
        if let Some(existing) = self.retained.get(&key) {
            if existing.request == request {
                count(&mut self.usage.replayed, &mut self.usage.overflowed);
                return existing.reply.clone();
            }
            count(&mut self.usage.conflicts, &mut self.usage.overflowed);
            return ReplyV1::Refused(RefusalV1::RequestConflict);
        }
        if !self.valid_command_shape(&request.command) {
            count(&mut self.usage.refused, &mut self.usage.overflowed);
            return ReplyV1::Refused(RefusalV1::InvalidInput);
        }
        if self.retained.len() >= self.limits.max_retained_requests {
            count(&mut self.usage.refused, &mut self.usage.overflowed);
            return ReplyV1::Refused(RefusalV1::RequestLimit);
        }
        let reply = self.execute_new(&participant, &request.command);
        match &reply {
            ReplyV1::Accepted(_) => count(&mut self.usage.accepted, &mut self.usage.overflowed),
            ReplyV1::Refused(_) => count(&mut self.usage.refused, &mut self.usage.overflowed),
        }
        self.retained.insert(
            key,
            RetainedRequest {
                request,
                reply: reply.clone(),
            },
        );
        reply
    }

    fn execute_new(&mut self, participant: &ParticipantId, command: &CommandV1) -> ReplyV1 {
        let result = match command {
            CommandV1::Snapshot { room_id } => {
                self.snapshot(participant, room_id).map(OutcomeV1::Snapshot)
            }
            CommandV1::Speak {
                room_id,
                expected_revision,
                logical_time,
                body,
                addressed_to,
            } => self.speak(
                participant,
                room_id,
                *expected_revision,
                *logical_time,
                body,
                addressed_to,
            ),
        };
        match result {
            Ok(outcome) => ReplyV1::Accepted(outcome),
            Err(refusal) => ReplyV1::Refused(refusal),
        }
    }

    fn valid_command_shape(&self, command: &CommandV1) -> bool {
        match command {
            CommandV1::Snapshot { room_id } => {
                !room_id.is_empty() && room_id.len() <= self.limits.max_room_id_bytes
            }
            CommandV1::Speak {
                room_id,
                body,
                addressed_to,
                ..
            } => {
                !room_id.is_empty()
                    && room_id.len() <= self.limits.max_room_id_bytes
                    && !body.is_empty()
                    && body.len() <= self.limits.max_message_body_bytes
                    && addressed_to.len() <= self.limits.max_addressed_participants
                    && addressed_to.iter().all(|participant| {
                        !participant.is_empty()
                            && participant.len() <= self.limits.max_participant_id_bytes
                    })
            }
        }
    }

    fn snapshot(
        &self,
        participant: &ParticipantId,
        room_id: &str,
    ) -> Result<RoomSnapshotV1, RefusalV1> {
        let room = self
            .engine
            .room(&RoomId::new(room_id))
            .ok_or(RefusalV1::UnknownRoom)?;
        if !room.members.iter().any(|(id, _)| id == participant) {
            return Err(RefusalV1::NotMember);
        }
        if room.members.len() > self.limits.max_snapshot_members
            || room.messages.len() > self.limits.max_snapshot_messages
        {
            return Err(RefusalV1::SnapshotLimit);
        }
        let members = room
            .members
            .into_iter()
            .map(|(id, role)| {
                let kind = self
                    .engine
                    .participant(&id)
                    .map(|participant| match participant.kind() {
                        ParticipantKind::Human => ParticipantKindV1::Human,
                        ParticipantKind::Agent => ParticipantKindV1::Agent,
                    })
                    .ok_or(RefusalV1::Unavailable)?;
                Ok(MemberV1 {
                    participant_id: id.as_str().to_owned(),
                    kind,
                    role: match role {
                        Role::Owner => RoleV1::Owner,
                        Role::Moderator => RoleV1::Moderator,
                        Role::Member => RoleV1::Member,
                    },
                })
            })
            .collect::<Result<Vec<_>, RefusalV1>>()?;
        let messages = room
            .messages
            .into_iter()
            .map(|message| MessageV1 {
                sequence: message.sequence(),
                author: message.author().as_str().to_owned(),
                logical_time: message.at().0,
                body: message.body().to_owned(),
                addressed_to: message
                    .addressed_to()
                    .iter()
                    .map(|id| id.as_str().to_owned())
                    .collect(),
            })
            .collect::<Vec<_>>();
        let estimated = snapshot_size(room_id, &room.name, &members, &messages)
            .ok_or(RefusalV1::SnapshotLimit)?;
        if estimated > self.limits.max_snapshot_bytes {
            return Err(RefusalV1::SnapshotLimit);
        }
        Ok(RoomSnapshotV1 {
            revision: self.engine.revision().0,
            room_id: room.id.as_str().to_owned(),
            name: room.name,
            members,
            messages,
            next_message_sequence: room.next_message_sequence,
        })
    }

    fn speak(
        &mut self,
        participant: &ParticipantId,
        room_id: &str,
        expected_revision: u64,
        logical_time: u64,
        body: &str,
        addressed_to: &[String],
    ) -> Result<OutcomeV1, RefusalV1> {
        let command = if addressed_to.is_empty() {
            EngineCommand::Speak {
                room_id: room_id.into(),
                author: participant.clone(),
                at: agora_engine::LogicalTime(logical_time),
                body: body.to_owned(),
            }
        } else {
            EngineCommand::SpeakAddressed {
                room_id: room_id.into(),
                author: participant.clone(),
                at: agora_engine::LogicalTime(logical_time),
                body: body.to_owned(),
                addressed_to: addressed_to
                    .iter()
                    .map(|id| ParticipantId::new(id.clone()))
                    .collect(),
            }
        };
        let transition = self
            .engine
            .apply_revisioned(RevisionedCommandV2 {
                expected_revision: Revision(expected_revision),
                command,
            })
            .map_err(map_refusal)?;
        let agora_engine::OutcomeV2::MessageAccepted { sequence, .. } = transition.outcome else {
            return Err(RefusalV1::Unavailable);
        };
        Ok(OutcomeV1::MessageAccepted {
            revision: transition.revision.0,
            sequence,
        })
    }
}

fn snapshot_size(
    room_id: &str,
    room_name: &str,
    members: &[MemberV1],
    messages: &[MessageV1],
) -> Option<usize> {
    let mut total = room_id.len().checked_add(room_name.len())?;
    for member in members {
        total = total.checked_add(member.participant_id.len())?;
    }
    for message in messages {
        total = total.checked_add(message.author.len())?;
        total = total.checked_add(message.body.len())?;
        for addressed in &message.addressed_to {
            total = total.checked_add(addressed.len())?;
        }
    }
    Some(total)
}

fn map_refusal(refusal: EngineRefusal) -> RefusalV1 {
    match refusal {
        EngineRefusal::RevisionConflict { actual, .. } => RefusalV1::Stale {
            actual_revision: actual.0,
        },
        EngineRefusal::UnknownRoom(_) => RefusalV1::UnknownRoom,
        EngineRefusal::NotMember(_) => RefusalV1::NotMember,
        EngineRefusal::NotAuthorized(_)
        | EngineRefusal::InvitationRequired(_)
        | EngineRefusal::OwnerCannotLeave => RefusalV1::NotAuthorized,
        EngineRefusal::ParticipantLimit
        | EngineRefusal::RoomLimit
        | EngineRefusal::MemberLimit
        | EngineRefusal::InvitationLimit
        | EngineRefusal::MessageRetentionDisabled
        | EngineRefusal::AddressedParticipantLimit
        | EngineRefusal::EventOutboxFull => RefusalV1::Capacity,
        EngineRefusal::MessageSequenceExhausted
        | EngineRefusal::RevisionExhausted
        | EngineRefusal::EventSequenceExhausted => RefusalV1::Exhausted,
        EngineRefusal::InvalidParticipantId
        | EngineRefusal::InvalidRoomId
        | EngineRefusal::InvalidDisplayName
        | EngineRefusal::InvalidRoomName
        | EngineRefusal::InvalidMessageBody
        | EngineRefusal::NonCanonicalAddressing
        | EngineRefusal::InvalidRoleChange
        | EngineRefusal::DuplicateParticipant(_)
        | EngineRefusal::DuplicateRoom(_)
        | EngineRefusal::UnknownParticipant(_)
        | EngineRefusal::AlreadyMember(_) => RefusalV1::InvalidInput,
    }
}
