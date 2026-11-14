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
    fn message_id_option(&mut self, value: Option<MessageId>) -> Result<(), CheckpointError> {
        self.byte(u8::from(value.is_some()))?;
        if let Some(value) = value {
            self.raw(&value.into_bytes())?;
        }
        Ok(())
    }
    fn message(
        &mut self,
        m: &Message,
        include_vellum_artifacts: bool,
    ) -> Result<(), CheckpointError> {
        self.raw(&m.id.0)?;
        self.number(m.sequence)?;
        self.text(m.author.as_str())?;
        self.number(m.at.0)?;
        self.text(&m.body)?;
        self.count(m.addressed_to.len())?;
        for id in &m.addressed_to {
            self.text(id.as_str())?
        }
        self.message_id_option(m.reply_to_root)?;
        match m.topic_id {
            None => self.byte(0)?,
            Some(topic_id) => {
                self.byte(1)?;
                self.raw(&topic_id.into_bytes())?;
            }
        }
        self.count(m.attachments.len())?;
        for attachment in &m.attachments {
            self.raw(&attachment.attachment_id.into_bytes())?;
            match &attachment.descriptor {
                None => self.byte(0)?,
                Some(descriptor) => {
                    if descriptor.attachment_id != attachment.attachment_id {
                        return Err(CheckpointError::Invalid);
                    }
                    self.byte(1)?;
                    self.raw(&descriptor.store_id.into_bytes())?;
                    self.raw(&descriptor.object_key.into_bytes())?;
                    self.number(descriptor.declared_bytes)?;
                    self.text(&descriptor.media_type)?;
                    self.text(&descriptor.display_label)?;
                    match descriptor.integrity {
                        AttachmentIntegrityV1::Sha256(digest) => {
                            self.byte(1)?;
                            self.raw(&digest.into_bytes())?;
                        }
                    }
                }
            }
        }
        if include_vellum_artifacts {
            self.count(m.vellum_artifacts.len())?;
            for artifact in &m.vellum_artifacts {
                self.raw(&artifact.reference_id.into_bytes())?;
                match artifact.target {
                    None => self.byte(0)?,
                    Some(target) => {
                        self.byte(1)?;
                        self.number(target.workspace_id())?;
                        self.number(target.document_id())?;
                        self.number(target.source_revision())?;
                    }
                }
            }
        }
        self.count(m.revisions.len())?;
        for revision in &m.revisions {
            self.number(u64::from(revision.number))?;
            self.text(revision.editor.as_str())?;
            self.number(revision.at.0)?;
            self.byte(match revision.availability {
                MessageRevisionAvailabilityV1::Available => 0,
                MessageRevisionAvailabilityV1::Tombstoned => 1,
            })?;
            self.text(&revision.body)?;
        }
        Ok(())
    }
    fn event(
        &mut self,
        e: &DomainEventV2,
        include_vellum_artifacts: bool,
    ) -> Result<(), CheckpointError> {
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
            DomainEventV2::PeerConversationCreated {
                room_id,
                initiator,
                profile,
                founding_members,
            } => {
                self.byte(22)?;
                self.text(room_id.as_str())?;
                self.text(initiator.as_str())?;
                self.byte(conversation_profile_tag(*profile))?;
                self.count(founding_members.len())?;
                for participant in founding_members {
                    self.text(participant.as_str())?;
                }
                Ok(())
            }
            DomainEventV2::TopicCreated {
                room_id,
                actor,
                topic_id,
                name,
            } => {
                self.byte(16)?;
                self.text(room_id.as_str())?;
                self.text(actor.as_str())?;
                self.raw(&topic_id.into_bytes())?;
                self.text(name)
            }
            DomainEventV2::TopicRenamed {
                room_id,
                actor,
                topic_id,
                previous,
                name,
            } => {
                self.byte(17)?;
                self.text(room_id.as_str())?;
                self.text(actor.as_str())?;
                self.raw(&topic_id.into_bytes())?;
                self.text(previous)?;
                self.text(name)
            }
            DomainEventV2::TopicLifecycleChanged {
                room_id,
                actor,
                topic_id,
                previous_resolved,
                resolved,
            } => {
                self.byte(18)?;
                self.text(room_id.as_str())?;
                self.text(actor.as_str())?;
                self.raw(&topic_id.into_bytes())?;
                self.byte(u8::from(*previous_resolved))?;
                self.byte(u8::from(*resolved))
            }
            DomainEventV2::TopicSupersessionChanged {
                room_id,
                actor,
                topic_id,
                previous_successor_topic_id,
                successor_topic_id,
            } => {
                self.byte(19)?;
                self.text(room_id.as_str())?;
                self.text(actor.as_str())?;
                self.raw(&topic_id.into_bytes())?;
                self.byte(u8::from(previous_successor_topic_id.is_some()))?;
                if let Some(previous) = previous_successor_topic_id {
                    self.raw(&previous.into_bytes())?;
                }
                self.byte(u8::from(successor_topic_id.is_some()))?;
                if let Some(successor) = successor_topic_id {
                    self.raw(&successor.into_bytes())?;
                }
                Ok(())
            }
            DomainEventV2::TopicAliasChanged {
                room_id,
                actor,
                topic_id,
                alias,
                added,
            } => {
                self.byte(20)?;
                self.text(room_id.as_str())?;
                self.text(actor.as_str())?;
                self.raw(&topic_id.into_bytes())?;
                self.text(alias)?;
                self.byte(u8::from(*added))
            }
            DomainEventV2::TopicTagChanged {
                room_id,
                actor,
                topic_id,
                tag,
                added,
            } => {
                self.byte(21)?;
                self.text(room_id.as_str())?;
                self.text(actor.as_str())?;
                self.raw(&topic_id.into_bytes())?;
                self.text(tag)?;
                self.byte(u8::from(*added))
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
                self.message(message, include_vellum_artifacts)
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
            DomainEventV2::MessageWithdrawn {
                room_id,
                actor,
                message_id,
                sequence,
            } => {
                self.byte(11)?;
                self.text(room_id.as_str())?;
                self.text(actor.as_str())?;
                self.raw(&message_id.into_bytes())?;
                self.number(*sequence)
            }
            DomainEventV2::MessageRedacted {
                room_id,
                actor,
                author,
                message_id,
                sequence,
                reason,
                causation,
            } => {
                self.byte(12)?;
                self.text(room_id.as_str())?;
                self.text(actor.as_str())?;
                self.text(author.as_str())?;
                self.raw(&message_id.into_bytes())?;
                self.number(*sequence)?;
                self.text(reason.as_str())?;
                self.raw(&causation.into_bytes())
            }
            DomainEventV2::ThreadLifecycleChanged {
                room_id,
                actor,
                root_message_id,
                archived,
                reason,
                causation,
            } => {
                self.byte(13)?;
                self.text(room_id.as_str())?;
                self.text(actor.as_str())?;
                self.raw(&root_message_id.into_bytes())?;
                self.byte(u8::from(*archived))?;
                self.text(reason.as_str())?;
                self.raw(&causation.into_bytes())
            }
            DomainEventV2::ReactionChanged {
                room_id,
                actor,
                message_id,
                key,
                reacted,
            } => {
                self.byte(14)?;
                self.text(room_id.as_str())?;
                self.text(actor.as_str())?;
                self.raw(&message_id.into_bytes())?;
                self.text(key)?;
                self.byte(u8::from(*reacted))
            }
            DomainEventV2::MessageEdited {
                room_id,
                actor,
                message_id,
                message_revision,
                at,
                body,
            } => {
                self.byte(15)?;
                self.text(room_id.as_str())?;
                self.text(actor.as_str())?;
                self.raw(&message_id.into_bytes())?;
                self.number(u64::from(*message_revision))?;
                self.number(at.0)?;
                self.text(body)
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
    fn message_id_option(&mut self) -> Result<Option<MessageId>, CheckpointError> {
        match self.byte()? {
            0 => Ok(None),
            1 => Ok(Some(MessageId::from_bytes(self.fixed()?))),
            _ => Err(CheckpointError::Invalid),
        }
    }
    fn message(
        &mut self,
        l: Limits,
        has_vellum_artifacts: bool,
    ) -> Result<Message, CheckpointError> {
        let id = MessageId(self.fixed()?);
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
        let reply_to_root = self.message_id_option()?;
        let topic_id = match self.byte()? {
            0 => None,
            1 => Some(TopicId::from_bytes(self.fixed()?)),
            _ => return Err(CheckpointError::Invalid),
        };
        let attachment_count = self.count(MAX_ATTACHMENTS_PER_MESSAGE_V1)?;
        let mut attachments = Vec::new();
        attachments
            .try_reserve(attachment_count)
            .map_err(|_| CheckpointError::Capacity)?;
        for _ in 0..attachment_count {
            let attachment_id = AttachmentIdV1::new(self.fixed()?);
            let descriptor = match self.byte()? {
                0 => None,
                1 => {
                    let store_id = AttachmentStoreIdV1::new(self.fixed()?);
                    let object_key = AttachmentObjectKeyV1::new(self.fixed()?);
                    let declared_bytes = self.number()?;
                    let media_type = self.text(l.max_attachment_media_type_bytes)?;
                    let display_label = self.text(l.max_attachment_display_label_bytes)?;
                    let integrity = match self.byte()? {
                        1 => AttachmentIntegrityV1::Sha256(Sha256DigestV1::new(self.fixed()?)),
                        _ => return Err(CheckpointError::Invalid),
                    };
                    Some(AttachmentDescriptorV1::new(
                        attachment_id,
                        store_id,
                        object_key,
                        declared_bytes,
                        media_type,
                        display_label,
                        integrity,
                    ))
                }
                _ => return Err(CheckpointError::Invalid),
            };
            attachments.push(MessageAttachmentV1 {
                attachment_id,
                descriptor,
            });
        }
        let mut vellum_artifacts = Vec::new();
        if has_vellum_artifacts {
            let count = self.count(MAX_VELLUM_ARTIFACT_REFERENCES_PER_MESSAGE_V1)?;
            vellum_artifacts
                .try_reserve(count)
                .map_err(|_| CheckpointError::Capacity)?;
            for _ in 0..count {
                let reference_id = VellumArtifactReferenceIdV1::new(self.fixed()?);
                let target = match self.byte()? {
                    0 => None,
                    1 => Some(
                        VellumDocumentTargetV1::new(self.number()?, self.number()?, self.number()?)
                            .ok_or(CheckpointError::Invalid)?,
                    ),
                    _ => return Err(CheckpointError::Invalid),
                };
                vellum_artifacts.push(MessageVellumArtifactReferenceV1 {
                    reference_id,
                    target,
                });
            }
        }
        let revision_count = self.count(MAX_MESSAGE_REVISIONS_V1)?;
        if revision_count == 0 {
            return Err(CheckpointError::Invalid);
        }
        let mut revisions = Vec::new();
        revisions
            .try_reserve(revision_count)
            .map_err(|_| CheckpointError::Capacity)?;
        for _ in 0..revision_count {
            let number = self
                .number()?
                .try_into()
                .map_err(|_| CheckpointError::Invalid)?;
            let editor = self.participant(l)?;
            let at = LogicalTime(self.number()?);
            let availability = match self.byte()? {
                0 => MessageRevisionAvailabilityV1::Available,
                1 => MessageRevisionAvailabilityV1::Tombstoned,
                _ => return Err(CheckpointError::Invalid),
            };
            let body = self.text(l.max_message_body_bytes)?;
            revisions.push(MessageRevisionV1 {
                number,
                editor,
                at,
                availability,
                body,
            });
        }
        Ok(Message {
            id,
            sequence,
            author,
            at,
            body,
            addressed_to,
            reply_to_root,
            topic_id,
            attachments,
            vellum_artifacts,
            revisions,
        })
    }
    fn event(
        &mut self,
        l: Limits,
        has_vellum_artifacts: bool,
        has_conversation_profiles: bool,
    ) -> Result<DomainEventV2, CheckpointError> {
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
                message: self.message(l, has_vellum_artifacts)?,
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
            11 => DomainEventV2::MessageWithdrawn {
                room_id: self.room(l)?,
                actor: self.participant(l)?,
                message_id: MessageId::from_bytes(self.fixed()?),
                sequence: self.number()?,
            },
            12 => DomainEventV2::MessageRedacted {
                room_id: self.room(l)?,
                actor: self.participant(l)?,
                author: self.participant(l)?,
                message_id: MessageId::from_bytes(self.fixed()?),
                sequence: self.number()?,
                reason: ModerationReasonV1::new(self.text(l.max_moderation_reason_bytes)?),
                causation: ModerationCausationV1::new(self.fixed()?),
            },
            13 => DomainEventV2::ThreadLifecycleChanged {
                room_id: self.room(l)?,
                actor: self.participant(l)?,
                root_message_id: MessageId::from_bytes(self.fixed()?),
                archived: match self.byte()? {
                    0 => false,
                    1 => true,
                    _ => return Err(CheckpointError::Invalid),
                },
                reason: ModerationReasonV1::new(self.text(l.max_moderation_reason_bytes)?),
                causation: ModerationCausationV1::new(self.fixed()?),
            },
            14 => DomainEventV2::ReactionChanged {
                room_id: self.room(l)?,
                actor: self.participant(l)?,
                message_id: MessageId::from_bytes(self.fixed()?),
                key: self.text(MAX_REACTION_KEY_BYTES_V1)?,
                reacted: match self.byte()? {
                    0 => false,
                    1 => true,
                    _ => return Err(CheckpointError::Invalid),
                },
            },
            15 => DomainEventV2::MessageEdited {
                room_id: self.room(l)?,
                actor: self.participant(l)?,
                message_id: MessageId::from_bytes(self.fixed()?),
                message_revision: self
                    .number()?
                    .try_into()
                    .map_err(|_| CheckpointError::Invalid)?,
                at: LogicalTime(self.number()?),
                body: self.text(l.max_message_body_bytes)?,
            },
            16 => DomainEventV2::TopicCreated {
                room_id: self.room(l)?,
                actor: self.participant(l)?,
                topic_id: TopicId::from_bytes(self.fixed()?),
                name: self.text(l.max_topic_name_bytes)?,
            },
            17 => DomainEventV2::TopicRenamed {
                room_id: self.room(l)?,
                actor: self.participant(l)?,
                topic_id: TopicId::from_bytes(self.fixed()?),
                previous: self.text(l.max_topic_name_bytes)?,
                name: self.text(l.max_topic_name_bytes)?,
            },
            18 => DomainEventV2::TopicLifecycleChanged {
                room_id: self.room(l)?,
                actor: self.participant(l)?,
                topic_id: TopicId::from_bytes(self.fixed()?),
                previous_resolved: match self.byte()? {
                    0 => false,
                    1 => true,
                    _ => return Err(CheckpointError::Invalid),
                },
                resolved: match self.byte()? {
                    0 => false,
                    1 => true,
                    _ => return Err(CheckpointError::Invalid),
                },
            },
            19 => DomainEventV2::TopicSupersessionChanged {
                room_id: self.room(l)?,
                actor: self.participant(l)?,
                topic_id: TopicId::from_bytes(self.fixed()?),
                previous_successor_topic_id: match self.byte()? {
                    0 => None,
                    1 => Some(TopicId::from_bytes(self.fixed()?)),
                    _ => return Err(CheckpointError::Invalid),
                },
                successor_topic_id: match self.byte()? {
                    0 => None,
                    1 => Some(TopicId::from_bytes(self.fixed()?)),
                    _ => return Err(CheckpointError::Invalid),
                },
            },
            20 => DomainEventV2::TopicAliasChanged {
                room_id: self.room(l)?,
                actor: self.participant(l)?,
                topic_id: TopicId::from_bytes(self.fixed()?),
                alias: self.text(l.max_topic_name_bytes)?,
                added: match self.byte()? {
                    0 => false,
                    1 => true,
                    _ => return Err(CheckpointError::Invalid),
                },
            },
            21 => DomainEventV2::TopicTagChanged {
                room_id: self.room(l)?,
                actor: self.participant(l)?,
                topic_id: TopicId::from_bytes(self.fixed()?),
                tag: self.text(l.max_topic_name_bytes)?,
                added: match self.byte()? {
                    0 => false,
                    1 => true,
                    _ => return Err(CheckpointError::Invalid),
                },
            },
            22 if has_conversation_profiles => {
                let room_id = self.room(l)?;
                let initiator = self.participant(l)?;
                let profile = read_conversation_profile(self.byte()?)?;
                let count = self.count(l.max_members_per_room)?;
                let mut founding_members = Vec::new();
                founding_members
                    .try_reserve_exact(count)
                    .map_err(|_| CheckpointError::Capacity)?;
                for _ in 0..count {
                    let participant = self.participant(l)?;
                    if founding_members
                        .last()
                        .is_some_and(|old| old >= &participant)
                    {
                        return Err(CheckpointError::NonCanonical);
                    }
                    founding_members.push(participant);
                }
                DomainEventV2::PeerConversationCreated {
                    room_id,
                    initiator,
                    profile,
                    founding_members,
                }
            }
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
fn conversation_profile_tag(value: ConversationProfileV1) -> u8 {
    match value {
        ConversationProfileV1::Ordinary => 0,
        ConversationProfileV1::Direct => 1,
        ConversationProfileV1::GroupDirect => 2,
    }
}
fn read_conversation_profile(value: u8) -> Result<ConversationProfileV1, CheckpointError> {
    match value {
        0 => Ok(ConversationProfileV1::Ordinary),
        1 => Ok(ConversationProfileV1::Direct),
        2 => Ok(ConversationProfileV1::GroupDirect),
        _ => Err(CheckpointError::Invalid),
    }
}
fn notification_intent_tag(value: NotificationIntentV1) -> u8 {
    match value {
        NotificationIntentV1::Default => 0,
        NotificationIntentV1::AllActivity => 1,
        NotificationIntentV1::AddressedOnly => 2,
        NotificationIntentV1::None => 3,
    }
}
fn read_notification_intent(value: u8) -> Result<NotificationIntentV1, CheckpointError> {
    match value {
        0 => Ok(NotificationIntentV1::Default),
        1 => Ok(NotificationIntentV1::AllActivity),
        2 => Ok(NotificationIntentV1::AddressedOnly),
        3 => Ok(NotificationIntentV1::None),
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
fn limits_values(l: Limits) -> [usize; 21] {
    [
        l.max_participants,
        l.max_rooms,
        l.max_members_per_room,
        l.max_invitations_per_room,
        l.max_bans_per_room,
        l.max_messages_per_room,
        l.max_topics_per_room,
        l.max_pending_events,
        l.max_retained_changes,
        l.max_publication_receipts,
        l.max_addressed_participants_per_message,
        l.max_participant_id_bytes,
        l.max_room_id_bytes,
        l.max_display_name_bytes,
        l.max_room_name_bytes,
        l.max_topic_name_bytes,
        l.max_message_body_bytes,
        l.max_moderation_reason_bytes,
        l.max_attachment_declared_bytes,
        l.max_attachment_media_type_bytes,
        l.max_attachment_display_label_bytes,
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
        max_topics_per_room: r.size(c.max_topics_per_room)?,
        max_pending_events: r.size(c.max_pending_events)?,
        max_retained_changes: r.size(c.max_retained_changes)?,
        max_publication_receipts: r.size(c.max_publication_receipts)?,
        max_addressed_participants_per_message: r.size(c.max_addressed_participants_per_message)?,
        max_participant_id_bytes: r.size(c.max_participant_id_bytes)?,
        max_room_id_bytes: r.size(c.max_room_id_bytes)?,
        max_display_name_bytes: r.size(c.max_display_name_bytes)?,
        max_room_name_bytes: r.size(c.max_room_name_bytes)?,
        max_topic_name_bytes: r.size(c.max_topic_name_bytes)?,
        max_message_body_bytes: r.size(c.max_message_body_bytes)?,
        max_moderation_reason_bytes: r.size(c.max_moderation_reason_bytes)?,
        max_attachment_declared_bytes: r.size(c.max_attachment_declared_bytes)?,
        max_attachment_media_type_bytes: r.size(c.max_attachment_media_type_bytes)?,
        max_attachment_display_label_bytes: r.size(c.max_attachment_display_label_bytes)?,
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
        let include_vellum_artifacts = engine_has_vellum_artifacts(self);
        let include_conversation_profiles = engine_has_conversation_profiles(self);
        let include_attention_preferences = !self.attention_preferences.is_empty();
        let include_private_saves = !self.private_saves.is_empty();
        w.raw(b"AGORACP\0")?;
        w.byte(
            20 + u8::from(include_vellum_artifacts)
                + (u8::from(include_conversation_profiles) * 2)
                + (u8::from(include_attention_preferences) * 4)
                + (u8::from(include_private_saves) * 8),
        )?;
        for n in limits_values(self.limits) {
            w.count(n)?
        }
        w.number(self.revision.0)?;
        w.number(self.next_event_sequence.0)?;
        w.option(self.last_published_event)?;
        w.option(self.acknowledged_through)?;
        w.option(self.discarded_changes_through)?;
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
            if include_conversation_profiles {
                w.byte(conversation_profile_tag(room.profile))?;
                w.count(room.founding_members.len())?;
                for participant in &room.founding_members {
                    w.text(participant.as_str())?;
                }
            }
            w.number(room.message_identity_incarnation)?;
            w.number(room.next_message_identity)?;
            w.number(room.next_message_sequence)?;
            w.number(room.topic_identity_incarnation)?;
            w.number(room.next_topic_identity)?;
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
                w.message(m, include_vellum_artifacts)?
            }
            w.count(room.thread_roots.len())?;
            for root in &room.thread_roots {
                w.raw(&root.into_bytes())?;
            }
            w.count(room.archived_threads.len())?;
            for root in &room.archived_threads {
                w.raw(&root.into_bytes())?;
            }
            w.count(room.reactions.len())?;
            for (message_id, by_key) in &room.reactions {
                w.raw(&message_id.into_bytes())?;
                w.count(by_key.len())?;
                for (key, participants) in by_key {
                    w.text(key)?;
                    w.count(participants.len())?;
                    for participant in participants {
                        w.text(participant.as_str())?;
                    }
                }
            }
            w.count(room.topics.len())?;
            for (topic_id, topic) in &room.topics {
                w.raw(&topic_id.into_bytes())?;
                w.text(&topic.name)?;
                w.text(topic.creator.as_str())?;
                w.byte(u8::from(topic.resolved))?;
                w.count(topic.aliases.len())?;
                for alias in &topic.aliases {
                    w.text(alias)?;
                }
                w.count(topic.tags.len())?;
                for tag in &topic.tags {
                    w.text(tag)?;
                }
            }
            w.count(room.topic_successors.len())?;
            for (topic_id, successor_topic_id) in &room.topic_successors {
                w.raw(&topic_id.into_bytes())?;
                w.raw(&successor_topic_id.into_bytes())?;
            }
        }
        w.count(self.read_positions.len())?;
        for ((participant, room_id, scope), position) in &self.read_positions {
            w.text(participant.as_str())?;
            w.text(room_id.as_str())?;
            match scope {
                ReadScopeV1::Room => w.byte(1)?,
                ReadScopeV1::Thread(root_message_id) => {
                    w.byte(2)?;
                    w.raw(&root_message_id.into_bytes())?;
                }
                ReadScopeV1::Topic(topic_id) => {
                    w.byte(3)?;
                    w.raw(&topic_id.into_bytes())?;
                }
            }
            w.raw(&position.target_message_id.into_bytes())?;
            w.number(position.target_sequence)?;
            w.number(position.observed_revision.0)?;
        }
        if include_attention_preferences {
            w.count(self.attention_preferences.len())?;
            for ((participant, room_id, scope), preference) in &self.attention_preferences {
                w.text(participant.as_str())?;
                w.text(room_id.as_str())?;
                match scope {
                    AttentionScopeV1::Room => w.byte(1)?,
                    AttentionScopeV1::Thread(root_message_id) => {
                        w.byte(2)?;
                        w.raw(&root_message_id.into_bytes())?;
                    }
                    AttentionScopeV1::Topic(topic_id) => {
                        w.byte(3)?;
                        w.raw(&topic_id.into_bytes())?;
                    }
                }
                w.byte(u8::from(preference.following))?;
                w.byte(u8::from(preference.muted))?;
                w.byte(notification_intent_tag(preference.notification_intent))?;
            }
        }
        if include_private_saves {
            w.count(self.private_saves.len())?;
            for ((participant, room_id, message_id), original_sequence) in &self.private_saves {
                w.text(participant.as_str())?;
                w.text(room_id.as_str())?;
                w.raw(&message_id.into_bytes())?;
                w.number(*original_sequence)?;
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
            w.event(&e.event, include_vellum_artifacts)?
        }
        w.count(self.retained_changes.len())?;
        for e in &self.retained_changes {
            w.number(e.sequence.0)?;
            w.number(e.revision.0)?;
            w.event(&e.event, include_vellum_artifacts)?
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
        if r.raw(8)? != b"AGORACP\0" {
            return Err(CheckpointError::Profile);
        }
        let profile = r.byte()?;
        if !matches!(profile, 20..=35) {
            return Err(CheckpointError::Profile);
        }
        let feature_bits = profile - 20;
        let has_vellum_artifacts = feature_bits & 1 != 0;
        let has_conversation_profiles = feature_bits & 2 != 0;
        let has_attention_preferences = feature_bits & 4 != 0;
        let has_private_saves = feature_bits & 8 != 0;
        let l = read_limits(&mut r, bounds.maximum_engine_limits)?;
        let mut e = Engine::with_limits(l);
        e.revision = Revision(r.number()?);
        e.next_event_sequence = EventSequence(r.number()?);
        e.last_published_event = r.option()?;
        e.acknowledged_through = r.option()?;
        e.discarded_changes_through = r.option()?;
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
            let (profile, founding_members) = if has_conversation_profiles {
                let profile = read_conversation_profile(r.byte()?)?;
                let count = r.count(l.max_members_per_room)?;
                let mut founding_members = BTreeSet::new();
                for _ in 0..count {
                    let participant = r.participant(l)?;
                    if founding_members
                        .last()
                        .is_some_and(|old| old >= &participant)
                    {
                        return Err(CheckpointError::NonCanonical);
                    }
                    founding_members.insert(participant);
                }
                (profile, founding_members)
            } else {
                (ConversationProfileV1::Ordinary, BTreeSet::new())
            };
            let message_identity_incarnation = r.number()?;
            let next_message_identity = r.number()?;
            let next_message_sequence = r.number()?;
            let topic_identity_incarnation = r.number()?;
            let next_topic_identity = r.number()?;
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
                messages.push_back(r.message(l, has_vellum_artifacts)?)
            }
            let n = r.count(l.max_messages_per_room)?;
            let mut thread_roots = BTreeSet::new();
            for _ in 0..n {
                let root = MessageId::from_bytes(r.fixed()?);
                if thread_roots.last().is_some_and(|old| old >= &root) {
                    return Err(CheckpointError::NonCanonical);
                }
                thread_roots.insert(root);
            }
            let n = r.count(l.max_messages_per_room)?;
            let mut archived_threads = BTreeSet::new();
            for _ in 0..n {
                let root = MessageId::from_bytes(r.fixed()?);
                if archived_threads.last().is_some_and(|old| old >= &root) {
                    return Err(CheckpointError::NonCanonical);
                }
                archived_threads.insert(root);
            }
            let n = r.count(l.max_messages_per_room)?;
            let mut reactions = BTreeMap::new();
            for _ in 0..n {
                let message_id = MessageId::from_bytes(r.fixed()?);
                if reactions
                    .last_key_value()
                    .is_some_and(|(old, _)| old >= &message_id)
                {
                    return Err(CheckpointError::NonCanonical);
                }
                let mut by_key = BTreeMap::new();
                let keys = r.count(MAX_REACTION_KEYS_PER_MESSAGE_V1)?;
                for _ in 0..keys {
                    let key = r.text(MAX_REACTION_KEY_BYTES_V1)?;
                    if key.is_empty() || by_key.last_key_value().is_some_and(|(old, _)| old >= &key)
                    {
                        return Err(CheckpointError::NonCanonical);
                    }
                    let mut participants = BTreeSet::new();
                    let count = r.count(l.max_participants)?;
                    if count == 0 {
                        return Err(CheckpointError::NonCanonical);
                    }
                    for _ in 0..count {
                        let participant = r.participant(l)?;
                        if participants.last().is_some_and(|old| old >= &participant) {
                            return Err(CheckpointError::NonCanonical);
                        }
                        participants.insert(participant);
                    }
                    by_key.insert(key, participants);
                }
                if by_key.is_empty() {
                    return Err(CheckpointError::NonCanonical);
                }
                reactions.insert(message_id, by_key);
            }
            let mut topics = BTreeMap::new();
            let count = r.count(l.max_topics_per_room)?;
            for _ in 0..count {
                let topic_id = TopicId::from_bytes(r.fixed()?);
                let topic = TopicV1 {
                    id: topic_id,
                    name: r.text(l.max_topic_name_bytes)?,
                    creator: r.participant(l)?,
                    resolved: match r.byte()? {
                        0 => false,
                        1 => true,
                        _ => return Err(CheckpointError::Invalid),
                    },
                    aliases: {
                        let count = r.count(MAX_TOPIC_ALIASES_PER_TOPIC_V1)?;
                        let mut aliases = BTreeSet::new();
                        for _ in 0..count {
                            let alias = r.text(l.max_topic_name_bytes)?;
                            if alias.is_empty()
                                || aliases.last().is_some_and(|previous| previous >= &alias)
                            {
                                return Err(CheckpointError::NonCanonical);
                            }
                            aliases.insert(alias);
                        }
                        aliases
                    },
                    tags: {
                        let count = r.count(MAX_TOPIC_TAGS_PER_TOPIC_V1)?;
                        let mut tags = BTreeSet::new();
                        for _ in 0..count {
                            let tag = r.text(l.max_topic_name_bytes)?;
                            if tag.is_empty()
                                || tags.last().is_some_and(|previous| previous >= &tag)
                            {
                                return Err(CheckpointError::NonCanonical);
                            }
                            tags.insert(tag);
                        }
                        tags
                    },
                };
                insert(&mut topics, topic_id, topic)?;
            }
            let mut topic_successors = BTreeMap::new();
            let count = r.count(l.max_topics_per_room)?;
            for _ in 0..count {
                let topic_id = TopicId::from_bytes(r.fixed()?);
                let successor_topic_id = TopicId::from_bytes(r.fixed()?);
                insert(&mut topic_successors, topic_id, successor_topic_id)?;
            }
            insert(
                &mut e.rooms,
                id.clone(),
                Room {
                    id,
                    name,
                    profile,
                    admission,
                    founding_members,
                    members,
                    invitations,
                    bans,
                    messages,
                    thread_roots,
                    archived_threads,
                    reactions,
                    topics,
                    topic_successors,
                    topic_identity_incarnation,
                    next_topic_identity,
                    message_identity_incarnation,
                    next_message_identity,
                    next_message_sequence,
                },
            )?;
        }
        let count = r.count(MAX_READ_POSITIONS_V1)?;
        for _ in 0..count {
            let participant = r.participant(l)?;
            let room_id = r.room(l)?;
            let scope = match r.byte()? {
                1 => ReadScopeV1::Room,
                2 => ReadScopeV1::Thread(MessageId::from_bytes(r.fixed()?)),
                3 => ReadScopeV1::Topic(TopicId::from_bytes(r.fixed()?)),
                _ => return Err(CheckpointError::Invalid),
            };
            let position = ReadPositionV1 {
                room_id: room_id.clone(),
                scope,
                target_message_id: MessageId::from_bytes(r.fixed()?),
                target_sequence: r.number()?,
                observed_revision: Revision(r.number()?),
            };
            insert(
                &mut e.read_positions,
                (participant, room_id, scope),
                position,
            )?;
        }
        if has_attention_preferences {
            let count = r.count(MAX_ATTENTION_PREFERENCES_V1)?;
            for _ in 0..count {
                let participant = r.participant(l)?;
                let room_id = r.room(l)?;
                let scope = match r.byte()? {
                    1 => AttentionScopeV1::Room,
                    2 => AttentionScopeV1::Thread(MessageId::from_bytes(r.fixed()?)),
                    3 => AttentionScopeV1::Topic(TopicId::from_bytes(r.fixed()?)),
                    _ => return Err(CheckpointError::Invalid),
                };
                let preference = AttentionPreferenceV1::new(
                    match r.byte()? {
                        0 => false,
                        1 => true,
                        _ => return Err(CheckpointError::Invalid),
                    },
                    match r.byte()? {
                        0 => false,
                        1 => true,
                        _ => return Err(CheckpointError::Invalid),
                    },
                    read_notification_intent(r.byte()?)?,
                );
                if preference.is_default() {
                    return Err(CheckpointError::NonCanonical);
                }
                insert(
                    &mut e.attention_preferences,
                    (participant, room_id, scope),
                    preference,
                )?;
            }
        }
        if has_private_saves {
            let count = r.count(MAX_PRIVATE_SAVES_V1)?;
            for _ in 0..count {
                let participant = r.participant(l)?;
                let room_id = r.room(l)?;
                let message_id = MessageId::from_bytes(r.fixed()?);
                let original_sequence = r.number()?;
                insert(
                    &mut e.private_saves,
                    (participant, room_id, message_id),
                    original_sequence,
                )?;
            }
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
                event: r.event(l, has_vellum_artifacts, has_conversation_profiles)?,
            })
        }
        let n = r.count(l.max_retained_changes)?;
        e.retained_changes
            .try_reserve(n)
            .map_err(|_| CheckpointError::Capacity)?;
        for _ in 0..n {
            e.retained_changes.push_back(EventEnvelopeV2 {
                sequence: EventSequence(r.number()?),
                revision: Revision(r.number()?),
                event: r.event(l, has_vellum_artifacts, has_conversation_profiles)?,
            })
        }
        if r.pos != bytes.len() {
            return Err(CheckpointError::Trailing);
        }
        validate(&e)?;
        if has_vellum_artifacts && !engine_has_vellum_artifacts(&e) {
            return Err(CheckpointError::NonCanonical);
        }
        if has_conversation_profiles && !engine_has_conversation_profiles(&e) {
            return Err(CheckpointError::NonCanonical);
        }
        if has_attention_preferences == e.attention_preferences.is_empty() {
            return Err(CheckpointError::NonCanonical);
        }
        if has_private_saves == e.private_saves.is_empty() {
            return Err(CheckpointError::NonCanonical);
        }
        Ok(e)
    }
}

fn engine_has_vellum_artifacts(engine: &Engine) -> bool {
    engine
        .rooms
        .values()
        .flat_map(|room| &room.messages)
        .any(|message| !message.vellum_artifacts.is_empty())
        || engine
            .pending_events
            .iter()
            .chain(&engine.retained_changes)
            .any(|envelope| {
                matches!(
                    &envelope.event,
                    DomainEventV2::MessageAccepted { message, .. }
                        if !message.vellum_artifacts.is_empty()
                )
            })
}

fn engine_has_conversation_profiles(engine: &Engine) -> bool {
    engine
        .rooms
        .values()
        .any(|room| room.profile != ConversationProfileV1::Ordinary)
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

impl BanPageCursor {
    pub fn prepare_checkpoint(
        &self,
        max_snapshot_bytes: usize,
    ) -> Result<CursorCheckpointV1, CheckpointError> {
        prepare_cursor(
            5,
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
        let mut r = cursor_reader(bytes, bounds, 5)?;
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

impl ThreadMessagePageCursor {
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
        writer.byte(6)?;
        writer.number(self.revision.0)?;
        writer.text(self.observer.as_str())?;
        writer.text(self.room_id.as_str())?;
        match self.selector {
            ThreadMessageSelector::Roots => writer.byte(0)?,
            ThreadMessageSelector::Replies(root) => {
                writer.byte(1)?;
                writer.raw(&root.into_bytes())?;
            }
        }
        writer.number(self.after_sequence)?;
        Ok(CursorCheckpointV1(writer.bytes))
    }

    pub fn recover_checkpoint(
        bytes: &[u8],
        bounds: CursorRecoveryLimitsV1,
    ) -> Result<Self, CheckpointError> {
        let mut reader = cursor_reader(bytes, bounds, 6)?;
        let revision = Revision(reader.number()?);
        let observer = ParticipantId::new(reader.text(bounds.max_participant_id_bytes)?);
        let room_id = RoomId::new(reader.text(bounds.max_room_id_bytes)?);
        let selector = match reader.byte()? {
            0 => ThreadMessageSelector::Roots,
            1 => ThreadMessageSelector::Replies(MessageId::from_bytes(reader.fixed()?)),
            _ => return Err(CheckpointError::Invalid),
        };
        let cursor = Self {
            revision,
            observer,
            room_id,
            selector,
            after_sequence: reader.number()?,
        };
        finish_cursor(&reader)?;
        if cursor.observer.as_str().is_empty() || cursor.room_id.as_str().is_empty() {
            return Err(CheckpointError::Invalid);
        }
        Ok(cursor)
    }
}

impl MessageRevisionPageCursorV1 {
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
        writer.byte(7)?;
        writer.number(self.revision.0)?;
        writer.text(self.observer.as_str())?;
        writer.text(self.room_id.as_str())?;
        writer.raw(&self.message_id.into_bytes())?;
        writer.number(u64::from(self.after_revision))?;
        Ok(CursorCheckpointV1(writer.bytes))
    }

    pub fn recover_checkpoint(
        bytes: &[u8],
        bounds: CursorRecoveryLimitsV1,
    ) -> Result<Self, CheckpointError> {
        let mut reader = cursor_reader(bytes, bounds, 7)?;
        let cursor = Self {
            revision: Revision(reader.number()?),
            observer: ParticipantId::new(reader.text(bounds.max_participant_id_bytes)?),
            room_id: RoomId::new(reader.text(bounds.max_room_id_bytes)?),
            message_id: MessageId::from_bytes(reader.fixed()?),
            after_revision: reader
                .number()?
                .try_into()
                .map_err(|_| CheckpointError::Invalid)?,
        };
        finish_cursor(&reader)?;
        if cursor.observer.as_str().is_empty() || cursor.room_id.as_str().is_empty() {
            return Err(CheckpointError::Invalid);
        }
        Ok(cursor)
    }
}

impl TopicPageCursorV1 {
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
        writer.byte(8)?;
        writer.number(self.revision.0)?;
        writer.text(self.observer.as_str())?;
        writer.text(self.room_id.as_str())?;
        writer.raw(&self.after.into_bytes())?;
        Ok(CursorCheckpointV1(writer.bytes))
    }
    pub fn recover_checkpoint(
        bytes: &[u8],
        bounds: CursorRecoveryLimitsV1,
    ) -> Result<Self, CheckpointError> {
        let mut reader = cursor_reader(bytes, bounds, 8)?;
        let cursor = Self {
            revision: Revision(reader.number()?),
            observer: ParticipantId::new(reader.text(bounds.max_participant_id_bytes)?),
            room_id: RoomId::new(reader.text(bounds.max_room_id_bytes)?),
            after: TopicId::from_bytes(reader.fixed()?),
        };
        finish_cursor(&reader)?;
        if cursor.observer.as_str().is_empty() || cursor.room_id.as_str().is_empty() {
            return Err(CheckpointError::Invalid);
        }
        Ok(cursor)
    }
}

impl TopicMessagePageCursorV1 {
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
        writer.byte(9)?;
        writer.number(self.revision.0)?;
        writer.text(self.observer.as_str())?;
        writer.text(self.room_id.as_str())?;
        writer.raw(&self.topic_id.into_bytes())?;
        writer.number(self.after_sequence)?;
        Ok(CursorCheckpointV1(writer.bytes))
    }
    pub fn recover_checkpoint(
        bytes: &[u8],
        bounds: CursorRecoveryLimitsV1,
    ) -> Result<Self, CheckpointError> {
        let mut reader = cursor_reader(bytes, bounds, 9)?;
        let cursor = Self {
            revision: Revision(reader.number()?),
            observer: ParticipantId::new(reader.text(bounds.max_participant_id_bytes)?),
            room_id: RoomId::new(reader.text(bounds.max_room_id_bytes)?),
            topic_id: TopicId::from_bytes(reader.fixed()?),
            after_sequence: reader.number()?,
        };
        finish_cursor(&reader)?;
        if cursor.observer.as_str().is_empty() || cursor.room_id.as_str().is_empty() {
            return Err(CheckpointError::Invalid);
        }
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
    valid_message_parts(
        e,
        room,
        MessageFacts {
            id: m.id,
            sequence: m.sequence,
            author: &m.author,
            at: m.at,
            body: &m.body,
        },
        false,
    )?;
    validate_addressing(e, &m.addressed_to)?;
    if m.attachments.len() > MAX_ATTACHMENTS_PER_MESSAGE_V1 {
        return Err(CheckpointError::Invalid);
    }
    let attachment_limits = AttachmentDescriptorLimitsV1 {
        max_declared_bytes: e
            .limits
            .max_attachment_declared_bytes
            .try_into()
            .map_err(|_| CheckpointError::Invalid)?,
        max_media_type_bytes: e.limits.max_attachment_media_type_bytes,
        max_display_label_bytes: e.limits.max_attachment_display_label_bytes,
    };
    let mut attachment_ids = BTreeSet::new();
    let mut object_refs = BTreeSet::new();
    for attachment in &m.attachments {
        if !attachment_ids.insert(attachment.attachment_id) {
            return Err(CheckpointError::NonCanonical);
        }
        if m.is_withdrawn() == attachment.descriptor.is_some() {
            return Err(CheckpointError::Invalid);
        }
        if let Some(descriptor) = &attachment.descriptor
            && (descriptor.attachment_id != attachment.attachment_id
                || descriptor.validate(attachment_limits).is_err()
                || !object_refs.insert((descriptor.store_id, descriptor.object_key)))
        {
            return Err(CheckpointError::Invalid);
        }
    }
    if m.vellum_artifacts.len() > MAX_VELLUM_ARTIFACT_REFERENCES_PER_MESSAGE_V1 {
        return Err(CheckpointError::Invalid);
    }
    let mut artifact_ids = BTreeSet::new();
    let mut artifact_targets = BTreeSet::new();
    for artifact in &m.vellum_artifacts {
        if !artifact_ids.insert(artifact.reference_id) {
            return Err(CheckpointError::NonCanonical);
        }
        if m.is_withdrawn() == artifact.target.is_some() {
            return Err(CheckpointError::Invalid);
        }
        if let Some(target) = artifact.target
            && (!artifact_targets.insert(target)
                || VellumDocumentTargetV1::new(
                    target.workspace_id(),
                    target.document_id(),
                    target.source_revision(),
                )
                .is_none())
        {
            return Err(CheckpointError::Invalid);
        }
    }
    if m.revisions.is_empty() || m.revisions.len() > MAX_MESSAGE_REVISIONS_V1 {
        return Err(CheckpointError::Invalid);
    }
    let tombstoned = m.is_withdrawn();
    for (index, revision) in m.revisions.iter().enumerate() {
        if revision.number as usize != index
            || revision.editor != m.author
            || !e.participants.contains_key(&revision.editor)
            || revision.body.len() > e.limits.max_message_body_bytes
            || match revision.availability {
                MessageRevisionAvailabilityV1::Available => tombstoned || revision.body.is_empty(),
                MessageRevisionAvailabilityV1::Tombstoned => {
                    !tombstoned || !revision.body.is_empty()
                }
            }
        {
            return Err(CheckpointError::Invalid);
        }
    }
    let first = m.revisions.first().ok_or(CheckpointError::Invalid)?;
    let last = m.revisions.last().ok_or(CheckpointError::Invalid)?;
    if first.at != m.at || last.body != m.body {
        return Err(CheckpointError::Invalid);
    }
    if m.topic_id
        .is_some_and(|topic_id| !room.topics.contains_key(&topic_id))
    {
        return Err(CheckpointError::Invalid);
    }
    if let Some(root) = m.reply_to_root {
        let mut incarnation = [0_u8; 8];
        let mut sequence = [0_u8; 8];
        incarnation.copy_from_slice(&root.0[..8]);
        sequence.copy_from_slice(&root.0[8..]);
        let root_sequence = u64::from_be_bytes(sequence);
        let retained_start = room
            .next_message_sequence
            .checked_sub(room.messages.len() as u64)
            .ok_or(CheckpointError::Invalid)?;
        if root == m.id
            || u64::from_be_bytes(incarnation) != room.message_identity_incarnation
            || root_sequence >= m.sequence
            || (m.sequence >= retained_start && !room.thread_roots.contains(&root))
        {
            return Err(CheckpointError::Invalid);
        }
        if let Some(root_message) = room.messages.iter().find(|message| message.id == root)
            && (root_message.reply_to_root.is_some()
                || root_message.sequence >= m.sequence
                || root_message.topic_id != m.topic_id)
        {
            return Err(CheckpointError::Invalid);
        }
    }
    Ok(())
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
struct MessageFacts<'a> {
    id: MessageId,
    sequence: u64,
    author: &'a ParticipantId,
    at: LogicalTime,
    body: &'a str,
}
fn valid_message_parts(
    e: &Engine,
    room: &Room,
    facts: MessageFacts<'_>,
    allow_withdrawn_body: bool,
) -> Result<(), CheckpointError> {
    if facts.body.len() > e.limits.max_message_body_bytes
        || (allow_withdrawn_body && facts.body.is_empty())
    {
        return Err(CheckpointError::Invalid);
    }
    if !e.participants.contains_key(facts.author)
        || facts.sequence >= room.next_message_sequence
        || facts.id != MessageId::allocated(room.message_identity_incarnation, facts.sequence)
    {
        return Err(CheckpointError::Invalid);
    }
    let start = room
        .next_message_sequence
        .checked_sub(room.messages.len() as u64)
        .ok_or(CheckpointError::Invalid)?;
    if facts.sequence >= start {
        let index: usize = (facts.sequence - start)
            .try_into()
            .map_err(|_| CheckpointError::Invalid)?;
        let current = room.messages.get(index).ok_or(CheckpointError::Invalid)?;
        if current.id != facts.id
            || current.sequence != facts.sequence
            || &current.author != facts.author
            || current.at != facts.at
            || (current.body != facts.body
                && !current
                    .revisions
                    .iter()
                    .any(|revision| revision.body == facts.body)
                && !(allow_withdrawn_body && current.is_withdrawn()))
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
        || e.retained_changes.len() > l.max_retained_changes
        || e.read_positions.len() > MAX_READ_POSITIONS_V1
        || e.attention_preferences.len() > MAX_ATTENTION_PREFERENCES_V1
        || e.private_saves.len() > MAX_PRIVATE_SAVES_V1
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
    if e.discarded_changes_through
        .is_some_and(|discarded| e.last_published_event.is_none_or(|last| discarded > last))
    {
        return Err(CheckpointError::Invalid);
    }
    let first_retained_change = e
        .discarded_changes_through
        .map_or(Some(0), |discarded| discarded.0.checked_add(1))
        .ok_or(CheckpointError::Invalid)?;
    if e.revision.0.checked_sub(first_retained_change) != Some(e.retained_changes.len() as u64) {
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
    let mut room_incarnations = BTreeSet::new();
    for (id, room) in &e.rooms {
        if id != &room.id
            || room.members.len() > l.max_members_per_room
            || room.invitations.len() > l.max_invitations_per_room
            || room.bans.len() > l.max_bans_per_room
            || room.messages.len() > l.max_messages_per_room
            || room.topics.len() > l.max_topics_per_room
            || room.topic_successors.len() > room.topics.len()
            || room.thread_roots.len() > room.messages.len()
            || room.archived_threads.len() > room.messages.len()
        {
            return Err(CheckpointError::Invalid);
        }
        bounded(id.as_str(), l.max_room_id_bytes)?;
        bounded(&room.name, l.max_room_name_bytes)?;
        match room.profile {
            ConversationProfileV1::Ordinary => {
                if !room.founding_members.is_empty() {
                    return Err(CheckpointError::Invalid);
                }
                owner(room)?;
            }
            ConversationProfileV1::Direct | ConversationProfileV1::GroupDirect => {
                let valid_cardinality = match room.profile {
                    ConversationProfileV1::Direct => room.founding_members.len() == 2,
                    ConversationProfileV1::GroupDirect => room.founding_members.len() >= 3,
                    ConversationProfileV1::Ordinary => unreachable!(),
                };
                if !valid_cardinality
                    || room.admission != AdmissionPolicy::InviteOnly
                    || !room.invitations.is_empty()
                    || !room.bans.is_empty()
                    || room.members.iter().any(|(id, role)| {
                        *role != Role::Member || !room.founding_members.contains(id)
                    })
                {
                    return Err(CheckpointError::Invalid);
                }
            }
        }
        if room
            .members
            .keys()
            .chain(room.invitations.iter())
            .chain(room.bans.keys())
            .chain(room.founding_members.iter())
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
        if room.message_identity_incarnation >= e.revision.0
            || !room_incarnations.insert(room.message_identity_incarnation)
            || room.next_message_identity != room.next_message_sequence
        {
            return Err(CheckpointError::Invalid);
        }
        if room.topic_identity_incarnation >= e.revision.0
            || room.next_topic_identity != room.topics.len() as u64
        {
            return Err(CheckpointError::Invalid);
        }
        let mut names = BTreeSet::new();
        for (topic_id, topic) in &room.topics {
            if topic.id != *topic_id
                || *topic_id
                    != TopicId::allocated(
                        room.topic_identity_incarnation,
                        topic_id.0[8..]
                            .try_into()
                            .map(u64::from_be_bytes)
                            .map_err(|_| CheckpointError::Invalid)?,
                    )
                || !e.participants.contains_key(&topic.creator)
                || !names.insert(topic.name.clone())
                || topic.aliases.len() > MAX_TOPIC_ALIASES_PER_TOPIC_V1
                || topic.tags.len() > MAX_TOPIC_TAGS_PER_TOPIC_V1
            {
                return Err(CheckpointError::Invalid);
            }
            bounded(&topic.name, l.max_topic_name_bytes)?;
            for alias in &topic.aliases {
                bounded(alias, l.max_topic_name_bytes)?;
                if !names.insert(alias.clone()) {
                    return Err(CheckpointError::Invalid);
                }
            }
            for tag in &topic.tags {
                bounded(tag, l.max_topic_name_bytes)?;
            }
        }
        for (topic_id, successor_topic_id) in &room.topic_successors {
            if topic_id == successor_topic_id
                || !room.topics.get(topic_id).is_some_and(TopicV1::is_resolved)
                || !room.topics.contains_key(successor_topic_id)
                || room.topic_successors.contains_key(successor_topic_id)
                || room
                    .topic_successors
                    .values()
                    .any(|candidate| candidate == topic_id)
            {
                return Err(CheckpointError::Invalid);
            }
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
            if m.sequence != start + i as u64
                || m.id != MessageId::allocated(room.message_identity_incarnation, m.sequence)
            {
                return Err(CheckpointError::Invalid);
            }
            valid_message(e, room, m)?
        }
        for root in &room.thread_roots {
            if !room
                .messages
                .iter()
                .any(|message| message.reply_to_root == Some(*root))
                || room
                    .messages
                    .iter()
                    .find(|message| message.id == *root)
                    .is_some_and(|message| message.reply_to_root.is_some())
            {
                return Err(CheckpointError::Invalid);
            }
        }
        for root in &room.archived_threads {
            if !room
                .messages
                .iter()
                .any(|message| message.id == *root && message.reply_to_root.is_none())
            {
                return Err(CheckpointError::Invalid);
            }
        }
        for (message_id, by_key) in &room.reactions {
            if !room
                .messages
                .iter()
                .any(|message| message.id == *message_id)
                || by_key.is_empty()
                || by_key.len() > MAX_REACTION_KEYS_PER_MESSAGE_V1
            {
                return Err(CheckpointError::Invalid);
            }
            for (key, participants) in by_key {
                bounded(key, MAX_REACTION_KEY_BYTES_V1)?;
                if participants.is_empty()
                    || participants
                        .iter()
                        .any(|id| !e.participants.contains_key(id))
                {
                    return Err(CheckpointError::Invalid);
                }
            }
        }
        let joined_after_creation = match room.profile {
            ConversationProfileV1::Ordinary => room.members.len().saturating_sub(1),
            ConversationProfileV1::Direct | ConversationProfileV1::GroupDirect => 0,
        };
        minimum = minimum
            .checked_add(room.next_message_sequence)
            .and_then(|n| n.checked_add(joined_after_creation as u64))
            .and_then(|n| n.checked_add(room.invitations.len() as u64))
            .and_then(|n| n.checked_add(room.bans.len() as u64))
            .and_then(|n| n.checked_add(room.topics.len() as u64))
            .and_then(|n| n.checked_add(room.topic_successors.len() as u64))
            .and_then(|n| {
                n.checked_add(
                    room.topics
                        .values()
                        .map(|topic| topic.aliases.len() as u64)
                        .sum(),
                )
            })
            .and_then(|n| {
                n.checked_add(
                    room.topics
                        .values()
                        .map(|topic| topic.tags.len() as u64)
                        .sum(),
                )
            })
            .ok_or(CheckpointError::Invalid)?;
    }
    for ((participant, room_id, scope), position) in &e.read_positions {
        let room = e.rooms.get(room_id).ok_or(CheckpointError::Invalid)?;
        if !e.participants.contains_key(participant)
            || &position.room_id != room_id
            || &position.scope != scope
            || position.observed_revision > e.revision
            || position.target_sequence >= room.next_message_sequence
            || position.target_message_id
                != MessageId::allocated(room.message_identity_incarnation, position.target_sequence)
        {
            return Err(CheckpointError::Invalid);
        }
        match scope {
            ReadScopeV1::Room => {}
            ReadScopeV1::Thread(root_message_id) => {
                if root_message_id.0[..8] != room.message_identity_incarnation.to_be_bytes()
                    || u64::from_be_bytes(
                        root_message_id.0[8..]
                            .try_into()
                            .map_err(|_| CheckpointError::Invalid)?,
                    ) >= room.next_message_sequence
                    || room
                        .messages
                        .iter()
                        .find(|message| message.id == *root_message_id)
                        .is_some_and(|message| message.reply_to_root.is_some())
                {
                    return Err(CheckpointError::Invalid);
                }
            }
            ReadScopeV1::Topic(topic_id) => {
                if !room.topics.contains_key(topic_id) {
                    return Err(CheckpointError::Invalid);
                }
            }
        }
        if let Some(target) = room
            .messages
            .iter()
            .find(|message| message.id == position.target_message_id)
        {
            let in_scope = match scope {
                ReadScopeV1::Room => true,
                ReadScopeV1::Thread(root) => {
                    target.id == *root || target.reply_to_root == Some(*root)
                }
                ReadScopeV1::Topic(topic) => target.topic_id == Some(*topic),
            };
            if target.sequence != position.target_sequence || !in_scope {
                return Err(CheckpointError::Invalid);
            }
        }
    }
    for ((participant, room_id, scope), preference) in &e.attention_preferences {
        let room = e.rooms.get(room_id).ok_or(CheckpointError::Invalid)?;
        if !e.participants.contains_key(participant) || preference.is_default() {
            return Err(CheckpointError::Invalid);
        }
        match scope {
            AttentionScopeV1::Room => {}
            AttentionScopeV1::Thread(root_message_id) => {
                if root_message_id.0[..8] != room.message_identity_incarnation.to_be_bytes()
                    || u64::from_be_bytes(
                        root_message_id.0[8..]
                            .try_into()
                            .map_err(|_| CheckpointError::Invalid)?,
                    ) >= room.next_message_sequence
                    || room
                        .messages
                        .iter()
                        .find(|message| message.id == *root_message_id)
                        .is_some_and(|message| message.reply_to_root.is_some())
                {
                    return Err(CheckpointError::Invalid);
                }
            }
            AttentionScopeV1::Topic(topic_id) => {
                if !room.topics.contains_key(topic_id) {
                    return Err(CheckpointError::Invalid);
                }
            }
        }
    }
    for ((participant, room_id, message_id), original_sequence) in &e.private_saves {
        let room = e.rooms.get(room_id).ok_or(CheckpointError::Invalid)?;
        if !e.participants.contains_key(participant)
            || message_id.0[..8] != room.message_identity_incarnation.to_be_bytes()
            || u64::from_be_bytes(
                message_id.0[8..]
                    .try_into()
                    .map_err(|_| CheckpointError::Invalid)?,
            ) >= room.next_message_identity
            || *original_sequence >= room.next_message_sequence
        {
            return Err(CheckpointError::Invalid);
        }
        if let Some(message) = room
            .messages
            .iter()
            .find(|message| message.id == *message_id)
            && message.sequence != *original_sequence
        {
            return Err(CheckpointError::Invalid);
        }
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
        let message_id = MessageId::allocated(room.message_identity_incarnation, *sequence);
        if room_id != &p.room_id
            || p.transition.revision.0 == 0
            || p.transition.revision > e.revision
            || p.transition.event_sequence.0.checked_add(1) != Some(p.transition.revision.0)
            || revisions.insert(p.transition.revision, p).is_some()
            || !messages.insert((room_id, message_id))
        {
            return Err(CheckpointError::Invalid);
        }
        valid_message_parts(
            e,
            room,
            MessageFacts {
                id: message_id,
                sequence: *sequence,
                author: &p.author,
                at: p.at,
                body: &p.body,
            },
            true,
        )?;
        if e.message(room_id, *sequence).is_some_and(|message| {
            !message.addressed_to.is_empty() || message.reply_to_root.is_some()
        }) {
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
            let expected_message_id = MessageId::allocated(
                e.rooms
                    .get(room_id)
                    .ok_or(CheckpointError::Invalid)?
                    .message_identity_incarnation,
                *sequence,
            );
            if room_id != &p.room_id
                || message.id != expected_message_id
                || message.sequence != *sequence
                || message.author != p.author
                || message.at != p.at
                || (!message.is_withdrawn() && message.body != p.body)
                || !message.addressed_to.is_empty()
            {
                return Err(CheckpointError::Invalid);
            }
        }
    }
    validate_topic_event_history(e, &e.pending_events)?;
    for (i, envelope) in e.retained_changes.iter().enumerate() {
        if envelope.sequence.0 != first_retained_change + i as u64
            || envelope.revision.0 != envelope.sequence.0 + 1
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
            let expected_message_id = MessageId::allocated(
                e.rooms
                    .get(room_id)
                    .ok_or(CheckpointError::Invalid)?
                    .message_identity_incarnation,
                *sequence,
            );
            if room_id != &p.room_id
                || message.id != expected_message_id
                || message.sequence != *sequence
                || message.author != p.author
                || message.at != p.at
                || (!message.is_withdrawn() && message.body != p.body)
                || !message.addressed_to.is_empty()
            {
                return Err(CheckpointError::Invalid);
            }
        }
    }
    validate_topic_event_history(e, &e.retained_changes)?;
    Ok(())
}

fn validate_topic_event_history(
    engine: &Engine,
    events: &VecDeque<EventEnvelopeV2>,
) -> Result<(), CheckpointError> {
    let mut names = BTreeMap::<(RoomId, TopicId), String>::new();
    let mut lifecycle = BTreeMap::<(RoomId, TopicId), bool>::new();
    let mut supersession = BTreeMap::<(RoomId, TopicId), Option<TopicId>>::new();
    for envelope in events {
        match &envelope.event {
            DomainEventV2::TopicCreated {
                room_id,
                topic_id,
                name,
                ..
            } => {
                match names.entry((room_id.clone(), *topic_id)) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(name.clone());
                    }
                    std::collections::btree_map::Entry::Occupied(_) => {
                        return Err(CheckpointError::Invalid);
                    }
                }
                lifecycle.insert((room_id.clone(), *topic_id), false);
            }
            DomainEventV2::TopicRenamed {
                room_id,
                topic_id,
                previous,
                name,
                ..
            } => {
                let current = names
                    .entry((room_id.clone(), *topic_id))
                    .or_insert_with(|| previous.clone());
                if current != previous {
                    return Err(CheckpointError::Invalid);
                }
                current.clone_from(name);
            }
            DomainEventV2::TopicLifecycleChanged {
                room_id,
                topic_id,
                previous_resolved,
                resolved,
                ..
            } => {
                let current = lifecycle
                    .entry((room_id.clone(), *topic_id))
                    .or_insert(*previous_resolved);
                if current != previous_resolved || previous_resolved == resolved {
                    return Err(CheckpointError::Invalid);
                }
                *current = *resolved;
            }
            DomainEventV2::TopicSupersessionChanged {
                room_id,
                topic_id,
                previous_successor_topic_id,
                successor_topic_id,
                ..
            } => {
                let current = supersession
                    .entry((room_id.clone(), *topic_id))
                    .or_insert(*previous_successor_topic_id);
                if current != previous_successor_topic_id
                    || previous_successor_topic_id == successor_topic_id
                {
                    return Err(CheckpointError::Invalid);
                }
                *current = *successor_topic_id;
            }
            _ => {}
        }
    }
    for ((room_id, topic_id), name) in names {
        let topic = engine
            .rooms
            .get(&room_id)
            .and_then(|room| room.topics.get(&topic_id))
            .ok_or(CheckpointError::Invalid)?;
        if topic.name != name {
            return Err(CheckpointError::Invalid);
        }
    }
    for ((room_id, topic_id), resolved) in lifecycle {
        let topic = engine
            .rooms
            .get(&room_id)
            .and_then(|room| room.topics.get(&topic_id))
            .ok_or(CheckpointError::Invalid)?;
        if topic.resolved != resolved {
            return Err(CheckpointError::Invalid);
        }
    }
    for ((room_id, topic_id), successor_topic_id) in supersession {
        let current = engine
            .rooms
            .get(&room_id)
            .ok_or(CheckpointError::Invalid)?
            .topic_successors
            .get(&topic_id)
            .copied();
        if current != successor_topic_id {
            return Err(CheckpointError::Invalid);
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
            if r.profile != ConversationProfileV1::Ordinary
                || !r.founding_members.is_empty()
                || owner(r)? != id
            {
                return Err(CheckpointError::Invalid);
            }
        }
        DomainEventV2::PeerConversationCreated {
            room_id,
            initiator,
            profile,
            founding_members,
        } => {
            let r = room(room_id)?;
            participant(initiator)?;
            if r.profile != *profile
                || *profile == ConversationProfileV1::Ordinary
                || founding_members.windows(2).any(|pair| pair[0] >= pair[1])
                || !founding_members.contains(initiator)
                || r.founding_members.iter().ne(founding_members.iter())
                || (*profile == ConversationProfileV1::Direct && founding_members.len() != 2)
                || (*profile == ConversationProfileV1::GroupDirect
                    && !(3..=e.limits.max_members_per_room).contains(&founding_members.len()))
            {
                return Err(CheckpointError::Invalid);
            }
        }
        DomainEventV2::TopicCreated {
            room_id,
            actor,
            topic_id,
            name,
        } => {
            let r = room(room_id)?;
            participant(actor)?;
            bounded(name, e.limits.max_topic_name_bytes)?;
            let topic = r.topics.get(topic_id).ok_or(CheckpointError::Invalid)?;
            if topic.creator != *actor {
                return Err(CheckpointError::Invalid);
            }
        }
        DomainEventV2::TopicRenamed {
            room_id,
            actor,
            topic_id,
            previous,
            name,
        } => {
            let r = room(room_id)?;
            participant(actor)?;
            bounded(previous, e.limits.max_topic_name_bytes)?;
            bounded(name, e.limits.max_topic_name_bytes)?;
            if previous == name || !r.topics.contains_key(topic_id) {
                return Err(CheckpointError::Invalid);
            }
        }
        DomainEventV2::TopicLifecycleChanged {
            room_id,
            actor,
            topic_id,
            previous_resolved,
            resolved,
        } => {
            let r = room(room_id)?;
            participant(actor)?;
            if previous_resolved == resolved || !r.topics.contains_key(topic_id) {
                return Err(CheckpointError::Invalid);
            }
        }
        DomainEventV2::TopicSupersessionChanged {
            room_id,
            actor,
            topic_id,
            previous_successor_topic_id,
            successor_topic_id,
        } => {
            let r = room(room_id)?;
            participant(actor)?;
            if previous_successor_topic_id == successor_topic_id
                || !r.topics.contains_key(topic_id)
                || previous_successor_topic_id
                    .is_some_and(|candidate| !r.topics.contains_key(&candidate))
                || successor_topic_id.is_some_and(|candidate| {
                    candidate == *topic_id || !r.topics.contains_key(&candidate)
                })
            {
                return Err(CheckpointError::Invalid);
            }
        }
        DomainEventV2::TopicAliasChanged {
            room_id,
            actor,
            topic_id,
            alias,
            added: _,
        } => {
            let r = room(room_id)?;
            participant(actor)?;
            bounded(alias, e.limits.max_topic_name_bytes)?;
            if !r.topics.contains_key(topic_id) {
                return Err(CheckpointError::Invalid);
            }
        }
        DomainEventV2::TopicTagChanged {
            room_id,
            actor,
            topic_id,
            tag,
            added: _,
        } => {
            let r = room(room_id)?;
            participant(actor)?;
            bounded(tag, e.limits.max_topic_name_bytes)?;
            if !r.topics.contains_key(topic_id) {
                return Err(CheckpointError::Invalid);
            }
        }
        DomainEventV2::InvitationRecorded {
            room_id,
            participant: id,
        } => {
            if room(room_id)?.profile != ConversationProfileV1::Ordinary {
                return Err(CheckpointError::Invalid);
            }
            participant(id)?;
        }
        DomainEventV2::ParticipantJoined {
            room_id,
            participant: id,
        } => {
            if room(room_id)?.profile != ConversationProfileV1::Ordinary {
                return Err(CheckpointError::Invalid);
            }
            participant(id)?;
        }
        DomainEventV2::ParticipantLeft {
            room_id,
            participant: id,
        } => {
            let room = room(room_id)?;
            if room.profile == ConversationProfileV1::Ordinary && owner(room)? == id {
                return Err(CheckpointError::Invalid);
            }
            if room.profile != ConversationProfileV1::Ordinary
                && !room.founding_members.contains(id)
            {
                return Err(CheckpointError::Invalid);
            }
            participant(id)?;
        }
        DomainEventV2::RoleChanged {
            room_id,
            participant: id,
            role,
        } => {
            if room(room_id)?.profile != ConversationProfileV1::Ordinary {
                return Err(CheckpointError::Invalid);
            }
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
            if r.profile != ConversationProfileV1::Ordinary
                || owner(r)? != actor
                || previous == admission
            {
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
            if room(room_id)?.profile != ConversationProfileV1::Ordinary {
                return Err(CheckpointError::Invalid);
            }
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
        DomainEventV2::MessageWithdrawn {
            room_id,
            actor,
            message_id,
            sequence,
        } => {
            let r = room(room_id)?;
            participant(actor)?;
            if *sequence >= r.next_message_sequence
                || *message_id != MessageId::allocated(r.message_identity_incarnation, *sequence)
            {
                return Err(CheckpointError::Invalid);
            }
            if let Some(message) = e.message_by_id(room_id, *message_id)
                && (message.sequence != *sequence
                    || message.author != *actor
                    || !message.is_withdrawn())
            {
                return Err(CheckpointError::Invalid);
            }
        }
        DomainEventV2::MessageRedacted {
            room_id,
            actor,
            author,
            message_id,
            sequence,
            reason,
            ..
        } => {
            let r = room(room_id)?;
            participant(actor)?;
            participant(author)?;
            bounded(reason.as_str(), e.limits.max_moderation_reason_bytes)?;
            if actor == author
                || *sequence >= r.next_message_sequence
                || *message_id != MessageId::allocated(r.message_identity_incarnation, *sequence)
            {
                return Err(CheckpointError::Invalid);
            }
            if let Some(message) = e.message_by_id(room_id, *message_id)
                && (message.sequence != *sequence
                    || message.author != *author
                    || !message.is_withdrawn())
            {
                return Err(CheckpointError::Invalid);
            }
        }
        DomainEventV2::ThreadLifecycleChanged {
            room_id,
            actor,
            root_message_id,
            reason,
            ..
        } => {
            let r = room(room_id)?;
            participant(actor)?;
            bounded(reason.as_str(), e.limits.max_moderation_reason_bytes)?;
            let mut sequence = [0_u8; 8];
            sequence.copy_from_slice(&root_message_id.0[8..]);
            let sequence = u64::from_be_bytes(sequence);
            if sequence >= r.next_message_sequence
                || *root_message_id
                    != MessageId::allocated(r.message_identity_incarnation, sequence)
            {
                return Err(CheckpointError::Invalid);
            }
            if let Some(root) = e.message_by_id(room_id, *root_message_id)
                && root.reply_to_root.is_some()
            {
                return Err(CheckpointError::Invalid);
            }
        }
        DomainEventV2::ReactionChanged {
            room_id,
            actor,
            message_id,
            key,
            ..
        } => {
            let r = room(room_id)?;
            participant(actor)?;
            bounded(key, MAX_REACTION_KEY_BYTES_V1)?;
            let mut sequence = [0_u8; 8];
            sequence.copy_from_slice(&message_id.0[8..]);
            let sequence = u64::from_be_bytes(sequence);
            if sequence >= r.next_message_sequence
                || *message_id != MessageId::allocated(r.message_identity_incarnation, sequence)
            {
                return Err(CheckpointError::Invalid);
            }
        }
        DomainEventV2::MessageEdited {
            room_id,
            actor,
            message_id,
            message_revision,
            body,
            ..
        } => {
            let r = room(room_id)?;
            participant(actor)?;
            if body.len() > e.limits.max_message_body_bytes || *message_revision == 0 {
                return Err(CheckpointError::Invalid);
            }
            let mut sequence = [0_u8; 8];
            sequence.copy_from_slice(&message_id.0[8..]);
            let sequence = u64::from_be_bytes(sequence);
            if sequence >= r.next_message_sequence
                || *message_id != MessageId::allocated(r.message_identity_incarnation, sequence)
            {
                return Err(CheckpointError::Invalid);
            }
            if let Some(message) = e.message_by_id(room_id, *message_id)
                && (message.author != *actor
                    || message.current_revision() < *message_revision
                    || (!message.is_withdrawn()
                        && !message.revisions.iter().any(|revision| {
                            revision.number == *message_revision && revision.body == *body
                        })))
            {
                return Err(CheckpointError::Invalid);
            }
        }
    }
    Ok(())
}
