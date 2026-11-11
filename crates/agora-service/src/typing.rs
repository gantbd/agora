//! Bounded volatile ownership for room, thread, and topic typing signals.

use crate::transient::{
    PresenceSourceClassV1, PresenceSourceV1, TransientParticipantKindV1, TransientTimeV1,
};
use agora_engine::{
    Engine, MemberPageCursor, MessageId, ObservationRefusal, ParticipantId, ParticipantKind,
    RoomId, TopicId,
};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TypingTargetV1 {
    Room,
    Thread(MessageId),
    Topic(TopicId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypingLimitsV1 {
    pub max_signals: usize,
    pub max_targets_per_source: usize,
    pub max_lifetime: u64,
    pub max_page_items: usize,
}

impl Default for TypingLimitsV1 {
    fn default() -> Self {
        Self {
            max_signals: 4_096,
            max_targets_per_source: 16,
            max_lifetime: 30,
            max_page_items: 256,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypingRefusalV1 {
    InvalidLimits,
    InvalidLifetime,
    ClockRegression,
    NotMember,
    InvalidTarget,
    Capacity,
    FidelityLost,
    InvalidWorkBudget,
    InvalidPage,
    ForeignCursor,
    StaleCursor,
    EngineChanged,
}

#[derive(Clone, Eq, PartialEq)]
struct TypingSignalV1 {
    issued_at: TransientTimeV1,
    expires_at: TransientTimeV1,
}

#[derive(Clone, Eq, PartialEq)]
pub struct TypingEntryV1 {
    pub participant_id: ParticipantId,
    pub participant_kind: TransientParticipantKindV1,
    pub active: bool,
    pub source: Option<PresenceSourceClassV1>,
    pub expires_at: Option<TransientTimeV1>,
}

impl fmt::Debug for TypingEntryV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TypingEntryV1")
            .field("participant_id", &"<redacted>")
            .field("participant_kind", &self.participant_kind)
            .field("active", &self.active)
            .field("source", &self.source)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct TypingCursorV1 {
    observer: ParticipantId,
    room_id: RoomId,
    target: TypingTargetV1,
    generation: u64,
    as_of: TransientTimeV1,
    member_cursor: MemberPageCursor,
}

impl fmt::Debug for TypingCursorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TypingCursorV1(<opaque>)")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypingPageV1 {
    pub entries: Vec<TypingEntryV1>,
    pub next: Option<TypingCursorV1>,
    pub as_of: TransientTimeV1,
}

pub struct TypingCollaborationV1 {
    limits: TypingLimitsV1,
    signals: BTreeMap<(RoomId, ParticipantId, TypingTargetV1, PresenceSourceV1), TypingSignalV1>,
    generation: u64,
    fidelity_lost: bool,
    last_time: Option<TransientTimeV1>,
}

impl fmt::Debug for TypingCollaborationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TypingCollaborationV1")
            .field("signal_count", &self.signals.len())
            .field("generation", &self.generation)
            .field("fidelity_lost", &self.fidelity_lost)
            .finish_non_exhaustive()
    }
}

impl TypingCollaborationV1 {
    pub fn new(limits: TypingLimitsV1) -> Result<Self, TypingRefusalV1> {
        if limits.max_signals == 0
            || limits.max_targets_per_source == 0
            || limits.max_lifetime == 0
            || limits.max_page_items == 0
        {
            return Err(TypingRefusalV1::InvalidLimits);
        }
        Ok(Self {
            limits,
            signals: BTreeMap::new(),
            generation: 0,
            fidelity_lost: false,
            last_time: None,
        })
    }

    pub fn limits(&self) -> TypingLimitsV1 {
        self.limits
    }

    pub fn signal_count(&self) -> usize {
        self.signals.len()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn fidelity_lost(&self) -> bool {
        self.fidelity_lost
    }

    #[allow(clippy::too_many_arguments)]
    pub fn publish(
        &mut self,
        engine: &Engine,
        room_id: &RoomId,
        participant: &ParticipantId,
        target: TypingTargetV1,
        source: PresenceSourceV1,
        now: TransientTimeV1,
        lifetime: u64,
    ) -> Result<TransientTimeV1, TypingRefusalV1> {
        self.validate_time(now)?;
        if self.fidelity_lost {
            return Err(TypingRefusalV1::FidelityLost);
        }
        validate_authority(engine, room_id, participant, target)?;
        if lifetime == 0 || lifetime > self.limits.max_lifetime {
            return Err(TypingRefusalV1::InvalidLifetime);
        }
        let expires_at = TransientTimeV1(
            now.0
                .checked_add(lifetime)
                .ok_or(TypingRefusalV1::InvalidLifetime)?,
        );
        let key = (room_id.clone(), participant.clone(), target, source);
        if !self.signals.contains_key(&key) {
            if self.signals.len() == self.limits.max_signals {
                return Err(TypingRefusalV1::Capacity);
            }
            let targets = self
                .signals
                .keys()
                .filter(|(room, actor, _, candidate)| {
                    room == room_id && actor == participant && *candidate == source
                })
                .count();
            if targets == self.limits.max_targets_per_source {
                return Err(TypingRefusalV1::Capacity);
            }
        }
        let next = TypingSignalV1 {
            issued_at: now,
            expires_at,
        };
        if self.signals.get(&key) != Some(&next) {
            self.advance_publication_generation()?;
            self.signals.insert(key, next);
        }
        self.last_time = Some(now);
        Ok(expires_at)
    }

    pub fn clear(
        &mut self,
        engine: &Engine,
        room_id: &RoomId,
        participant: &ParticipantId,
        target: TypingTargetV1,
        source: PresenceSourceV1,
        now: TransientTimeV1,
    ) -> Result<bool, TypingRefusalV1> {
        self.validate_time(now)?;
        if engine.member_role(room_id, participant).is_none() {
            return Err(TypingRefusalV1::NotMember);
        }
        let removed = self
            .signals
            .remove(&(room_id.clone(), participant.clone(), target, source))
            .is_some();
        if removed {
            self.advance_cleanup_generation();
        }
        self.last_time = Some(now);
        Ok(removed)
    }

    pub fn retire_source(&mut self, source: PresenceSourceV1) -> usize {
        let prior = self.signals.len();
        self.signals
            .retain(|(_, _, _, candidate), _| *candidate != source);
        let removed = prior - self.signals.len();
        if removed > 0 {
            self.advance_cleanup_generation();
        }
        removed
    }

    /// Trusted transition retirement used for leave, kick, and ban composition.
    pub fn retire_participant_room(
        &mut self,
        room_id: &RoomId,
        participant: &ParticipantId,
    ) -> usize {
        let prior = self.signals.len();
        self.signals
            .retain(|(room, actor, _, _), _| room != room_id || actor != participant);
        let removed = prior - self.signals.len();
        if removed > 0 {
            self.advance_cleanup_generation();
        }
        removed
    }

    pub fn reconcile(&mut self, engine: &Engine) -> usize {
        let prior = self.signals.len();
        self.signals.retain(|(room, participant, target, _), _| {
            engine.member_role(room, participant).is_some() && target_exists(engine, room, *target)
        });
        let removed = prior - self.signals.len();
        if removed > 0 {
            self.advance_cleanup_generation();
        }
        removed
    }

    pub fn expire(
        &mut self,
        now: TransientTimeV1,
        budget: usize,
    ) -> Result<usize, TypingRefusalV1> {
        self.validate_time(now)?;
        if budget == 0 || budget > self.limits.max_signals {
            return Err(TypingRefusalV1::InvalidWorkBudget);
        }
        let mut expired = Vec::new();
        expired
            .try_reserve_exact(budget)
            .map_err(|_| TypingRefusalV1::Capacity)?;
        expired.extend(
            self.signals
                .iter()
                .filter(|(_, signal)| signal.expires_at <= now)
                .take(budget)
                .map(|(key, _)| key.clone()),
        );
        if !expired.is_empty() {
            self.advance_cleanup_generation();
        }
        for key in &expired {
            self.signals.remove(key);
        }
        self.last_time = Some(now);
        Ok(expired.len())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn page(
        &mut self,
        engine: &Engine,
        observer: &ParticipantId,
        room_id: &RoomId,
        target: TypingTargetV1,
        cursor: Option<&TypingCursorV1>,
        limit: usize,
        now: TransientTimeV1,
        as_of: TransientTimeV1,
    ) -> Result<TypingPageV1, TypingRefusalV1> {
        self.validate_time(now)?;
        if as_of > now {
            return Err(TypingRefusalV1::StaleCursor);
        }
        if self.fidelity_lost {
            return Err(TypingRefusalV1::FidelityLost);
        }
        validate_authority(engine, room_id, observer, target)?;
        if limit == 0 || limit > self.limits.max_page_items {
            return Err(TypingRefusalV1::InvalidPage);
        }
        let member_cursor = if let Some(cursor) = cursor {
            if &cursor.observer != observer || &cursor.room_id != room_id || cursor.target != target
            {
                return Err(TypingRefusalV1::ForeignCursor);
            }
            if cursor.generation != self.generation || cursor.as_of != as_of {
                return Err(TypingRefusalV1::StaleCursor);
            }
            Some(&cursor.member_cursor)
        } else {
            None
        };
        let members = engine
            .member_page(observer, room_id, member_cursor, limit)
            .map_err(map_observation_refusal)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(members.members.len())
            .map_err(|_| TypingRefusalV1::Capacity)?;
        for member in members.members {
            let winner = self
                .signals
                .iter()
                .filter(|((room, participant, candidate, _), signal)| {
                    room == room_id
                        && participant == &member.participant_id
                        && candidate == &target
                        && as_of < signal.expires_at
                })
                .max_by_key(|((_, _, _, source), signal)| (signal.issued_at, *source));
            entries.push(if let Some((_, signal)) = winner {
                TypingEntryV1 {
                    participant_id: member.participant_id,
                    participant_kind: map_kind(member.kind),
                    active: true,
                    source: Some(PresenceSourceClassV1::ParticipantSession),
                    expires_at: Some(signal.expires_at),
                }
            } else {
                TypingEntryV1 {
                    participant_id: member.participant_id,
                    participant_kind: map_kind(member.kind),
                    active: false,
                    source: None,
                    expires_at: None,
                }
            });
        }
        let next = members.next.map(|member_cursor| TypingCursorV1 {
            observer: observer.clone(),
            room_id: room_id.clone(),
            target,
            generation: self.generation,
            as_of,
            member_cursor,
        });
        self.last_time = Some(now);
        Ok(TypingPageV1 {
            entries,
            next,
            as_of,
        })
    }

    fn validate_time(&self, now: TransientTimeV1) -> Result<(), TypingRefusalV1> {
        if self.last_time.is_some_and(|last| now < last) {
            return Err(TypingRefusalV1::ClockRegression);
        }
        Ok(())
    }

    fn advance_publication_generation(&mut self) -> Result<(), TypingRefusalV1> {
        if let Some(next) = self.generation.checked_add(1) {
            self.generation = next;
            Ok(())
        } else {
            self.fidelity_lost = true;
            Err(TypingRefusalV1::FidelityLost)
        }
    }

    fn advance_cleanup_generation(&mut self) {
        if let Some(next) = self.generation.checked_add(1) {
            self.generation = next;
        } else {
            self.fidelity_lost = true;
        }
    }
}

fn validate_authority(
    engine: &Engine,
    room_id: &RoomId,
    participant: &ParticipantId,
    target: TypingTargetV1,
) -> Result<(), TypingRefusalV1> {
    if engine.member_role(room_id, participant).is_none() {
        return Err(TypingRefusalV1::NotMember);
    }
    if !target_exists(engine, room_id, target) {
        return Err(TypingRefusalV1::InvalidTarget);
    }
    Ok(())
}

fn target_exists(engine: &Engine, room_id: &RoomId, target: TypingTargetV1) -> bool {
    match target {
        TypingTargetV1::Room => engine.room(room_id).is_some(),
        TypingTargetV1::Thread(root) => engine
            .message_by_id(room_id, root)
            .is_some_and(|message| message.reply_to_root().is_none()),
        TypingTargetV1::Topic(topic) => engine.topic(room_id, topic).is_some(),
    }
}

fn map_kind(kind: ParticipantKind) -> TransientParticipantKindV1 {
    match kind {
        ParticipantKind::Human => TransientParticipantKindV1::Human,
        ParticipantKind::Agent => TransientParticipantKindV1::Agent,
    }
}

fn map_observation_refusal(refusal: ObservationRefusal) -> TypingRefusalV1 {
    match refusal {
        ObservationRefusal::ForeignCursor => TypingRefusalV1::ForeignCursor,
        ObservationRefusal::Stale { .. } => TypingRefusalV1::EngineChanged,
        ObservationRefusal::InvalidLimit => TypingRefusalV1::InvalidPage,
        ObservationRefusal::Capacity | ObservationRefusal::InconsistentState => {
            TypingRefusalV1::Capacity
        }
        ObservationRefusal::InvalidTarget => TypingRefusalV1::InvalidTarget,
        ObservationRefusal::UnknownParticipant
        | ObservationRefusal::UnknownRoom
        | ObservationRefusal::NotMember
        | ObservationRefusal::NotAuthorized => TypingRefusalV1::NotMember,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agora_engine::{AdmissionPolicy, CommandV2, Participant};

    fn fixture() -> (Engine, ParticipantId, RoomId) {
        let mut engine = Engine::new();
        let participant = ParticipantId::new("participant");
        let room = RoomId::new("room");
        engine
            .apply(CommandV2::RegisterParticipant(Participant::new(
                "participant",
                "participant",
                ParticipantKind::Human,
            )))
            .unwrap();
        engine
            .apply(CommandV2::CreateRoom {
                room_id: room.clone(),
                name: "room".to_owned(),
                owner: participant.clone(),
                admission: AdmissionPolicy::Open,
            })
            .unwrap();
        (engine, participant, room)
    }

    #[test]
    fn generation_saturation_fences_observation_but_never_cleanup() {
        let (engine, participant, room) = fixture();
        let source = PresenceSourceV1::new([7; 16]);
        let mut owner = TypingCollaborationV1::new(TypingLimitsV1::default()).unwrap();
        owner
            .publish(
                &engine,
                &room,
                &participant,
                TypingTargetV1::Room,
                source,
                TransientTimeV1(1),
                1,
            )
            .unwrap();
        owner.generation = u64::MAX;
        assert_eq!(owner.retire_source(source), 1);
        assert!(owner.fidelity_lost());
        assert_eq!(
            owner.page(
                &engine,
                &participant,
                &room,
                TypingTargetV1::Room,
                None,
                1,
                TransientTimeV1(2),
                TransientTimeV1(2),
            ),
            Err(TypingRefusalV1::FidelityLost)
        );
    }

    #[test]
    fn generation_saturation_refuses_publication_before_mutation() {
        let (engine, participant, room) = fixture();
        let mut owner = TypingCollaborationV1::new(TypingLimitsV1::default()).unwrap();
        owner.generation = u64::MAX;

        assert_eq!(
            owner.publish(
                &engine,
                &room,
                &participant,
                TypingTargetV1::Room,
                PresenceSourceV1::new([8; 16]),
                TransientTimeV1(1),
                1,
            ),
            Err(TypingRefusalV1::FidelityLost)
        );
        assert_eq!(owner.signal_count(), 0);
        assert!(owner.fidelity_lost());
    }
}
