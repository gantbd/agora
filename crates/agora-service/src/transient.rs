//! Bounded volatile ownership for collaboration signals.
//!
//! This module is intentionally not wired into Protocol V2 yet. It establishes
//! the state owner and its authority, clock, privacy, cursor, and retirement
//! rules without advertising an incomplete public capability.

use agora_engine::{
    Engine, MemberPageCursor, ObservationRefusal, ParticipantId, ParticipantKind, RoomId,
};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TransientTimeV1(pub u64);

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PresenceSourceV1([u8; 16]);

impl PresenceSourceV1 {
    pub const fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }
}

impl fmt::Debug for PresenceSourceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PresenceSourceV1(<opaque>)")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresenceStateV1 {
    Available,
    Away,
    Busy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresenceViewStateV1 {
    Unknown,
    Available,
    Away,
    Busy,
}

impl From<PresenceStateV1> for PresenceViewStateV1 {
    fn from(value: PresenceStateV1) -> Self {
        match value {
            PresenceStateV1::Available => Self::Available,
            PresenceStateV1::Away => Self::Away,
            PresenceStateV1::Busy => Self::Busy,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresenceSourceClassV1 {
    ParticipantSession,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransientParticipantKindV1 {
    Human,
    Agent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransientLimitsV1 {
    pub max_signals: usize,
    pub max_sources_per_participant: usize,
    pub max_lifetime: u64,
    pub max_page_items: usize,
}

impl Default for TransientLimitsV1 {
    fn default() -> Self {
        Self {
            max_signals: 4_096,
            max_sources_per_participant: 8,
            max_lifetime: 300,
            max_page_items: 256,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransientRefusalV1 {
    InvalidLimits,
    InvalidLifetime,
    ClockRegression,
    NotMember,
    Capacity,
    FidelityLost,
    InvalidWorkBudget,
    InvalidPage,
    ForeignCursor,
    StaleCursor,
    EngineChanged,
}

#[derive(Clone, Eq, PartialEq)]
struct PresenceSignalV1 {
    state: PresenceStateV1,
    issued_at: TransientTimeV1,
    expires_at: TransientTimeV1,
}

#[derive(Clone, Eq, PartialEq)]
pub struct PresenceEntryV1 {
    pub participant_id: ParticipantId,
    pub participant_kind: TransientParticipantKindV1,
    pub state: PresenceViewStateV1,
    pub source: Option<PresenceSourceClassV1>,
    pub expires_at: Option<TransientTimeV1>,
}

impl fmt::Debug for PresenceEntryV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PresenceEntryV1")
            .field("participant_id", &"<redacted>")
            .field("participant_kind", &self.participant_kind)
            .field("state", &self.state)
            .field("source", &self.source)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct PresenceCursorV1 {
    observer: ParticipantId,
    room_id: RoomId,
    generation: u64,
    as_of: TransientTimeV1,
    member_cursor: MemberPageCursor,
}

impl fmt::Debug for PresenceCursorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PresenceCursorV1(<opaque>)")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresencePageV1 {
    pub entries: Vec<PresenceEntryV1>,
    pub next: Option<PresenceCursorV1>,
    pub as_of: TransientTimeV1,
}

/// Separate volatile owner for expiring collaboration facts.
///
/// Construction is fallible so zero bounds never become implicit unlimited
/// profiles. The owner starts empty: process recovery therefore projects every
/// participant as `Unknown` until a current session publishes again.
pub struct TransientCollaborationV1 {
    limits: TransientLimitsV1,
    signals: BTreeMap<(RoomId, ParticipantId, PresenceSourceV1), PresenceSignalV1>,
    generation: u64,
    fidelity_lost: bool,
    last_time: Option<TransientTimeV1>,
}

impl TransientCollaborationV1 {
    pub fn new(limits: TransientLimitsV1) -> Result<Self, TransientRefusalV1> {
        if limits.max_signals == 0
            || limits.max_sources_per_participant == 0
            || limits.max_lifetime == 0
            || limits.max_page_items == 0
        {
            return Err(TransientRefusalV1::InvalidLimits);
        }
        Ok(Self {
            limits,
            signals: BTreeMap::new(),
            generation: 0,
            fidelity_lost: false,
            last_time: None,
        })
    }

    pub fn limits(&self) -> TransientLimitsV1 {
        self.limits
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn signal_count(&self) -> usize {
        self.signals.len()
    }

    pub fn fidelity_lost(&self) -> bool {
        self.fidelity_lost
    }

    #[allow(clippy::too_many_arguments)]
    pub fn publish_presence(
        &mut self,
        engine: &Engine,
        room_id: &RoomId,
        participant: &ParticipantId,
        source: PresenceSourceV1,
        state: PresenceStateV1,
        now: TransientTimeV1,
        lifetime: u64,
    ) -> Result<TransientTimeV1, TransientRefusalV1> {
        self.validate_time(now)?;
        if self.fidelity_lost {
            return Err(TransientRefusalV1::FidelityLost);
        }
        if engine.member_role(room_id, participant).is_none() {
            return Err(TransientRefusalV1::NotMember);
        }
        if lifetime == 0 || lifetime > self.limits.max_lifetime {
            return Err(TransientRefusalV1::InvalidLifetime);
        }
        let expires_at = TransientTimeV1(
            now.0
                .checked_add(lifetime)
                .ok_or(TransientRefusalV1::InvalidLifetime)?,
        );
        let key = (room_id.clone(), participant.clone(), source);
        let replacement = self.signals.contains_key(&key);
        if !replacement {
            if self.signals.len() == self.limits.max_signals {
                return Err(TransientRefusalV1::Capacity);
            }
            let sources = self
                .signals
                .keys()
                .filter(|(room, actor, _)| room == room_id && actor == participant)
                .count();
            if sources == self.limits.max_sources_per_participant {
                return Err(TransientRefusalV1::Capacity);
            }
        }
        let next = PresenceSignalV1 {
            state,
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

    /// String-bound composition seam for hosts that intentionally do not
    /// depend on engine types.
    #[allow(clippy::too_many_arguments)]
    pub fn publish_presence_for(
        &mut self,
        engine: &Engine,
        room_id: &str,
        participant: &str,
        source: PresenceSourceV1,
        state: PresenceStateV1,
        now: TransientTimeV1,
        lifetime: u64,
    ) -> Result<TransientTimeV1, TransientRefusalV1> {
        self.publish_presence(
            engine,
            &RoomId::new(room_id),
            &ParticipantId::new(participant),
            source,
            state,
            now,
            lifetime,
        )
    }

    pub fn clear_presence(
        &mut self,
        engine: &Engine,
        room_id: &RoomId,
        participant: &ParticipantId,
        source: PresenceSourceV1,
        now: TransientTimeV1,
    ) -> Result<bool, TransientRefusalV1> {
        self.validate_time(now)?;
        if engine.member_role(room_id, participant).is_none() {
            return Err(TransientRefusalV1::NotMember);
        }
        let removed = self
            .signals
            .contains_key(&(room_id.clone(), participant.clone(), source));
        if removed {
            self.advance_cleanup_generation();
            self.signals
                .remove(&(room_id.clone(), participant.clone(), source));
        }
        self.last_time = Some(now);
        Ok(removed)
    }

    pub fn clear_presence_for(
        &mut self,
        engine: &Engine,
        room_id: &str,
        participant: &str,
        source: PresenceSourceV1,
        now: TransientTimeV1,
    ) -> Result<bool, TransientRefusalV1> {
        self.clear_presence(
            engine,
            &RoomId::new(room_id),
            &ParticipantId::new(participant),
            source,
            now,
        )
    }

    /// Trusted lifecycle retirement for one Host-session source.
    pub fn retire_source(&mut self, source: PresenceSourceV1) -> usize {
        let removed = self
            .signals
            .keys()
            .filter(|(_, _, candidate)| *candidate == source)
            .count();
        if removed > 0 {
            self.advance_cleanup_generation();
        }
        self.signals
            .retain(|(_, _, candidate), _| *candidate != source);
        removed
    }

    /// Trusted transition retirement used for leave, kick, and ban composition.
    pub fn retire_participant_room(
        &mut self,
        room_id: &RoomId,
        participant: &ParticipantId,
    ) -> usize {
        let removed = self
            .signals
            .keys()
            .filter(|(room, actor, _)| room == room_id && actor == participant)
            .count();
        if removed > 0 {
            self.advance_cleanup_generation();
        }
        self.signals
            .retain(|(room, actor, _), _| room != room_id || actor != participant);
        removed
    }

    /// Retires every signal whose participant is no longer a current member.
    /// Cleanup remains available even after observation-generation fidelity is
    /// lost, so lifecycle safety never depends on counter headroom.
    pub fn reconcile_membership(&mut self, engine: &Engine) -> usize {
        let removed = self
            .signals
            .keys()
            .filter(|(room, participant, _)| engine.member_role(room, participant).is_none())
            .count();
        if removed > 0 {
            self.advance_cleanup_generation();
        }
        self.signals
            .retain(|(room, participant, _), _| engine.member_role(room, participant).is_some());
        removed
    }

    /// Removes at most `budget` expired records using the qualified clock.
    ///
    /// Observation never performs hidden cleanup. A runtime drives this method
    /// explicitly when it wants bounded capacity reclamation.
    pub fn expire(
        &mut self,
        now: TransientTimeV1,
        budget: usize,
    ) -> Result<usize, TransientRefusalV1> {
        self.validate_time(now)?;
        if budget == 0 || budget > self.limits.max_signals {
            return Err(TransientRefusalV1::InvalidWorkBudget);
        }
        let mut expired = Vec::new();
        expired
            .try_reserve_exact(budget)
            .map_err(|_| TransientRefusalV1::Capacity)?;
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

    pub fn presence_page(
        &mut self,
        engine: &Engine,
        observer: &ParticipantId,
        room_id: &RoomId,
        cursor: Option<&PresenceCursorV1>,
        limit: usize,
        as_of: TransientTimeV1,
    ) -> Result<PresencePageV1, TransientRefusalV1> {
        self.presence_page_at(engine, observer, room_id, cursor, limit, as_of, as_of)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn presence_page_at(
        &mut self,
        engine: &Engine,
        observer: &ParticipantId,
        room_id: &RoomId,
        cursor: Option<&PresenceCursorV1>,
        limit: usize,
        now: TransientTimeV1,
        as_of: TransientTimeV1,
    ) -> Result<PresencePageV1, TransientRefusalV1> {
        self.validate_time(now)?;
        if as_of > now {
            return Err(TransientRefusalV1::StaleCursor);
        }
        if self.fidelity_lost {
            return Err(TransientRefusalV1::FidelityLost);
        }
        if limit == 0 || limit > self.limits.max_page_items {
            return Err(TransientRefusalV1::InvalidPage);
        }
        let member_cursor = if let Some(cursor) = cursor {
            if &cursor.observer != observer || &cursor.room_id != room_id {
                return Err(TransientRefusalV1::ForeignCursor);
            }
            if cursor.generation != self.generation || cursor.as_of != as_of {
                return Err(TransientRefusalV1::StaleCursor);
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
            .map_err(|_| TransientRefusalV1::Capacity)?;
        for member in members.members {
            let winner = self
                .signals
                .iter()
                .filter(|((room, participant, _), signal)| {
                    room == room_id
                        && participant == &member.participant_id
                        && as_of < signal.expires_at
                })
                .max_by_key(|((_, _, source), signal)| (signal.issued_at, *source));
            entries.push(if let Some((_, signal)) = winner {
                PresenceEntryV1 {
                    participant_id: member.participant_id,
                    participant_kind: map_participant_kind(member.kind),
                    state: signal.state.into(),
                    source: Some(PresenceSourceClassV1::ParticipantSession),
                    expires_at: Some(signal.expires_at),
                }
            } else {
                PresenceEntryV1 {
                    participant_id: member.participant_id,
                    participant_kind: map_participant_kind(member.kind),
                    state: PresenceViewStateV1::Unknown,
                    source: None,
                    expires_at: None,
                }
            });
        }
        let next = members.next.map(|member_cursor| PresenceCursorV1 {
            observer: observer.clone(),
            room_id: room_id.clone(),
            generation: self.generation,
            as_of,
            member_cursor,
        });
        self.last_time = Some(now);
        Ok(PresencePageV1 {
            entries,
            next,
            as_of,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn presence_page_for(
        &mut self,
        engine: &Engine,
        observer: &str,
        room_id: &str,
        cursor: Option<&PresenceCursorV1>,
        limit: usize,
        as_of: TransientTimeV1,
    ) -> Result<PresencePageV1, TransientRefusalV1> {
        self.presence_page(
            engine,
            &ParticipantId::new(observer),
            &RoomId::new(room_id),
            cursor,
            limit,
            as_of,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn presence_page_at_for(
        &mut self,
        engine: &Engine,
        observer: &str,
        room_id: &str,
        cursor: Option<&PresenceCursorV1>,
        limit: usize,
        now: TransientTimeV1,
        as_of: TransientTimeV1,
    ) -> Result<PresencePageV1, TransientRefusalV1> {
        self.presence_page_at(
            engine,
            &ParticipantId::new(observer),
            &RoomId::new(room_id),
            cursor,
            limit,
            now,
            as_of,
        )
    }

    fn validate_time(&self, now: TransientTimeV1) -> Result<(), TransientRefusalV1> {
        if self.last_time.is_some_and(|last| now < last) {
            return Err(TransientRefusalV1::ClockRegression);
        }
        Ok(())
    }

    fn advance_publication_generation(&mut self) -> Result<(), TransientRefusalV1> {
        if let Some(next) = self.generation.checked_add(1) {
            self.generation = next;
            Ok(())
        } else {
            self.fidelity_lost = true;
            Err(TransientRefusalV1::FidelityLost)
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

fn map_observation_refusal(refusal: ObservationRefusal) -> TransientRefusalV1 {
    match refusal {
        ObservationRefusal::ForeignCursor => TransientRefusalV1::ForeignCursor,
        ObservationRefusal::Stale { .. } => TransientRefusalV1::EngineChanged,
        ObservationRefusal::InvalidLimit => TransientRefusalV1::InvalidPage,
        ObservationRefusal::Capacity | ObservationRefusal::InconsistentState => {
            TransientRefusalV1::Capacity
        }
        ObservationRefusal::UnknownParticipant
        | ObservationRefusal::UnknownRoom
        | ObservationRefusal::NotMember
        | ObservationRefusal::NotAuthorized => TransientRefusalV1::NotMember,
        ObservationRefusal::InvalidTarget => TransientRefusalV1::EngineChanged,
    }
}

fn map_participant_kind(kind: ParticipantKind) -> TransientParticipantKindV1 {
    match kind {
        ParticipantKind::Human => TransientParticipantKindV1::Human,
        ParticipantKind::Agent => TransientParticipantKindV1::Agent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agora_engine::{AdmissionPolicy, CommandV2, Participant, ParticipantKind};

    #[test]
    fn generation_saturation_fences_observation_but_never_cleanup() {
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
        let source = PresenceSourceV1::new([7; 16]);
        let mut owner = TransientCollaborationV1::new(TransientLimitsV1::default()).unwrap();
        owner
            .publish_presence(
                &engine,
                &room,
                &participant,
                source,
                PresenceStateV1::Available,
                TransientTimeV1(1),
                1,
            )
            .unwrap();
        owner.generation = u64::MAX;
        assert_eq!(owner.retire_source(source), 1);
        assert!(owner.fidelity_lost());
        assert_eq!(
            owner.presence_page(&engine, &participant, &room, None, 1, TransientTimeV1(2),),
            Err(TransientRefusalV1::FidelityLost)
        );
    }

    #[test]
    fn generation_saturation_refuses_publication_before_mutation() {
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
        let mut owner = TransientCollaborationV1::new(TransientLimitsV1::default()).unwrap();
        owner.generation = u64::MAX;

        assert_eq!(
            owner.publish_presence(
                &engine,
                &room,
                &participant,
                PresenceSourceV1::new([8; 16]),
                PresenceStateV1::Available,
                TransientTimeV1(1),
                1,
            ),
            Err(TransientRefusalV1::FidelityLost)
        );
        assert_eq!(owner.signal_count(), 0);
        assert!(owner.fidelity_lost());
    }
}
