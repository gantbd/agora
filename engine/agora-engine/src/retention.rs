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
    identity_binding: ContentDigestV1,
    scope_binding: ContentDigestV1,
}

impl RetentionTombstoneV1 {
    pub fn new(
        class: RecordClassV1,
        reason: RetentionTombstoneReasonV1,
        policy_revision: RetentionPolicyRevisionV1,
        transition_engine_revision: u64,
        identity_binding: ContentDigestV1,
        scope_binding: ContentDigestV1,
    ) -> Option<Self> {
        (transition_engine_revision != 0).then_some(Self {
            class,
            reason,
            policy_revision,
            transition_engine_revision,
            identity_binding,
            scope_binding,
        })
    }

    pub fn identity_binding(self) -> ContentDigestV1 {
        self.identity_binding
    }

    pub fn scope_binding(self) -> ContentDigestV1 {
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
    retired: BTreeMap<(ContentDigestV1, ContentDigestV1), RetentionTombstoneV1>,
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

    pub fn is_retired(&self, identity: ContentDigestV1, scope: ContentDigestV1) -> bool {
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
