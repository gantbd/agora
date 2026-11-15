//! Owner-native administrative projections of Service V2 records.
//!
//! One profile projects the canonical complete service checkpoint as one
//! `OperationalCopies` record. It deliberately does not reinterpret checkpoint
//! internals as room, change, receipt, private, or federation records. The
//! result is suitable only for the administrative export profile and remains
//! subject to separate Host authority and custody admission.
//!
//! The receipt profile separately projects one canonical generation containing
//! only live exact request/reply receipts and receipt-retirement tombstones. It
//! is bound to the Service-owned receipt revision and never relabels the full
//! Service checkpoint.

use super::ServiceV2;
use super::persistence::{ServiceCheckpointErrorV2, ServiceCheckpointScopeV1};
use agora_engine::retention::{
    ContentDigestProfileV1, ContentDigestV1, ExportMaterializationRefusalV1,
    ExportMaterializationV1, ExportProfileV1, ExportRecordV1, ExportRequestIdV1,
    ExportScopeBindingV1, ExportSourceFragmentV1, RecordClassV1, RetentionFrontierV1,
    RetentionIdentityBindingV1, RetentionPolicyRevisionV1, RetentionRecordIdV1,
    RetentionScopeBindingV1, RetentionTombstoneReasonV1, RetentionTombstoneV1,
    materialize_export_sources_v1,
};
use agora_protocol::v2::{ClientIdV2, RequestIdV2};
use agora_wire::{
    WireLimits,
    v2::{
        WireReplyV2, WireRequestV2, decode_reply_v2, decode_request_v2, encode_reply_v2,
        encode_request_v2,
    },
};
use std::fmt;

const REQUEST_RECEIPT_RECORD_MAGIC_V1: &[u8; 8] = b"AGARRC1\0";
const REQUEST_RECEIPT_RECORD_VERSION_V1: u8 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdministrativeCheckpointSourceRefusalV1 {
    InvalidLimit,
    NoCurrentPolicy,
    OperationalCopiesDisabled,
    OrdinalExhausted,
    Checkpoint(ServiceCheckpointErrorV2),
    RecordLimit,
}

/// One immutable, bounded recovery-image source captured from a borrowed
/// authoritative service owner.
#[derive(Clone, Eq, PartialEq)]
pub struct AdministrativeCheckpointSourceV1 {
    fragment: ExportSourceFragmentV1,
    max_record_bytes: usize,
}

impl AdministrativeCheckpointSourceV1 {
    pub fn capture(
        service: &ServiceV2,
        checkpoint_scope: ServiceCheckpointScopeV1,
        max_snapshot_bytes: usize,
        max_record_bytes: usize,
    ) -> Result<Self, AdministrativeCheckpointSourceRefusalV1> {
        if max_snapshot_bytes == 0 || max_record_bytes == 0 {
            return Err(AdministrativeCheckpointSourceRefusalV1::InvalidLimit);
        }
        let policy = service
            .retention_policy()
            .ok_or(AdministrativeCheckpointSourceRefusalV1::NoCurrentPolicy)?;
        if !policy
            .bound(RecordClassV1::OperationalCopies)
            .retains_records()
        {
            return Err(AdministrativeCheckpointSourceRefusalV1::OperationalCopiesDisabled);
        }
        let source_engine_revision = service.engine().revision().0;
        let ordinal = source_engine_revision
            .checked_add(1)
            .ok_or(AdministrativeCheckpointSourceRefusalV1::OrdinalExhausted)?;
        let checkpoint = service
            .prepare_checkpoint(checkpoint_scope, max_snapshot_bytes)
            .map_err(AdministrativeCheckpointSourceRefusalV1::Checkpoint)?;
        let id = RetentionRecordIdV1::new(RecordClassV1::OperationalCopies, ordinal)
            .expect("revision successor is nonzero");
        let record = ExportRecordV1::new(id, checkpoint.bytes().to_vec(), max_record_bytes)
            .ok_or(AdministrativeCheckpointSourceRefusalV1::RecordLimit)?;
        let frontier =
            RetentionFrontierV1::new(RecordClassV1::OperationalCopies, ordinal - 1, Some(ordinal))
                .expect("the captured operational copy has an exact frontier");
        let fragment = ExportSourceFragmentV1::new(
            source_engine_revision,
            policy.revision,
            RecordClassV1::OperationalCopies,
            frontier,
            vec![record],
            1,
            max_record_bytes,
        )
        .map_err(|_| AdministrativeCheckpointSourceRefusalV1::RecordLimit)?;
        Ok(Self {
            fragment,
            max_record_bytes,
        })
    }

    pub const fn source_engine_revision(&self) -> u64 {
        self.fragment.source_engine_revision()
    }

    pub const fn policy_revision(&self) -> RetentionPolicyRevisionV1 {
        self.fragment.policy_revision()
    }

    pub fn record_id(&self) -> RetentionRecordIdV1 {
        self.fragment.records()[0].id
    }

    pub fn checkpoint_byte_length(&self) -> usize {
        self.fragment.records()[0].payload().len()
    }

    /// Materializes this exact recovery image under the administrative profile.
    /// Digest qualification and later authority/custody remain external.
    pub fn materialize<D>(
        &self,
        request: ExportRequestIdV1,
        scope: ExportScopeBindingV1,
        max_chunks: usize,
        max_chunk_bytes: usize,
        digest: D,
    ) -> Result<ExportMaterializationV1, ExportMaterializationRefusalV1>
    where
        D: FnMut(&[u8]) -> ContentDigestV1,
    {
        materialize_export_sources_v1(
            request,
            ExportProfileV1::Administrative,
            scope,
            ContentDigestProfileV1::Sha256,
            std::slice::from_ref(&self.fragment),
            1,
            self.max_record_bytes,
            max_chunks,
            max_chunk_bytes,
            digest,
        )
    }
}

impl fmt::Debug for AdministrativeCheckpointSourceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdministrativeCheckpointSourceV1")
            .field(
                "source_engine_revision",
                &self.fragment.source_engine_revision(),
            )
            .field("policy_revision", &self.fragment.policy_revision())
            .field("record_id", &self.fragment.records()[0].id)
            .field(
                "checkpoint_byte_length",
                &self.fragment.records()[0].payload().len(),
            )
            .field("checkpoint", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestReceiptSourceRefusalV1 {
    InvalidLimit,
    NoCurrentPolicy,
    RequestReceiptsDisabled,
    RevisionUnavailable,
    RevisionExhausted,
    Wire,
    RecordLimit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestReceiptRecordErrorV1 {
    InvalidLimit,
    Limit,
    Truncated,
    Profile,
    Invalid,
    Wire,
    NonCanonical,
    Trailing,
}

#[derive(Clone, Eq, PartialEq)]
struct LiveReceiptRecordV1 {
    client: ClientIdV2,
    request: RequestIdV2,
    request_bytes: Vec<u8>,
    reply_bytes: Vec<u8>,
}

/// Canonically recovered owner-native receipt snapshot.
#[derive(Clone, Eq, PartialEq)]
pub struct RequestReceiptRecordV1 {
    revision: u64,
    live: Vec<LiveReceiptRecordV1>,
    retired: Vec<RetentionTombstoneV1>,
}

impl RequestReceiptRecordV1 {
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub fn live_receipt_count(&self) -> usize {
        self.live.len()
    }

    pub fn retired_receipt_count(&self) -> usize {
        self.retired.len()
    }

    pub fn canonical_bytes(
        &self,
        max_record_bytes: usize,
    ) -> Result<Vec<u8>, RequestReceiptRecordErrorV1> {
        if max_record_bytes == 0 {
            return Err(RequestReceiptRecordErrorV1::InvalidLimit);
        }
        encode_recovered_request_receipt_record_v1(self, max_record_bytes)
            .map_err(|_| RequestReceiptRecordErrorV1::Limit)
    }
}

impl fmt::Debug for RequestReceiptRecordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RequestReceiptRecordV1")
            .field("revision", &self.revision)
            .field("live_receipt_count", &self.live.len())
            .field("retired_receipt_count", &self.retired.len())
            .field("receipts", &"<redacted>")
            .finish()
    }
}

/// One immutable Service-owned receipt generation. Earlier generations are
/// superseded snapshots, not evidence that external copies were erased.
#[derive(Clone, Eq, PartialEq)]
pub struct RequestReceiptSourceV1 {
    fragment: ExportSourceFragmentV1,
    max_record_bytes: usize,
}

impl RequestReceiptSourceV1 {
    pub fn capture(
        service: &ServiceV2,
        max_record_bytes: usize,
    ) -> Result<Self, RequestReceiptSourceRefusalV1> {
        if max_record_bytes == 0 {
            return Err(RequestReceiptSourceRefusalV1::InvalidLimit);
        }
        let policy = service
            .retention_policy()
            .ok_or(RequestReceiptSourceRefusalV1::NoCurrentPolicy)?;
        if !policy
            .bound(RecordClassV1::RequestReceipts)
            .retains_records()
        {
            return Err(RequestReceiptSourceRefusalV1::RequestReceiptsDisabled);
        }
        if service.receipt_revision_exhausted() {
            return Err(RequestReceiptSourceRefusalV1::RevisionExhausted);
        }
        let revision = service
            .receipt_revision()
            .ok_or(RequestReceiptSourceRefusalV1::RevisionUnavailable)?;
        let record = receipt_record_from_service(service, revision, max_record_bytes)?;
        let id = RetentionRecordIdV1::new(RecordClassV1::RequestReceipts, revision)
            .expect("receipt revisions are nonzero");
        let record = ExportRecordV1::new(id, record, max_record_bytes)
            .ok_or(RequestReceiptSourceRefusalV1::RecordLimit)?;
        let frontier =
            RetentionFrontierV1::new(RecordClassV1::RequestReceipts, revision - 1, Some(revision))
                .expect("receipt snapshot revision has an exact frontier");
        let fragment = ExportSourceFragmentV1::new(
            service.engine().revision().0,
            policy.revision,
            RecordClassV1::RequestReceipts,
            frontier,
            vec![record],
            1,
            max_record_bytes,
        )
        .map_err(|_| RequestReceiptSourceRefusalV1::RecordLimit)?;
        Ok(Self {
            fragment,
            max_record_bytes,
        })
    }

    pub fn fragment(&self) -> &ExportSourceFragmentV1 {
        &self.fragment
    }

    pub const fn source_engine_revision(&self) -> u64 {
        self.fragment.source_engine_revision()
    }

    pub const fn policy_revision(&self) -> RetentionPolicyRevisionV1 {
        self.fragment.policy_revision()
    }

    pub fn receipt_revision(&self) -> u64 {
        self.fragment.records()[0].id.ordinal
    }

    pub fn record_payload(&self) -> &[u8] {
        self.fragment.records()[0].payload()
    }

    pub fn materialize<D>(
        &self,
        request: ExportRequestIdV1,
        scope: ExportScopeBindingV1,
        max_chunks: usize,
        max_chunk_bytes: usize,
        digest: D,
    ) -> Result<ExportMaterializationV1, ExportMaterializationRefusalV1>
    where
        D: FnMut(&[u8]) -> ContentDigestV1,
    {
        materialize_export_sources_v1(
            request,
            ExportProfileV1::Administrative,
            scope,
            ContentDigestProfileV1::Sha256,
            std::slice::from_ref(&self.fragment),
            1,
            self.max_record_bytes,
            max_chunks,
            max_chunk_bytes,
            digest,
        )
    }
}

impl fmt::Debug for RequestReceiptSourceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RequestReceiptSourceV1")
            .field("source_engine_revision", &self.source_engine_revision())
            .field("policy_revision", &self.policy_revision())
            .field("receipt_revision", &self.receipt_revision())
            .field("record_byte_length", &self.record_payload().len())
            .field("record", &"<redacted>")
            .finish()
    }
}

fn receipt_wire_limits(limits: super::ServiceLimitsV2, max_record_bytes: usize) -> WireLimits {
    WireLimits {
        max_frame_bytes: max_record_bytes,
        max_text_bytes: limits
            .max_message_body_bytes
            .max(limits.max_room_name_bytes)
            .max(limits.max_room_id_bytes)
            .max(limits.max_participant_id_bytes)
            .max(limits.max_moderation_reason_bytes),
        max_fields: max_record_bytes,
        max_list_items: limits
            .max_page_items
            .try_into()
            .unwrap_or(usize::MAX)
            .max(limits.max_addressed_participants),
    }
}

struct RecordWriterV1 {
    bytes: Vec<u8>,
    maximum: usize,
}

impl RecordWriterV1 {
    fn raw(&mut self, value: &[u8]) -> Result<(), RequestReceiptSourceRefusalV1> {
        if self
            .bytes
            .len()
            .checked_add(value.len())
            .is_none_or(|length| length > self.maximum)
        {
            return Err(RequestReceiptSourceRefusalV1::RecordLimit);
        }
        self.bytes
            .try_reserve(value.len())
            .map_err(|_| RequestReceiptSourceRefusalV1::RecordLimit)?;
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn byte(&mut self, value: u8) -> Result<(), RequestReceiptSourceRefusalV1> {
        self.raw(&[value])
    }

    fn number(&mut self, value: u64) -> Result<(), RequestReceiptSourceRefusalV1> {
        self.raw(&value.to_be_bytes())
    }

    fn size(&mut self, value: usize) -> Result<(), RequestReceiptSourceRefusalV1> {
        self.number(
            value
                .try_into()
                .map_err(|_| RequestReceiptSourceRefusalV1::RecordLimit)?,
        )
    }

    fn blob(&mut self, value: &[u8]) -> Result<(), RequestReceiptSourceRefusalV1> {
        self.size(value.len())?;
        self.raw(value)
    }
}

fn receipt_record_from_service(
    service: &ServiceV2,
    revision: u64,
    max_record_bytes: usize,
) -> Result<Vec<u8>, RequestReceiptSourceRefusalV1> {
    let wire_limits = receipt_wire_limits(service.limits(), max_record_bytes);
    let mut writer = RecordWriterV1 {
        bytes: Vec::new(),
        maximum: max_record_bytes,
    };
    writer.raw(REQUEST_RECEIPT_RECORD_MAGIC_V1)?;
    writer.byte(REQUEST_RECEIPT_RECORD_VERSION_V1)?;
    writer.number(revision)?;
    writer.size(service.retained.len())?;
    for ((client, request_id), retained) in &service.retained {
        let request = encode_request_v2(
            &WireRequestV2::Execute {
                client: *client,
                request: retained.request.clone(),
            },
            wire_limits,
        )
        .map_err(|_| RequestReceiptSourceRefusalV1::Wire)?;
        let reply = encode_reply_v2(&WireReplyV2::Executed(retained.reply.clone()), wire_limits)
            .map_err(|_| RequestReceiptSourceRefusalV1::Wire)?;
        writer.number(client.0)?;
        writer.number(request_id.0)?;
        writer.blob(&request)?;
        writer.blob(&reply)?;
    }
    let retired = service.retired_receipts.checkpoint();
    writer.size(retired.len())?;
    for tombstone in retired {
        writer.byte(tombstone.class as u8)?;
        writer.byte(tombstone.reason as u8)?;
        writer.number(tombstone.policy_revision.get())?;
        writer.number(tombstone.transition_engine_revision)?;
        writer.raw(&tombstone.identity_binding().as_bytes())?;
        writer.raw(&tombstone.scope_binding().as_bytes())?;
    }
    Ok(writer.bytes)
}

struct RecordReaderV1<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> RecordReaderV1<'a> {
    fn raw(&mut self, length: usize) -> Result<&'a [u8], RequestReceiptRecordErrorV1> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(RequestReceiptRecordErrorV1::Truncated)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(RequestReceiptRecordErrorV1::Truncated)?;
        self.position = end;
        Ok(value)
    }

    fn byte(&mut self) -> Result<u8, RequestReceiptRecordErrorV1> {
        Ok(self.raw(1)?[0])
    }

    fn number(&mut self) -> Result<u64, RequestReceiptRecordErrorV1> {
        Ok(u64::from_be_bytes(
            self.raw(8)?
                .try_into()
                .map_err(|_| RequestReceiptRecordErrorV1::Truncated)?,
        ))
    }

    fn size(&mut self, maximum: usize) -> Result<usize, RequestReceiptRecordErrorV1> {
        let value =
            usize::try_from(self.number()?).map_err(|_| RequestReceiptRecordErrorV1::Limit)?;
        (value <= maximum)
            .then_some(value)
            .ok_or(RequestReceiptRecordErrorV1::Limit)
    }

    fn blob(&mut self, maximum: usize) -> Result<&'a [u8], RequestReceiptRecordErrorV1> {
        let length = self.size(maximum)?;
        self.raw(length)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], RequestReceiptRecordErrorV1> {
        self.raw(N)?
            .try_into()
            .map_err(|_| RequestReceiptRecordErrorV1::Truncated)
    }
}

pub fn recover_request_receipt_record_v1(
    bytes: &[u8],
    limits: super::ServiceLimitsV2,
    max_record_bytes: usize,
) -> Result<RequestReceiptRecordV1, RequestReceiptRecordErrorV1> {
    if max_record_bytes == 0 {
        return Err(RequestReceiptRecordErrorV1::InvalidLimit);
    }
    if bytes.len() > max_record_bytes {
        return Err(RequestReceiptRecordErrorV1::Limit);
    }
    let wire_limits = receipt_wire_limits(limits, max_record_bytes);
    let mut reader = RecordReaderV1 { bytes, position: 0 };
    if reader.raw(REQUEST_RECEIPT_RECORD_MAGIC_V1.len())? != REQUEST_RECEIPT_RECORD_MAGIC_V1
        || reader.byte()? != REQUEST_RECEIPT_RECORD_VERSION_V1
    {
        return Err(RequestReceiptRecordErrorV1::Profile);
    }
    let revision = reader.number()?;
    if revision == 0 {
        return Err(RequestReceiptRecordErrorV1::Invalid);
    }
    let live_count = reader.size(limits.max_retained_requests)?;
    let mut live = Vec::new();
    live.try_reserve(live_count)
        .map_err(|_| RequestReceiptRecordErrorV1::Limit)?;
    let mut previous = None;
    for _ in 0..live_count {
        let client = ClientIdV2(reader.number()?);
        let request_id = RequestIdV2(reader.number()?);
        let key = (client, request_id);
        if previous.is_some_and(|prior| prior >= key) {
            return Err(RequestReceiptRecordErrorV1::NonCanonical);
        }
        let request_bytes = reader.blob(max_record_bytes)?.to_vec();
        let reply_bytes = reader.blob(max_record_bytes)?.to_vec();
        let request = decode_request_v2(&request_bytes, wire_limits)
            .map_err(|_| RequestReceiptRecordErrorV1::Wire)?;
        let reply = decode_reply_v2(&reply_bytes, wire_limits)
            .map_err(|_| RequestReceiptRecordErrorV1::Wire)?;
        let WireRequestV2::Execute {
            client: decoded_client,
            request: decoded_request,
        } = &request
        else {
            return Err(RequestReceiptRecordErrorV1::Invalid);
        };
        if *decoded_client != client
            || decoded_request.request_id != request_id
            || !matches!(reply, WireReplyV2::Executed(_))
            || encode_request_v2(&request, wire_limits).ok().as_deref() != Some(&request_bytes)
            || encode_reply_v2(&reply, wire_limits).ok().as_deref() != Some(&reply_bytes)
        {
            return Err(RequestReceiptRecordErrorV1::NonCanonical);
        }
        live.push(LiveReceiptRecordV1 {
            client,
            request: request_id,
            request_bytes,
            reply_bytes,
        });
        previous = Some(key);
    }
    let retired_count = reader.size(limits.max_retained_requests)?;
    let mut retired = Vec::new();
    retired
        .try_reserve(retired_count)
        .map_err(|_| RequestReceiptRecordErrorV1::Limit)?;
    let mut previous_binding = None;
    for _ in 0..retired_count {
        if reader.byte()? != RecordClassV1::RequestReceipts as u8
            || reader.byte()? != RetentionTombstoneReasonV1::ReceiptRetired as u8
        {
            return Err(RequestReceiptRecordErrorV1::Invalid);
        }
        let policy_revision = RetentionPolicyRevisionV1::new(reader.number()?)
            .ok_or(RequestReceiptRecordErrorV1::Invalid)?;
        let transition_engine_revision = reader.number()?;
        let identity = RetentionIdentityBindingV1::new(reader.fixed()?)
            .ok_or(RequestReceiptRecordErrorV1::Invalid)?;
        let scope = RetentionScopeBindingV1::new(reader.fixed()?)
            .ok_or(RequestReceiptRecordErrorV1::Invalid)?;
        let binding = (identity, scope);
        if previous_binding.is_some_and(|prior| prior >= binding) {
            return Err(RequestReceiptRecordErrorV1::NonCanonical);
        }
        retired.push(RetentionTombstoneV1::new(
            RecordClassV1::RequestReceipts,
            RetentionTombstoneReasonV1::ReceiptRetired,
            policy_revision,
            transition_engine_revision,
            identity,
            scope,
        ));
        previous_binding = Some(binding);
    }
    if reader.position != bytes.len() {
        return Err(RequestReceiptRecordErrorV1::Trailing);
    }
    let recovered = RequestReceiptRecordV1 {
        revision,
        live,
        retired,
    };
    let canonical = encode_recovered_request_receipt_record_v1(&recovered, max_record_bytes)
        .map_err(|_| RequestReceiptRecordErrorV1::Limit)?;
    if canonical != bytes {
        return Err(RequestReceiptRecordErrorV1::NonCanonical);
    }
    Ok(recovered)
}

fn encode_recovered_request_receipt_record_v1(
    record: &RequestReceiptRecordV1,
    max_record_bytes: usize,
) -> Result<Vec<u8>, RequestReceiptSourceRefusalV1> {
    let mut writer = RecordWriterV1 {
        bytes: Vec::new(),
        maximum: max_record_bytes,
    };
    writer.raw(REQUEST_RECEIPT_RECORD_MAGIC_V1)?;
    writer.byte(REQUEST_RECEIPT_RECORD_VERSION_V1)?;
    writer.number(record.revision)?;
    writer.size(record.live.len())?;
    for receipt in &record.live {
        writer.number(receipt.client.0)?;
        writer.number(receipt.request.0)?;
        writer.blob(&receipt.request_bytes)?;
        writer.blob(&receipt.reply_bytes)?;
    }
    writer.size(record.retired.len())?;
    for tombstone in &record.retired {
        writer.byte(tombstone.class as u8)?;
        writer.byte(tombstone.reason as u8)?;
        writer.number(tombstone.policy_revision.get())?;
        writer.number(tombstone.transition_engine_revision)?;
        writer.raw(&tombstone.identity_binding().as_bytes())?;
        writer.raw(&tombstone.scope_binding().as_bytes())?;
    }
    Ok(writer.bytes)
}
