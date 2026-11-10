//! Bounded canonical V2 semantic checkpoints. No I/O, clocks or authentication.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryLimits {
    pub max_snapshot_bytes: usize,
    pub maximum_engine_limits: Limits,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckpointError {
    Limit,
    Capacity,
    Truncated,
    Profile,
    Invalid,
    NonCanonical,
    Trailing,
}
impl fmt::Display for CheckpointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Agora checkpoint: {self:?}")
    }
}
impl std::error::Error for CheckpointError {}
pub struct CheckpointV2(Vec<u8>);
impl CheckpointV2 {
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Bounds for restoring an opaque observation cursor from durable service state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CursorRecoveryLimitsV1 {
    pub max_snapshot_bytes: usize,
    pub max_participant_id_bytes: usize,
    pub max_room_id_bytes: usize,
}

/// Canonical private-state representation of one opaque observation cursor.
///
/// The bytes are suitable for an Agora service checkpoint. They are not a wire
/// cursor, credential, or authority token.
pub struct CursorCheckpointV1(Vec<u8>);

impl CursorCheckpointV1 {
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for CursorCheckpointV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CursorCheckpointV1(<redacted>)")
    }
}
impl fmt::Debug for CheckpointV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CheckpointV2(<redacted>)")
    }
}
struct Writer {
    bytes: Vec<u8>,
    max: usize,
}
impl Writer {
    fn raw(&mut self, v: &[u8]) -> Result<(), CheckpointError> {
        if self
            .bytes
            .len()
            .checked_add(v.len())
            .is_none_or(|n| n > self.max)
        {
            return Err(CheckpointError::Limit);
        }
        self.bytes
            .try_reserve(v.len())
            .map_err(|_| CheckpointError::Capacity)?;
        self.bytes.extend_from_slice(v);
        Ok(())
    }
    fn byte(&mut self, v: u8) -> Result<(), CheckpointError> {
        self.raw(&[v])
    }
    fn number(&mut self, v: u64) -> Result<(), CheckpointError> {
        self.raw(&v.to_be_bytes())
    }
    fn count(&mut self, v: usize) -> Result<(), CheckpointError> {
        self.number(v.try_into().map_err(|_| CheckpointError::Limit)?)
    }
    fn text(&mut self, v: &str) -> Result<(), CheckpointError> {
        self.raw(
            &u32::try_from(v.len())
                .map_err(|_| CheckpointError::Limit)?
                .to_be_bytes(),
        )?;
        self.raw(v.as_bytes())
    }
    fn option(&mut self, v: Option<EventSequence>) -> Result<(), CheckpointError> {
        self.byte(u8::from(v.is_some()))?;
        if let Some(v) = v {
            self.number(v.0)?
        }
        Ok(())
    }
    fn message(&mut self, m: &Message) -> Result<(), CheckpointError> {
        self.number(m.sequence)?;
        self.text(m.author.as_str())?;
        self.number(m.at.0)?;
        self.text(&m.body)?;
        self.count(m.addressed_to.len())?;
        for id in &m.addressed_to {
            self.text(id.as_str())?
        }
        Ok(())
    }
    fn event(&mut self, e: &DomainEventV2) -> Result<(), CheckpointError> {
        match e {
            DomainEventV2::ParticipantRegistered {
                participant_id,
                kind,
            } => {
                self.byte(0)?;
                self.text(participant_id.as_str())?;
                self.byte(kind_tag(*kind))
            }
            DomainEventV2::RoomCreated {
                room_id,
                owner,
                admission,
            } => {
                self.byte(1)?;
                self.text(room_id.as_str())?;
                self.text(owner.as_str())?;
                self.byte(admission_tag(*admission))
            }
            DomainEventV2::InvitationRecorded {
                room_id,
                participant,
            } => {
                self.byte(2)?;
                self.text(room_id.as_str())?;
                self.text(participant.as_str())
            }
            DomainEventV2::ParticipantJoined {
                room_id,
                participant,
            } => {
                self.byte(3)?;
                self.text(room_id.as_str())?;
                self.text(participant.as_str())
            }
            DomainEventV2::ParticipantLeft {
                room_id,
                participant,
            } => {
                self.byte(4)?;
                self.text(room_id.as_str())?;
                self.text(participant.as_str())
            }
            DomainEventV2::RoleChanged {
                room_id,
                participant,
                role,
            } => {
                self.byte(5)?;
                self.text(room_id.as_str())?;
                self.text(participant.as_str())?;
                self.byte(role_tag(*role))
            }
            DomainEventV2::MessageAccepted { room_id, message } => {
                self.byte(6)?;
                self.text(room_id.as_str())?;
                self.message(message)
            }
            DomainEventV2::ParticipantKicked {
                room_id,
                actor,
                participant,
                reason,
                causation,
            }
            | DomainEventV2::ParticipantBanned {
                room_id,
                actor,
                participant,
                reason,
                causation,
            }
            | DomainEventV2::ParticipantUnbanned {
                room_id,
                actor,
                participant,
                reason,
                causation,
            } => {
                let tag = match e {
                    DomainEventV2::ParticipantKicked { .. } => 7,
                    DomainEventV2::ParticipantBanned { .. } => 8,
                    DomainEventV2::ParticipantUnbanned { .. } => 9,
                    _ => unreachable!(),
                };
                self.byte(tag)?;
                self.text(room_id.as_str())?;
                self.text(actor.as_str())?;
                self.text(participant.as_str())?;
                self.text(reason.as_str())?;
                self.raw(&causation.into_bytes())
            }
            DomainEventV2::RoomPolicyChanged {
                room_id,
                actor,
                previous,
                admission,
            } => {
                self.byte(10)?;
                self.text(room_id.as_str())?;
                self.text(actor.as_str())?;
                self.byte(admission_tag(*previous))?;
                self.byte(admission_tag(*admission))
            }
        }
    }
}
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl<'a> Reader<'a> {
    fn raw(&mut self, n: usize) -> Result<&'a [u8], CheckpointError> {
        let end = self.pos.checked_add(n).ok_or(CheckpointError::Limit)?;
        let v = self
            .bytes
            .get(self.pos..end)
            .ok_or(CheckpointError::Truncated)?;
        self.pos = end;
        Ok(v)
    }
    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], CheckpointError> {
        self.raw(N)?
            .try_into()
            .map_err(|_| CheckpointError::Truncated)
    }
    fn byte(&mut self) -> Result<u8, CheckpointError> {
        Ok(self.fixed::<1>()?[0])
    }
    fn number(&mut self) -> Result<u64, CheckpointError> {
        Ok(u64::from_be_bytes(self.fixed()?))
    }
    fn size(&mut self, max: usize) -> Result<usize, CheckpointError> {
        let n: usize = self
            .number()?
            .try_into()
            .map_err(|_| CheckpointError::Limit)?;
        if n > max {
            return Err(CheckpointError::Limit);
        }
        Ok(n)
    }
    fn count(&mut self, max: usize) -> Result<usize, CheckpointError> {
        let n = self.size(max)?;
        if n > self.bytes.len() - self.pos {
            return Err(CheckpointError::Truncated);
        }
        Ok(n)
    }
    fn text(&mut self, max: usize) -> Result<String, CheckpointError> {
        let n = u32::from_be_bytes(self.fixed()?) as usize;
        if n > max {
            return Err(CheckpointError::Limit);
        }
        let value = std::str::from_utf8(self.raw(n)?).map_err(|_| CheckpointError::Invalid)?;
        let mut s = String::new();
        s.try_reserve(n).map_err(|_| CheckpointError::Capacity)?;
        s.push_str(value);
        Ok(s)
    }
    fn participant(&mut self, l: Limits) -> Result<ParticipantId, CheckpointError> {
        Ok(ParticipantId::new(self.text(l.max_participant_id_bytes)?))
    }
    fn room(&mut self, l: Limits) -> Result<RoomId, CheckpointError> {
        Ok(RoomId::new(self.text(l.max_room_id_bytes)?))
    }
    fn option(&mut self) -> Result<Option<EventSequence>, CheckpointError> {
        match self.byte()? {
            0 => Ok(None),
            1 => Ok(Some(EventSequence(self.number()?))),
            _ => Err(CheckpointError::Invalid),
        }
    }
    fn message(&mut self, l: Limits) -> Result<Message, CheckpointError> {
        let sequence = self.number()?;
        let author = self.participant(l)?;
        let at = LogicalTime(self.number()?);
        let body = self.text(l.max_message_body_bytes)?;
        let n = self.count(l.max_addressed_participants_per_message)?;
        let mut addressed_to = Vec::new();
        addressed_to
            .try_reserve(n)
            .map_err(|_| CheckpointError::Capacity)?;
        for _ in 0..n {
            addressed_to.push(self.participant(l)?)
        }
        Ok(Message {
            sequence,
            author,
            at,
            body,
            addressed_to,
        })
    }
    fn event(&mut self, l: Limits) -> Result<DomainEventV2, CheckpointError> {
        Ok(match self.byte()? {
            0 => DomainEventV2::ParticipantRegistered {
                participant_id: self.participant(l)?,
                kind: read_kind(self.byte()?)?,
            },
            1 => DomainEventV2::RoomCreated {
                room_id: self.room(l)?,
                owner: self.participant(l)?,
                admission: read_admission(self.byte()?)?,
            },
            2 => DomainEventV2::InvitationRecorded {
                room_id: self.room(l)?,
                participant: self.participant(l)?,
            },
            3 => DomainEventV2::ParticipantJoined {
                room_id: self.room(l)?,
                participant: self.participant(l)?,
            },
            4 => DomainEventV2::ParticipantLeft {
                room_id: self.room(l)?,
                participant: self.participant(l)?,
            },
            5 => DomainEventV2::RoleChanged {
                room_id: self.room(l)?,
                participant: self.participant(l)?,
                role: read_role(self.byte()?)?,
            },
            6 => DomainEventV2::MessageAccepted {
                room_id: self.room(l)?,
                message: self.message(l)?,
            },
            tag @ 7..=9 => {
                let room_id = self.room(l)?;
                let actor = self.participant(l)?;
                let participant = self.participant(l)?;
                let reason = ModerationReasonV1::new(self.text(l.max_moderation_reason_bytes)?);
                let causation = ModerationCausationV1::new(self.fixed()?);
                match tag {
                    7 => DomainEventV2::ParticipantKicked {
                        room_id,
                        actor,
                        participant,
                        reason,
                        causation,
                    },
                    8 => DomainEventV2::ParticipantBanned {
                        room_id,
                        actor,
                        participant,
                        reason,
                        causation,
                    },
                    9 => DomainEventV2::ParticipantUnbanned {
                        room_id,
                        actor,
                        participant,
                        reason,
                        causation,
                    },
                    _ => unreachable!(),
                }
            }
            10 => DomainEventV2::RoomPolicyChanged {
                room_id: self.room(l)?,
                actor: self.participant(l)?,
                previous: read_admission(self.byte()?)?,
                admission: read_admission(self.byte()?)?,
            },
            _ => return Err(CheckpointError::Invalid),
        })
    }
}
fn kind_tag(v: ParticipantKind) -> u8 {
    match v {
        ParticipantKind::Human => 0,
        ParticipantKind::Agent => 1,
    }
}
fn read_kind(v: u8) -> Result<ParticipantKind, CheckpointError> {
    match v {
        0 => Ok(ParticipantKind::Human),
        1 => Ok(ParticipantKind::Agent),
        _ => Err(CheckpointError::Invalid),
    }
}
fn admission_tag(v: AdmissionPolicy) -> u8 {
    match v {
        AdmissionPolicy::Open => 0,
        AdmissionPolicy::InviteOnly => 1,
    }
}
fn read_admission(v: u8) -> Result<AdmissionPolicy, CheckpointError> {
    match v {
        0 => Ok(AdmissionPolicy::Open),
        1 => Ok(AdmissionPolicy::InviteOnly),
        _ => Err(CheckpointError::Invalid),
    }
}
fn role_tag(v: Role) -> u8 {
    match v {
        Role::Owner => 0,
        Role::Moderator => 1,
        Role::Member => 2,
    }
}
fn read_role(v: u8) -> Result<Role, CheckpointError> {
    match v {
        0 => Ok(Role::Owner),
        1 => Ok(Role::Moderator),
        2 => Ok(Role::Member),
        _ => Err(CheckpointError::Invalid),
    }
}
fn limits_values(l: Limits) -> [usize; 15] {
    [
        l.max_participants,
        l.max_rooms,
        l.max_members_per_room,
        l.max_invitations_per_room,
        l.max_bans_per_room,
        l.max_messages_per_room,
        l.max_pending_events,
        l.max_publication_receipts,
        l.max_addressed_participants_per_message,
        l.max_participant_id_bytes,
        l.max_room_id_bytes,
        l.max_display_name_bytes,
        l.max_room_name_bytes,
        l.max_message_body_bytes,
        l.max_moderation_reason_bytes,
    ]
}
fn read_limits(r: &mut Reader<'_>, c: Limits) -> Result<Limits, CheckpointError> {
    Ok(Limits {
        max_participants: r.size(c.max_participants)?,
        max_rooms: r.size(c.max_rooms)?,
        max_members_per_room: r.size(c.max_members_per_room)?,
        max_invitations_per_room: r.size(c.max_invitations_per_room)?,
        max_bans_per_room: r.size(c.max_bans_per_room)?,
        max_messages_per_room: r.size(c.max_messages_per_room)?,
        max_pending_events: r.size(c.max_pending_events)?,
        max_publication_receipts: r.size(c.max_publication_receipts)?,
        max_addressed_participants_per_message: r.size(c.max_addressed_participants_per_message)?,
        max_participant_id_bytes: r.size(c.max_participant_id_bytes)?,
        max_room_id_bytes: r.size(c.max_room_id_bytes)?,
        max_display_name_bytes: r.size(c.max_display_name_bytes)?,
        max_room_name_bytes: r.size(c.max_room_name_bytes)?,
        max_message_body_bytes: r.size(c.max_message_body_bytes)?,
        max_moderation_reason_bytes: r.size(c.max_moderation_reason_bytes)?,
    })
}
fn insert<K: Ord, V>(map: &mut BTreeMap<K, V>, key: K, value: V) -> Result<(), CheckpointError> {
    if map.last_key_value().is_some_and(|(old, _)| old >= &key) {
        return Err(CheckpointError::NonCanonical);
    }
    map.insert(key, value);
    Ok(())
}

impl Engine {
    pub fn prepare_checkpoint(
        &self,
        max_snapshot_bytes: usize,
    ) -> Result<CheckpointV2, CheckpointError> {
        validate(self)?;
        let mut w = Writer {
            bytes: Vec::new(),
            max: max_snapshot_bytes,
        };
        w.raw(b"AGORACP\0")?;
        w.byte(4)?;
        for n in limits_values(self.limits) {
            w.count(n)?
        }
        w.number(self.revision.0)?;
        w.number(self.next_event_sequence.0)?;
        w.option(self.last_published_event)?;
        w.option(self.acknowledged_through)?;
        w.count(self.participants.len())?;
        for (id, p) in &self.participants {
            w.text(id.as_str())?;
            w.text(&p.display_name)?;
            w.byte(kind_tag(p.kind))?
        }
        w.count(self.rooms.len())?;
        for (id, room) in &self.rooms {
            w.text(id.as_str())?;
            w.text(&room.name)?;
            w.byte(admission_tag(room.admission))?;
            w.number(room.next_message_sequence)?;
            w.count(room.members.len())?;
            for (id, role) in &room.members {
                w.text(id.as_str())?;
                w.byte(role_tag(*role))?
            }
            w.count(room.invitations.len())?;
            for id in &room.invitations {
                w.text(id.as_str())?
            }
            w.count(room.bans.len())?;
            for (id, role) in &room.bans {
                w.text(id.as_str())?;
                w.byte(role_tag(*role))?
            }
            w.count(room.messages.len())?;
            for m in &room.messages {
                w.message(m)?
            }
        }
        w.count(self.publications.len())?;
        for (key, p) in &self.publications {
            w.raw(&key.0)?;
            w.text(p.room_id.as_str())?;
            w.text(p.author.as_str())?;
            w.number(p.at.0)?;
            w.text(&p.body)?;
            w.number(p.transition.revision.0)?;
            w.number(p.transition.event_sequence.0)?;
            let OutcomeV2::MessageAccepted { room_id, sequence } = &p.transition.outcome else {
                return Err(CheckpointError::Invalid);
            };
            w.text(room_id.as_str())?;
            w.number(*sequence)?;
        }
        w.count(self.pending_events.len())?;
        for e in &self.pending_events {
            w.number(e.sequence.0)?;
            w.number(e.revision.0)?;
            w.event(&e.event)?
        }
        Ok(CheckpointV2(w.bytes))
    }
    pub fn recover_checkpoint(
        bytes: &[u8],
        bounds: RecoveryLimits,
    ) -> Result<Self, CheckpointError> {
        if bytes.len() > bounds.max_snapshot_bytes {
            return Err(CheckpointError::Limit);
        }
        let mut r = Reader { bytes, pos: 0 };
        if r.raw(8)? != b"AGORACP\0" || r.byte()? != 4 {
            return Err(CheckpointError::Profile);
        }
        let l = read_limits(&mut r, bounds.maximum_engine_limits)?;
        let mut e = Engine::with_limits(l);
        e.revision = Revision(r.number()?);
        e.next_event_sequence = EventSequence(r.number()?);
        e.last_published_event = r.option()?;
        e.acknowledged_through = r.option()?;
        let n = r.count(l.max_participants)?;
        for _ in 0..n {
            let id = r.participant(l)?;
            let p = Participant::new(
                id.clone(),
                r.text(l.max_display_name_bytes)?,
                read_kind(r.byte()?)?,
            );
            insert(&mut e.participants, id, p)?
        }
        let n = r.count(l.max_rooms)?;
        for _ in 0..n {
            let id = r.room(l)?;
            let name = r.text(l.max_room_name_bytes)?;
            let admission = read_admission(r.byte()?)?;
            let next_message_sequence = r.number()?;
            let mut members = BTreeMap::new();
            let n = r.count(l.max_members_per_room)?;
            for _ in 0..n {
                let id = r.participant(l)?;
                let role = read_role(r.byte()?)?;
                insert(&mut members, id, role)?
            }
            let mut invitations = BTreeSet::new();
            let n = r.count(l.max_invitations_per_room)?;
            for _ in 0..n {
                let id = r.participant(l)?;
                if invitations.last().is_some_and(|old| old >= &id) {
                    return Err(CheckpointError::NonCanonical);
                }
                invitations.insert(id);
            }
            let n = r.count(l.max_bans_per_room)?;
            let mut bans = BTreeMap::new();
            for _ in 0..n {
                let id = r.participant(l)?;
                if bans.last_key_value().is_some_and(|(old, _)| old >= &id) {
                    return Err(CheckpointError::NonCanonical);
                }
                let role = read_role(r.byte()?)?;
                if bans.insert(id, role).is_some() {
                    return Err(CheckpointError::NonCanonical);
                }
            }
            let n = r.count(l.max_messages_per_room)?;
            let mut messages = VecDeque::new();
            messages
                .try_reserve(n)
                .map_err(|_| CheckpointError::Capacity)?;
            for _ in 0..n {
                messages.push_back(r.message(l)?)
            }
            insert(
                &mut e.rooms,
                id.clone(),
                Room {
                    id,
                    name,
                    admission,
                    members,
                    invitations,
                    bans,
                    messages,
                    next_message_sequence,
                },
            )?;
        }
        let n = r.count(l.max_publication_receipts)?;
        for _ in 0..n {
            let key = PublicationKeyV2::new(r.fixed()?);
            let room_id = r.room(l)?;
            let author = r.participant(l)?;
            let at = LogicalTime(r.number()?);
            let body = r.text(l.max_message_body_bytes)?;
            let revision = Revision(r.number()?);
            let event_sequence = EventSequence(r.number()?);
            let outcome = OutcomeV2::MessageAccepted {
                room_id: r.room(l)?,
                sequence: r.number()?,
            };
            insert(
                &mut e.publications,
                key,
                PublicationRecordV2 {
                    room_id,
                    author,
                    at,
                    body,
                    transition: TransitionV2 {
                        revision,
                        event_sequence,
                        outcome,
                    },
                },
            )?;
        }
        let n = r.count(l.max_pending_events)?;
        e.pending_events
            .try_reserve(n)
            .map_err(|_| CheckpointError::Capacity)?;
        for _ in 0..n {
            e.pending_events.push_back(EventEnvelopeV2 {
                sequence: EventSequence(r.number()?),
                revision: Revision(r.number()?),
                event: r.event(l)?,
            })
        }
        if r.pos != bytes.len() {
            return Err(CheckpointError::Trailing);
        }
        validate(&e)?;
        Ok(e)
    }
}

const CURSOR_MAGIC: &[u8; 8] = b"AGORACUR";

fn prepare_cursor(
    kind: u8,
    revision: Revision,
    observer: &ParticipantId,
    room_id: Option<&RoomId>,
    after_text: Option<&str>,
    after_sequence: Option<u64>,
    max_snapshot_bytes: usize,
) -> Result<CursorCheckpointV1, CheckpointError> {
    let mut w = Writer {
        bytes: Vec::new(),
        max: max_snapshot_bytes,
    };
    w.raw(CURSOR_MAGIC)?;
    w.byte(1)?;
    w.byte(kind)?;
    w.number(revision.0)?;
    w.text(observer.as_str())?;
    if let Some(room_id) = room_id {
        w.text(room_id.as_str())?;
    }
    if let Some(after) = after_text {
        w.text(after)?;
    }
    if let Some(after) = after_sequence {
        w.number(after)?;
    }
    Ok(CursorCheckpointV1(w.bytes))
}

fn cursor_reader<'a>(
    bytes: &'a [u8],
    bounds: CursorRecoveryLimitsV1,
    kind: u8,
) -> Result<Reader<'a>, CheckpointError> {
    if bytes.len() > bounds.max_snapshot_bytes {
        return Err(CheckpointError::Limit);
    }
    let mut r = Reader { bytes, pos: 0 };
    if r.raw(CURSOR_MAGIC.len())? != CURSOR_MAGIC || r.byte()? != 1 || r.byte()? != kind {
        return Err(CheckpointError::Profile);
    }
    Ok(r)
}

fn finish_cursor(r: &Reader<'_>) -> Result<(), CheckpointError> {
    if r.pos == r.bytes.len() {
        Ok(())
    } else {
        Err(CheckpointError::Trailing)
    }
}

impl RoomPageCursor {
    pub fn prepare_checkpoint(
        &self,
        max_snapshot_bytes: usize,
    ) -> Result<CursorCheckpointV1, CheckpointError> {
        prepare_cursor(
            1,
            self.revision,
            &self.observer,
            None,
            Some(self.after.as_str()),
            None,
            max_snapshot_bytes,
        )
    }

    pub fn recover_checkpoint(
        bytes: &[u8],
        bounds: CursorRecoveryLimitsV1,
    ) -> Result<Self, CheckpointError> {
        let mut r = cursor_reader(bytes, bounds, 1)?;
        let cursor = Self {
            revision: Revision(r.number()?),
            observer: ParticipantId::new(r.text(bounds.max_participant_id_bytes)?),
            after: RoomId::new(r.text(bounds.max_room_id_bytes)?),
        };
        finish_cursor(&r)?;
        Ok(cursor)
    }
}

impl MemberPageCursor {
    pub fn prepare_checkpoint(
        &self,
        max_snapshot_bytes: usize,
    ) -> Result<CursorCheckpointV1, CheckpointError> {
        prepare_cursor(
            2,
            self.revision,
            &self.observer,
            Some(&self.room_id),
            Some(self.after.as_str()),
            None,
            max_snapshot_bytes,
        )
    }

    pub fn recover_checkpoint(
        bytes: &[u8],
        bounds: CursorRecoveryLimitsV1,
    ) -> Result<Self, CheckpointError> {
        let mut r = cursor_reader(bytes, bounds, 2)?;
        let cursor = Self {
            revision: Revision(r.number()?),
            observer: ParticipantId::new(r.text(bounds.max_participant_id_bytes)?),
            room_id: RoomId::new(r.text(bounds.max_room_id_bytes)?),
            after: ParticipantId::new(r.text(bounds.max_participant_id_bytes)?),
        };
        finish_cursor(&r)?;
        Ok(cursor)
    }
}

impl MessagePageCursor {
    pub fn prepare_checkpoint(
        &self,
        max_snapshot_bytes: usize,
    ) -> Result<CursorCheckpointV1, CheckpointError> {
        prepare_cursor(
            3,
            self.revision,
            &self.observer,
            Some(&self.room_id),
            None,
            Some(self.after_sequence),
            max_snapshot_bytes,
        )
    }

    pub fn recover_checkpoint(
        bytes: &[u8],
        bounds: CursorRecoveryLimitsV1,
    ) -> Result<Self, CheckpointError> {
        let mut r = cursor_reader(bytes, bounds, 3)?;
        let cursor = Self {
            revision: Revision(r.number()?),
            observer: ParticipantId::new(r.text(bounds.max_participant_id_bytes)?),
            room_id: RoomId::new(r.text(bounds.max_room_id_bytes)?),
            after_sequence: r.number()?,
        };
        finish_cursor(&r)?;
        Ok(cursor)
    }
}

impl RoomChangePosition {
    pub fn prepare_checkpoint(
        &self,
        max_snapshot_bytes: usize,
    ) -> Result<CursorCheckpointV1, CheckpointError> {
        let mut writer = Writer {
            bytes: Vec::new(),
            max: max_snapshot_bytes,
        };
        writer.raw(CURSOR_MAGIC)?;
        writer.byte(1)?;
        writer.byte(4)?;
        writer.number(self.observed_revision.0)?;
        writer.text(self.observer.as_str())?;
        writer.text(self.room_id.as_str())?;
        writer.option(self.after)?;
        Ok(CursorCheckpointV1(writer.bytes))
    }

    pub fn recover_checkpoint(
        bytes: &[u8],
        bounds: CursorRecoveryLimitsV1,
    ) -> Result<Self, CheckpointError> {
        let mut reader = cursor_reader(bytes, bounds, 4)?;
        let position = Self {
            observed_revision: Revision(reader.number()?),
            observer: ParticipantId::new(reader.text(bounds.max_participant_id_bytes)?),
            room_id: RoomId::new(reader.text(bounds.max_room_id_bytes)?),
            after: reader.option()?,
        };
        finish_cursor(&reader)?;
        if position.observer.as_str().is_empty()
            || position.room_id.as_str().is_empty()
            || match position.after {
                Some(after) => after.0.checked_add(1) != Some(position.observed_revision.0),
                None => position.observed_revision != Revision(0),
            }
        {
            return Err(CheckpointError::Invalid);
        }
        Ok(position)
    }
}
fn bounded(s: &str, max: usize) -> Result<(), CheckpointError> {
    if s.is_empty() || s.len() > max {
        Err(CheckpointError::Invalid)
    } else {
        Ok(())
    }
}
fn owner(room: &Room) -> Result<&ParticipantId, CheckpointError> {
    let mut owners = room
        .members
        .iter()
        .filter(|(_, role)| **role == Role::Owner);
    let (id, _) = owners.next().ok_or(CheckpointError::Invalid)?;
    if owners.next().is_some() {
        return Err(CheckpointError::Invalid);
    }
    Ok(id)
}
fn valid_message(e: &Engine, room: &Room, m: &Message) -> Result<(), CheckpointError> {
    valid_message_parts(e, room, m.sequence, &m.author, m.at, &m.body)?;
    validate_addressing(e, &m.addressed_to)
}
fn validate_addressing(e: &Engine, addressed_to: &[ParticipantId]) -> Result<(), CheckpointError> {
    if addressed_to.len() > e.limits.max_addressed_participants_per_message
        || addressed_to.windows(2).any(|pair| pair[0] >= pair[1])
        || addressed_to
            .iter()
            .any(|id| !e.participants.contains_key(id))
    {
        return Err(CheckpointError::Invalid);
    }
    Ok(())
}
fn valid_message_parts(
    e: &Engine,
    room: &Room,
    sequence: u64,
    author: &ParticipantId,
    at: LogicalTime,
    body: &str,
) -> Result<(), CheckpointError> {
    bounded(body, e.limits.max_message_body_bytes)?;
    if !e.participants.contains_key(author) || sequence >= room.next_message_sequence {
        return Err(CheckpointError::Invalid);
    }
    let start = room
        .next_message_sequence
        .checked_sub(room.messages.len() as u64)
        .ok_or(CheckpointError::Invalid)?;
    if sequence >= start {
        let index: usize = (sequence - start)
            .try_into()
            .map_err(|_| CheckpointError::Invalid)?;
        let current = room.messages.get(index).ok_or(CheckpointError::Invalid)?;
        if current.sequence != sequence
            || &current.author != author
            || current.at != at
            || current.body != body
        {
            return Err(CheckpointError::Invalid);
        }
    }
    Ok(())
}
fn validate(e: &Engine) -> Result<(), CheckpointError> {
    let l = e.limits;
    if e.participants.len() > l.max_participants
        || e.rooms.len() > l.max_rooms
        || e.publications.len() > l.max_publication_receipts
        || e.pending_events.len() > l.max_pending_events
    {
        return Err(CheckpointError::Limit);
    }
    if e.next_event_sequence.0 != e.revision.0
        || e.last_published_event != e.revision.0.checked_sub(1).map(EventSequence)
    {
        return Err(CheckpointError::Invalid);
    }
    if e.acknowledged_through
        .is_some_and(|a| e.last_published_event.is_none_or(|last| a > last))
    {
        return Err(CheckpointError::Invalid);
    }
    let first = e
        .acknowledged_through
        .map_or(Some(0), |a| a.0.checked_add(1))
        .ok_or(CheckpointError::Invalid)?;
    if e.revision.0.checked_sub(first) != Some(e.pending_events.len() as u64) {
        return Err(CheckpointError::Invalid);
    }
    let mut minimum = (e.participants.len() as u64)
        .checked_add(e.rooms.len() as u64)
        .ok_or(CheckpointError::Invalid)?;
    for (id, p) in &e.participants {
        if id != &p.id {
            return Err(CheckpointError::Invalid);
        }
        bounded(id.as_str(), l.max_participant_id_bytes)?;
        bounded(&p.display_name, l.max_display_name_bytes)?
    }
    for (id, room) in &e.rooms {
        if id != &room.id
            || room.members.len() > l.max_members_per_room
            || room.invitations.len() > l.max_invitations_per_room
            || room.bans.len() > l.max_bans_per_room
            || room.messages.len() > l.max_messages_per_room
        {
            return Err(CheckpointError::Invalid);
        }
        bounded(id.as_str(), l.max_room_id_bytes)?;
        bounded(&room.name, l.max_room_name_bytes)?;
        owner(room)?;
        if room
            .members
            .keys()
            .chain(room.invitations.iter())
            .chain(room.bans.keys())
            .any(|id| !e.participants.contains_key(id))
            || room
                .invitations
                .iter()
                .any(|id| room.members.contains_key(id))
            || room
                .bans
                .keys()
                .any(|id| room.members.contains_key(id) || room.invitations.contains(id))
        {
            return Err(CheckpointError::Invalid);
        }
        if l.max_messages_per_room == 0 && room.next_message_sequence != 0 {
            return Err(CheckpointError::Invalid);
        }
        if room.messages.len() as u64
            != room
                .next_message_sequence
                .min(l.max_messages_per_room as u64)
        {
            return Err(CheckpointError::Invalid);
        }
        let start = room
            .next_message_sequence
            .checked_sub(room.messages.len() as u64)
            .ok_or(CheckpointError::Invalid)?;
        for (i, m) in room.messages.iter().enumerate() {
            if m.sequence != start + i as u64 {
                return Err(CheckpointError::Invalid);
            }
            valid_message(e, room, m)?
        }
        minimum = minimum
            .checked_add(room.next_message_sequence)
            .and_then(|n| n.checked_add((room.members.len() - 1) as u64))
            .and_then(|n| n.checked_add(room.invitations.len() as u64))
            .and_then(|n| n.checked_add(room.bans.len() as u64))
            .ok_or(CheckpointError::Invalid)?;
    }
    if minimum > e.revision.0 {
        return Err(CheckpointError::Invalid);
    }
    let mut revisions = BTreeMap::new();
    let mut messages = BTreeSet::new();
    for p in e.publications.values() {
        let room = e.rooms.get(&p.room_id).ok_or(CheckpointError::Invalid)?;
        let OutcomeV2::MessageAccepted { room_id, sequence } = &p.transition.outcome else {
            return Err(CheckpointError::Invalid);
        };
        if room_id != &p.room_id
            || p.transition.revision.0 == 0
            || p.transition.revision > e.revision
            || p.transition.event_sequence.0.checked_add(1) != Some(p.transition.revision.0)
            || revisions.insert(p.transition.revision, p).is_some()
            || !messages.insert((room_id, *sequence))
        {
            return Err(CheckpointError::Invalid);
        }
        valid_message_parts(e, room, *sequence, &p.author, p.at, &p.body)?;
        if e.message(room_id, *sequence)
            .is_some_and(|message| !message.addressed_to.is_empty())
        {
            return Err(CheckpointError::Invalid);
        }
    }
    for (i, envelope) in e.pending_events.iter().enumerate() {
        if envelope.sequence.0 != first + i as u64 || envelope.revision.0 != envelope.sequence.0 + 1
        {
            return Err(CheckpointError::Invalid);
        }
        validate_event(e, &envelope.event)?;
        if let Some(p) = revisions.get(&envelope.revision) {
            let DomainEventV2::MessageAccepted {
                room_id, message, ..
            } = &envelope.event
            else {
                return Err(CheckpointError::Invalid);
            };
            let OutcomeV2::MessageAccepted { sequence, .. } = &p.transition.outcome else {
                return Err(CheckpointError::Invalid);
            };
            if room_id != &p.room_id
                || message.sequence != *sequence
                || message.author != p.author
                || message.at != p.at
                || message.body != p.body
                || !message.addressed_to.is_empty()
            {
                return Err(CheckpointError::Invalid);
            }
        }
    }
    Ok(())
}
fn validate_event(e: &Engine, event: &DomainEventV2) -> Result<(), CheckpointError> {
    let participant = |id: &ParticipantId| e.participants.get(id).ok_or(CheckpointError::Invalid);
    let room = |id: &RoomId| e.rooms.get(id).ok_or(CheckpointError::Invalid);
    match event {
        DomainEventV2::ParticipantRegistered {
            participant_id,
            kind,
        } => {
            if participant(participant_id)?.kind != *kind {
                return Err(CheckpointError::Invalid);
            }
        }
        DomainEventV2::RoomCreated {
            room_id,
            owner: id,
            admission: _,
        } => {
            let r = room(room_id)?;
            if owner(r)? != id {
                return Err(CheckpointError::Invalid);
            }
        }
        DomainEventV2::InvitationRecorded {
            room_id,
            participant: id,
        } => {
            room(room_id)?;
            participant(id)?;
        }
        DomainEventV2::ParticipantJoined {
            room_id,
            participant: id,
        }
        | DomainEventV2::ParticipantLeft {
            room_id,
            participant: id,
        } => {
            if owner(room(room_id)?)? == id {
                return Err(CheckpointError::Invalid);
            }
            participant(id)?;
        }
        DomainEventV2::RoleChanged {
            room_id,
            participant: id,
            role,
        } => {
            room(room_id)?;
            participant(id)?;
            if *role == Role::Owner {
                return Err(CheckpointError::Invalid);
            }
        }
        DomainEventV2::RoomPolicyChanged {
            room_id,
            actor,
            previous,
            admission,
        } => {
            let r = room(room_id)?;
            participant(actor)?;
            if owner(r)? != actor || previous == admission {
                return Err(CheckpointError::Invalid);
            }
        }
        DomainEventV2::ParticipantKicked {
            room_id,
            actor,
            participant: id,
            reason,
            ..
        }
        | DomainEventV2::ParticipantBanned {
            room_id,
            actor,
            participant: id,
            reason,
            ..
        }
        | DomainEventV2::ParticipantUnbanned {
            room_id,
            actor,
            participant: id,
            reason,
            ..
        } => {
            room(room_id)?;
            participant(actor)?;
            participant(id)?;
            bounded(reason.as_str(), e.limits.max_moderation_reason_bytes)?;
            if actor == id || owner(room(room_id)?)? == id {
                return Err(CheckpointError::Invalid);
            }
        }
        DomainEventV2::MessageAccepted { room_id, message } => {
            let r = room(room_id)?;
            valid_message(e, r, message)?;
        }
    }
    Ok(())
}
