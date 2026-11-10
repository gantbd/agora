//! Serialized volatile implementation of Agora Protocol V2.

use agora_engine::{
    AdmissionPolicy, CommandV2 as EngineCommand, Engine, MemberPageCursor, MessagePageCursor,
    ObservationRefusal, OutcomeV2 as EngineOutcome, ParticipantId, ParticipantKind,
    Refusal as EngineRefusal, Revision, RevisionedCommandV2, Role, RoomChangeGap,
    RoomChangePosition, RoomChangeResult, RoomId, RoomPageCursor,
};
use agora_protocol::v2::{
    AdmissionPolicyV2, ChangeGapV2, ClientIdV2, CommandV2, MemberPageV2, MemberV2, MessagePageV2,
    MessageV2, OutcomeV2, PageCursorV2, ParticipantKindV2, RefusalV2, ReplyV2, RequestIdV2,
    RequestV2, RoleV2, RoomChangePageV2, RoomChangeV2, RoomEventV2, RoomPageV2, RoomSummaryV2,
};
use std::collections::{BTreeMap, VecDeque};

pub mod persistence;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceLimitsV2 {
    pub max_clients: usize,
    pub max_retired_clients: usize,
    pub max_retained_requests: usize,
    pub max_retained_cursors: usize,
    pub max_page_items: u32,
    pub max_participant_id_bytes: usize,
    pub max_room_id_bytes: usize,
    pub max_room_name_bytes: usize,
    pub max_message_body_bytes: usize,
    pub max_addressed_participants: usize,
}

impl Default for ServiceLimitsV2 {
    fn default() -> Self {
        Self {
            max_clients: 64,
            max_retired_clients: 256,
            max_retained_requests: 4_096,
            max_retained_cursors: 4_096,
            max_page_items: 256,
            max_participant_id_bytes: 128,
            max_room_id_bytes: 128,
            max_room_name_bytes: 256,
            max_message_body_bytes: 16 * 1_024,
            max_addressed_participants: 64,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UsageV2 {
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
struct RetainedRequestV2 {
    request: RequestV2,
    reply: ReplyV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CursorPage {
    Rooms(RoomPageCursor),
    Members {
        room_id: RoomId,
        cursor: MemberPageCursor,
    },
    Messages {
        room_id: RoomId,
        cursor: MessagePageCursor,
    },
    Changes {
        room_id: RoomId,
        position: RoomChangePosition,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CursorRecord {
    owner: ClientIdV2,
    page: CursorPage,
}

/// One serialized volatile Protocol V2 owner.
#[derive(Debug)]
pub struct ServiceV2 {
    engine: Engine,
    limits: ServiceLimitsV2,
    clients: BTreeMap<ClientIdV2, ParticipantId>,
    retired_clients: VecDeque<ClientIdV2>,
    retained: BTreeMap<(ClientIdV2, RequestIdV2), RetainedRequestV2>,
    cursors: BTreeMap<PageCursorV2, CursorRecord>,
    next_client: u64,
    next_cursor: u64,
    usage: UsageV2,
}

impl ServiceV2 {
    pub fn new(engine: Engine, limits: ServiceLimitsV2) -> Self {
        Self {
            engine,
            limits,
            clients: BTreeMap::new(),
            retired_clients: VecDeque::new(),
            retained: BTreeMap::new(),
            cursors: BTreeMap::new(),
            next_client: 0,
            next_cursor: 0,
            usage: UsageV2::default(),
        }
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn limits(&self) -> ServiceLimitsV2 {
        self.limits
    }

    /// Trusted-host inspection of the participant bound to an active client.
    /// A client handle remains a correlation value, not a credential.
    pub fn participant_for_client(&self, client: ClientIdV2) -> Option<&ParticipantId> {
        self.clients.get(&client)
    }

    pub fn active_client_count(&self) -> usize {
        self.clients.len()
    }

    pub fn usage(&self) -> UsageV2 {
        self.usage
    }

    pub fn connect(&mut self, participant: &str) -> Result<ClientIdV2, RefusalV2> {
        count(&mut self.usage.connect_offers, &mut self.usage.overflowed);
        if !valid_text(participant, self.limits.max_participant_id_bytes) {
            return Err(RefusalV2::InvalidInput);
        }
        if self.clients.len() >= self.limits.max_clients {
            return Err(RefusalV2::ClientLimit);
        }
        let participant_id = ParticipantId::new(participant);
        if self.engine.participant(&participant_id).is_none() {
            return Err(RefusalV2::UnknownParticipant);
        }
        let client = ClientIdV2(self.next_client);
        self.next_client = self
            .next_client
            .checked_add(1)
            .ok_or(RefusalV2::Exhausted)?;
        self.clients.insert(client, participant_id);
        count(&mut self.usage.connected, &mut self.usage.overflowed);
        Ok(client)
    }

    pub fn disconnect(&mut self, client: ClientIdV2) -> Result<bool, RefusalV2> {
        if self.retired_clients.contains(&client) {
            return Ok(false);
        }
        if !self.clients.contains_key(&client) {
            return Err(RefusalV2::UnknownClient);
        }
        self.clients.remove(&client);
        if self.limits.max_retired_clients > 0 {
            if self.retired_clients.len() >= self.limits.max_retired_clients {
                self.retired_clients.pop_front();
            }
            self.retired_clients.push_back(client);
        }
        self.retained.retain(|(owner, _), _| *owner != client);
        self.cursors.retain(|_, record| record.owner != client);
        count(&mut self.usage.disconnected, &mut self.usage.overflowed);
        Ok(true)
    }

    pub fn execute(&mut self, client: ClientIdV2, request: RequestV2) -> ReplyV2 {
        count(&mut self.usage.request_offers, &mut self.usage.overflowed);
        let Some(participant) = self.clients.get(&client).cloned() else {
            count(&mut self.usage.refused, &mut self.usage.overflowed);
            return ReplyV2::Refused(RefusalV2::UnknownClient);
        };
        let key = (client, request.request_id);
        if let Some(existing) = self.retained.get(&key) {
            if existing.request == request {
                count(&mut self.usage.replayed, &mut self.usage.overflowed);
                return existing.reply.clone();
            }
            count(&mut self.usage.conflicts, &mut self.usage.overflowed);
            return ReplyV2::Refused(RefusalV2::RequestConflict);
        }
        if !self.valid_command_shape(&request.command) {
            count(&mut self.usage.refused, &mut self.usage.overflowed);
            return ReplyV2::Refused(RefusalV2::InvalidInput);
        }
        if self.retained.len() >= self.limits.max_retained_requests {
            count(&mut self.usage.refused, &mut self.usage.overflowed);
            return ReplyV2::Refused(RefusalV2::RequestLimit);
        }
        let reply = match self.execute_new(client, &participant, &request.command) {
            Ok(outcome) => ReplyV2::Accepted(outcome),
            Err(refusal) => ReplyV2::Refused(refusal),
        };
        match &reply {
            ReplyV2::Accepted(_) => count(&mut self.usage.accepted, &mut self.usage.overflowed),
            ReplyV2::Refused(_) => count(&mut self.usage.refused, &mut self.usage.overflowed),
        }
        self.retained.insert(
            key,
            RetainedRequestV2 {
                request,
                reply: reply.clone(),
            },
        );
        reply
    }

    fn execute_new(
        &mut self,
        client: ClientIdV2,
        participant: &ParticipantId,
        command: &CommandV2,
    ) -> Result<OutcomeV2, RefusalV2> {
        match command {
            CommandV2::Rooms { cursor, limit } => self.rooms(client, participant, *cursor, *limit),
            CommandV2::Members {
                room_id,
                cursor,
                limit,
            } => self.members(client, participant, room_id, *cursor, *limit),
            CommandV2::Messages {
                room_id,
                cursor,
                limit,
            } => self.messages(client, participant, room_id, *cursor, *limit),
            CommandV2::OpenChanges {
                room_id,
                observed_revision,
            } => self.open_changes(client, participant, room_id, *observed_revision),
            CommandV2::Changes {
                room_id,
                cursor,
                limit,
            } => self.changes(client, participant, room_id, *cursor, *limit),
            CommandV2::CreateRoom {
                room_id,
                name,
                admission,
                expected_revision,
            } => self.mutate(
                *expected_revision,
                EngineCommand::CreateRoom {
                    room_id: RoomId::new(room_id),
                    name: name.clone(),
                    owner: participant.clone(),
                    admission: map_admission_to_engine(*admission),
                },
            ),
            CommandV2::Invite {
                room_id,
                participant_id,
                expected_revision,
            } => self.mutate(
                *expected_revision,
                EngineCommand::Invite {
                    room_id: RoomId::new(room_id),
                    actor: participant.clone(),
                    participant: ParticipantId::new(participant_id),
                },
            ),
            CommandV2::Join {
                room_id,
                expected_revision,
            } => self.mutate(
                *expected_revision,
                EngineCommand::Join {
                    room_id: RoomId::new(room_id),
                    participant: participant.clone(),
                },
            ),
            CommandV2::Leave {
                room_id,
                expected_revision,
            } => self.mutate(
                *expected_revision,
                EngineCommand::Leave {
                    room_id: RoomId::new(room_id),
                    participant: participant.clone(),
                },
            ),
            CommandV2::SetRole {
                room_id,
                participant_id,
                role,
                expected_revision,
            } => self.mutate(
                *expected_revision,
                EngineCommand::SetRole {
                    room_id: RoomId::new(room_id),
                    actor: participant.clone(),
                    participant: ParticipantId::new(participant_id),
                    role: map_role_to_engine(*role),
                },
            ),
            CommandV2::Speak {
                room_id,
                expected_revision,
                logical_time,
                body,
                addressed_to,
            } => {
                let command = if addressed_to.is_empty() {
                    EngineCommand::Speak {
                        room_id: RoomId::new(room_id),
                        author: participant.clone(),
                        at: agora_engine::LogicalTime(*logical_time),
                        body: body.clone(),
                    }
                } else {
                    EngineCommand::SpeakAddressed {
                        room_id: RoomId::new(room_id),
                        author: participant.clone(),
                        at: agora_engine::LogicalTime(*logical_time),
                        body: body.clone(),
                        addressed_to: addressed_to.iter().map(ParticipantId::new).collect(),
                    }
                };
                self.mutate(*expected_revision, command)
            }
        }
    }

    fn mutate(
        &mut self,
        expected_revision: u64,
        command: EngineCommand,
    ) -> Result<OutcomeV2, RefusalV2> {
        let transition = self
            .engine
            .apply_revisioned(RevisionedCommandV2 {
                expected_revision: Revision(expected_revision),
                command,
            })
            .map_err(map_engine_refusal)?;
        let revision = transition.revision.0;
        match transition.outcome {
            EngineOutcome::RoomCreated(_) => Ok(OutcomeV2::RoomCreated { revision }),
            EngineOutcome::InvitationRecorded { .. } => {
                Ok(OutcomeV2::InvitationRecorded { revision })
            }
            EngineOutcome::Joined { .. } => Ok(OutcomeV2::Joined { revision }),
            EngineOutcome::Left { .. } => Ok(OutcomeV2::Left { revision }),
            EngineOutcome::RoleChanged { .. } => Ok(OutcomeV2::RoleChanged { revision }),
            EngineOutcome::MessageAccepted { sequence, .. } => {
                Ok(OutcomeV2::MessageAccepted { revision, sequence })
            }
            EngineOutcome::ParticipantRegistered(_) => Err(RefusalV2::Unavailable),
        }
    }

    fn rooms(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        cursor_id: Option<PageCursorV2>,
        limit: u32,
    ) -> Result<OutcomeV2, RefusalV2> {
        let cursor = match cursor_id {
            Some(id) => match self.cursor_for(client, id)? {
                CursorPage::Rooms(cursor) => Some(cursor),
                _ => return Err(RefusalV2::InvalidCursor),
            },
            None => None,
        };
        let page = self
            .engine
            .room_page(observer, cursor.as_ref(), limit as usize)
            .map_err(map_observation_refusal)?;
        let revision = page.revision.0;
        let next_page = page.next.map(CursorPage::Rooms);
        let rooms = page
            .rooms
            .into_iter()
            .map(|room| {
                Ok(RoomSummaryV2 {
                    room_id: room.id.as_str().to_owned(),
                    name: room.name,
                    admission: map_admission_from_engine(room.admission),
                    observer_role: map_role_from_engine(room.observer_role),
                    member_count: room
                        .member_count
                        .try_into()
                        .map_err(|_| RefusalV2::Unavailable)?,
                    retained_message_count: room
                        .retained_message_count
                        .try_into()
                        .map_err(|_| RefusalV2::Unavailable)?,
                    next_message_sequence: room.next_message_sequence,
                })
            })
            .collect::<Result<Vec<_>, RefusalV2>>()?;
        let next = self.replace_cursor(client, cursor_id, next_page)?;
        Ok(OutcomeV2::Rooms(RoomPageV2 {
            revision,
            rooms,
            next,
        }))
    }

    fn members(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        room_id: &str,
        cursor_id: Option<PageCursorV2>,
        limit: u32,
    ) -> Result<OutcomeV2, RefusalV2> {
        let room = RoomId::new(room_id);
        let cursor = match cursor_id {
            Some(id) => match self.cursor_for(client, id)? {
                CursorPage::Members {
                    room_id: bound_room,
                    cursor,
                } if bound_room == room => Some(cursor),
                _ => return Err(RefusalV2::InvalidCursor),
            },
            None => None,
        };
        let page = self
            .engine
            .member_page(observer, &room, cursor.as_ref(), limit as usize)
            .map_err(map_observation_refusal)?;
        let next_page = page.next.map(|cursor| CursorPage::Members {
            room_id: room.clone(),
            cursor,
        });
        let next = self.replace_cursor(client, cursor_id, next_page)?;
        let members = page
            .members
            .into_iter()
            .map(|member| MemberV2 {
                participant_id: member.participant_id.as_str().to_owned(),
                display_name: member.display_name,
                kind: match member.kind {
                    ParticipantKind::Human => ParticipantKindV2::Human,
                    ParticipantKind::Agent => ParticipantKindV2::Agent,
                },
                role: map_role_from_engine(member.role),
            })
            .collect();
        Ok(OutcomeV2::Members(MemberPageV2 {
            revision: page.revision.0,
            room_id: page.room_id.as_str().to_owned(),
            members,
            next,
        }))
    }

    fn messages(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        room_id: &str,
        cursor_id: Option<PageCursorV2>,
        limit: u32,
    ) -> Result<OutcomeV2, RefusalV2> {
        let room = RoomId::new(room_id);
        let cursor = match cursor_id {
            Some(id) => match self.cursor_for(client, id)? {
                CursorPage::Messages {
                    room_id: bound_room,
                    cursor,
                } if bound_room == room => Some(cursor),
                _ => return Err(RefusalV2::InvalidCursor),
            },
            None => None,
        };
        let page = self
            .engine
            .message_page(observer, &room, cursor.as_ref(), limit as usize)
            .map_err(map_observation_refusal)?;
        let next_page = page.next.map(|cursor| CursorPage::Messages {
            room_id: room.clone(),
            cursor,
        });
        let next = self.replace_cursor(client, cursor_id, next_page)?;
        let messages = page
            .messages
            .into_iter()
            .map(|message| MessageV2 {
                sequence: message.sequence(),
                author: message.author().as_str().to_owned(),
                logical_time: message.at().0,
                body: message.body().to_owned(),
                addressed_to: message
                    .addressed_to()
                    .iter()
                    .map(|participant| participant.as_str().to_owned())
                    .collect(),
            })
            .collect();
        Ok(OutcomeV2::Messages(MessagePageV2 {
            revision: page.revision.0,
            room_id: page.room_id.as_str().to_owned(),
            messages,
            oldest_retained_sequence: page.oldest_retained_sequence,
            next_message_sequence: page.next_message_sequence,
            next,
        }))
    }

    fn open_changes(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        room_id: &str,
        observed_revision: u64,
    ) -> Result<OutcomeV2, RefusalV2> {
        let room = RoomId::new(room_id);
        let position = self
            .engine
            .room_change_position(observer, &room, Revision(observed_revision))
            .map_err(map_observation_refusal)?;
        let cursor = self
            .replace_cursor(
                client,
                None,
                Some(CursorPage::Changes {
                    room_id: room,
                    position,
                }),
            )?
            .ok_or(RefusalV2::Unavailable)?;
        Ok(OutcomeV2::ChangesOpened {
            revision: observed_revision,
            cursor,
        })
    }

    fn changes(
        &mut self,
        client: ClientIdV2,
        observer: &ParticipantId,
        room_id: &str,
        cursor_id: PageCursorV2,
        limit: u32,
    ) -> Result<OutcomeV2, RefusalV2> {
        let room = RoomId::new(room_id);
        let position = match self.cursor_for(client, cursor_id)? {
            CursorPage::Changes {
                room_id: bound_room,
                position,
            } if bound_room == room => position,
            _ => return Err(RefusalV2::InvalidCursor),
        };
        match self
            .engine
            .room_change_page(observer, &room, &position, limit as usize)
            .map_err(map_observation_refusal)?
        {
            RoomChangeResult::Gap {
                cause,
                current_revision,
            } => {
                self.replace_cursor(client, Some(cursor_id), None)?;
                Ok(OutcomeV2::ChangeGap {
                    current_revision: current_revision.0,
                    cause: match cause {
                        RoomChangeGap::Retention {
                            acknowledged_through,
                        } => ChangeGapV2::Retention {
                            acknowledged_through: acknowledged_through.0,
                        },
                        RoomChangeGap::MembershipChanged => ChangeGapV2::MembershipChanged,
                    },
                })
            }
            RoomChangeResult::Page(page) => {
                let next_position = page.next;
                let next = self
                    .replace_cursor(
                        client,
                        Some(cursor_id),
                        Some(CursorPage::Changes {
                            room_id: room,
                            position: next_position,
                        }),
                    )?
                    .ok_or(RefusalV2::Unavailable)?;
                let changes = page
                    .changes
                    .into_iter()
                    .map(|change| {
                        let event = match change.event {
                            agora_engine::DomainEventV2::ParticipantRegistered { .. } => {
                                return Err(RefusalV2::Unavailable);
                            }
                            agora_engine::DomainEventV2::RoomCreated {
                                owner, admission, ..
                            } => RoomEventV2::RoomCreated {
                                owner: owner.as_str().to_owned(),
                                admission: map_admission_from_engine(admission),
                            },
                            agora_engine::DomainEventV2::InvitationRecorded {
                                participant, ..
                            } => RoomEventV2::InvitationRecorded {
                                participant_id: participant.as_str().to_owned(),
                            },
                            agora_engine::DomainEventV2::ParticipantJoined {
                                participant, ..
                            } => RoomEventV2::ParticipantJoined {
                                participant_id: participant.as_str().to_owned(),
                            },
                            agora_engine::DomainEventV2::ParticipantLeft {
                                participant, ..
                            } => RoomEventV2::ParticipantLeft {
                                participant_id: participant.as_str().to_owned(),
                            },
                            agora_engine::DomainEventV2::RoleChanged {
                                participant, role, ..
                            } => RoomEventV2::RoleChanged {
                                participant_id: participant.as_str().to_owned(),
                                role: map_role_from_engine(role),
                            },
                            agora_engine::DomainEventV2::MessageAccepted { message, .. } => {
                                RoomEventV2::MessageAccepted(map_message_from_engine(message))
                            }
                        };
                        Ok(RoomChangeV2 {
                            event_sequence: change.sequence.0,
                            revision: change.revision.0,
                            event,
                        })
                    })
                    .collect::<Result<Vec<_>, RefusalV2>>()?;
                Ok(OutcomeV2::Changes(RoomChangePageV2 {
                    revision: page.revision.0,
                    changes,
                    next,
                    has_more: page.has_more,
                }))
            }
        }
    }

    fn cursor_for(
        &self,
        client: ClientIdV2,
        cursor: PageCursorV2,
    ) -> Result<CursorPage, RefusalV2> {
        let record = self.cursors.get(&cursor).ok_or(RefusalV2::InvalidCursor)?;
        if record.owner != client {
            return Err(RefusalV2::InvalidCursor);
        }
        Ok(record.page.clone())
    }

    fn replace_cursor(
        &mut self,
        client: ClientIdV2,
        old: Option<PageCursorV2>,
        next: Option<CursorPage>,
    ) -> Result<Option<PageCursorV2>, RefusalV2> {
        if next.is_some() && old.is_none() && self.cursors.len() >= self.limits.max_retained_cursors
        {
            return Err(RefusalV2::CursorLimit);
        }
        let next_id = if next.is_some() {
            let id = PageCursorV2(self.next_cursor);
            self.next_cursor = self
                .next_cursor
                .checked_add(1)
                .ok_or(RefusalV2::Exhausted)?;
            Some(id)
        } else {
            None
        };
        if let Some(old) = old {
            self.cursors.remove(&old);
        }
        if let (Some(id), Some(page)) = (next_id, next) {
            self.cursors.insert(
                id,
                CursorRecord {
                    owner: client,
                    page,
                },
            );
        }
        Ok(next_id)
    }

    fn valid_command_shape(&self, command: &CommandV2) -> bool {
        let valid_page = |limit: u32| limit > 0 && limit <= self.limits.max_page_items;
        let valid_room = |room: &str| valid_text(room, self.limits.max_room_id_bytes);
        let valid_participant =
            |participant: &str| valid_text(participant, self.limits.max_participant_id_bytes);
        match command {
            CommandV2::Rooms { limit, .. } => valid_page(*limit),
            CommandV2::Members { room_id, limit, .. }
            | CommandV2::Messages { room_id, limit, .. } => {
                valid_room(room_id) && valid_page(*limit)
            }
            CommandV2::OpenChanges { room_id, .. } => valid_room(room_id),
            CommandV2::Changes { room_id, limit, .. } => valid_room(room_id) && valid_page(*limit),
            CommandV2::CreateRoom { room_id, name, .. } => {
                valid_room(room_id) && valid_text(name, self.limits.max_room_name_bytes)
            }
            CommandV2::Invite {
                room_id,
                participant_id,
                ..
            }
            | CommandV2::SetRole {
                room_id,
                participant_id,
                ..
            } => valid_room(room_id) && valid_participant(participant_id),
            CommandV2::Join { room_id, .. } | CommandV2::Leave { room_id, .. } => {
                valid_room(room_id)
            }
            CommandV2::Speak {
                room_id,
                body,
                addressed_to,
                ..
            } => {
                valid_room(room_id)
                    && valid_text(body, self.limits.max_message_body_bytes)
                    && addressed_to.len() <= self.limits.max_addressed_participants
                    && addressed_to
                        .iter()
                        .all(|participant| valid_participant(participant))
            }
        }
    }
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.len() <= maximum
}

fn map_message_from_engine(message: agora_engine::Message) -> MessageV2 {
    MessageV2 {
        sequence: message.sequence(),
        author: message.author().as_str().to_owned(),
        logical_time: message.at().0,
        body: message.body().to_owned(),
        addressed_to: message
            .addressed_to()
            .iter()
            .map(|participant| participant.as_str().to_owned())
            .collect(),
    }
}

fn map_admission_to_engine(value: AdmissionPolicyV2) -> AdmissionPolicy {
    match value {
        AdmissionPolicyV2::Open => AdmissionPolicy::Open,
        AdmissionPolicyV2::InviteOnly => AdmissionPolicy::InviteOnly,
    }
}

fn map_admission_from_engine(value: AdmissionPolicy) -> AdmissionPolicyV2 {
    match value {
        AdmissionPolicy::Open => AdmissionPolicyV2::Open,
        AdmissionPolicy::InviteOnly => AdmissionPolicyV2::InviteOnly,
    }
}

fn map_role_to_engine(value: RoleV2) -> Role {
    match value {
        RoleV2::Owner => Role::Owner,
        RoleV2::Moderator => Role::Moderator,
        RoleV2::Member => Role::Member,
    }
}

fn map_role_from_engine(value: Role) -> RoleV2 {
    match value {
        Role::Owner => RoleV2::Owner,
        Role::Moderator => RoleV2::Moderator,
        Role::Member => RoleV2::Member,
    }
}

fn map_observation_refusal(refusal: ObservationRefusal) -> RefusalV2 {
    match refusal {
        ObservationRefusal::UnknownParticipant => RefusalV2::UnknownParticipant,
        ObservationRefusal::UnknownRoom => RefusalV2::UnknownRoom,
        ObservationRefusal::NotMember => RefusalV2::NotMember,
        ObservationRefusal::InvalidLimit => RefusalV2::InvalidInput,
        ObservationRefusal::ForeignCursor => RefusalV2::InvalidCursor,
        ObservationRefusal::Stale { actual_revision } => RefusalV2::Stale {
            actual_revision: actual_revision.0,
        },
        ObservationRefusal::Capacity => RefusalV2::Capacity,
        ObservationRefusal::InconsistentState => RefusalV2::Unavailable,
    }
}

fn map_engine_refusal(refusal: EngineRefusal) -> RefusalV2 {
    match refusal {
        EngineRefusal::RevisionConflict { actual, .. } => RefusalV2::Stale {
            actual_revision: actual.0,
        },
        EngineRefusal::UnknownParticipant(_) => RefusalV2::UnknownParticipant,
        EngineRefusal::UnknownRoom(_) => RefusalV2::UnknownRoom,
        EngineRefusal::NotMember(_) => RefusalV2::NotMember,
        EngineRefusal::NotAuthorized(_)
        | EngineRefusal::InvitationRequired(_)
        | EngineRefusal::OwnerCannotLeave => RefusalV2::NotAuthorized,
        EngineRefusal::ParticipantLimit
        | EngineRefusal::RoomLimit
        | EngineRefusal::MemberLimit
        | EngineRefusal::InvitationLimit
        | EngineRefusal::MessageRetentionDisabled
        | EngineRefusal::AddressedParticipantLimit
        | EngineRefusal::EventOutboxFull => RefusalV2::Capacity,
        EngineRefusal::MessageSequenceExhausted
        | EngineRefusal::RevisionExhausted
        | EngineRefusal::EventSequenceExhausted => RefusalV2::Exhausted,
        EngineRefusal::InvalidParticipantId
        | EngineRefusal::InvalidRoomId
        | EngineRefusal::InvalidDisplayName
        | EngineRefusal::InvalidRoomName
        | EngineRefusal::InvalidMessageBody
        | EngineRefusal::NonCanonicalAddressing
        | EngineRefusal::InvalidRoleChange
        | EngineRefusal::DuplicateParticipant(_)
        | EngineRefusal::DuplicateRoom(_)
        | EngineRefusal::AlreadyMember(_) => RefusalV2::InvalidInput,
    }
}
