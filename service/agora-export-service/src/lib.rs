//! Independent bounded custody for already materialized Agora exports.
//!
//! This owner does not project conversation state, authorize an exporter,
//! implement a digest algorithm, or mutate Agora. It admits only a complete
//! materialization bound to the caller-supplied current engine and policy
//! revisions, retains exact replay/conflict state, and owns a separate canonical
//! checkpoint. The enclosing trusted host must supply the named digest
//! implementation and durable publication.

use agora_engine::retention::{
    ContentDigestProfileV1, ContentDigestV1, ExportChunkPayloadV1, ExportChunkV1, ExportManifestV1,
    ExportMaterializationApplyV1, ExportMaterializationRefusalV1, ExportMaterializationV1,
    ExportProfileV1, ExportRegistryRefusalV1, ExportRegistryV1, ExportRequestIdV1,
    ExportRetirementV1, ExportScopeBindingV1, RECORD_CLASSES_V1, RetentionFrontierV1,
    RetentionPolicyRevisionV1,
};
use std::collections::BTreeMap;
use std::fmt;

const MAGIC: &[u8; 8] = b"AGEXPSV\0";
const VERSION_V1: u8 = 1;
const VERSION_V2: u8 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExportServiceLimitsV1 {
    pub max_exports: usize,
    pub max_retired_exports: usize,
    pub max_chunks_per_export: usize,
    pub max_chunk_bytes: usize,
}

impl Default for ExportServiceLimitsV1 {
    fn default() -> Self {
        Self {
            max_exports: 64,
            max_retired_exports: 64,
            max_chunks_per_export: 256,
            max_chunk_bytes: 64 * 1_024,
        }
    }
}

impl ExportServiceLimitsV1 {
    fn valid(self) -> bool {
        self.max_exports != 0
            && self.max_retired_exports != 0
            && self.max_chunks_per_export != 0
            && self.max_chunk_bytes >= 8
            && self.max_chunk_bytes <= u32::MAX as usize
    }

    fn within(self, maximum: Self) -> bool {
        self.max_exports <= maximum.max_exports
            && self.max_retired_exports <= maximum.max_retired_exports
            && self.max_chunks_per_export <= maximum.max_chunks_per_export
            && self.max_chunk_bytes <= maximum.max_chunk_bytes
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct ExportCheckpointScopeV1([u8; 16]);

impl ExportCheckpointScopeV1 {
    pub fn new(bytes: [u8; 16]) -> Option<Self> {
        (bytes != [0; 16]).then_some(Self(bytes))
    }

    pub const fn as_bytes(self) -> [u8; 16] {
        self.0
    }
}

impl fmt::Debug for ExportCheckpointScopeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ExportCheckpointScopeV1(<redacted>)")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExportAdmissionV1 {
    pub current_engine_revision: u64,
    pub current_policy_revision: RetentionPolicyRevisionV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportServiceRefusalV1 {
    InvalidLimits,
    MaterializationLimit,
    StaleSource {
        expected: u64,
        actual: u64,
    },
    PolicyMismatch {
        expected: RetentionPolicyRevisionV1,
        actual: RetentionPolicyRevisionV1,
    },
    Registry(ExportRegistryRefusalV1),
    Retired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportRetirementApplyV1 {
    Retired,
    Replay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportRetirementRefusalV1 {
    StaleSource,
    PolicyMismatch,
    Missing,
    Conflict,
    Capacity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportCheckpointErrorV1 {
    InvalidLimit,
    Limit,
    Truncated,
    Magic,
    Version,
    Scope,
    Invalid,
    NonCanonical,
    Digest,
    Capacity,
    Trailing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExportRecoveryLimitsV1 {
    pub max_snapshot_bytes: usize,
    pub maximum_service_limits: ExportServiceLimitsV1,
}

#[derive(Clone, Eq, PartialEq)]
pub struct ExportCheckpointV1 {
    bytes: Vec<u8>,
}

impl ExportCheckpointV1 {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl fmt::Debug for ExportCheckpointV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExportCheckpointV1")
            .field("byte_length", &self.bytes.len())
            .field("bytes", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportServiceV1 {
    limits: ExportServiceLimitsV1,
    registry: ExportRegistryV1,
    retired: BTreeMap<ExportRequestIdV1, ExportRetirementV1>,
}

impl ExportServiceV1 {
    pub fn new(limits: ExportServiceLimitsV1) -> Result<Self, ExportServiceRefusalV1> {
        if !limits.valid() {
            return Err(ExportServiceRefusalV1::InvalidLimits);
        }
        Ok(Self {
            limits,
            registry: ExportRegistryV1::new(limits.max_exports)
                .map_err(ExportServiceRefusalV1::Registry)?,
            retired: BTreeMap::new(),
        })
    }

    pub const fn limits(&self) -> ExportServiceLimitsV1 {
        self.limits
    }

    pub fn len(&self) -> usize {
        self.registry.len()
    }

    pub fn is_empty(&self) -> bool {
        self.registry.is_empty() && self.retired.is_empty()
    }

    pub fn materialization(&self, request: ExportRequestIdV1) -> Option<&ExportMaterializationV1> {
        self.registry.materialization(request)
    }

    pub fn retired_count(&self) -> usize {
        self.retired.len()
    }

    pub fn retirement(&self, request: ExportRequestIdV1) -> Option<&ExportRetirementV1> {
        self.retired.get(&request)
    }

    /// Admits a result already produced by a separately qualified source
    /// projector and digest implementation. A refusal leaves custody unchanged.
    pub fn admit(
        &mut self,
        admission: ExportAdmissionV1,
        materialization: ExportMaterializationV1,
    ) -> Result<ExportMaterializationApplyV1, ExportServiceRefusalV1> {
        if self.retired.contains_key(&materialization.manifest.request) {
            return Err(ExportServiceRefusalV1::Retired);
        }
        if materialization.chunks().len() > self.limits.max_chunks_per_export
            || materialization
                .chunks()
                .iter()
                .any(|chunk| chunk.bytes().len() > self.limits.max_chunk_bytes)
        {
            return Err(ExportServiceRefusalV1::MaterializationLimit);
        }
        if materialization.manifest.source_engine_revision != admission.current_engine_revision {
            return Err(ExportServiceRefusalV1::StaleSource {
                expected: materialization.manifest.source_engine_revision,
                actual: admission.current_engine_revision,
            });
        }
        if materialization.manifest.policy_revision != admission.current_policy_revision {
            return Err(ExportServiceRefusalV1::PolicyMismatch {
                expected: materialization.manifest.policy_revision,
                actual: admission.current_policy_revision,
            });
        }
        self.registry
            .offer(materialization)
            .map_err(ExportServiceRefusalV1::Registry)
    }

    /// Atomically replaces one exact retained export copy with a bounded
    /// non-reuse tombstone. This is logical custody retirement, not proof of
    /// physical reclamation or erasure from other copies.
    pub fn retire(
        &mut self,
        admission: ExportAdmissionV1,
        retirement: ExportRetirementV1,
    ) -> Result<ExportRetirementApplyV1, ExportRetirementRefusalV1> {
        if retirement.source_engine_revision != admission.current_engine_revision {
            return Err(ExportRetirementRefusalV1::StaleSource);
        }
        if retirement.policy_revision != admission.current_policy_revision {
            return Err(ExportRetirementRefusalV1::PolicyMismatch);
        }
        if let Some(retained) = self.retired.get(&retirement.request) {
            return if retained == &retirement {
                Ok(ExportRetirementApplyV1::Replay)
            } else {
                Err(ExportRetirementRefusalV1::Conflict)
            };
        }
        let materialization = self
            .registry
            .materialization(retirement.request)
            .ok_or(ExportRetirementRefusalV1::Missing)?;
        if ExportRetirementV1::from_manifest(&materialization.manifest) != retirement {
            return Err(ExportRetirementRefusalV1::Conflict);
        }
        if self.retired.len() == self.limits.max_retired_exports {
            return Err(ExportRetirementRefusalV1::Capacity);
        }
        let removed = self
            .registry
            .remove(retirement.request)
            .expect("preflighted export remains present");
        debug_assert_eq!(
            ExportRetirementV1::from_manifest(&removed.manifest),
            retirement
        );
        self.retired.insert(retirement.request, retirement);
        Ok(ExportRetirementApplyV1::Retired)
    }

    pub fn prepare_checkpoint<D>(
        &self,
        scope: ExportCheckpointScopeV1,
        max_snapshot_bytes: usize,
        mut digest: D,
    ) -> Result<ExportCheckpointV1, ExportCheckpointErrorV1>
    where
        D: FnMut(&[u8]) -> ContentDigestV1,
    {
        if max_snapshot_bytes == 0 {
            return Err(ExportCheckpointErrorV1::InvalidLimit);
        }
        let mut writer = Writer::new(max_snapshot_bytes);
        writer.raw(MAGIC)?;
        let version = if self.retired.is_empty()
            && self.limits.max_retired_exports == self.limits.max_exports
        {
            VERSION_V1
        } else {
            VERSION_V2
        };
        writer.byte(version)?;
        writer.raw(&scope.as_bytes())?;
        writer.size(self.limits.max_exports)?;
        if version == VERSION_V2 {
            writer.size(self.limits.max_retired_exports)?;
        }
        writer.size(self.limits.max_chunks_per_export)?;
        writer.size(self.limits.max_chunk_bytes)?;
        let materializations = self.registry.checkpoint();
        writer.size(materializations.len())?;
        for materialization in materializations {
            write_materialization(&mut writer, &materialization)?;
        }
        if version == VERSION_V2 {
            writer.size(self.retired.len())?;
            for retirement in self.retired.values() {
                write_retirement(&mut writer, *retirement)?;
            }
        }
        let checkpoint_digest = digest(&writer.bytes).as_bytes();
        writer.raw(&checkpoint_digest)?;
        Ok(ExportCheckpointV1 {
            bytes: writer.bytes,
        })
    }

    pub fn recover_checkpoint<D>(
        bytes: &[u8],
        expected_scope: ExportCheckpointScopeV1,
        recovery: ExportRecoveryLimitsV1,
        mut digest: D,
    ) -> Result<Self, ExportCheckpointErrorV1>
    where
        D: FnMut(&[u8]) -> ContentDigestV1,
    {
        if recovery.max_snapshot_bytes < 32 || !recovery.maximum_service_limits.valid() {
            return Err(ExportCheckpointErrorV1::InvalidLimit);
        }
        if bytes.len() > recovery.max_snapshot_bytes {
            return Err(ExportCheckpointErrorV1::Limit);
        }
        let (body, encoded_digest) = bytes
            .split_at_checked(
                bytes
                    .len()
                    .checked_sub(32)
                    .ok_or(ExportCheckpointErrorV1::Truncated)?,
            )
            .ok_or(ExportCheckpointErrorV1::Truncated)?;
        if digest(body).as_bytes() != encoded_digest {
            return Err(ExportCheckpointErrorV1::Digest);
        }
        let mut reader = Reader::new(body);
        if reader.fixed::<8>()? != *MAGIC {
            return Err(ExportCheckpointErrorV1::Magic);
        }
        let version = reader.byte()?;
        if !matches!(version, VERSION_V1 | VERSION_V2) {
            return Err(ExportCheckpointErrorV1::Version);
        }
        if reader.fixed::<16>()? != expected_scope.as_bytes() {
            return Err(ExportCheckpointErrorV1::Scope);
        }
        let max_exports = reader.size(recovery.maximum_service_limits.max_exports)?;
        let limits = ExportServiceLimitsV1 {
            max_exports,
            max_retired_exports: if version == VERSION_V2 {
                reader.size(recovery.maximum_service_limits.max_retired_exports)?
            } else {
                max_exports
            },
            max_chunks_per_export: reader
                .size(recovery.maximum_service_limits.max_chunks_per_export)?,
            max_chunk_bytes: reader.size(recovery.maximum_service_limits.max_chunk_bytes)?,
        };
        if !limits.valid() || !limits.within(recovery.maximum_service_limits) {
            return Err(ExportCheckpointErrorV1::Invalid);
        }
        let count = reader.size(limits.max_exports)?;
        let mut materializations = Vec::new();
        materializations
            .try_reserve(count)
            .map_err(|_| ExportCheckpointErrorV1::Capacity)?;
        for _ in 0..count {
            materializations.push(read_materialization(&mut reader, limits, &mut digest)?);
        }
        let mut retired = BTreeMap::new();
        if version == VERSION_V2 {
            let retired_count = reader.size(limits.max_retired_exports)?;
            for _ in 0..retired_count {
                let retirement = read_retirement(&mut reader)?;
                if retired
                    .last_key_value()
                    .is_some_and(|(prior, _)| prior >= &retirement.request)
                    || retired.insert(retirement.request, retirement).is_some()
                {
                    return Err(ExportCheckpointErrorV1::NonCanonical);
                }
            }
        }
        if !reader.is_complete() {
            return Err(ExportCheckpointErrorV1::Trailing);
        }
        let registry = ExportRegistryV1::recover(limits.max_exports, materializations)
            .map_err(|_| ExportCheckpointErrorV1::NonCanonical)?;
        if retired
            .keys()
            .any(|request| registry.materialization(*request).is_some())
        {
            return Err(ExportCheckpointErrorV1::NonCanonical);
        }
        Ok(Self {
            limits,
            registry,
            retired,
        })
    }
}

fn write_retirement(
    writer: &mut Writer,
    retirement: ExportRetirementV1,
) -> Result<(), ExportCheckpointErrorV1> {
    writer.raw(&retirement.request.as_bytes())?;
    writer.u16(retirement.profile as u16)?;
    writer.raw(&retirement.scope.as_bytes())?;
    writer.u64(retirement.source_engine_revision)?;
    writer.u64(retirement.policy_revision.get())?;
    writer.raw(&retirement.manifest_digest.as_bytes())
}

fn read_retirement(reader: &mut Reader<'_>) -> Result<ExportRetirementV1, ExportCheckpointErrorV1> {
    let request =
        ExportRequestIdV1::new(reader.fixed::<16>()?).ok_or(ExportCheckpointErrorV1::Invalid)?;
    let profile = match reader.u16()? {
        1 => ExportProfileV1::RoomVisible,
        2 => ExportProfileV1::ParticipantPrivate,
        3 => ExportProfileV1::Administrative,
        _ => return Err(ExportCheckpointErrorV1::Invalid),
    };
    let scope =
        ExportScopeBindingV1::new(reader.fixed::<32>()?).ok_or(ExportCheckpointErrorV1::Invalid)?;
    let source_engine_revision = reader.u64()?;
    let policy_revision =
        RetentionPolicyRevisionV1::new(reader.u64()?).ok_or(ExportCheckpointErrorV1::Invalid)?;
    let manifest_digest = ContentDigestV1::new(reader.fixed::<32>()?);
    Ok(ExportRetirementV1 {
        request,
        profile,
        scope,
        source_engine_revision,
        policy_revision,
        manifest_digest,
    })
}

fn write_materialization(
    writer: &mut Writer,
    materialization: &ExportMaterializationV1,
) -> Result<(), ExportCheckpointErrorV1> {
    let manifest = &materialization.manifest;
    writer.raw(&manifest.request.as_bytes())?;
    writer.u16(manifest.profile as u16)?;
    writer.raw(&manifest.scope.as_bytes())?;
    writer.u16(manifest.digest_profile as u16)?;
    writer.u64(manifest.source_engine_revision)?;
    writer.u64(manifest.policy_revision.get())?;
    for included in manifest.included_classes {
        writer.byte(u8::from(included))?;
    }
    for frontier in manifest.frontiers {
        writer.byte(frontier.class as u8)?;
        writer.u64(frontier.discarded_through)?;
        writer.byte(u8::from(frontier.oldest_retained.is_some()))?;
        writer.u64(frontier.oldest_retained.unwrap_or(0))?;
    }
    writer.size(manifest.chunks().len())?;
    for (metadata, payload) in manifest.chunks().iter().zip(materialization.chunks()) {
        writer.u32(metadata.index)?;
        writer.u32(metadata.byte_length)?;
        writer.raw(&metadata.digest.as_bytes())?;
        writer.blob(payload.bytes())?;
    }
    writer.raw(&manifest.manifest_digest.as_bytes())
}

fn read_materialization<D>(
    reader: &mut Reader<'_>,
    limits: ExportServiceLimitsV1,
    digest: &mut D,
) -> Result<ExportMaterializationV1, ExportCheckpointErrorV1>
where
    D: FnMut(&[u8]) -> ContentDigestV1,
{
    let request =
        ExportRequestIdV1::new(reader.fixed::<16>()?).ok_or(ExportCheckpointErrorV1::Invalid)?;
    let profile = match reader.u16()? {
        1 => ExportProfileV1::RoomVisible,
        2 => ExportProfileV1::ParticipantPrivate,
        3 => ExportProfileV1::Administrative,
        _ => return Err(ExportCheckpointErrorV1::Invalid),
    };
    let scope =
        ExportScopeBindingV1::new(reader.fixed::<32>()?).ok_or(ExportCheckpointErrorV1::Invalid)?;
    let digest_profile = match reader.u16()? {
        1 => ContentDigestProfileV1::Sha256,
        _ => return Err(ExportCheckpointErrorV1::Invalid),
    };
    let source_engine_revision = reader.u64()?;
    let policy_revision =
        RetentionPolicyRevisionV1::new(reader.u64()?).ok_or(ExportCheckpointErrorV1::Invalid)?;
    let mut included_classes = [false; 6];
    for included in &mut included_classes {
        *included = match reader.byte()? {
            0 => false,
            1 => true,
            _ => return Err(ExportCheckpointErrorV1::NonCanonical),
        };
    }
    let mut frontiers = [RetentionFrontierV1::new(RECORD_CLASSES_V1[0], 0, None)
        .expect("zero frontier is valid"); 6];
    for (frontier, class) in frontiers.iter_mut().zip(RECORD_CLASSES_V1) {
        if reader.byte()? != class as u8 {
            return Err(ExportCheckpointErrorV1::NonCanonical);
        }
        let discarded_through = reader.u64()?;
        let oldest_retained = match reader.byte()? {
            0 => {
                if reader.u64()? != 0 {
                    return Err(ExportCheckpointErrorV1::NonCanonical);
                }
                None
            }
            1 => Some(reader.u64()?),
            _ => return Err(ExportCheckpointErrorV1::NonCanonical),
        };
        *frontier = RetentionFrontierV1::new(class, discarded_through, oldest_retained)
            .ok_or(ExportCheckpointErrorV1::Invalid)?;
    }
    let chunk_count = reader.size(limits.max_chunks_per_export)?;
    if chunk_count == 0 {
        return Err(ExportCheckpointErrorV1::Invalid);
    }
    let mut metadata = Vec::new();
    let mut payloads = Vec::new();
    metadata
        .try_reserve(chunk_count)
        .map_err(|_| ExportCheckpointErrorV1::Capacity)?;
    payloads
        .try_reserve(chunk_count)
        .map_err(|_| ExportCheckpointErrorV1::Capacity)?;
    for expected_index in 0..chunk_count {
        let index = reader.u32()?;
        if usize::try_from(index) != Ok(expected_index) {
            return Err(ExportCheckpointErrorV1::NonCanonical);
        }
        let byte_length = reader.u32()?;
        let chunk_digest = ContentDigestV1::new(reader.fixed::<32>()?);
        let bytes = reader.blob(limits.max_chunk_bytes)?.to_vec();
        metadata.push(
            ExportChunkV1::new(index, byte_length, chunk_digest)
                .ok_or(ExportCheckpointErrorV1::Invalid)?,
        );
        payloads.push(
            ExportChunkPayloadV1::new(index, bytes, limits.max_chunk_bytes)
                .ok_or(ExportCheckpointErrorV1::Invalid)?,
        );
    }
    let manifest_digest = ContentDigestV1::new(reader.fixed::<32>()?);
    let manifest = ExportManifestV1::new(
        request,
        profile,
        scope,
        digest_profile,
        source_engine_revision,
        policy_revision,
        included_classes,
        frontiers,
        metadata,
        manifest_digest,
        limits.max_chunks_per_export,
    )
    .ok_or(ExportCheckpointErrorV1::NonCanonical)?;
    ExportMaterializationV1::recover(
        manifest,
        payloads,
        limits.max_chunks_per_export,
        limits.max_chunk_bytes,
        digest,
    )
    .map_err(|error| match error {
        ExportMaterializationRefusalV1::DigestMismatch => ExportCheckpointErrorV1::Digest,
        ExportMaterializationRefusalV1::InvalidLimit => ExportCheckpointErrorV1::InvalidLimit,
        _ => ExportCheckpointErrorV1::NonCanonical,
    })
}

struct Writer {
    bytes: Vec<u8>,
    maximum: usize,
}

impl Writer {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
        }
    }

    fn raw(&mut self, bytes: &[u8]) -> Result<(), ExportCheckpointErrorV1> {
        let resulting = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or(ExportCheckpointErrorV1::Limit)?;
        if resulting > self.maximum {
            return Err(ExportCheckpointErrorV1::Limit);
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    fn byte(&mut self, value: u8) -> Result<(), ExportCheckpointErrorV1> {
        self.raw(&[value])
    }

    fn u16(&mut self, value: u16) -> Result<(), ExportCheckpointErrorV1> {
        self.raw(&value.to_be_bytes())
    }

    fn u32(&mut self, value: u32) -> Result<(), ExportCheckpointErrorV1> {
        self.raw(&value.to_be_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), ExportCheckpointErrorV1> {
        self.raw(&value.to_be_bytes())
    }

    fn size(&mut self, value: usize) -> Result<(), ExportCheckpointErrorV1> {
        self.u64(u64::try_from(value).map_err(|_| ExportCheckpointErrorV1::Limit)?)
    }

    fn blob(&mut self, bytes: &[u8]) -> Result<(), ExportCheckpointErrorV1> {
        self.size(bytes.len())?;
        self.raw(bytes)
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], ExportCheckpointErrorV1> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(ExportCheckpointErrorV1::Truncated)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(ExportCheckpointErrorV1::Truncated)?;
        self.position = end;
        Ok(value)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], ExportCheckpointErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| ExportCheckpointErrorV1::Truncated)
    }

    fn byte(&mut self) -> Result<u8, ExportCheckpointErrorV1> {
        Ok(self.fixed::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, ExportCheckpointErrorV1> {
        Ok(u16::from_be_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, ExportCheckpointErrorV1> {
        Ok(u32::from_be_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, ExportCheckpointErrorV1> {
        Ok(u64::from_be_bytes(self.fixed()?))
    }

    fn size(&mut self, maximum: usize) -> Result<usize, ExportCheckpointErrorV1> {
        let value = usize::try_from(self.u64()?).map_err(|_| ExportCheckpointErrorV1::Limit)?;
        (value <= maximum)
            .then_some(value)
            .ok_or(ExportCheckpointErrorV1::Limit)
    }

    fn blob(&mut self, maximum: usize) -> Result<&'a [u8], ExportCheckpointErrorV1> {
        let length = self.size(maximum)?;
        self.take(length)
    }

    fn is_complete(&self) -> bool {
        self.position == self.bytes.len()
    }
}
