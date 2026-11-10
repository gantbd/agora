//! A deterministic, bounded chat-room core for humans and software agents.
//!
//! Agora performs no I/O and does not authenticate callers. Hosts must bind an
//! authenticated principal to the [`ParticipantId`] used in commands.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
pub mod persistence;

/// Resource ceilings for one engine profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_participants: usize,
    pub max_rooms: usize,
    pub max_members_per_room: usize,
    pub max_invitations_per_room: usize,
    pub max_messages_per_room: usize,
    pub max_pending_events: usize,
    pub max_publication_receipts: usize,
    pub max_addressed_participants_per_message: usize,
    pub max_participant_id_bytes: usize,
    pub max_room_id_bytes: usize,
    pub max_display_name_bytes: usize,
    pub max_room_name_bytes: usize,
    pub max_message_body_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_participants: 1_024,
            max_rooms: 256,
            max_members_per_room: 256,
            max_invitations_per_room: 256,
            max_messages_per_room: 1_024,
            max_pending_events: 4_096,
            max_publication_receipts: 4_096,
            max_addressed_participants_per_message: 64,
            max_participant_id_bytes: 128,
            max_room_id_bytes: 128,
            max_display_name_bytes: 256,
            max_room_name_bytes: 256,
            max_message_body_bytes: 16 * 1_024,
        }
    }
}

macro_rules! string_id {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self::new(value)
            }
        }
    };
}

string_id!(ParticipantId);
string_id!(RoomId);

/// Whether a participant is operated directly by a person or by software.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParticipantKind {
    Human,
    Agent,
}

/// A registered identity. Registration is not authentication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Participant {
    id: ParticipantId,
    display_name: String,
    kind: ParticipantKind,
}

impl Participant {
    pub fn new(
        id: impl Into<ParticipantId>,
        display_name: impl Into<String>,
        kind: ParticipantKind,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            kind,
        }
    }

    pub fn id(&self) -> &ParticipantId {
        &self.id
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn kind(&self) -> ParticipantKind {
        self.kind
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionPolicy {
    Open,
    InviteOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    Owner,
    Moderator,
    Member,
}

/// A caller-supplied timestamp in a host-defined logical epoch.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LogicalTime(pub u64);

/// A retained room message. Debug output deliberately redacts the body.
#[derive(Clone, Eq, PartialEq)]
pub struct Message {
    sequence: u64,
    author: ParticipantId,
    at: LogicalTime,
    body: String,
    addressed_to: Vec<ParticipantId>,
}

impl Message {
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn author(&self) -> &ParticipantId {
        &self.author
    }

    pub fn at(&self) -> LogicalTime {
        self.at
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    /// Explicit room members addressed by this message in canonical ID order.
    /// Addressing is conversation data, not permission or proof of execution.
    pub fn addressed_to(&self) -> &[ParticipantId] {
        &self.addressed_to
    }
}

impl fmt::Debug for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Message")
            .field("sequence", &self.sequence)
            .field("author", &self.author)
            .field("at", &self.at)
            .field("body_bytes", &self.body.len())
            .field("addressed_count", &self.addressed_to.len())
            .finish()
    }
}

/// Immutable public room observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoomView {
    pub id: RoomId,
    pub name: String,
    pub admission: AdmissionPolicy,
    pub members: Vec<(ParticipantId, Role)>,
    pub pending_invitations: Vec<ParticipantId>,
    pub messages: Vec<Message>,
    pub next_message_sequence: u64,
}

#[derive(Debug)]
struct Room {
    id: RoomId,
    name: String,
    admission: AdmissionPolicy,
    members: BTreeMap<ParticipantId, Role>,
    invitations: BTreeSet<ParticipantId>,
    messages: VecDeque<Message>,
    next_message_sequence: u64,
}

impl Room {
    fn view(&self) -> RoomView {
        RoomView {
            id: self.id.clone(),
            name: self.name.clone(),
            admission: self.admission,
            members: self
                .members
                .iter()
                .map(|(id, role)| (id.clone(), *role))
                .collect(),
            pending_invitations: self.invitations.iter().cloned().collect(),
            messages: self.messages.iter().cloned().collect(),
            next_message_sequence: self.next_message_sequence,
        }
    }
}

/// The closed command vocabulary for the v0.3 participant-neutral profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandV2 {
    RegisterParticipant(Participant),
    CreateRoom {
        room_id: RoomId,
        name: String,
        owner: ParticipantId,
        admission: AdmissionPolicy,
    },
    Invite {
        room_id: RoomId,
        actor: ParticipantId,
        participant: ParticipantId,
    },
    Join {
        room_id: RoomId,
        participant: ParticipantId,
    },
    Leave {
        room_id: RoomId,
        participant: ParticipantId,
    },
    SetRole {
        room_id: RoomId,
        actor: ParticipantId,
        participant: ParticipantId,
        role: Role,
    },
    Speak {
        room_id: RoomId,
        author: ParticipantId,
        at: LogicalTime,
        body: String,
    },
    /// Atomically publishes a message with explicit participant-neutral
    /// addressing. `addressed_to` must be strictly increasing by participant ID.
    SpeakAddressed {
        room_id: RoomId,
        author: ParticipantId,
        at: LogicalTime,
        body: String,
        addressed_to: Vec<ParticipantId>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutcomeV2 {
    ParticipantRegistered(ParticipantId),
    RoomCreated(RoomId),
    InvitationRecorded {
        room_id: RoomId,
        participant: ParticipantId,
    },
    Joined {
        room_id: RoomId,
        participant: ParticipantId,
    },
    Left {
        room_id: RoomId,
        participant: ParticipantId,
    },
    RoleChanged {
        room_id: RoomId,
        participant: ParticipantId,
        role: Role,
    },
    MessageAccepted {
        room_id: RoomId,
        sequence: u64,
    },
}

/// Typed refusal reasons. Every refusal preserves authoritative state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Refusal {
    InvalidParticipantId,
    InvalidRoomId,
    InvalidDisplayName,
    InvalidRoomName,
    InvalidMessageBody,
    ParticipantLimit,
    RoomLimit,
    MemberLimit,
    InvitationLimit,
    MessageRetentionDisabled,
    MessageSequenceExhausted,
    EventOutboxFull,
    RevisionExhausted,
    EventSequenceExhausted,
    AddressedParticipantLimit,
    NonCanonicalAddressing,
    DuplicateParticipant(ParticipantId),
    DuplicateRoom(RoomId),
    UnknownParticipant(ParticipantId),
    UnknownRoom(RoomId),
    AlreadyMember(ParticipantId),
    NotMember(ParticipantId),
    InvitationRequired(ParticipantId),
    NotAuthorized(ParticipantId),
    OwnerCannotLeave,
    InvalidRoleChange,
    RevisionConflict {
        expected: Revision,
        actual: Revision,
    },
}

/// Monotone engine state revision.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Revision(pub u64);

/// Monotone outbox position.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct EventSequence(pub u64);

/// Optimistic command envelope used by concurrent or remote hosts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionedCommandV2 {
    pub expected_revision: Revision,
    pub command: CommandV2,
}

/// Opaque adapter-supplied identity for one response publication.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PublicationKeyV2([u8; 16]);

impl PublicationKeyV2 {
    pub const fn new(value: [u8; 16]) -> Self {
        Self(value)
    }
}

impl fmt::Debug for PublicationKeyV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PublicationKeyV2(<opaque>)")
    }
}

/// Replay-safe response publication profile layered over the closed v0.3
/// command vocabulary. The expected revision applies only to first acceptance;
/// matching replay observes the retained transition.
#[derive(Clone, Eq, PartialEq)]
pub struct PublicationCommandV2 {
    expected_revision: Revision,
    key: PublicationKeyV2,
    room_id: RoomId,
    author: ParticipantId,
    at: LogicalTime,
    body: String,
}

impl PublicationCommandV2 {
    pub fn new(
        expected_revision: Revision,
        key: PublicationKeyV2,
        room_id: RoomId,
        author: ParticipantId,
        at: LogicalTime,
        body: impl Into<String>,
    ) -> Self {
        Self {
            expected_revision,
            key,
            room_id,
            author,
            at,
            body: body.into(),
        }
    }
}

impl fmt::Debug for PublicationCommandV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PublicationCommandV2")
            .field("expected_revision", &self.expected_revision)
            .field("key", &self.key)
            .field("room_id", &self.room_id)
            .field("author", &self.author)
            .field("at", &self.at)
            .field("body_bytes", &self.body.len())
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublicationRefusalV2 {
    IdempotencyConflict,
    PublicationReceiptLimit,
    Core(Refusal),
}

impl From<Refusal> for PublicationRefusalV2 {
    fn from(value: Refusal) -> Self {
        Self::Core(value)
    }
}

/// Accepted transition metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionV2 {
    pub revision: Revision,
    pub event_sequence: EventSequence,
    pub outcome: OutcomeV2,
}

/// Ordered facts emitted after accepted commands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DomainEventV2 {
    ParticipantRegistered {
        participant_id: ParticipantId,
        kind: ParticipantKind,
    },
    RoomCreated {
        room_id: RoomId,
        owner: ParticipantId,
        admission: AdmissionPolicy,
    },
    InvitationRecorded {
        room_id: RoomId,
        participant: ParticipantId,
    },
    ParticipantJoined {
        room_id: RoomId,
        participant: ParticipantId,
    },
    ParticipantLeft {
        room_id: RoomId,
        participant: ParticipantId,
    },
    RoleChanged {
        room_id: RoomId,
        participant: ParticipantId,
        role: Role,
    },
    MessageAccepted {
        room_id: RoomId,
        message: Message,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventEnvelopeV2 {
    pub sequence: EventSequence,
    pub revision: Revision,
    pub event: DomainEventV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcknowledgeError {
    Regressed {
        requested: EventSequence,
        acknowledged_through: EventSequence,
    },
    BeyondPublished {
        requested: EventSequence,
        last_published: Option<EventSequence>,
    },
}

#[derive(Clone, Eq, PartialEq)]
struct PublicationRecordV2 {
    room_id: RoomId,
    author: ParticipantId,
    at: LogicalTime,
    body: String,
    transition: TransitionV2,
}

impl fmt::Debug for PublicationRecordV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PublicationRecordV2")
            .field("room_id", &self.room_id)
            .field("author", &self.author)
            .field("at", &self.at)
            .field("body_bytes", &self.body.len())
            .field("transition", &self.transition)
            .finish()
    }
}

/// The authoritative, serial in-process engine.
#[derive(Debug)]
pub struct Engine {
    limits: Limits,
    participants: BTreeMap<ParticipantId, Participant>,
    rooms: BTreeMap<RoomId, Room>,
    revision: Revision,
    next_event_sequence: EventSequence,
    last_published_event: Option<EventSequence>,
    acknowledged_through: Option<EventSequence>,
    pending_events: VecDeque<EventEnvelopeV2>,
    publications: BTreeMap<PublicationKeyV2, PublicationRecordV2>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Self::with_limits(Limits::default())
    }

    pub fn with_limits(limits: Limits) -> Self {
        Self {
            limits,
            participants: BTreeMap::new(),
            rooms: BTreeMap::new(),
            revision: Revision(0),
            next_event_sequence: EventSequence(0),
            last_published_event: None,
            acknowledged_through: None,
            pending_events: VecDeque::new(),
            publications: BTreeMap::new(),
        }
    }

    pub fn limits(&self) -> Limits {
        self.limits
    }

    pub fn participant(&self, id: &ParticipantId) -> Option<&Participant> {
        self.participants.get(id)
    }

    pub fn participants(&self) -> impl ExactSizeIterator<Item = &Participant> {
        self.participants.values()
    }

    pub fn room(&self, id: &RoomId) -> Option<RoomView> {
        self.rooms.get(id).map(Room::view)
    }

    /// Exact retained-message observation without copying the room history.
    pub fn message(&self, id: &RoomId, sequence: u64) -> Option<&Message> {
        let room = self.rooms.get(id)?;
        let start = room
            .next_message_sequence
            .checked_sub(room.messages.len() as u64)?;
        let index: usize = sequence.checked_sub(start)?.try_into().ok()?;
        room.messages.get(index).filter(|m| m.sequence == sequence)
    }

    pub fn rooms(&self) -> impl Iterator<Item = RoomView> + '_ {
        self.rooms.values().map(Room::view)
    }
    pub fn room_ids(&self) -> impl ExactSizeIterator<Item = &RoomId> {
        self.rooms.keys()
    }

    pub fn revision(&self) -> Revision {
        self.revision
    }

    pub fn pending_events(&self) -> impl ExactSizeIterator<Item = &EventEnvelopeV2> {
        self.pending_events.iter()
    }

    pub fn acknowledged_through(&self) -> Option<EventSequence> {
        self.acknowledged_through
    }

    /// Publishes one message under a durable-store-compatible idempotency key.
    /// Matching replay returns the original transition even when its original
    /// optimistic revision is now stale.
    pub fn publish(
        &mut self,
        command: PublicationCommandV2,
    ) -> Result<TransitionV2, PublicationRefusalV2> {
        if let Some(existing) = self.publications.get(&command.key) {
            if existing.room_id == command.room_id
                && existing.author == command.author
                && existing.at == command.at
                && existing.body == command.body
            {
                return Ok(existing.transition.clone());
            }
            return Err(PublicationRefusalV2::IdempotencyConflict);
        }
        if self.publications.len() >= self.limits.max_publication_receipts {
            return Err(PublicationRefusalV2::PublicationReceiptLimit);
        }

        let record = PublicationRecordV2 {
            room_id: command.room_id.clone(),
            author: command.author.clone(),
            at: command.at,
            body: command.body.clone(),
            transition: self.apply_revisioned(RevisionedCommandV2 {
                expected_revision: command.expected_revision,
                command: CommandV2::Speak {
                    room_id: command.room_id,
                    author: command.author,
                    at: command.at,
                    body: command.body,
                },
            })?,
        };
        let transition = record.transition.clone();
        self.publications.insert(command.key, record);
        Ok(transition)
    }

    /// Acknowledges and removes the published prefix through `sequence`.
    /// Repeated acknowledgment of an already removed prefix is an identity.
    pub fn acknowledge_events(
        &mut self,
        sequence: EventSequence,
    ) -> Result<usize, AcknowledgeError> {
        if let Some(acknowledged_through) = self.acknowledged_through
            && sequence < acknowledged_through
        {
            return Err(AcknowledgeError::Regressed {
                requested: sequence,
                acknowledged_through,
            });
        }
        if self.last_published_event.is_none_or(|last| sequence > last) {
            return Err(AcknowledgeError::BeyondPublished {
                requested: sequence,
                last_published: self.last_published_event,
            });
        }
        let mut removed = 0;
        while self
            .pending_events
            .front()
            .is_some_and(|event| event.sequence <= sequence)
        {
            self.pending_events.pop_front();
            removed += 1;
        }
        self.acknowledged_through = Some(sequence);
        Ok(removed)
    }

    /// Applies a command against the current revision.
    ///
    /// Hosts coordinating concurrent writers should use [`Engine::apply_revisioned`]
    /// and persist the returned revision with the accepted event.
    pub fn apply(&mut self, command: CommandV2) -> Result<OutcomeV2, Refusal> {
        self.apply_revisioned(RevisionedCommandV2 {
            expected_revision: self.revision,
            command,
        })
        .map(|transition| transition.outcome)
    }

    /// Applies exactly one optimistic command and publishes exactly one event.
    pub fn apply_revisioned(
        &mut self,
        envelope: RevisionedCommandV2,
    ) -> Result<TransitionV2, Refusal> {
        if envelope.expected_revision != self.revision {
            return Err(Refusal::RevisionConflict {
                expected: envelope.expected_revision,
                actual: self.revision,
            });
        }
        if self.pending_events.len() >= self.limits.max_pending_events {
            return Err(Refusal::EventOutboxFull);
        }
        let next_revision = Revision(
            self.revision
                .0
                .checked_add(1)
                .ok_or(Refusal::RevisionExhausted)?,
        );
        let event_sequence = self.next_event_sequence;
        let next_event_sequence = EventSequence(
            event_sequence
                .0
                .checked_add(1)
                .ok_or(Refusal::EventSequenceExhausted)?,
        );
        let event_source = envelope.command.clone();
        let outcome = self.apply_command(envelope.command)?;
        let event = self.event_for(&event_source, &outcome);

        self.revision = next_revision;
        self.next_event_sequence = next_event_sequence;
        self.last_published_event = Some(event_sequence);
        self.pending_events.push_back(EventEnvelopeV2 {
            sequence: event_sequence,
            revision: next_revision,
            event,
        });
        Ok(TransitionV2 {
            revision: next_revision,
            event_sequence,
            outcome,
        })
    }

    fn apply_command(&mut self, command: CommandV2) -> Result<OutcomeV2, Refusal> {
        match command {
            CommandV2::RegisterParticipant(participant) => self.register(participant),
            CommandV2::CreateRoom {
                room_id,
                name,
                owner,
                admission,
            } => self.create_room(room_id, name, owner, admission),
            CommandV2::Invite {
                room_id,
                actor,
                participant,
            } => self.invite(room_id, actor, participant),
            CommandV2::Join {
                room_id,
                participant,
            } => self.join(room_id, participant),
            CommandV2::Leave {
                room_id,
                participant,
            } => self.leave(room_id, participant),
            CommandV2::SetRole {
                room_id,
                actor,
                participant,
                role,
            } => self.set_role(room_id, actor, participant, role),
            CommandV2::Speak {
                room_id,
                author,
                at,
                body,
            } => self.speak(room_id, author, at, body, Vec::new()),
            CommandV2::SpeakAddressed {
                room_id,
                author,
                at,
                body,
                addressed_to,
            } => self.speak(room_id, author, at, body, addressed_to),
        }
    }

    fn event_for(&self, command: &CommandV2, outcome: &OutcomeV2) -> DomainEventV2 {
        match (command, outcome) {
            (
                CommandV2::RegisterParticipant(participant),
                OutcomeV2::ParticipantRegistered(participant_id),
            ) => DomainEventV2::ParticipantRegistered {
                participant_id: participant_id.clone(),
                kind: participant.kind,
            },
            (
                CommandV2::CreateRoom {
                    owner, admission, ..
                },
                OutcomeV2::RoomCreated(room_id),
            ) => DomainEventV2::RoomCreated {
                room_id: room_id.clone(),
                owner: owner.clone(),
                admission: *admission,
            },
            (
                CommandV2::Invite { .. },
                OutcomeV2::InvitationRecorded {
                    room_id,
                    participant,
                },
            ) => DomainEventV2::InvitationRecorded {
                room_id: room_id.clone(),
                participant: participant.clone(),
            },
            (
                CommandV2::Join { .. },
                OutcomeV2::Joined {
                    room_id,
                    participant,
                },
            ) => DomainEventV2::ParticipantJoined {
                room_id: room_id.clone(),
                participant: participant.clone(),
            },
            (
                CommandV2::Leave { .. },
                OutcomeV2::Left {
                    room_id,
                    participant,
                },
            ) => DomainEventV2::ParticipantLeft {
                room_id: room_id.clone(),
                participant: participant.clone(),
            },
            (
                CommandV2::SetRole { .. },
                OutcomeV2::RoleChanged {
                    room_id,
                    participant,
                    role,
                },
            ) => DomainEventV2::RoleChanged {
                room_id: room_id.clone(),
                participant: participant.clone(),
                role: *role,
            },
            (
                CommandV2::Speak {
                    room_id,
                    author,
                    at,
                    body,
                },
                OutcomeV2::MessageAccepted { sequence, .. },
            ) => DomainEventV2::MessageAccepted {
                room_id: room_id.clone(),
                message: Message {
                    sequence: *sequence,
                    author: author.clone(),
                    at: *at,
                    body: body.clone(),
                    addressed_to: Vec::new(),
                },
            },
            (
                CommandV2::SpeakAddressed {
                    room_id,
                    author,
                    at,
                    body,
                    addressed_to,
                },
                OutcomeV2::MessageAccepted { sequence, .. },
            ) => DomainEventV2::MessageAccepted {
                room_id: room_id.clone(),
                message: Message {
                    sequence: *sequence,
                    author: author.clone(),
                    at: *at,
                    body: body.clone(),
                    addressed_to: addressed_to.clone(),
                },
            },
            _ => unreachable!("command and outcome are produced by one exhaustive transition"),
        }
    }

    fn register(&mut self, participant: Participant) -> Result<OutcomeV2, Refusal> {
        self.validate_nonempty_bounded(
            participant.id.as_str(),
            self.limits.max_participant_id_bytes,
            Refusal::InvalidParticipantId,
        )?;
        self.validate_nonempty_bounded(
            &participant.display_name,
            self.limits.max_display_name_bytes,
            Refusal::InvalidDisplayName,
        )?;
        if self.participants.contains_key(&participant.id) {
            return Err(Refusal::DuplicateParticipant(participant.id));
        }
        if self.participants.len() >= self.limits.max_participants {
            return Err(Refusal::ParticipantLimit);
        }
        let id = participant.id.clone();
        self.participants.insert(id.clone(), participant);
        Ok(OutcomeV2::ParticipantRegistered(id))
    }

    fn create_room(
        &mut self,
        room_id: RoomId,
        name: String,
        owner: ParticipantId,
        admission: AdmissionPolicy,
    ) -> Result<OutcomeV2, Refusal> {
        self.validate_nonempty_bounded(
            room_id.as_str(),
            self.limits.max_room_id_bytes,
            Refusal::InvalidRoomId,
        )?;
        self.validate_nonempty_bounded(
            &name,
            self.limits.max_room_name_bytes,
            Refusal::InvalidRoomName,
        )?;
        if !self.participants.contains_key(&owner) {
            return Err(Refusal::UnknownParticipant(owner));
        }
        if self.rooms.contains_key(&room_id) {
            return Err(Refusal::DuplicateRoom(room_id));
        }
        if self.rooms.len() >= self.limits.max_rooms {
            return Err(Refusal::RoomLimit);
        }
        if self.limits.max_members_per_room == 0 {
            return Err(Refusal::MemberLimit);
        }
        let mut members = BTreeMap::new();
        members.insert(owner, Role::Owner);
        self.rooms.insert(
            room_id.clone(),
            Room {
                id: room_id.clone(),
                name,
                admission,
                members,
                invitations: BTreeSet::new(),
                messages: VecDeque::new(),
                next_message_sequence: 0,
            },
        );
        Ok(OutcomeV2::RoomCreated(room_id))
    }

    fn invite(
        &mut self,
        room_id: RoomId,
        actor: ParticipantId,
        participant: ParticipantId,
    ) -> Result<OutcomeV2, Refusal> {
        if !self.participants.contains_key(&participant) {
            return Err(Refusal::UnknownParticipant(participant));
        }
        let room = self.room_ref(&room_id)?;
        if !matches!(
            room.members.get(&actor),
            Some(Role::Owner | Role::Moderator)
        ) {
            return Err(Refusal::NotAuthorized(actor));
        }
        if room.members.contains_key(&participant) {
            return Err(Refusal::AlreadyMember(participant));
        }
        if !room.invitations.contains(&participant)
            && room.invitations.len() >= self.limits.max_invitations_per_room
        {
            return Err(Refusal::InvitationLimit);
        }
        self.rooms
            .get_mut(&room_id)
            .expect("room established by validation")
            .invitations
            .insert(participant.clone());
        Ok(OutcomeV2::InvitationRecorded {
            room_id,
            participant,
        })
    }

    fn join(&mut self, room_id: RoomId, participant: ParticipantId) -> Result<OutcomeV2, Refusal> {
        if !self.participants.contains_key(&participant) {
            return Err(Refusal::UnknownParticipant(participant));
        }
        let room = self.room_ref(&room_id)?;
        if room.members.contains_key(&participant) {
            return Err(Refusal::AlreadyMember(participant));
        }
        if room.members.len() >= self.limits.max_members_per_room {
            return Err(Refusal::MemberLimit);
        }
        if room.admission == AdmissionPolicy::InviteOnly && !room.invitations.contains(&participant)
        {
            return Err(Refusal::InvitationRequired(participant));
        }
        let room = self
            .rooms
            .get_mut(&room_id)
            .expect("room established by validation");
        room.invitations.remove(&participant);
        room.members.insert(participant.clone(), Role::Member);
        Ok(OutcomeV2::Joined {
            room_id,
            participant,
        })
    }

    fn leave(&mut self, room_id: RoomId, participant: ParticipantId) -> Result<OutcomeV2, Refusal> {
        let room = self.room_ref(&room_id)?;
        let Some(role) = room.members.get(&participant) else {
            return Err(Refusal::NotMember(participant));
        };
        if *role == Role::Owner {
            return Err(Refusal::OwnerCannotLeave);
        }
        self.rooms
            .get_mut(&room_id)
            .expect("room established by validation")
            .members
            .remove(&participant);
        Ok(OutcomeV2::Left {
            room_id,
            participant,
        })
    }

    fn set_role(
        &mut self,
        room_id: RoomId,
        actor: ParticipantId,
        participant: ParticipantId,
        role: Role,
    ) -> Result<OutcomeV2, Refusal> {
        let room = self.room_ref(&room_id)?;
        if room.members.get(&actor) != Some(&Role::Owner) {
            return Err(Refusal::NotAuthorized(actor));
        }
        let Some(current) = room.members.get(&participant) else {
            return Err(Refusal::NotMember(participant));
        };
        if *current == Role::Owner || role == Role::Owner {
            return Err(Refusal::InvalidRoleChange);
        }
        self.rooms
            .get_mut(&room_id)
            .expect("room established by validation")
            .members
            .insert(participant.clone(), role);
        Ok(OutcomeV2::RoleChanged {
            room_id,
            participant,
            role,
        })
    }

    fn speak(
        &mut self,
        room_id: RoomId,
        author: ParticipantId,
        at: LogicalTime,
        body: String,
        addressed_to: Vec<ParticipantId>,
    ) -> Result<OutcomeV2, Refusal> {
        self.validate_nonempty_bounded(
            &body,
            self.limits.max_message_body_bytes,
            Refusal::InvalidMessageBody,
        )?;
        if self.limits.max_messages_per_room == 0 {
            return Err(Refusal::MessageRetentionDisabled);
        }
        let room = self.room_ref(&room_id)?;
        if !room.members.contains_key(&author) {
            return Err(Refusal::NotMember(author));
        }
        if addressed_to.len() > self.limits.max_addressed_participants_per_message {
            return Err(Refusal::AddressedParticipantLimit);
        }
        if addressed_to.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(Refusal::NonCanonicalAddressing);
        }
        if let Some(id) = addressed_to
            .iter()
            .find(|id| !room.members.contains_key(*id))
        {
            return Err(Refusal::NotMember(id.clone()));
        }
        let sequence = room.next_message_sequence;
        let next = sequence
            .checked_add(1)
            .ok_or(Refusal::MessageSequenceExhausted)?;
        let room = self
            .rooms
            .get_mut(&room_id)
            .expect("room established by validation");
        if room.messages.len() == self.limits.max_messages_per_room {
            room.messages.pop_front();
        }
        room.messages.push_back(Message {
            sequence,
            author,
            at,
            body,
            addressed_to,
        });
        room.next_message_sequence = next;
        Ok(OutcomeV2::MessageAccepted { room_id, sequence })
    }

    fn room_ref(&self, room_id: &RoomId) -> Result<&Room, Refusal> {
        self.rooms
            .get(room_id)
            .ok_or_else(|| Refusal::UnknownRoom(room_id.clone()))
    }

    fn validate_nonempty_bounded(
        &self,
        value: &str,
        max_bytes: usize,
        refusal: Refusal,
    ) -> Result<(), Refusal> {
        if value.is_empty() || value.len() > max_bytes {
            Err(refusal)
        } else {
            Ok(())
        }
    }
}
