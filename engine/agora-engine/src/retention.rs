//! Closed values for Agora logical retention, exact export, and store reclamation.
//!
//! This module performs no authorization or I/O. It defines Agora-owned meaning;
//! hosts and adapters must provide authority and physical effects separately.

use std::collections::BTreeMap;
use std::fmt;
use std::num::NonZeroUsize;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum RecordClassV1 {
    RoomProjection = 1,
    AuthoritativeChanges = 2,
    RequestReceipts = 3,
    PrivateParticipantState = 4,
    FederationState = 5,
    OperationalCopies = 6,
}

pub const RECORD_CLASSES_V1: [RecordClassV1; 6] = [
    RecordClassV1::RoomProjection,
    RecordClassV1::AuthoritativeChanges,
    RecordClassV1::RequestReceipts,
    RecordClassV1::PrivateParticipantState,
    RecordClassV1::FederationState,
    RecordClassV1::OperationalCopies,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetentionBoundV1 {
    /// This class retains no records under the policy. This never means
    /// unbounded retention and does not reconstruct records if later enabled.
    Disabled,
    Entries(NonZeroUsize),
}

impl RetentionBoundV1 {
    pub fn entries(maximum: usize) -> Option<Self> {
        NonZeroUsize::new(maximum).map(Self::Entries)
    }

    pub fn maximum(self) -> Option<usize> {
        match self {
            Self::Disabled => None,
            Self::Entries(value) => Some(value.get()),
        }
    }

    pub fn retains_records(self) -> bool {
        matches!(self, Self::Entries(_))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RetentionPolicyRevisionV1(u64);

impl RetentionPolicyRevisionV1 {
    pub fn new(value: u64) -> Option<Self> {
        (value != 0).then_some(Self(value))
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetentionPolicyV1 {
    pub revision: RetentionPolicyRevisionV1,
    pub effective_engine_revision: u64,
    bounds: [RetentionBoundV1; 6],
}

impl RetentionPolicyV1 {
    pub fn new(
        revision: RetentionPolicyRevisionV1,
        effective_engine_revision: u64,
        bounds: [RetentionBoundV1; 6],
    ) -> Self {
        Self {
            revision,
            effective_engine_revision,
            bounds,
        }
    }

    pub fn bound(self, class: RecordClassV1) -> RetentionBoundV1 {
        self.bounds[class as usize - 1]
    }

    pub fn bounds(self) -> [RetentionBoundV1; 6] {
        self.bounds
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetentionFrontierV1 {
    pub class: RecordClassV1,
    pub discarded_through: u64,
    pub oldest_retained: Option<u64>,
}

impl RetentionFrontierV1 {
    pub fn new(
        class: RecordClassV1,
        discarded_through: u64,
        oldest_retained: Option<u64>,
    ) -> Option<Self> {
        let valid = oldest_retained.is_none_or(|oldest| oldest != 0 && oldest > discarded_through);
        valid.then_some(Self {
            class,
            discarded_through,
            oldest_retained,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetentionTransitionV1 {
    pub policy_revision: RetentionPolicyRevisionV1,
    pub prior_engine_revision: u64,
    pub resulting_engine_revision: u64,
    pub discarded_records: [u64; 6],
    pub frontiers: [RetentionFrontierV1; 6],
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RetentionRecordIdV1 {
    pub class: RecordClassV1,
    pub ordinal: u64,
}

impl RetentionRecordIdV1 {
    pub fn new(class: RecordClassV1, ordinal: u64) -> Option<Self> {
        (ordinal != 0).then_some(Self { class, ordinal })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetentionRecordV1 {
    pub id: RetentionRecordIdV1,
    dependencies: Vec<RetentionRecordIdV1>,
}

impl RetentionRecordV1 {
    pub fn new(
        id: RetentionRecordIdV1,
        dependencies: Vec<RetentionRecordIdV1>,
        max_dependencies: usize,
    ) -> Option<Self> {
        let canonical = dependencies.len() <= max_dependencies
            && dependencies.windows(2).all(|pair| pair[0] < pair[1])
            && dependencies.binary_search(&id).is_err();
        canonical.then_some(Self { id, dependencies })
    }

    pub fn dependencies(&self) -> &[RetentionRecordIdV1] {
        &self.dependencies
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetentionPlanRefusalV1 {
    InvalidLimit,
    StalePolicy,
    NonCanonicalRecords,
    MissingDependency,
    CountOverflow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetentionApplicationPlanV1 {
    pub transition: RetentionTransitionV1,
    retained: Vec<RetentionRecordIdV1>,
    discarded: Vec<RetentionRecordIdV1>,
}

impl RetentionApplicationPlanV1 {
    pub fn retained(&self) -> &[RetentionRecordIdV1] {
        &self.retained
    }

    pub fn discarded(&self) -> &[RetentionRecordIdV1] {
        &self.discarded
    }
}

/// Plans one deterministic logical-retention transition over a complete,
/// canonically ordered record inventory. The plan performs no mutation or I/O.
/// Explicit dependencies may preserve records beyond their class bound; a
/// missing dependency refuses the complete plan.
pub fn plan_retention_v1(
    policy: RetentionPolicyV1,
    source_engine_revision: u64,
    records: &[RetentionRecordV1],
    max_records: usize,
    max_dependencies: usize,
) -> Result<RetentionApplicationPlanV1, RetentionPlanRefusalV1> {
    if max_records == 0 || records.len() > max_records {
        return Err(RetentionPlanRefusalV1::InvalidLimit);
    }
    if policy.effective_engine_revision > source_engine_revision {
        return Err(RetentionPlanRefusalV1::StalePolicy);
    }
    if records.windows(2).any(|pair| pair[0].id >= pair[1].id)
        || records
            .iter()
            .any(|record| record.dependencies.len() > max_dependencies)
    {
        return Err(RetentionPlanRefusalV1::NonCanonicalRecords);
    }
    let by_id: BTreeMap<_, _> = records
        .iter()
        .enumerate()
        .map(|(index, record)| (record.id, index))
        .collect();
    if records
        .iter()
        .flat_map(|record| &record.dependencies)
        .any(|dependency| !by_id.contains_key(dependency))
    {
        return Err(RetentionPlanRefusalV1::MissingDependency);
    }

    let mut retained = vec![false; records.len()];
    for class in RECORD_CLASSES_V1 {
        let maximum = policy.bound(class).maximum().unwrap_or(0);
        for (_, index) in by_id
            .range(
                RetentionRecordIdV1 { class, ordinal: 1 }..=RetentionRecordIdV1 {
                    class,
                    ordinal: u64::MAX,
                },
            )
            .rev()
            .take(maximum)
        {
            retained[*index] = true;
        }
    }
    let mut changed = true;
    while changed {
        changed = false;
        for (index, record) in records.iter().enumerate() {
            if !retained[index] {
                continue;
            }
            for dependency in &record.dependencies {
                let dependency_index = by_id[dependency];
                if !retained[dependency_index] {
                    retained[dependency_index] = true;
                    changed = true;
                }
            }
        }
    }

    let mut retained_ids = Vec::new();
    let mut discarded_ids = Vec::new();
    let mut discarded_records = [0_u64; 6];
    for (record, keep) in records.iter().zip(&retained) {
        if *keep {
            retained_ids.push(record.id);
        } else {
            discarded_records[record.id.class as usize - 1] = discarded_records
                [record.id.class as usize - 1]
                .checked_add(1)
                .ok_or(RetentionPlanRefusalV1::CountOverflow)?;
            discarded_ids.push(record.id);
        }
    }
    let frontiers = RECORD_CLASSES_V1.map(|class| {
        let oldest_retained = retained_ids
            .iter()
            .find(|record| record.class == class)
            .map(|record| record.ordinal);
        let discarded_through = records
            .iter()
            .take_while(|record| record.id.class <= class)
            .filter(|record| record.id.class == class)
            .take_while(|record| !retained[by_id[&record.id]])
            .map(|record| record.id.ordinal)
            .last()
            .unwrap_or(0);
        RetentionFrontierV1::new(class, discarded_through, oldest_retained)
            .expect("planner constructs an ordered frontier")
    });
    let transition = RetentionTransitionV1::new(
        policy.revision,
        source_engine_revision,
        source_engine_revision,
        discarded_records,
        frontiers,
    )
    .expect("planner constructs exact class order");
    Ok(RetentionApplicationPlanV1 {
        transition,
        retained: retained_ids,
        discarded: discarded_ids,
    })
}

impl RetentionTransitionV1 {
    pub fn new(
        policy_revision: RetentionPolicyRevisionV1,
        prior_engine_revision: u64,
        resulting_engine_revision: u64,
        discarded_records: [u64; 6],
        frontiers: [RetentionFrontierV1; 6],
    ) -> Option<Self> {
        let exact_classes = frontiers
            .iter()
            .zip(RECORD_CLASSES_V1)
            .all(|(frontier, class)| frontier.class == class);
        (resulting_engine_revision >= prior_engine_revision && exact_classes).then_some(Self {
            policy_revision,
            prior_engine_revision,
            resulting_engine_revision,
            discarded_records,
            frontiers,
        })
    }
}

macro_rules! redacted_nonzero_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 16]);

        impl $name {
            pub fn new(bytes: [u8; 16]) -> Option<Self> {
                (bytes != [0; 16]).then_some(Self(bytes))
            }

            pub fn as_bytes(self) -> [u8; 16] {
                self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(concat!(stringify!($name), "(<redacted>)"))
            }
        }
    };
}

redacted_nonzero_id!(ExportRequestIdV1);
redacted_nonzero_id!(StoreGenerationIdV1);

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExportScopeBindingV1([u8; 32]);

impl ExportScopeBindingV1 {
    pub fn new(bytes: [u8; 32]) -> Option<Self> {
        (bytes != [0; 32]).then_some(Self(bytes))
    }

    pub fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Debug for ExportScopeBindingV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ExportScopeBindingV1(<redacted>)")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum ExportProfileV1 {
    RoomVisible = 1,
    ParticipantPrivate = 2,
    Administrative = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum ContentDigestProfileV1 {
    Sha256 = 1,
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContentDigestV1([u8; 32]);

impl ContentDigestV1 {
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Debug for ContentDigestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ContentDigestV1(<digest>)")
    }
}

macro_rules! redacted_binding {
    ($name:ident) => {
        #[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);

        impl $name {
            pub fn new(bytes: [u8; 32]) -> Option<Self> {
                (bytes != [0; 32]).then_some(Self(bytes))
            }

            pub fn as_bytes(self) -> [u8; 32] {
                self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(concat!(stringify!($name), "(<redacted>)"))
            }
        }
    };
}

redacted_binding!(RetentionIdentityBindingV1);
redacted_binding!(RetentionScopeBindingV1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExportChunkV1 {
    pub index: u32,
    pub byte_length: u32,
    pub digest: ContentDigestV1,
}

impl ExportChunkV1 {
    pub fn new(index: u32, byte_length: u32, digest: ContentDigestV1) -> Option<Self> {
        (byte_length != 0).then_some(Self {
            index,
            byte_length,
            digest,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportManifestV1 {
    pub request: ExportRequestIdV1,
    pub profile: ExportProfileV1,
    pub scope: ExportScopeBindingV1,
    pub digest_profile: ContentDigestProfileV1,
    pub source_engine_revision: u64,
    pub policy_revision: RetentionPolicyRevisionV1,
    pub included_classes: [bool; 6],
    pub frontiers: [RetentionFrontierV1; 6],
    chunks: Vec<ExportChunkV1>,
    pub manifest_digest: ContentDigestV1,
}

impl ExportManifestV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        request: ExportRequestIdV1,
        profile: ExportProfileV1,
        scope: ExportScopeBindingV1,
        digest_profile: ContentDigestProfileV1,
        source_engine_revision: u64,
        policy_revision: RetentionPolicyRevisionV1,
        included_classes: [bool; 6],
        frontiers: [RetentionFrontierV1; 6],
        chunks: Vec<ExportChunkV1>,
        manifest_digest: ContentDigestV1,
        max_chunks: usize,
    ) -> Option<Self> {
        let exact_classes = frontiers
            .iter()
            .zip(RECORD_CLASSES_V1)
            .all(|(frontier, class)| frontier.class == class);
        let canonical_chunks = !chunks.is_empty()
            && chunks.len() <= max_chunks
            && chunks
                .iter()
                .enumerate()
                .all(|(index, chunk)| usize::try_from(chunk.index) == Ok(index));
        (max_chunks != 0 && exact_classes && canonical_chunks).then_some(Self {
            request,
            profile,
            scope,
            digest_profile,
            source_engine_revision,
            policy_revision,
            included_classes,
            frontiers,
            chunks,
            manifest_digest,
        })
    }

    pub fn chunks(&self) -> &[ExportChunkV1] {
        &self.chunks
    }

    fn digest_body(&self) -> Vec<u8> {
        let mut body = EXPORT_MANIFEST_DOMAIN_V1.to_vec();
        body.extend_from_slice(&self.request.as_bytes());
        body.extend_from_slice(&(self.profile as u16).to_be_bytes());
        body.extend_from_slice(&self.scope.as_bytes());
        body.extend_from_slice(&(self.digest_profile as u16).to_be_bytes());
        body.extend_from_slice(&self.source_engine_revision.to_be_bytes());
        body.extend_from_slice(&self.policy_revision.get().to_be_bytes());
        body.extend(self.included_classes.map(u8::from));
        for frontier in self.frontiers {
            body.push(frontier.class as u8);
            body.extend_from_slice(&frontier.discarded_through.to_be_bytes());
            body.push(u8::from(frontier.oldest_retained.is_some()));
            body.extend_from_slice(&frontier.oldest_retained.unwrap_or(0).to_be_bytes());
        }
        body.extend_from_slice(&(self.chunks.len() as u32).to_be_bytes());
        for chunk in &self.chunks {
            body.extend_from_slice(&chunk.index.to_be_bytes());
            body.extend_from_slice(&chunk.byte_length.to_be_bytes());
            body.extend_from_slice(&chunk.digest.as_bytes());
        }
        body
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct ExportRecordV1 {
    pub id: RetentionRecordIdV1,
    payload: Vec<u8>,
}

impl ExportRecordV1 {
    pub fn new(
        id: RetentionRecordIdV1,
        payload: Vec<u8>,
        max_payload_bytes: usize,
    ) -> Option<Self> {
        (!payload.is_empty() && payload.len() <= max_payload_bytes).then_some(Self { id, payload })
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

impl fmt::Debug for ExportRecordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExportRecordV1")
            .field("id", &self.id)
            .field("payload", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct ExportChunkPayloadV1 {
    pub index: u32,
    bytes: Vec<u8>,
}

impl ExportChunkPayloadV1 {
    pub fn new(index: u32, bytes: Vec<u8>, max_chunk_bytes: usize) -> Option<Self> {
        (!bytes.is_empty()
            && bytes.len() <= max_chunk_bytes
            && bytes.starts_with(EXPORT_CHUNK_DOMAIN_V1))
        .then_some(Self { index, bytes })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl fmt::Debug for ExportChunkPayloadV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExportChunkPayloadV1")
            .field("index", &self.index)
            .field("byte_length", &self.bytes.len())
            .field("bytes", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportMaterializationV1 {
    pub manifest: ExportManifestV1,
    chunks: Vec<ExportChunkPayloadV1>,
}

impl ExportMaterializationV1 {
    pub fn chunks(&self) -> &[ExportChunkPayloadV1] {
        &self.chunks
    }

    pub fn recover<D>(
        manifest: ExportManifestV1,
        chunks: Vec<ExportChunkPayloadV1>,
        max_chunks: usize,
        max_chunk_bytes: usize,
        mut digest: D,
    ) -> Result<Self, ExportMaterializationRefusalV1>
    where
        D: FnMut(&[u8]) -> ContentDigestV1,
    {
        if max_chunks == 0 || max_chunk_bytes < EXPORT_CHUNK_DOMAIN_V1.len() {
            return Err(ExportMaterializationRefusalV1::InvalidLimit);
        }
        if chunks.is_empty()
            || chunks.len() > max_chunks
            || chunks.len() != manifest.chunks.len()
            || chunks.iter().zip(&manifest.chunks).enumerate().any(
                |(index, (payload, metadata))| {
                    usize::try_from(payload.index) != Ok(index)
                        || payload.index != metadata.index
                        || payload.bytes.is_empty()
                        || payload.bytes.len() > max_chunk_bytes
                        || u32::try_from(payload.bytes.len()) != Ok(metadata.byte_length)
                        || !payload.bytes.starts_with(EXPORT_CHUNK_DOMAIN_V1)
                },
            )
        {
            return Err(ExportMaterializationRefusalV1::NonCanonicalChunks);
        }
        if chunks
            .iter()
            .zip(&manifest.chunks)
            .any(|(payload, metadata)| digest(&payload.bytes) != metadata.digest)
            || digest(&manifest.digest_body()) != manifest.manifest_digest
        {
            return Err(ExportMaterializationRefusalV1::DigestMismatch);
        }
        Ok(Self { manifest, chunks })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportMaterializationApplyV1 {
    Materialized,
    Replay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportRegistryRefusalV1 {
    InvalidLimit,
    Conflict,
    Capacity,
    NonCanonicalCheckpoint,
}

/// Bounded exact export results keyed by their nonzero request identity.
///
/// This registry owns replay/conflict meaning only. It performs no authority
/// decision, source projection, persistence, publication, or custody action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportRegistryV1 {
    maximum: usize,
    materializations: BTreeMap<ExportRequestIdV1, ExportMaterializationV1>,
}

impl ExportRegistryV1 {
    pub fn new(maximum: usize) -> Result<Self, ExportRegistryRefusalV1> {
        if maximum == 0 {
            return Err(ExportRegistryRefusalV1::InvalidLimit);
        }
        Ok(Self {
            maximum,
            materializations: BTreeMap::new(),
        })
    }

    pub fn recover(
        maximum: usize,
        checkpoint: Vec<ExportMaterializationV1>,
    ) -> Result<Self, ExportRegistryRefusalV1> {
        let mut registry = Self::new(maximum)?;
        if checkpoint.len() > maximum
            || checkpoint
                .windows(2)
                .any(|pair| pair[0].manifest.request >= pair[1].manifest.request)
        {
            return Err(ExportRegistryRefusalV1::NonCanonicalCheckpoint);
        }
        for materialization in checkpoint {
            if registry.offer(materialization) != Ok(ExportMaterializationApplyV1::Materialized) {
                return Err(ExportRegistryRefusalV1::NonCanonicalCheckpoint);
            }
        }
        Ok(registry)
    }

    pub fn offer(
        &mut self,
        materialization: ExportMaterializationV1,
    ) -> Result<ExportMaterializationApplyV1, ExportRegistryRefusalV1> {
        let request = materialization.manifest.request;
        if let Some(retained) = self.materializations.get(&request) {
            return if retained == &materialization {
                Ok(ExportMaterializationApplyV1::Replay)
            } else {
                Err(ExportRegistryRefusalV1::Conflict)
            };
        }
        if self.materializations.len() == self.maximum {
            return Err(ExportRegistryRefusalV1::Capacity);
        }
        self.materializations.insert(request, materialization);
        Ok(ExportMaterializationApplyV1::Materialized)
    }

    pub fn materialization(&self, request: ExportRequestIdV1) -> Option<&ExportMaterializationV1> {
        self.materializations.get(&request)
    }

    pub fn checkpoint(&self) -> Vec<ExportMaterializationV1> {
        self.materializations.values().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.materializations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.materializations.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportMaterializationRefusalV1 {
    InvalidLimit,
    NonCanonicalRecords,
    ExcludedRecordClass,
    FrontierMismatch,
    OversizedRecord,
    ChunkLimit,
    NonCanonicalChunks,
    DigestMismatch,
}

const EXPORT_CHUNK_DOMAIN_V1: &[u8; 8] = b"AGEXCH1\0";
const EXPORT_MANIFEST_DOMAIN_V1: &[u8; 8] = b"AGEXMF1\0";

/// Materializes one complete, bounded export inventory into canonical chunks.
///
/// `digest` is supplied by the enclosing qualified composition so this no-I/O
/// engine does not implement a cryptographic provider. The implementation must
/// match `digest_profile`; qualification of that binding belongs to the host.
/// It is called once for each chunk and once for the canonical manifest body.
/// Equal inputs and an equal digest implementation produce byte-identical output.
#[allow(clippy::too_many_arguments)]
pub fn materialize_export_v1<D>(
    request: ExportRequestIdV1,
    profile: ExportProfileV1,
    scope: ExportScopeBindingV1,
    digest_profile: ContentDigestProfileV1,
    source_engine_revision: u64,
    policy_revision: RetentionPolicyRevisionV1,
    included_classes: [bool; 6],
    frontiers: [RetentionFrontierV1; 6],
    records: &[ExportRecordV1],
    max_records: usize,
    max_record_bytes: usize,
    max_chunks: usize,
    max_chunk_bytes: usize,
    mut digest: D,
) -> Result<ExportMaterializationV1, ExportMaterializationRefusalV1>
where
    D: FnMut(&[u8]) -> ContentDigestV1,
{
    if max_records == 0
        || max_record_bytes == 0
        || max_chunks == 0
        || max_chunk_bytes < EXPORT_CHUNK_DOMAIN_V1.len()
        || max_record_bytes > u32::MAX as usize
        || max_chunk_bytes > u32::MAX as usize
        || !included_classes.iter().any(|included| *included)
        || records.len() > max_records
    {
        return Err(ExportMaterializationRefusalV1::InvalidLimit);
    }
    if records.windows(2).any(|pair| pair[0].id >= pair[1].id) {
        return Err(ExportMaterializationRefusalV1::NonCanonicalRecords);
    }
    if records.iter().any(|record| {
        !included_classes[record.id.class as usize - 1]
            || record.payload.is_empty()
            || record.payload.len() > max_record_bytes
    }) {
        return Err(
            if records
                .iter()
                .any(|record| !included_classes[record.id.class as usize - 1])
            {
                ExportMaterializationRefusalV1::ExcludedRecordClass
            } else {
                ExportMaterializationRefusalV1::OversizedRecord
            },
        );
    }
    let exact_frontiers = frontiers
        .iter()
        .zip(RECORD_CLASSES_V1)
        .all(|(frontier, class)| frontier.class == class);
    let records_match_frontiers = RECORD_CLASSES_V1.iter().all(|class| {
        let class_records: Vec<_> = records
            .iter()
            .filter(|record| record.id.class == *class)
            .collect();
        if included_classes[*class as usize - 1] {
            class_records.first().map(|record| record.id.ordinal)
                == frontiers[*class as usize - 1].oldest_retained
                && class_records.iter().all(|record| {
                    record.id.ordinal > frontiers[*class as usize - 1].discarded_through
                })
        } else {
            class_records.is_empty()
        }
    });
    if !exact_frontiers || !records_match_frontiers {
        return Err(ExportMaterializationRefusalV1::FrontierMismatch);
    }

    let mut bodies = Vec::new();
    let mut current = EXPORT_CHUNK_DOMAIN_V1.to_vec();
    for record in records {
        let payload_length = u32::try_from(record.payload.len())
            .map_err(|_| ExportMaterializationRefusalV1::OversizedRecord)?;
        let frame_length = 1_usize
            .checked_add(8)
            .and_then(|length| length.checked_add(4))
            .and_then(|length| length.checked_add(record.payload.len()))
            .ok_or(ExportMaterializationRefusalV1::OversizedRecord)?;
        if EXPORT_CHUNK_DOMAIN_V1
            .len()
            .checked_add(frame_length)
            .is_none_or(|length| length > max_chunk_bytes)
        {
            return Err(ExportMaterializationRefusalV1::OversizedRecord);
        }
        if current
            .len()
            .checked_add(frame_length)
            .is_none_or(|length| length > max_chunk_bytes)
        {
            bodies.push(current);
            current = EXPORT_CHUNK_DOMAIN_V1.to_vec();
        }
        current.push(record.id.class as u8);
        current.extend_from_slice(&record.id.ordinal.to_be_bytes());
        current.extend_from_slice(&payload_length.to_be_bytes());
        current.extend_from_slice(&record.payload);
    }
    bodies.push(current);
    if bodies.len() > max_chunks {
        return Err(ExportMaterializationRefusalV1::ChunkLimit);
    }

    let mut chunks = Vec::with_capacity(bodies.len());
    let mut chunk_metadata = Vec::with_capacity(bodies.len());
    for (index, bytes) in bodies.into_iter().enumerate() {
        let index = u32::try_from(index).map_err(|_| ExportMaterializationRefusalV1::ChunkLimit)?;
        let byte_length =
            u32::try_from(bytes.len()).map_err(|_| ExportMaterializationRefusalV1::InvalidLimit)?;
        chunk_metadata.push(
            ExportChunkV1::new(index, byte_length, digest(&bytes))
                .expect("canonical export chunks are nonempty"),
        );
        chunks.push(
            ExportChunkPayloadV1::new(index, bytes, max_chunk_bytes)
                .expect("materializer constructs a canonical chunk payload"),
        );
    }

    let provisional_manifest = ExportManifestV1::new(
        request,
        profile,
        scope,
        digest_profile,
        source_engine_revision,
        policy_revision,
        included_classes,
        frontiers,
        chunk_metadata.clone(),
        ContentDigestV1::new([0; 32]),
        max_chunks,
    )
    .expect("materializer constructs a canonical manifest");
    let manifest = ExportManifestV1::new(
        request,
        profile,
        scope,
        digest_profile,
        source_engine_revision,
        policy_revision,
        included_classes,
        frontiers,
        chunk_metadata,
        digest(&provisional_manifest.digest_body()),
        max_chunks,
    )
    .expect("materializer constructs a canonical manifest");
    Ok(ExportMaterializationV1 { manifest, chunks })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RetentionTombstoneReasonV1 {
    ProjectionEvicted = 1,
    ReceiptRetired = 2,
    PrivateStateRetired = 3,
    FederationStateRetired = 4,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct RetentionTombstoneV1 {
    pub class: RecordClassV1,
    pub reason: RetentionTombstoneReasonV1,
    pub policy_revision: RetentionPolicyRevisionV1,
    pub transition_engine_revision: u64,
    identity_binding: RetentionIdentityBindingV1,
    scope_binding: RetentionScopeBindingV1,
}

impl RetentionTombstoneV1 {
    pub fn new(
        class: RecordClassV1,
        reason: RetentionTombstoneReasonV1,
        policy_revision: RetentionPolicyRevisionV1,
        transition_engine_revision: u64,
        identity_binding: RetentionIdentityBindingV1,
        scope_binding: RetentionScopeBindingV1,
    ) -> Self {
        Self {
            class,
            reason,
            policy_revision,
            transition_engine_revision,
            identity_binding,
            scope_binding,
        }
    }

    pub fn identity_binding(self) -> RetentionIdentityBindingV1 {
        self.identity_binding
    }

    pub fn scope_binding(self) -> RetentionScopeBindingV1 {
        self.scope_binding
    }
}

impl fmt::Debug for RetentionTombstoneV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetentionTombstoneV1")
            .field("class", &self.class)
            .field("reason", &self.reason)
            .field("policy_revision", &self.policy_revision)
            .field(
                "transition_engine_revision",
                &self.transition_engine_revision,
            )
            .field("identity_binding", &"<redacted>")
            .field("scope_binding", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiptRetirementApplyV1 {
    Retired,
    Replay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiptRetirementRefusalV1 {
    InvalidLimit,
    WrongClass,
    WrongReason,
    Conflict,
    Capacity,
    NonCanonicalCheckpoint,
}

/// Bounded exact tombstones for request receipts removed from replay storage.
///
/// Membership means the same identity/scope pair must never execute again. It
/// does not mean any payload, backup, or remote copy was erased.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceiptRetirementRegistryV1 {
    maximum: usize,
    retired: BTreeMap<(RetentionIdentityBindingV1, RetentionScopeBindingV1), RetentionTombstoneV1>,
}

impl ReceiptRetirementRegistryV1 {
    pub fn new(maximum: usize) -> Result<Self, ReceiptRetirementRefusalV1> {
        if maximum == 0 {
            return Err(ReceiptRetirementRefusalV1::InvalidLimit);
        }
        Ok(Self {
            maximum,
            retired: BTreeMap::new(),
        })
    }

    pub fn recover(
        maximum: usize,
        checkpoint: Vec<RetentionTombstoneV1>,
    ) -> Result<Self, ReceiptRetirementRefusalV1> {
        let mut registry = Self::new(maximum)?;
        if checkpoint.len() > maximum {
            return Err(ReceiptRetirementRefusalV1::NonCanonicalCheckpoint);
        }
        let mut prior_key = None;
        for tombstone in checkpoint {
            let key = (tombstone.identity_binding, tombstone.scope_binding);
            if prior_key.is_some_and(|prior| prior >= key)
                || registry.retire(tombstone) != Ok(ReceiptRetirementApplyV1::Retired)
            {
                return Err(ReceiptRetirementRefusalV1::NonCanonicalCheckpoint);
            }
            prior_key = Some(key);
        }
        Ok(registry)
    }

    pub fn retire(
        &mut self,
        tombstone: RetentionTombstoneV1,
    ) -> Result<ReceiptRetirementApplyV1, ReceiptRetirementRefusalV1> {
        if tombstone.class != RecordClassV1::RequestReceipts {
            return Err(ReceiptRetirementRefusalV1::WrongClass);
        }
        if tombstone.reason != RetentionTombstoneReasonV1::ReceiptRetired {
            return Err(ReceiptRetirementRefusalV1::WrongReason);
        }
        let key = (tombstone.identity_binding, tombstone.scope_binding);
        if let Some(retained) = self.retired.get(&key) {
            return if retained == &tombstone {
                Ok(ReceiptRetirementApplyV1::Replay)
            } else {
                Err(ReceiptRetirementRefusalV1::Conflict)
            };
        }
        if self.retired.len() == self.maximum {
            return Err(ReceiptRetirementRefusalV1::Capacity);
        }
        self.retired.insert(key, tombstone);
        Ok(ReceiptRetirementApplyV1::Retired)
    }

    pub fn is_retired(
        &self,
        identity: RetentionIdentityBindingV1,
        scope: RetentionScopeBindingV1,
    ) -> bool {
        self.retired.contains_key(&(identity, scope))
    }

    pub fn checkpoint(&self) -> Vec<RetentionTombstoneV1> {
        self.retired.values().copied().collect()
    }

    pub fn len(&self) -> usize {
        self.retired.len()
    }

    pub fn is_empty(&self) -> bool {
        self.retired.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreReclamationPlanV1 {
    pub source_engine_revision: u64,
    pub source_store_revision: u64,
    pub replacement_store_revision: u64,
    pub replacement_checkpoint_digest: ContentDigestV1,
    obsolete_generations: Vec<StoreGenerationIdV1>,
}

impl StoreReclamationPlanV1 {
    pub fn new(
        source_engine_revision: u64,
        source_store_revision: u64,
        replacement_store_revision: u64,
        replacement_checkpoint_digest: ContentDigestV1,
        obsolete_generations: Vec<StoreGenerationIdV1>,
        max_generations: usize,
    ) -> Option<Self> {
        let mut canonical = obsolete_generations.clone();
        canonical.sort_unstable();
        canonical.dedup();
        (source_store_revision != 0
            && replacement_store_revision > source_store_revision
            && !obsolete_generations.is_empty()
            && obsolete_generations.len() <= max_generations
            && canonical == obsolete_generations)
            .then_some(Self {
                source_engine_revision,
                source_store_revision,
                replacement_store_revision,
                replacement_checkpoint_digest,
                obsolete_generations,
            })
    }

    pub fn obsolete_generations(&self) -> &[StoreGenerationIdV1] {
        &self.obsolete_generations
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ReclamationStageV1 {
    NotAttempted = 1,
    LogicallyUnreachable = 2,
    GenerationUnlinked = 3,
    ProviderAttested = 4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReclamationEvidenceV1 {
    pub generation: StoreGenerationIdV1,
    pub replacement_store_revision: u64,
    pub stage: ReclamationStageV1,
}

impl ReclamationEvidenceV1 {
    pub fn new(
        plan: &StoreReclamationPlanV1,
        generation: StoreGenerationIdV1,
        stage: ReclamationStageV1,
    ) -> Option<Self> {
        plan.obsolete_generations
            .binary_search(&generation)
            .is_ok()
            .then_some(Self {
                generation,
                replacement_store_revision: plan.replacement_store_revision,
                stage,
            })
    }

    pub const fn proves_secure_erasure(self) -> bool {
        false
    }
}
