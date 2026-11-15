//! Canonical bounded persistence for the complete serialized V2 service owner.
//!
//! This profile is versioned independently from the public wire protocol. It
//! embeds canonical request/reply payloads, but the outer record owns migration,
//! service limits, engine state, private cursor state, and store-scope binding.

use super::*;
use agora_engine::{
    Limits, persistence::CheckpointError as EngineCheckpointError,
    persistence::CursorRecoveryLimitsV1, persistence::RecoveryLimits,
};
use agora_wire::{
    WireLimits, v2::WireReplyV2, v2::WireRequestV2, v2::decode_reply_v2, v2::decode_request_v2,
    v2::encode_reply_v2, v2::encode_request_v2,
};
use std::collections::BTreeSet;
use std::fmt;

const MAGIC: &[u8; 8] = b"AGORASV\0";
const VERSION_V24: u8 = 24;
const VERSION_V25: u8 = 25;
const VERSION_V26: u8 = 26;
const VERSION_V27: u8 = 27;
const VERSION_V28: u8 = 28;
const VERSION_V29: u8 = 29;
const VERSION_V30: u8 = 30;
const VERSION_V31: u8 = 31;
const VERSION_V32: u8 = 32;

fn command_uses_v26_vocabulary(command: &CommandV2) -> bool {
    matches!(
        command,
        CommandV2::CreatePeerConversation { .. }
            | CommandV2::ConversationProfile { .. }
            | CommandV2::AttentionPreference { .. }
            | CommandV2::SetAttentionPreference { .. }
            | CommandV2::Pins { .. }
            | CommandV2::SetPin { .. }
            | CommandV2::SavedReference { .. }
            | CommandV2::SavedReferences { .. }
            | CommandV2::SetSavedReference { .. }
    )
}

fn reply_uses_v26_vocabulary(reply: &ReplyV2) -> bool {
    let ReplyV2::Accepted(outcome) = reply else {
        return false;
    };
    match outcome {
        OutcomeV2::PeerConversationCreated { .. }
        | OutcomeV2::ConversationProfile(_)
        | OutcomeV2::AttentionPreference(_)
        | OutcomeV2::AttentionPreferenceSet(_)
        | OutcomeV2::Pins(_)
        | OutcomeV2::PinChanged { .. }
        | OutcomeV2::SavedReference(_)
        | OutcomeV2::SavedReferences(_)
        | OutcomeV2::SavedReferenceSet { .. } => true,
        OutcomeV2::Capabilities(capabilities) => capabilities.iter().any(|capability| {
            matches!(
                capability,
                CapabilityV2::ConversationProfilesV1
                    | CapabilityV2::PrivateAttentionPreferencesV1
                    | CapabilityV2::SharedPinsV1
                    | CapabilityV2::PrivateSavedReferencesV1
            )
        }),
        OutcomeV2::Changes(page) => page.changes.iter().any(|change| {
            matches!(
                change.event,
                RoomEventV2::PeerConversationCreated(_) | RoomEventV2::PinChanged { .. }
            )
        }),
        OutcomeV2::IdentifiedChanges(page) => page.changes.iter().any(|change| {
            matches!(
                change.event,
                IdentifiedRoomEventV1::PeerConversationCreated(_)
                    | IdentifiedRoomEventV1::PinChanged { .. }
            )
        }),
        OutcomeV2::AttachmentChanges(page) => page.identified.changes.iter().any(|change| {
            matches!(
                change.event,
                IdentifiedRoomEventV1::PeerConversationCreated(_)
                    | IdentifiedRoomEventV1::PinChanged { .. }
            )
        }),
        OutcomeV2::ThreadedChanges(page) => page.changes.iter().any(|change| {
            matches!(
                change.event,
                ThreadedRoomEventV1::PeerConversationCreated(_)
                    | ThreadedRoomEventV1::PinChanged { .. }
            )
        }),
        OutcomeV2::TopicChanges(page) => page.changes.iter().any(|change| {
            matches!(
                change.event,
                TopicRoomEventV1::PeerConversationCreated(_) | TopicRoomEventV1::PinChanged { .. }
            )
        }),
        _ => false,
    }
}

fn retained_uses_vellum(retained: &RetainedRequestV2) -> bool {
    matches!(
        retained.request.command,
        CommandV2::VellumArtifactMessages { .. } | CommandV2::SpeakWithVellumArtifacts { .. }
    ) || matches!(
        retained.reply,
        ReplyV2::Accepted(
            OutcomeV2::VellumArtifactMessages(_) | OutcomeV2::VellumArtifactMessageAccepted { .. }
        )
    )
}

fn command_uses_topic_collections(command: &CommandV2) -> bool {
    matches!(command, CommandV2::TopicCollection { .. })
}

fn reply_uses_topic_collections(reply: &ReplyV2) -> bool {
    matches!(reply, ReplyV2::Accepted(OutcomeV2::TopicCollection(_)))
        || matches!(
            reply,
            ReplyV2::Accepted(OutcomeV2::Capabilities(capabilities))
                if capabilities.contains(&CapabilityV2::TopicCollectionsV1)
        )
}

fn retained_uses_topic_collections(retained: &RetainedRequestV2) -> bool {
    command_uses_topic_collections(&retained.request.command)
        || reply_uses_topic_collections(&retained.reply)
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ServiceCheckpointScopeV1([u8; 16]);

impl ServiceCheckpointScopeV1 {
    pub const fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 16] {
        self.0
    }
}

impl fmt::Debug for ServiceCheckpointScopeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ServiceCheckpointScopeV1(<redacted>)")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceRecoveryLimitsV2 {
    pub max_snapshot_bytes: usize,
    pub max_engine_snapshot_bytes: usize,
    pub maximum_service_limits: ServiceLimitsV2,
    pub maximum_engine_limits: Limits,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceCheckpointErrorV2 {
    Limit,
    Capacity,
    Truncated,
    Profile,
    Scope,
    Invalid,
    NonCanonical,
    Trailing,
    Engine,
    Cursor,
    Wire,
}

impl fmt::Display for ServiceCheckpointErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Agora service checkpoint: {self:?}")
    }
}

impl std::error::Error for ServiceCheckpointErrorV2 {}

pub struct ServiceCheckpointV2(Vec<u8>);

impl ServiceCheckpointV2 {
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for ServiceCheckpointV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ServiceCheckpointV2(<redacted>)")
    }
}

struct Writer {
    bytes: Vec<u8>,
    max: usize,
}

impl Writer {
    fn raw(&mut self, value: &[u8]) -> Result<(), ServiceCheckpointErrorV2> {
        if self
            .bytes
            .len()
            .checked_add(value.len())
            .is_none_or(|length| length > self.max)
        {
            return Err(ServiceCheckpointErrorV2::Limit);
        }
        self.bytes
            .try_reserve(value.len())
            .map_err(|_| ServiceCheckpointErrorV2::Capacity)?;
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn byte(&mut self, value: u8) -> Result<(), ServiceCheckpointErrorV2> {
        self.raw(&[value])
    }

    fn number(&mut self, value: u64) -> Result<(), ServiceCheckpointErrorV2> {
        self.raw(&value.to_be_bytes())
    }

    fn size(&mut self, value: usize) -> Result<(), ServiceCheckpointErrorV2> {
        self.number(
            value
                .try_into()
                .map_err(|_| ServiceCheckpointErrorV2::Limit)?,
        )
    }

    fn blob(&mut self, value: &[u8]) -> Result<(), ServiceCheckpointErrorV2> {
        self.size(value.len())?;
        self.raw(value)
    }

    fn text(&mut self, value: &str) -> Result<(), ServiceCheckpointErrorV2> {
        self.blob(value.as_bytes())
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    fn raw(&mut self, length: usize) -> Result<&'a [u8], ServiceCheckpointErrorV2> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(ServiceCheckpointErrorV2::Limit)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(ServiceCheckpointErrorV2::Truncated)?;
        self.position = end;
        Ok(value)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], ServiceCheckpointErrorV2> {
        self.raw(N)?
            .try_into()
            .map_err(|_| ServiceCheckpointErrorV2::Truncated)
    }

    fn byte(&mut self) -> Result<u8, ServiceCheckpointErrorV2> {
        Ok(self.fixed::<1>()?[0])
    }

    fn number(&mut self) -> Result<u64, ServiceCheckpointErrorV2> {
        Ok(u64::from_be_bytes(self.fixed()?))
    }

    fn size(&mut self, maximum: usize) -> Result<usize, ServiceCheckpointErrorV2> {
        let value = usize::try_from(self.number()?).map_err(|_| ServiceCheckpointErrorV2::Limit)?;
        if value > maximum {
            return Err(ServiceCheckpointErrorV2::Limit);
        }
        Ok(value)
    }

    fn blob(&mut self, maximum: usize) -> Result<&'a [u8], ServiceCheckpointErrorV2> {
        let length = self.size(maximum)?;
        self.raw(length)
    }

    fn text(&mut self, maximum: usize) -> Result<String, ServiceCheckpointErrorV2> {
        let bytes = self.blob(maximum)?;
        let value = std::str::from_utf8(bytes).map_err(|_| ServiceCheckpointErrorV2::Invalid)?;
        let mut owned = String::new();
        owned
            .try_reserve(value.len())
            .map_err(|_| ServiceCheckpointErrorV2::Capacity)?;
        owned.push_str(value);
        Ok(owned)
    }
}

fn limits_values(limits: ServiceLimitsV2) -> [usize; 11] {
    [
        limits.max_clients,
        limits.max_retired_clients,
        limits.max_retained_requests,
        limits.max_retained_cursors,
        limits.max_page_items as usize,
        limits.max_participant_id_bytes,
        limits.max_room_id_bytes,
        limits.max_room_name_bytes,
        limits.max_message_body_bytes,
        limits.max_moderation_reason_bytes,
        limits.max_addressed_participants,
    ]
}

fn write_limits(
    writer: &mut Writer,
    limits: ServiceLimitsV2,
) -> Result<(), ServiceCheckpointErrorV2> {
    for value in limits_values(limits) {
        writer.size(value)?;
    }
    Ok(())
}

fn read_limits(
    reader: &mut Reader<'_>,
    maximum: ServiceLimitsV2,
) -> Result<ServiceLimitsV2, ServiceCheckpointErrorV2> {
    let values = limits_values(maximum);
    let max_clients = reader.size(values[0])?;
    let max_retired_clients = reader.size(values[1])?;
    let max_retained_requests = reader.size(values[2])?;
    let max_retained_cursors = reader.size(values[3])?;
    let max_page_items = reader.size(values[4])?;
    Ok(ServiceLimitsV2 {
        max_clients,
        max_retired_clients,
        max_retained_requests,
        max_retained_cursors,
        max_page_items: max_page_items
            .try_into()
            .map_err(|_| ServiceCheckpointErrorV2::Limit)?,
        max_participant_id_bytes: reader.size(values[5])?,
        max_room_id_bytes: reader.size(values[6])?,
        max_room_name_bytes: reader.size(values[7])?,
        max_message_body_bytes: reader.size(values[8])?,
        max_moderation_reason_bytes: reader.size(values[9])?,
        max_addressed_participants: reader.size(values[10])?,
    })
}

fn wire_limits(limits: ServiceLimitsV2, max_frame_bytes: usize) -> WireLimits {
    WireLimits {
        max_frame_bytes,
        max_text_bytes: limits
            .max_message_body_bytes
            .max(limits.max_room_name_bytes)
            .max(limits.max_room_id_bytes)
            .max(limits.max_participant_id_bytes)
            .max(limits.max_moderation_reason_bytes),
        max_fields: max_frame_bytes,
        max_list_items: limits
            .max_page_items
            .try_into()
            .unwrap_or(usize::MAX)
            .max(limits.max_addressed_participants),
    }
}

fn map_engine(error: EngineCheckpointError, cursor: bool) -> ServiceCheckpointErrorV2 {
    let _ = error;
    if cursor {
        ServiceCheckpointErrorV2::Cursor
    } else {
        ServiceCheckpointErrorV2::Engine
    }
}

fn valid_counter(next: u64, values: impl Iterator<Item = u64>) -> bool {
    values.into_iter().all(|value| value < next)
}

impl ServiceV2 {
    pub fn prepare_checkpoint(
        &self,
        scope: ServiceCheckpointScopeV1,
        max_snapshot_bytes: usize,
    ) -> Result<ServiceCheckpointV2, ServiceCheckpointErrorV2> {
        let engine = self
            .engine
            .prepare_checkpoint(max_snapshot_bytes)
            .map_err(|error| map_engine(error, false))?;
        let wire_limits = wire_limits(self.limits, max_snapshot_bytes);
        let mut writer = Writer {
            bytes: Vec::new(),
            max: max_snapshot_bytes,
        };
        let includes_vellum = engine
            .bytes()
            .get(8)
            .is_some_and(|profile| (20..=51).contains(profile) && (profile - 20) & 1 != 0)
            || self
                .cursors
                .values()
                .any(|record| matches!(record.page, CursorPage::VellumArtifactMessages { .. }))
            || self.retained.values().any(retained_uses_vellum);
        let includes_topic_collections = self
            .cursors
            .values()
            .any(|record| matches!(record.page, CursorPage::TopicCollection { .. }))
            || self.retained.values().any(retained_uses_topic_collections);
        let version = if self.receipt_revision != 0 || self.receipt_revision_exhausted {
            VERSION_V32
        } else if self.retention_policy.is_some() {
            VERSION_V31
        } else if self.retired_receipts.is_empty() {
            match (includes_topic_collections, includes_vellum) {
                (false, false) => VERSION_V26,
                (false, true) => VERSION_V27,
                (true, false) => VERSION_V28,
                (true, true) => VERSION_V29,
            }
        } else {
            VERSION_V30
        };
        writer.raw(MAGIC)?;
        writer.byte(version)?;
        writer.raw(&scope.0)?;
        write_limits(&mut writer, self.limits)?;
        writer.number(self.next_client)?;
        writer.number(self.next_cursor)?;
        writer.blob(engine.bytes())?;

        if version == VERSION_V32 {
            writer.byte(u8::from(self.retention_policy.is_some()))?;
        }
        if let Some(policy) = self.retention_policy {
            writer.number(policy.revision.get())?;
            writer.number(policy.effective_engine_revision)?;
            for bound in policy.bounds() {
                match bound {
                    RetentionBoundV1::Disabled => writer.byte(0)?,
                    RetentionBoundV1::Entries(maximum) => {
                        writer.byte(1)?;
                        writer.size(maximum.get())?;
                    }
                }
            }
        }
        if version == VERSION_V32 {
            writer.number(self.receipt_revision)?;
            writer.byte(u8::from(self.receipt_revision_exhausted))?;
        }

        writer.size(self.clients.len())?;
        for (client, participant) in &self.clients {
            writer.number(client.0)?;
            writer.text(participant.as_str())?;
        }

        writer.size(self.retired_clients.len())?;
        for client in &self.retired_clients {
            writer.number(client.0)?;
        }

        writer.size(self.retained.len())?;
        for ((client, request_id), retained) in &self.retained {
            writer.number(client.0)?;
            writer.number(request_id.0)?;
            let request = encode_request_v2(
                &WireRequestV2::Execute {
                    client: *client,
                    request: retained.request.clone(),
                },
                wire_limits,
            )
            .map_err(|_| ServiceCheckpointErrorV2::Wire)?;
            let reply =
                encode_reply_v2(&WireReplyV2::Executed(retained.reply.clone()), wire_limits)
                    .map_err(|_| ServiceCheckpointErrorV2::Wire)?;
            writer.blob(&request)?;
            writer.blob(&reply)?;
        }

        if version == VERSION_V32
            || self.retention_policy.is_some()
            || !self.retired_receipts.is_empty()
        {
            let tombstones = self.retired_receipts.checkpoint();
            writer.size(tombstones.len())?;
            for tombstone in tombstones {
                writer.number(tombstone.policy_revision.get())?;
                writer.number(tombstone.transition_engine_revision)?;
                writer.raw(&tombstone.identity_binding().as_bytes())?;
                writer.raw(&tombstone.scope_binding().as_bytes())?;
            }
        }

        writer.size(self.cursors.len())?;
        for (cursor_id, record) in &self.cursors {
            writer.number(cursor_id.0)?;
            writer.number(record.owner.0)?;
            match &record.page {
                CursorPage::Rooms(cursor) => {
                    writer.byte(1)?;
                    let checkpoint = cursor
                        .prepare_checkpoint(max_snapshot_bytes)
                        .map_err(|error| map_engine(error, true))?;
                    writer.blob(checkpoint.bytes())?;
                }
                CursorPage::Members { room_id, cursor } => {
                    writer.byte(2)?;
                    writer.text(room_id.as_str())?;
                    let checkpoint = cursor
                        .prepare_checkpoint(max_snapshot_bytes)
                        .map_err(|error| map_engine(error, true))?;
                    writer.blob(checkpoint.bytes())?;
                }
                CursorPage::Messages { room_id, cursor } => {
                    writer.byte(3)?;
                    writer.text(room_id.as_str())?;
                    let checkpoint = cursor
                        .prepare_checkpoint(max_snapshot_bytes)
                        .map_err(|error| map_engine(error, true))?;
                    writer.blob(checkpoint.bytes())?;
                }
                CursorPage::Changes { room_id, position } => {
                    writer.byte(4)?;
                    writer.text(room_id.as_str())?;
                    let checkpoint = position
                        .prepare_checkpoint(max_snapshot_bytes)
                        .map_err(|error| map_engine(error, true))?;
                    writer.blob(checkpoint.bytes())?;
                }
                CursorPage::Bans { room_id, cursor } => {
                    writer.byte(5)?;
                    writer.text(room_id.as_str())?;
                    let checkpoint = cursor
                        .prepare_checkpoint(max_snapshot_bytes)
                        .map_err(|error| map_engine(error, true))?;
                    writer.blob(checkpoint.bytes())?;
                }
                CursorPage::IdentifiedMessages { room_id, cursor } => {
                    writer.byte(6)?;
                    writer.text(room_id.as_str())?;
                    let checkpoint = cursor
                        .prepare_checkpoint(max_snapshot_bytes)
                        .map_err(|error| map_engine(error, true))?;
                    writer.blob(checkpoint.bytes())?;
                }
                CursorPage::IdentifiedChanges { room_id, position } => {
                    writer.byte(7)?;
                    writer.text(room_id.as_str())?;
                    let checkpoint = position
                        .prepare_checkpoint(max_snapshot_bytes)
                        .map_err(|error| map_engine(error, true))?;
                    writer.blob(checkpoint.bytes())?;
                }
                CursorPage::ThreadRoots { room_id, cursor } => {
                    writer.byte(8)?;
                    writer.text(room_id.as_str())?;
                    let checkpoint = cursor
                        .prepare_checkpoint(max_snapshot_bytes)
                        .map_err(|error| map_engine(error, true))?;
                    writer.blob(checkpoint.bytes())?;
                }
                CursorPage::ThreadReplies {
                    room_id,
                    root_message_id,
                    cursor,
                } => {
                    writer.byte(9)?;
                    writer.text(room_id.as_str())?;
                    writer.raw(&root_message_id.into_bytes())?;
                    let checkpoint = cursor
                        .prepare_checkpoint(max_snapshot_bytes)
                        .map_err(|error| map_engine(error, true))?;
                    writer.blob(checkpoint.bytes())?;
                }
                CursorPage::ThreadedChanges { room_id, position } => {
                    writer.byte(10)?;
                    writer.text(room_id.as_str())?;
                    let checkpoint = position
                        .prepare_checkpoint(max_snapshot_bytes)
                        .map_err(|error| map_engine(error, true))?;
                    writer.blob(checkpoint.bytes())?;
                }
                CursorPage::MessageRevisions {
                    room_id,
                    message_id,
                    cursor,
                } => {
                    writer.byte(11)?;
                    writer.text(room_id.as_str())?;
                    writer.raw(&message_id.into_bytes())?;
                    let checkpoint = cursor
                        .prepare_checkpoint(max_snapshot_bytes)
                        .map_err(|error| map_engine(error, true))?;
                    writer.blob(checkpoint.bytes())?;
                }
                CursorPage::Topics { room_id, cursor } => {
                    writer.byte(12)?;
                    writer.text(room_id.as_str())?;
                    let checkpoint = cursor
                        .prepare_checkpoint(max_snapshot_bytes)
                        .map_err(|error| map_engine(error, true))?;
                    writer.blob(checkpoint.bytes())?;
                }
                CursorPage::TopicCollection { room_id, cursor } => {
                    writer.byte(18)?;
                    writer.text(room_id.as_str())?;
                    let checkpoint = cursor
                        .prepare_checkpoint(max_snapshot_bytes)
                        .map_err(|error| map_engine(error, true))?;
                    writer.blob(checkpoint.bytes())?;
                }
                CursorPage::TopicMessages {
                    room_id,
                    topic_id,
                    cursor,
                } => {
                    writer.byte(13)?;
                    writer.text(room_id.as_str())?;
                    writer.raw(&topic_id.into_bytes())?;
                    let checkpoint = cursor
                        .prepare_checkpoint(max_snapshot_bytes)
                        .map_err(|error| map_engine(error, true))?;
                    writer.blob(checkpoint.bytes())?;
                }
                CursorPage::TopicChanges { room_id, position } => {
                    writer.byte(14)?;
                    writer.text(room_id.as_str())?;
                    let checkpoint = position
                        .prepare_checkpoint(max_snapshot_bytes)
                        .map_err(|error| map_engine(error, true))?;
                    writer.blob(checkpoint.bytes())?;
                }
                CursorPage::AttachmentMessages { room_id, cursor } => {
                    writer.byte(15)?;
                    writer.text(room_id.as_str())?;
                    let checkpoint = cursor
                        .prepare_checkpoint(max_snapshot_bytes)
                        .map_err(|error| map_engine(error, true))?;
                    writer.blob(checkpoint.bytes())?;
                }
                CursorPage::AttachmentChanges { room_id, position } => {
                    writer.byte(16)?;
                    writer.text(room_id.as_str())?;
                    let checkpoint = position
                        .prepare_checkpoint(max_snapshot_bytes)
                        .map_err(|error| map_engine(error, true))?;
                    writer.blob(checkpoint.bytes())?;
                }
                CursorPage::VellumArtifactMessages { room_id, cursor } => {
                    writer.byte(17)?;
                    writer.text(room_id.as_str())?;
                    let checkpoint = cursor
                        .prepare_checkpoint(max_snapshot_bytes)
                        .map_err(|error| map_engine(error, true))?;
                    writer.blob(checkpoint.bytes())?;
                }
            }
        }
        Ok(ServiceCheckpointV2(writer.bytes))
    }

    pub fn recover_checkpoint(
        bytes: &[u8],
        expected_scope: ServiceCheckpointScopeV1,
        bounds: ServiceRecoveryLimitsV2,
    ) -> Result<Self, ServiceCheckpointErrorV2> {
        if bytes.len() > bounds.max_snapshot_bytes {
            return Err(ServiceCheckpointErrorV2::Limit);
        }
        let mut reader = Reader { bytes, position: 0 };
        if reader.raw(MAGIC.len())? != MAGIC {
            return Err(ServiceCheckpointErrorV2::Profile);
        }
        let version = reader.byte()?;
        if !matches!(
            version,
            VERSION_V24
                | VERSION_V25
                | VERSION_V26
                | VERSION_V27
                | VERSION_V28
                | VERSION_V29
                | VERSION_V30
                | VERSION_V31
                | VERSION_V32
        ) {
            return Err(ServiceCheckpointErrorV2::Profile);
        }
        if reader.fixed::<16>()? != expected_scope.0 {
            return Err(ServiceCheckpointErrorV2::Scope);
        }
        let limits = read_limits(&mut reader, bounds.maximum_service_limits)?;
        let next_client = reader.number()?;
        let next_cursor = reader.number()?;
        let engine_bytes = reader.blob(bounds.max_engine_snapshot_bytes)?;
        let engine_profile = engine_bytes
            .get(MAGIC.len())
            .copied()
            .ok_or(ServiceCheckpointErrorV2::Engine)?;
        if matches!(version, VERSION_V24 | VERSION_V25) && engine_profile >= 22 {
            return Err(ServiceCheckpointErrorV2::Profile);
        }
        let engine = Engine::recover_checkpoint(
            engine_bytes,
            RecoveryLimits {
                max_snapshot_bytes: bounds.max_engine_snapshot_bytes,
                maximum_engine_limits: bounds.maximum_engine_limits,
            },
        )
        .map_err(|error| map_engine(error, false))?;

        let has_retention_policy = if version == VERSION_V32 {
            match reader.byte()? {
                0 => false,
                1 => true,
                _ => return Err(ServiceCheckpointErrorV2::Invalid),
            }
        } else {
            version == VERSION_V31
        };
        let retention_policy = if has_retention_policy {
            let revision = RetentionPolicyRevisionV1::new(reader.number()?)
                .ok_or(ServiceCheckpointErrorV2::Invalid)?;
            let effective_engine_revision = reader.number()?;
            if effective_engine_revision > engine.revision().0 {
                return Err(ServiceCheckpointErrorV2::Invalid);
            }
            let mut bounds = [RetentionBoundV1::Disabled; 6];
            for bound in &mut bounds {
                *bound = match reader.byte()? {
                    0 => RetentionBoundV1::Disabled,
                    1 => RetentionBoundV1::entries(reader.size(usize::MAX)?)
                        .ok_or(ServiceCheckpointErrorV2::Invalid)?,
                    _ => return Err(ServiceCheckpointErrorV2::Invalid),
                };
            }
            Some(RetentionPolicyV1::new(
                revision,
                effective_engine_revision,
                bounds,
            ))
        } else {
            None
        };
        let (receipt_revision, receipt_revision_exhausted) = if version == VERSION_V32 {
            let revision = reader.number()?;
            let exhausted = match reader.byte()? {
                0 => false,
                1 => true,
                _ => return Err(ServiceCheckpointErrorV2::Invalid),
            };
            if revision == 0 || (exhausted && revision != u64::MAX) {
                return Err(ServiceCheckpointErrorV2::Invalid);
            }
            (revision, exhausted)
        } else {
            (0, false)
        };

        let client_count = reader.size(limits.max_clients)?;
        let mut clients = BTreeMap::new();
        for _ in 0..client_count {
            let client = ClientIdV2(reader.number()?);
            let participant = ParticipantId::new(reader.text(limits.max_participant_id_bytes)?);
            if participant.as_str().is_empty()
                || engine.participant(&participant).is_none()
                || clients
                    .last_key_value()
                    .is_some_and(|(old, _)| old >= &client)
            {
                return Err(ServiceCheckpointErrorV2::NonCanonical);
            }
            clients.insert(client, participant);
        }
        if !valid_counter(next_client, clients.keys().map(|client| client.0)) {
            return Err(ServiceCheckpointErrorV2::Invalid);
        }

        let retired_count = reader.size(limits.max_retired_clients)?;
        let mut retired_clients = VecDeque::new();
        let mut retired_seen = BTreeSet::new();
        for _ in 0..retired_count {
            let client = ClientIdV2(reader.number()?);
            if client.0 >= next_client
                || clients.contains_key(&client)
                || !retired_seen.insert(client)
            {
                return Err(ServiceCheckpointErrorV2::Invalid);
            }
            retired_clients.push_back(client);
        }

        let retained_count = reader.size(limits.max_retained_requests)?;
        let wire_limits = wire_limits(limits, bounds.max_snapshot_bytes);
        let mut retained = BTreeMap::new();
        for _ in 0..retained_count {
            let client = ClientIdV2(reader.number()?);
            let request_id = RequestIdV2(reader.number()?);
            if !clients.contains_key(&client) {
                return Err(ServiceCheckpointErrorV2::Invalid);
            }
            let request =
                match decode_request_v2(reader.blob(bounds.max_snapshot_bytes)?, wire_limits)
                    .map_err(|_| ServiceCheckpointErrorV2::Wire)?
                {
                    WireRequestV2::Execute {
                        client: encoded_client,
                        request,
                    } if encoded_client == client && request.request_id == request_id => request,
                    _ => return Err(ServiceCheckpointErrorV2::Invalid),
                };
            let reply = match decode_reply_v2(reader.blob(bounds.max_snapshot_bytes)?, wire_limits)
                .map_err(|_| ServiceCheckpointErrorV2::Wire)?
            {
                WireReplyV2::Executed(reply) => reply,
                _ => return Err(ServiceCheckpointErrorV2::Invalid),
            };
            if matches!(version, VERSION_V24 | VERSION_V25)
                && (command_uses_v26_vocabulary(&request.command)
                    || reply_uses_v26_vocabulary(&reply))
            {
                return Err(ServiceCheckpointErrorV2::Profile);
            }
            if matches!(
                version,
                VERSION_V24 | VERSION_V25 | VERSION_V26 | VERSION_V27
            ) && (command_uses_topic_collections(&request.command)
                || reply_uses_topic_collections(&reply))
            {
                return Err(ServiceCheckpointErrorV2::Profile);
            }
            let key = (client, request_id);
            if retained
                .last_key_value()
                .is_some_and(|(old, _)| old >= &key)
            {
                return Err(ServiceCheckpointErrorV2::NonCanonical);
            }
            retained.insert(key, RetainedRequestV2 { request, reply });
        }

        let retired_receipts = if matches!(version, VERSION_V30 | VERSION_V31 | VERSION_V32) {
            let count = reader.size(limits.max_retained_requests)?;
            let mut tombstones = Vec::new();
            tombstones
                .try_reserve(count)
                .map_err(|_| ServiceCheckpointErrorV2::Capacity)?;
            for _ in 0..count {
                let policy_revision = RetentionPolicyRevisionV1::new(reader.number()?)
                    .ok_or(ServiceCheckpointErrorV2::Invalid)?;
                let transition_engine_revision = reader.number()?;
                let identity = RetentionIdentityBindingV1::new(reader.fixed::<32>()?)
                    .ok_or(ServiceCheckpointErrorV2::Invalid)?;
                let scope = RetentionScopeBindingV1::new(reader.fixed::<32>()?)
                    .ok_or(ServiceCheckpointErrorV2::Invalid)?;
                let (client, _) = receipt_retirement_key(identity, scope)
                    .ok_or(ServiceCheckpointErrorV2::Invalid)?;
                if client.0 >= next_client || transition_engine_revision > engine.revision().0 {
                    return Err(ServiceCheckpointErrorV2::Invalid);
                }
                tombstones.push(RetentionTombstoneV1::new(
                    RecordClassV1::RequestReceipts,
                    RetentionTombstoneReasonV1::ReceiptRetired,
                    policy_revision,
                    transition_engine_revision,
                    identity,
                    scope,
                ));
            }
            ReceiptRetirementRegistryV1::recover(limits.max_retained_requests.max(1), tombstones)
                .map_err(|_| ServiceCheckpointErrorV2::NonCanonical)?
        } else {
            ReceiptRetirementRegistryV1::new(limits.max_retained_requests.max(1))
                .map_err(|_| ServiceCheckpointErrorV2::Invalid)?
        };
        if retained.keys().any(|&(client, request)| {
            let (identity, scope) = receipt_retirement_bindings(client, request);
            retired_receipts.is_retired(identity, scope)
        }) {
            return Err(ServiceCheckpointErrorV2::Invalid);
        }

        let cursor_count = reader.size(limits.max_retained_cursors)?;
        let cursor_bounds = CursorRecoveryLimitsV1 {
            max_snapshot_bytes: bounds.max_snapshot_bytes,
            max_participant_id_bytes: limits.max_participant_id_bytes,
            max_room_id_bytes: limits.max_room_id_bytes,
            max_topic_name_bytes: limits.max_room_name_bytes,
        };
        let mut cursors = BTreeMap::new();
        for _ in 0..cursor_count {
            let cursor_id = PageCursorV2(reader.number()?);
            let owner = ClientIdV2(reader.number()?);
            let Some(participant) = clients.get(&owner) else {
                return Err(ServiceCheckpointErrorV2::Invalid);
            };
            let page = match reader.byte()? {
                1 => {
                    let cursor = RoomPageCursor::recover_checkpoint(
                        reader.blob(bounds.max_snapshot_bytes)?,
                        cursor_bounds,
                    )
                    .map_err(|error| map_engine(error, true))?;
                    if !cursor.is_bound_to(participant) {
                        return Err(ServiceCheckpointErrorV2::Invalid);
                    }
                    CursorPage::Rooms(cursor)
                }
                2 => {
                    let room_id = RoomId::new(reader.text(limits.max_room_id_bytes)?);
                    let cursor = MemberPageCursor::recover_checkpoint(
                        reader.blob(bounds.max_snapshot_bytes)?,
                        cursor_bounds,
                    )
                    .map_err(|error| map_engine(error, true))?;
                    if room_id.as_str().is_empty() || !cursor.is_bound_to(participant, &room_id) {
                        return Err(ServiceCheckpointErrorV2::Invalid);
                    }
                    CursorPage::Members { room_id, cursor }
                }
                3 => {
                    let room_id = RoomId::new(reader.text(limits.max_room_id_bytes)?);
                    let cursor = MessagePageCursor::recover_checkpoint(
                        reader.blob(bounds.max_snapshot_bytes)?,
                        cursor_bounds,
                    )
                    .map_err(|error| map_engine(error, true))?;
                    if room_id.as_str().is_empty() || !cursor.is_bound_to(participant, &room_id) {
                        return Err(ServiceCheckpointErrorV2::Invalid);
                    }
                    CursorPage::Messages { room_id, cursor }
                }
                4 => {
                    let room_id = RoomId::new(reader.text(limits.max_room_id_bytes)?);
                    let position = RoomChangePosition::recover_checkpoint(
                        reader.blob(bounds.max_snapshot_bytes)?,
                        cursor_bounds,
                    )
                    .map_err(|error| map_engine(error, true))?;
                    if room_id.as_str().is_empty()
                        || !position.is_bound_to(participant, &room_id)
                        || position.revision() > engine.revision()
                    {
                        return Err(ServiceCheckpointErrorV2::Invalid);
                    }
                    CursorPage::Changes { room_id, position }
                }
                5 => {
                    let room_id = RoomId::new(reader.text(limits.max_room_id_bytes)?);
                    let cursor = BanPageCursor::recover_checkpoint(
                        reader.blob(bounds.max_snapshot_bytes)?,
                        cursor_bounds,
                    )
                    .map_err(|error| map_engine(error, true))?;
                    if room_id.as_str().is_empty() || !cursor.is_bound_to(participant, &room_id) {
                        return Err(ServiceCheckpointErrorV2::Invalid);
                    }
                    CursorPage::Bans { room_id, cursor }
                }
                6 => {
                    let room_id = RoomId::new(reader.text(limits.max_room_id_bytes)?);
                    let cursor = MessagePageCursor::recover_checkpoint(
                        reader.blob(bounds.max_snapshot_bytes)?,
                        cursor_bounds,
                    )
                    .map_err(|error| map_engine(error, true))?;
                    if room_id.as_str().is_empty() || !cursor.is_bound_to(participant, &room_id) {
                        return Err(ServiceCheckpointErrorV2::Invalid);
                    }
                    CursorPage::IdentifiedMessages { room_id, cursor }
                }
                7 => {
                    let room_id = RoomId::new(reader.text(limits.max_room_id_bytes)?);
                    let position = RoomChangePosition::recover_checkpoint(
                        reader.blob(bounds.max_snapshot_bytes)?,
                        cursor_bounds,
                    )
                    .map_err(|error| map_engine(error, true))?;
                    if room_id.as_str().is_empty()
                        || !position.is_bound_to(participant, &room_id)
                        || position.revision() > engine.revision()
                    {
                        return Err(ServiceCheckpointErrorV2::Invalid);
                    }
                    CursorPage::IdentifiedChanges { room_id, position }
                }
                8 => {
                    let room_id = RoomId::new(reader.text(limits.max_room_id_bytes)?);
                    let cursor = ThreadMessagePageCursor::recover_checkpoint(
                        reader.blob(bounds.max_snapshot_bytes)?,
                        cursor_bounds,
                    )
                    .map_err(|error| map_engine(error, true))?;
                    if room_id.as_str().is_empty()
                        || !cursor.is_bound_to_roots(participant, &room_id)
                    {
                        return Err(ServiceCheckpointErrorV2::Invalid);
                    }
                    CursorPage::ThreadRoots { room_id, cursor }
                }
                9 => {
                    let room_id = RoomId::new(reader.text(limits.max_room_id_bytes)?);
                    let root_message_id = MessageId::from_bytes(reader.fixed()?);
                    let cursor = ThreadMessagePageCursor::recover_checkpoint(
                        reader.blob(bounds.max_snapshot_bytes)?,
                        cursor_bounds,
                    )
                    .map_err(|error| map_engine(error, true))?;
                    if room_id.as_str().is_empty()
                        || !cursor.is_bound_to_replies(participant, &room_id, root_message_id)
                    {
                        return Err(ServiceCheckpointErrorV2::Invalid);
                    }
                    CursorPage::ThreadReplies {
                        room_id,
                        root_message_id,
                        cursor,
                    }
                }
                10 => {
                    let room_id = RoomId::new(reader.text(limits.max_room_id_bytes)?);
                    let position = RoomChangePosition::recover_checkpoint(
                        reader.blob(bounds.max_snapshot_bytes)?,
                        cursor_bounds,
                    )
                    .map_err(|error| map_engine(error, true))?;
                    if room_id.as_str().is_empty()
                        || !position.is_bound_to(participant, &room_id)
                        || position.revision() > engine.revision()
                    {
                        return Err(ServiceCheckpointErrorV2::Invalid);
                    }
                    CursorPage::ThreadedChanges { room_id, position }
                }
                11 => {
                    let room_id = RoomId::new(reader.text(limits.max_room_id_bytes)?);
                    let message_id = MessageId::from_bytes(reader.fixed()?);
                    let cursor = MessageRevisionPageCursorV1::recover_checkpoint(
                        reader.blob(bounds.max_snapshot_bytes)?,
                        cursor_bounds,
                    )
                    .map_err(|error| map_engine(error, true))?;
                    if room_id.as_str().is_empty()
                        || !cursor.is_bound_to(participant, &room_id, message_id)
                    {
                        return Err(ServiceCheckpointErrorV2::Invalid);
                    }
                    CursorPage::MessageRevisions {
                        room_id,
                        message_id,
                        cursor,
                    }
                }
                12 => {
                    let room_id = RoomId::new(reader.text(limits.max_room_id_bytes)?);
                    let cursor = TopicPageCursorV1::recover_checkpoint(
                        reader.blob(bounds.max_snapshot_bytes)?,
                        cursor_bounds,
                    )
                    .map_err(|error| map_engine(error, true))?;
                    if room_id.as_str().is_empty() || !cursor.is_bound_to(participant, &room_id) {
                        return Err(ServiceCheckpointErrorV2::Invalid);
                    }
                    CursorPage::Topics { room_id, cursor }
                }
                13 => {
                    let room_id = RoomId::new(reader.text(limits.max_room_id_bytes)?);
                    let topic_id = TopicId::from_bytes(reader.fixed()?);
                    let cursor = TopicMessagePageCursorV1::recover_checkpoint(
                        reader.blob(bounds.max_snapshot_bytes)?,
                        cursor_bounds,
                    )
                    .map_err(|error| map_engine(error, true))?;
                    if room_id.as_str().is_empty()
                        || !cursor.is_bound_to(participant, &room_id, topic_id)
                    {
                        return Err(ServiceCheckpointErrorV2::Invalid);
                    }
                    CursorPage::TopicMessages {
                        room_id,
                        topic_id,
                        cursor,
                    }
                }
                14 => {
                    let room_id = RoomId::new(reader.text(limits.max_room_id_bytes)?);
                    let position = RoomChangePosition::recover_checkpoint(
                        reader.blob(bounds.max_snapshot_bytes)?,
                        cursor_bounds,
                    )
                    .map_err(|error| map_engine(error, true))?;
                    if room_id.as_str().is_empty()
                        || !position.is_bound_to(participant, &room_id)
                        || position.revision() > engine.revision()
                    {
                        return Err(ServiceCheckpointErrorV2::Invalid);
                    }
                    CursorPage::TopicChanges { room_id, position }
                }
                15 => {
                    let room_id = RoomId::new(reader.text(limits.max_room_id_bytes)?);
                    let cursor = MessagePageCursor::recover_checkpoint(
                        reader.blob(bounds.max_snapshot_bytes)?,
                        cursor_bounds,
                    )
                    .map_err(|error| map_engine(error, true))?;
                    if room_id.as_str().is_empty() || !cursor.is_bound_to(participant, &room_id) {
                        return Err(ServiceCheckpointErrorV2::Invalid);
                    }
                    CursorPage::AttachmentMessages { room_id, cursor }
                }
                16 => {
                    let room_id = RoomId::new(reader.text(limits.max_room_id_bytes)?);
                    let position = RoomChangePosition::recover_checkpoint(
                        reader.blob(bounds.max_snapshot_bytes)?,
                        cursor_bounds,
                    )
                    .map_err(|error| map_engine(error, true))?;
                    if room_id.as_str().is_empty()
                        || !position.is_bound_to(participant, &room_id)
                        || position.revision() > engine.revision()
                    {
                        return Err(ServiceCheckpointErrorV2::Invalid);
                    }
                    CursorPage::AttachmentChanges { room_id, position }
                }
                17 if matches!(
                    version,
                    VERSION_V25
                        | VERSION_V27
                        | VERSION_V29
                        | VERSION_V30
                        | VERSION_V31
                        | VERSION_V32
                ) =>
                {
                    let room_id = RoomId::new(reader.text(limits.max_room_id_bytes)?);
                    let cursor = MessagePageCursor::recover_checkpoint(
                        reader.blob(bounds.max_snapshot_bytes)?,
                        cursor_bounds,
                    )
                    .map_err(|error| map_engine(error, true))?;
                    if room_id.as_str().is_empty() || !cursor.is_bound_to(participant, &room_id) {
                        return Err(ServiceCheckpointErrorV2::Invalid);
                    }
                    CursorPage::VellumArtifactMessages { room_id, cursor }
                }
                18 if matches!(
                    version,
                    VERSION_V28 | VERSION_V29 | VERSION_V30 | VERSION_V31 | VERSION_V32
                ) =>
                {
                    let room_id = RoomId::new(reader.text(limits.max_room_id_bytes)?);
                    let cursor = TopicCollectionCursorV1::recover_checkpoint(
                        reader.blob(bounds.max_snapshot_bytes)?,
                        cursor_bounds,
                    )
                    .map_err(|error| map_engine(error, true))?;
                    if room_id.as_str().is_empty()
                        || !cursor.is_bound_to_observer_room(participant, &room_id)
                    {
                        return Err(ServiceCheckpointErrorV2::Invalid);
                    }
                    CursorPage::TopicCollection { room_id, cursor }
                }
                _ => return Err(ServiceCheckpointErrorV2::Invalid),
            };
            if cursor_id.0 >= next_cursor
                || cursors
                    .last_key_value()
                    .is_some_and(|(old, _)| old >= &cursor_id)
            {
                return Err(ServiceCheckpointErrorV2::NonCanonical);
            }
            cursors.insert(cursor_id, CursorRecord { owner, page });
        }
        if reader.position != bytes.len() {
            return Err(ServiceCheckpointErrorV2::Trailing);
        }

        let service = Self {
            engine,
            limits,
            clients,
            retired_clients,
            retained,
            receipt_revision,
            receipt_revision_exhausted,
            retention_policy,
            retired_receipts,
            cursors,
            next_client,
            next_cursor,
            // Operational evidence is process-local and deliberately does not
            // alter the semantic durable record or force replay checkpoints.
            usage: UsageV2::default(),
        };
        if service
            .retained
            .values()
            .any(|retained| !service.valid_command_shape(&retained.request.command))
        {
            return Err(ServiceCheckpointErrorV2::Invalid);
        }
        let canonical = service.prepare_checkpoint(expected_scope, bounds.max_snapshot_bytes)?;
        if matches!(version, VERSION_V24 | VERSION_V25)
            && canonical.bytes()[MAGIC.len()] != version + 2
        {
            return Err(ServiceCheckpointErrorV2::Profile);
        }
        let canonical_matches = if matches!(version, VERSION_V24 | VERSION_V25) {
            let mut legacy = canonical.bytes().to_vec();
            legacy[MAGIC.len()] = version;
            legacy == bytes
        } else {
            canonical.bytes() == bytes
        };
        if !canonical_matches {
            return Err(ServiceCheckpointErrorV2::NonCanonical);
        }
        Ok(service)
    }

    /// Replaces semantic state from a canonical checkpoint while retaining this
    /// process owner's passive operational counters.
    ///
    /// This is safe for a store result that proves no commit occurred. It must
    /// not be used to guess the result of an indeterminate commit.
    pub fn restore_checkpoint_preserving_usage(
        &mut self,
        bytes: &[u8],
        expected_scope: ServiceCheckpointScopeV1,
        bounds: ServiceRecoveryLimitsV2,
    ) -> Result<(), ServiceCheckpointErrorV2> {
        let usage = self.usage;
        let mut recovered = Self::recover_checkpoint(bytes, expected_scope, bounds)?;
        recovered.usage = usage;
        *self = recovered;
        Ok(())
    }
}
