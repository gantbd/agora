//! Canonical bounded persistence for authenticated Host V2 and its service.

use super::*;
use agora_service::v2::persistence::{ServiceCheckpointScopeV1, ServiceRecoveryLimitsV2};
use std::collections::BTreeSet;

const MAGIC: &[u8; 8] = b"AGORAHST";
const VERSION: u8 = 13;

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct HostCheckpointScopeV1([u8; 16]);

impl HostCheckpointScopeV1 {
    pub const fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    fn service_scope(self) -> ServiceCheckpointScopeV1 {
        ServiceCheckpointScopeV1::new(self.0)
    }
}

impl fmt::Debug for HostCheckpointScopeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HostCheckpointScopeV1(<redacted>)")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostRecoveryLimitsV2 {
    pub max_snapshot_bytes: usize,
    pub max_service_snapshot_bytes: usize,
    pub maximum_host_limits: HostLimits,
    pub service: ServiceRecoveryLimitsV2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostCheckpointErrorV2 {
    Limit,
    Capacity,
    Truncated,
    Profile,
    Scope,
    Invalid,
    NonCanonical,
    Trailing,
    Service,
}

impl fmt::Display for HostCheckpointErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Agora host checkpoint: {self:?}")
    }
}

impl std::error::Error for HostCheckpointErrorV2 {}

pub struct HostCheckpointV2(Vec<u8>);

impl HostCheckpointV2 {
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for HostCheckpointV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HostCheckpointV2(<redacted>)")
    }
}

struct Writer {
    bytes: Vec<u8>,
    max: usize,
}

impl Writer {
    fn raw(&mut self, value: &[u8]) -> Result<(), HostCheckpointErrorV2> {
        if self
            .bytes
            .len()
            .checked_add(value.len())
            .is_none_or(|length| length > self.max)
        {
            return Err(HostCheckpointErrorV2::Limit);
        }
        self.bytes
            .try_reserve(value.len())
            .map_err(|_| HostCheckpointErrorV2::Capacity)?;
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn byte(&mut self, value: u8) -> Result<(), HostCheckpointErrorV2> {
        self.raw(&[value])
    }

    fn number(&mut self, value: u64) -> Result<(), HostCheckpointErrorV2> {
        self.raw(&value.to_be_bytes())
    }

    fn size(&mut self, value: usize) -> Result<(), HostCheckpointErrorV2> {
        self.number(value.try_into().map_err(|_| HostCheckpointErrorV2::Limit)?)
    }

    fn blob(&mut self, value: &[u8]) -> Result<(), HostCheckpointErrorV2> {
        self.size(value.len())?;
        self.raw(value)
    }

    fn text(&mut self, value: &str) -> Result<(), HostCheckpointErrorV2> {
        self.blob(value.as_bytes())
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    fn raw(&mut self, length: usize) -> Result<&'a [u8], HostCheckpointErrorV2> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(HostCheckpointErrorV2::Limit)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(HostCheckpointErrorV2::Truncated)?;
        self.position = end;
        Ok(value)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], HostCheckpointErrorV2> {
        self.raw(N)?
            .try_into()
            .map_err(|_| HostCheckpointErrorV2::Truncated)
    }

    fn byte(&mut self) -> Result<u8, HostCheckpointErrorV2> {
        Ok(self.fixed::<1>()?[0])
    }

    fn number(&mut self) -> Result<u64, HostCheckpointErrorV2> {
        Ok(u64::from_be_bytes(self.fixed()?))
    }

    fn size(&mut self, maximum: usize) -> Result<usize, HostCheckpointErrorV2> {
        let value = usize::try_from(self.number()?).map_err(|_| HostCheckpointErrorV2::Limit)?;
        if value > maximum {
            return Err(HostCheckpointErrorV2::Limit);
        }
        Ok(value)
    }

    fn blob(&mut self, maximum: usize) -> Result<&'a [u8], HostCheckpointErrorV2> {
        let length = self.size(maximum)?;
        self.raw(length)
    }

    fn text(&mut self, maximum: usize) -> Result<String, HostCheckpointErrorV2> {
        let bytes = self.blob(maximum)?;
        let value = std::str::from_utf8(bytes).map_err(|_| HostCheckpointErrorV2::Invalid)?;
        let mut owned = String::new();
        owned
            .try_reserve(value.len())
            .map_err(|_| HostCheckpointErrorV2::Capacity)?;
        owned.push_str(value);
        Ok(owned)
    }
}

struct HostStateV2 {
    service: ServiceV2,
    limits: HostLimits,
    sessions: BTreeMap<HostSessionIdV2, SessionV2>,
    retired: VecDeque<HostSessionIdV2>,
    next_session: u64,
}

fn write_limits(writer: &mut Writer, limits: HostLimits) -> Result<(), HostCheckpointErrorV2> {
    for value in [
        limits.max_sessions,
        limits.max_retired_sessions,
        limits.wire.max_frame_bytes,
        limits.wire.max_text_bytes,
        limits.wire.max_fields,
        limits.wire.max_list_items,
    ] {
        writer.size(value)?;
    }
    Ok(())
}

fn read_limits(
    reader: &mut Reader<'_>,
    maximum: HostLimits,
) -> Result<HostLimits, HostCheckpointErrorV2> {
    Ok(HostLimits {
        max_sessions: reader.size(maximum.max_sessions)?,
        max_retired_sessions: reader.size(maximum.max_retired_sessions)?,
        wire: agora_wire::WireLimits {
            max_frame_bytes: reader.size(maximum.wire.max_frame_bytes)?,
            max_text_bytes: reader.size(maximum.wire.max_text_bytes)?,
            max_fields: reader.size(maximum.wire.max_fields)?,
            max_list_items: reader.size(maximum.wire.max_list_items)?,
        },
    })
}

fn session_ordinal(id: HostSessionIdV2) -> Option<u64> {
    if id.0[0] != b'2' || id.0[1..8] != [0; 7] {
        return None;
    }
    Some(u64::from_be_bytes(id.0[8..].try_into().ok()?))
}

fn validate_state(
    service: &ServiceV2,
    limits: HostLimits,
    sessions: &BTreeMap<HostSessionIdV2, SessionV2>,
    retired: &VecDeque<HostSessionIdV2>,
    next_session: u64,
) -> Result<(), HostCheckpointErrorV2> {
    if sessions.len() > limits.max_sessions
        || retired.len() > limits.max_retired_sessions
        || sessions.len() != service.active_client_count()
    {
        return Err(HostCheckpointErrorV2::Invalid);
    }
    let mut clients = BTreeSet::new();
    for (session_id, session) in sessions {
        if session_ordinal(*session_id).is_none_or(|ordinal| ordinal >= next_session)
            || !clients.insert(session.client)
            || service
                .participant_for_client(session.client)
                .is_none_or(|participant| participant.as_str() != session.participant)
        {
            return Err(HostCheckpointErrorV2::Invalid);
        }
    }
    let mut retired_seen = BTreeSet::new();
    for session_id in retired {
        if session_ordinal(*session_id).is_none_or(|ordinal| ordinal >= next_session)
            || sessions.contains_key(session_id)
            || !retired_seen.insert(*session_id)
        {
            return Err(HostCheckpointErrorV2::Invalid);
        }
    }
    Ok(())
}

fn encode_state(
    service_owner: &ServiceV2,
    limits: HostLimits,
    sessions: &BTreeMap<HostSessionIdV2, SessionV2>,
    retired: &VecDeque<HostSessionIdV2>,
    next_session: u64,
    scope: HostCheckpointScopeV1,
    max_snapshot_bytes: usize,
) -> Result<HostCheckpointV2, HostCheckpointErrorV2> {
    validate_state(service_owner, limits, sessions, retired, next_session)?;
    let service = service_owner
        .prepare_checkpoint(scope.service_scope(), max_snapshot_bytes)
        .map_err(|_| HostCheckpointErrorV2::Service)?;
    let mut writer = Writer {
        bytes: Vec::new(),
        max: max_snapshot_bytes,
    };
    writer.raw(MAGIC)?;
    writer.byte(VERSION)?;
    writer.raw(&scope.0)?;
    write_limits(&mut writer, limits)?;
    writer.number(next_session)?;
    writer.blob(service.bytes())?;
    writer.size(sessions.len())?;
    for (session_id, session) in sessions {
        writer.raw(&session_id.0)?;
        writer.raw(&session.subject.0)?;
        writer.text(&session.participant)?;
        writer.number(session.client.0)?;
        writer.number(session.last_verified_at.0)?;
    }
    writer.size(retired.len())?;
    for session_id in retired {
        writer.raw(&session_id.0)?;
    }
    Ok(HostCheckpointV2(writer.bytes))
}

fn decode_state(
    bytes: &[u8],
    expected_scope: HostCheckpointScopeV1,
    bounds: HostRecoveryLimitsV2,
) -> Result<HostStateV2, HostCheckpointErrorV2> {
    if bytes.len() > bounds.max_snapshot_bytes {
        return Err(HostCheckpointErrorV2::Limit);
    }
    let mut reader = Reader { bytes, position: 0 };
    if reader.raw(MAGIC.len())? != MAGIC || reader.byte()? != VERSION {
        return Err(HostCheckpointErrorV2::Profile);
    }
    if reader.fixed::<16>()? != expected_scope.0 {
        return Err(HostCheckpointErrorV2::Scope);
    }
    let limits = read_limits(&mut reader, bounds.maximum_host_limits)?;
    let next_session = reader.number()?;
    let service = ServiceV2::recover_checkpoint(
        reader.blob(bounds.max_service_snapshot_bytes)?,
        expected_scope.service_scope(),
        bounds.service,
    )
    .map_err(|_| HostCheckpointErrorV2::Service)?;
    let participant_maximum = service.limits().max_participant_id_bytes;

    let session_count = reader.size(limits.max_sessions)?;
    let mut sessions = BTreeMap::new();
    let mut clients = BTreeSet::new();
    for _ in 0..session_count {
        let session_id = HostSessionIdV2(reader.fixed()?);
        let subject = VerifiedSubjectV1(reader.fixed()?);
        let participant = reader.text(participant_maximum)?;
        let client = ClientIdV2(reader.number()?);
        let last_verified_at = HostTimeV1(reader.number()?);
        let Some(bound) = service.participant_for_client(client) else {
            return Err(HostCheckpointErrorV2::Invalid);
        };
        if participant.is_empty()
            || bound.as_str() != participant
            || session_ordinal(session_id).is_none_or(|ordinal| ordinal >= next_session)
            || !clients.insert(client)
            || sessions
                .last_key_value()
                .is_some_and(|(old, _)| old >= &session_id)
        {
            return Err(HostCheckpointErrorV2::Invalid);
        }
        sessions.insert(
            session_id,
            SessionV2 {
                subject,
                participant,
                client,
                last_verified_at,
            },
        );
    }
    if clients.len() != service.active_client_count() {
        return Err(HostCheckpointErrorV2::Invalid);
    }

    let retired_count = reader.size(limits.max_retired_sessions)?;
    let mut retired = VecDeque::new();
    let mut retired_seen = BTreeSet::new();
    for _ in 0..retired_count {
        let session_id = HostSessionIdV2(reader.fixed()?);
        if session_ordinal(session_id).is_none_or(|ordinal| ordinal >= next_session)
            || sessions.contains_key(&session_id)
            || !retired_seen.insert(session_id)
        {
            return Err(HostCheckpointErrorV2::Invalid);
        }
        retired.push_back(session_id);
    }
    if reader.position != bytes.len() {
        return Err(HostCheckpointErrorV2::Trailing);
    }
    let state = HostStateV2 {
        service,
        limits,
        sessions,
        retired,
        next_session,
    };
    let canonical = encode_state(
        &state.service,
        state.limits,
        &state.sessions,
        &state.retired,
        state.next_session,
        expected_scope,
        bounds.max_snapshot_bytes,
    )?;
    if canonical.bytes() != bytes {
        return Err(HostCheckpointErrorV2::NonCanonical);
    }
    Ok(state)
}

impl<V> HostV2<V> {
    pub fn prepare_checkpoint(
        &self,
        scope: HostCheckpointScopeV1,
        max_snapshot_bytes: usize,
    ) -> Result<HostCheckpointV2, HostCheckpointErrorV2> {
        encode_state(
            &self.service,
            self.limits,
            &self.sessions,
            &self.retired,
            self.next_session,
            scope,
            max_snapshot_bytes,
        )
    }

    pub fn recover_checkpoint(
        bytes: &[u8],
        expected_scope: HostCheckpointScopeV1,
        bounds: HostRecoveryLimitsV2,
        verifier: V,
    ) -> Result<Self, HostCheckpointErrorV2> {
        let state = decode_state(bytes, expected_scope, bounds)?;
        Ok(Self {
            service: state.service,
            verifier,
            limits: state.limits,
            sessions: state.sessions,
            retired: state.retired,
            next_session: state.next_session,
            usage: HostUsage::default(),
        })
    }

    pub fn restore_checkpoint_preserving_runtime(
        &mut self,
        bytes: &[u8],
        expected_scope: HostCheckpointScopeV1,
        bounds: HostRecoveryLimitsV2,
    ) -> Result<(), HostCheckpointErrorV2> {
        let state = decode_state(bytes, expected_scope, bounds)?;
        self.service = state.service;
        self.limits = state.limits;
        self.sessions = state.sessions;
        self.retired = state.retired;
        self.next_session = state.next_session;
        Ok(())
    }
}
