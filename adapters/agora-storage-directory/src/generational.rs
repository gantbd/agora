//! Reopen-gated generational checkpoint storage and bounded reclamation.
//!
//! Every committed revision has an immutable generation file. Head publication
//! retains prior generations. Reclamation is available only from an owner that
//! reopened and validated the exact current head, records the bounded plan
//! before unlinking, and directory-syncs the removals before returning
//! `GenerationUnlinked` evidence. No outcome proves secure erasure.

use super::{
    Stamp, crc32c, encode_record, open_private_directory, path_exists_no_follow, read_record,
    refuse_symlink, validate_private_directory, validate_private_file,
};
use agora_engine::retention::{
    ContentDigestV1, ReclamationEvidenceV1, ReclamationStageV1, StoreGenerationIdV1,
    StoreReclamationPlanV1,
};
use keystone_storage::{
    AcknowledgementStrength, Checkpoint, CheckpointStore, CommitOutcome, CommitReceipt, Revision,
    StoreRefusal,
};
use std::cell::Cell;
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

const HEAD: &str = "checkpoint.bin";
const LOCK: &str = ".owner.lock";
const PENDING: &str = ".pending";
const RECLAMATION: &str = ".reclamation";
const RECLAMATION_PENDING: &str = ".reclamation.pending";
const RECLAMATION_MAGIC: &[u8; 8] = b"AGGREC1\0";
const RECLAMATION_VERSION: u8 = 1;
const GENERATION_PREFIX: &[u8; 8] = b"AGGENV1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationalDirectoryLimitsV1 {
    pub max_key_bytes: usize,
    pub max_value_bytes: usize,
    pub max_live_generations: usize,
    pub max_reclamation_receipts: usize,
    pub max_generations_per_reclamation: usize,
}

impl Default for GenerationalDirectoryLimitsV1 {
    fn default() -> Self {
        Self {
            max_key_bytes: 1_024,
            max_value_bytes: 8 * 1_024 * 1_024,
            max_live_generations: 8,
            max_reclamation_receipts: 64,
            max_generations_per_reclamation: 8,
        }
    }
}

impl GenerationalDirectoryLimitsV1 {
    fn valid(self) -> bool {
        self.max_key_bytes != 0
            && self.max_value_bytes != 0
            && self.max_live_generations >= 3
            && self.max_reclamation_receipts != 0
            && self.max_reclamation_receipts <= u32::MAX as usize
            && self.max_generations_per_reclamation != 0
            && self.max_generations_per_reclamation <= u32::MAX as usize
            && self.max_generations_per_reclamation <= self.max_live_generations
    }

    fn record_limits(self) -> super::DirectoryLimitsV1 {
        super::DirectoryLimitsV1 {
            max_key_bytes: self.max_key_bytes,
            max_value_bytes: self.max_value_bytes,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenerationalDirectoryErrorV1 {
    InvalidLimits,
    AlreadyExists,
    Empty,
    Busy,
    UnsafeResource,
    LostResource,
    CrashResidue,
    Corrupt,
    Capacity,
    Fenced,
    Io(std::io::ErrorKind),
}

impl From<super::DirectoryErrorV1> for GenerationalDirectoryErrorV1 {
    fn from(value: super::DirectoryErrorV1) -> Self {
        match value {
            super::DirectoryErrorV1::InvalidLimits => Self::InvalidLimits,
            super::DirectoryErrorV1::AlreadyExists => Self::AlreadyExists,
            super::DirectoryErrorV1::Empty => Self::Empty,
            super::DirectoryErrorV1::Busy => Self::Busy,
            super::DirectoryErrorV1::UnsafeResource => Self::UnsafeResource,
            super::DirectoryErrorV1::LostResource => Self::LostResource,
            super::DirectoryErrorV1::CrashResidue => Self::CrashResidue,
            super::DirectoryErrorV1::Corrupt => Self::Corrupt,
            super::DirectoryErrorV1::Capacity => Self::Capacity,
            super::DirectoryErrorV1::Fenced => Self::Fenced,
            super::DirectoryErrorV1::Io(kind) => Self::Io(kind),
        }
    }
}

fn map_io(error: std::io::Error) -> GenerationalDirectoryErrorV1 {
    GenerationalDirectoryErrorV1::Io(error.kind())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenerationalReclamationRefusalV1 {
    InvalidLimit,
    Empty,
    NotReopened,
    NoObsoleteGeneration,
    Stale,
    Conflict,
    Capacity,
    OutcomeUnknown,
    Fenced,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenerationalReclamationApplyV1 {
    Reclaimed,
    Replay,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationalReclamationOutcomeV1 {
    pub apply: GenerationalReclamationApplyV1,
    evidence: Vec<ReclamationEvidenceV1>,
}

impl GenerationalReclamationOutcomeV1 {
    pub fn evidence(&self) -> &[ReclamationEvidenceV1] {
        &self.evidence
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReclamationRecordV1 {
    plan: StoreReclamationPlanV1,
    complete: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReclamationCutV1 {
    PlanPersisted,
    GenerationUnlinked(usize),
    DirectorySynced,
    CompletionPersisted,
}

impl ReclamationRecordV1 {
    fn key(&self) -> (u64, u64) {
        (
            self.plan.replacement_store_revision,
            self.plan.source_store_revision,
        )
    }
}

pub struct GenerationalDirectoryCheckpointStoreV1 {
    path: PathBuf,
    directory: File,
    lock: File,
    root_stamp: Stamp,
    lock_stamp: Stamp,
    head_stamp: Option<Stamp>,
    generation_stamps: BTreeMap<u64, Stamp>,
    reclamation_stamp: Option<Stamp>,
    reclamations: Vec<ReclamationRecordV1>,
    key: Option<String>,
    checkpoint: Option<Checkpoint>,
    reopened_revision: Option<Revision>,
    limits: GenerationalDirectoryLimitsV1,
    fenced: Cell<bool>,
}

impl std::fmt::Debug for GenerationalDirectoryCheckpointStoreV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GenerationalDirectoryCheckpointStoreV1")
            .field(
                "revision",
                &self
                    .checkpoint
                    .as_ref()
                    .map(|checkpoint| checkpoint.revision),
            )
            .field("live_generations", &self.generation_stamps.len())
            .field("reclamation_receipts", &self.reclamations.len())
            .field("reopened_revision", &self.reopened_revision)
            .field("fenced", &self.fenced.get())
            .finish()
    }
}

impl GenerationalDirectoryCheckpointStoreV1 {
    pub fn create_new(
        path: impl AsRef<Path>,
        limits: GenerationalDirectoryLimitsV1,
    ) -> Result<Self, GenerationalDirectoryErrorV1> {
        if !limits.valid() {
            return Err(GenerationalDirectoryErrorV1::InvalidLimits);
        }
        let path = path.as_ref();
        fs::DirBuilder::new()
            .mode(0o700)
            .create(path)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    GenerationalDirectoryErrorV1::AlreadyExists
                } else {
                    map_io(error)
                }
            })?;
        let directory = open_private_directory(path).map_err(GenerationalDirectoryErrorV1::from)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path.join(LOCK))
            .map_err(map_io)?;
        validate_private_file(&lock.metadata().map_err(map_io)?)
            .map_err(GenerationalDirectoryErrorV1::from)?;
        lock.try_lock()
            .map_err(|_| GenerationalDirectoryErrorV1::Busy)?;
        lock.sync_all().map_err(map_io)?;
        directory.sync_all().map_err(map_io)?;
        let path = fs::canonicalize(path).map_err(map_io)?;
        let root_stamp = Stamp::of(&directory.metadata().map_err(map_io)?);
        let lock_stamp = Stamp::of(&lock.metadata().map_err(map_io)?);
        Ok(Self {
            path,
            directory,
            lock,
            root_stamp,
            lock_stamp,
            head_stamp: None,
            generation_stamps: BTreeMap::new(),
            reclamation_stamp: None,
            reclamations: Vec::new(),
            key: None,
            checkpoint: None,
            reopened_revision: None,
            limits,
            fenced: Cell::new(false),
        })
    }

    pub fn open_existing(
        path: impl AsRef<Path>,
        limits: GenerationalDirectoryLimitsV1,
    ) -> Result<Self, GenerationalDirectoryErrorV1> {
        if !limits.valid() {
            return Err(GenerationalDirectoryErrorV1::InvalidLimits);
        }
        let directory =
            open_private_directory(path.as_ref()).map_err(GenerationalDirectoryErrorV1::from)?;
        let path = fs::canonicalize(path).map_err(map_io)?;
        refuse_symlink(&path.join(LOCK)).map_err(GenerationalDirectoryErrorV1::from)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path.join(LOCK))
            .map_err(map_io)?;
        validate_private_file(&lock.metadata().map_err(map_io)?)
            .map_err(GenerationalDirectoryErrorV1::from)?;
        lock.try_lock()
            .map_err(|_| GenerationalDirectoryErrorV1::Busy)?;
        if path_exists_no_follow(&path.join(PENDING)).map_err(GenerationalDirectoryErrorV1::from)?
            || path_exists_no_follow(&path.join(RECLAMATION_PENDING))
                .map_err(GenerationalDirectoryErrorV1::from)?
        {
            return Err(GenerationalDirectoryErrorV1::CrashResidue);
        }
        let head_path = path.join(HEAD);
        let (key, checkpoint, head_stamp) =
            if path_exists_no_follow(&head_path).map_err(GenerationalDirectoryErrorV1::from)? {
                let (key, checkpoint, stamp) = read_record(&head_path, limits.record_limits())
                    .map_err(GenerationalDirectoryErrorV1::from)?;
                (Some(key), Some(checkpoint), Some(stamp))
            } else {
                (None, None, None)
            };
        let (generation_stamps, reclamation_stamp) =
            scan_resources(&path, key.as_deref(), checkpoint.as_ref(), limits)?;
        let reclamations = if let Some(stamp) = reclamation_stamp {
            let (records, actual) = read_reclamations(&path.join(RECLAMATION), limits)?;
            if actual != stamp {
                return Err(GenerationalDirectoryErrorV1::LostResource);
            }
            records
        } else {
            Vec::new()
        };
        validate_reclamation_state(&reclamations, &generation_stamps, checkpoint.as_ref())?;
        let reopened_revision = checkpoint.as_ref().map(|value| value.revision);
        let root_stamp = Stamp::of(&directory.metadata().map_err(map_io)?);
        let lock_stamp = Stamp::of(&lock.metadata().map_err(map_io)?);
        Ok(Self {
            path,
            directory,
            lock,
            root_stamp,
            lock_stamp,
            head_stamp,
            generation_stamps,
            reclamation_stamp,
            reclamations,
            key,
            checkpoint,
            reopened_revision,
            limits,
            fenced: Cell::new(false),
        })
    }

    pub fn prepare_reclamation<D>(
        &mut self,
        source_engine_revision: u64,
        maximum: usize,
        mut digest: D,
    ) -> Result<StoreReclamationPlanV1, GenerationalReclamationRefusalV1>
    where
        D: FnMut(&[u8]) -> ContentDigestV1,
    {
        if maximum == 0 || maximum > self.limits.max_generations_per_reclamation {
            return Err(GenerationalReclamationRefusalV1::InvalidLimit);
        }
        self.check_resources()
            .map_err(|_| GenerationalReclamationRefusalV1::Fenced)?;
        if self.reclamations.iter().any(|record| !record.complete) {
            return Err(GenerationalReclamationRefusalV1::Conflict);
        }
        let checkpoint = self
            .checkpoint
            .as_ref()
            .ok_or(GenerationalReclamationRefusalV1::Empty)?;
        if self.reopened_revision != Some(checkpoint.revision) {
            return Err(GenerationalReclamationRefusalV1::NotReopened);
        }
        let obsolete: Vec<_> = self
            .generation_stamps
            .keys()
            .copied()
            .filter(|revision| *revision < checkpoint.revision.0)
            .take(maximum)
            .collect();
        let source_store_revision = obsolete
            .last()
            .copied()
            .filter(|revision| *revision != 0)
            .ok_or(GenerationalReclamationRefusalV1::NoObsoleteGeneration)?;
        StoreReclamationPlanV1::new(
            source_engine_revision,
            source_store_revision,
            checkpoint.revision.0,
            digest(&checkpoint.bytes),
            obsolete.into_iter().map(generation_id).collect(),
            self.limits.max_generations_per_reclamation,
        )
        .ok_or(GenerationalReclamationRefusalV1::Conflict)
    }

    pub fn reclaim<D>(
        &mut self,
        plan: StoreReclamationPlanV1,
        digest: D,
    ) -> Result<GenerationalReclamationOutcomeV1, GenerationalReclamationRefusalV1>
    where
        D: FnMut(&[u8]) -> ContentDigestV1,
    {
        self.reclaim_with_cut(plan, digest, |_| false)
    }

    fn reclaim_with_cut<D, C>(
        &mut self,
        plan: StoreReclamationPlanV1,
        mut digest: D,
        mut cut: C,
    ) -> Result<GenerationalReclamationOutcomeV1, GenerationalReclamationRefusalV1>
    where
        D: FnMut(&[u8]) -> ContentDigestV1,
        C: FnMut(ReclamationCutV1) -> bool,
    {
        self.check_resources()
            .map_err(|_| GenerationalReclamationRefusalV1::Fenced)?;
        let checkpoint = self
            .checkpoint
            .as_ref()
            .ok_or(GenerationalReclamationRefusalV1::Empty)?;
        if self.reopened_revision != Some(checkpoint.revision) {
            return Err(GenerationalReclamationRefusalV1::NotReopened);
        }
        if plan.replacement_store_revision != checkpoint.revision.0
            || plan.replacement_checkpoint_digest != digest(&checkpoint.bytes)
        {
            return Err(GenerationalReclamationRefusalV1::Stale);
        }
        let revisions = plan_revisions(&plan)?;
        if revisions.last().copied() != Some(plan.source_store_revision)
            || revisions
                .iter()
                .any(|revision| *revision >= checkpoint.revision.0)
        {
            return Err(GenerationalReclamationRefusalV1::Conflict);
        }
        let existing = self
            .reclamations
            .iter()
            .position(|record| record.plan == plan);
        if let Some(index) = existing {
            if self.reclamations[index].complete {
                if revisions
                    .iter()
                    .any(|revision| self.generation_stamps.contains_key(revision))
                {
                    self.fenced.set(true);
                    return Err(GenerationalReclamationRefusalV1::Fenced);
                }
                return Ok(reclamation_outcome(
                    &plan,
                    GenerationalReclamationApplyV1::Replay,
                ));
            }
        } else {
            if self.reclamations.len() == self.limits.max_reclamation_receipts {
                return Err(GenerationalReclamationRefusalV1::Capacity);
            }
            if self.reclamations.iter().any(|record| {
                record.key() == (plan.replacement_store_revision, plan.source_store_revision)
            }) || revisions
                .iter()
                .any(|revision| !self.generation_stamps.contains_key(revision))
            {
                return Err(GenerationalReclamationRefusalV1::Conflict);
            }
            let mut candidate = self.reclamations.clone();
            candidate.push(ReclamationRecordV1 {
                plan: plan.clone(),
                complete: false,
            });
            candidate.sort_by_key(ReclamationRecordV1::key);
            self.persist_reclamations(&candidate)?;
            self.reclamations = candidate;
            self.interrupt_reclamation(&mut cut, ReclamationCutV1::PlanPersisted)?;
        }

        for (index, revision) in revisions.iter().enumerate() {
            if self.generation_stamps.contains_key(revision) {
                if fs::remove_file(self.path.join(generation_name(*revision))).is_err() {
                    self.fenced.set(true);
                    return Err(GenerationalReclamationRefusalV1::OutcomeUnknown);
                }
                self.generation_stamps.remove(revision);
                self.interrupt_reclamation(&mut cut, ReclamationCutV1::GenerationUnlinked(index))?;
            }
        }
        if self.directory.sync_all().is_err() {
            self.fenced.set(true);
            return Err(GenerationalReclamationRefusalV1::OutcomeUnknown);
        }
        self.interrupt_reclamation(&mut cut, ReclamationCutV1::DirectorySynced)?;
        let mut candidate = self.reclamations.clone();
        let record = candidate
            .iter_mut()
            .find(|record| record.plan == plan)
            .expect("persisted reclamation remains indexed");
        record.complete = true;
        self.persist_reclamations(&candidate)?;
        self.reclamations = candidate;
        self.interrupt_reclamation(&mut cut, ReclamationCutV1::CompletionPersisted)?;
        Ok(reclamation_outcome(
            &plan,
            GenerationalReclamationApplyV1::Reclaimed,
        ))
    }

    pub fn live_generation_count(&self) -> usize {
        self.generation_stamps.len()
    }

    fn interrupt_reclamation<C>(
        &self,
        cut: &mut C,
        point: ReclamationCutV1,
    ) -> Result<(), GenerationalReclamationRefusalV1>
    where
        C: FnMut(ReclamationCutV1) -> bool,
    {
        if cut(point) {
            self.fenced.set(true);
            Err(GenerationalReclamationRefusalV1::OutcomeUnknown)
        } else {
            Ok(())
        }
    }

    #[cfg(test)]
    fn reclaim_interrupted_at<D>(
        &mut self,
        plan: StoreReclamationPlanV1,
        digest: D,
        target: ReclamationCutV1,
    ) -> Result<GenerationalReclamationOutcomeV1, GenerationalReclamationRefusalV1>
    where
        D: FnMut(&[u8]) -> ContentDigestV1,
    {
        self.reclaim_with_cut(plan, digest, |point| point == target)
    }

    fn check_resources(&self) -> Result<(), GenerationalDirectoryErrorV1> {
        if self.fenced.get() {
            return Err(GenerationalDirectoryErrorV1::Fenced);
        }
        if self.validate_resources().is_err() {
            self.fenced.set(true);
            return Err(GenerationalDirectoryErrorV1::LostResource);
        }
        Ok(())
    }

    fn validate_resources(&self) -> Result<(), GenerationalDirectoryErrorV1> {
        let root = fs::symlink_metadata(&self.path)
            .map_err(|_| GenerationalDirectoryErrorV1::LostResource)?;
        validate_private_directory(&root).map_err(GenerationalDirectoryErrorV1::from)?;
        if !self.root_stamp.same_object(Stamp::of(&root))
            || !self
                .root_stamp
                .same_object(Stamp::of(&self.directory.metadata().map_err(map_io)?))
        {
            return Err(GenerationalDirectoryErrorV1::LostResource);
        }
        let lock = fs::symlink_metadata(self.path.join(LOCK))
            .map_err(|_| GenerationalDirectoryErrorV1::LostResource)?;
        validate_private_file(&lock).map_err(GenerationalDirectoryErrorV1::from)?;
        if Stamp::of(&lock) != self.lock_stamp
            || Stamp::of(&self.lock.metadata().map_err(map_io)?) != self.lock_stamp
        {
            return Err(GenerationalDirectoryErrorV1::LostResource);
        }
        validate_optional_stamp(&self.path.join(HEAD), self.head_stamp)?;
        validate_optional_stamp(&self.path.join(RECLAMATION), self.reclamation_stamp)?;
        for (revision, stamp) in &self.generation_stamps {
            validate_optional_stamp(&self.path.join(generation_name(*revision)), Some(*stamp))?;
        }
        if path_exists_no_follow(&self.path.join(PENDING))
            .map_err(GenerationalDirectoryErrorV1::from)?
            || path_exists_no_follow(&self.path.join(RECLAMATION_PENDING))
                .map_err(GenerationalDirectoryErrorV1::from)?
        {
            return Err(GenerationalDirectoryErrorV1::CrashResidue);
        }
        let (generations, reclamation) = scan_resources(
            &self.path,
            self.key.as_deref(),
            self.checkpoint.as_ref(),
            self.limits,
        )?;
        if generations != self.generation_stamps || reclamation != self.reclamation_stamp {
            return Err(GenerationalDirectoryErrorV1::LostResource);
        }
        Ok(())
    }

    fn persist_reclamations(
        &mut self,
        records: &[ReclamationRecordV1],
    ) -> Result<(), GenerationalReclamationRefusalV1> {
        let bytes = encode_reclamations(records, self.limits)
            .map_err(|_| GenerationalReclamationRefusalV1::Capacity)?;
        let pending_path = self.path.join(RECLAMATION_PENDING);
        let mut pending = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&pending_path)
        {
            Ok(file) => file,
            Err(_) => {
                self.fenced.set(true);
                return Err(GenerationalReclamationRefusalV1::OutcomeUnknown);
            }
        };
        if pending.write_all(&bytes).is_err()
            || pending.sync_all().is_err()
            || fs::rename(&pending_path, self.path.join(RECLAMATION)).is_err()
            || self.directory.sync_all().is_err()
        {
            self.fenced.set(true);
            return Err(GenerationalReclamationRefusalV1::OutcomeUnknown);
        }
        let metadata = fs::symlink_metadata(self.path.join(RECLAMATION)).map_err(|_| {
            self.fenced.set(true);
            GenerationalReclamationRefusalV1::OutcomeUnknown
        })?;
        validate_private_file(&metadata).map_err(|_| {
            self.fenced.set(true);
            GenerationalReclamationRefusalV1::OutcomeUnknown
        })?;
        self.reclamation_stamp = Some(Stamp::of(&metadata));
        Ok(())
    }
}

impl CheckpointStore for GenerationalDirectoryCheckpointStoreV1 {
    type Error = GenerationalDirectoryErrorV1;

    fn load(&self, key: &str) -> Result<Option<Checkpoint>, Self::Error> {
        self.check_resources()?;
        if self.key.as_deref().is_some_and(|stored| stored != key) {
            return Ok(None);
        }
        Ok(self.checkpoint.clone())
    }

    fn compare_exchange(
        &mut self,
        key: &str,
        expected: Option<Revision>,
        bytes: &[u8],
    ) -> Result<CommitOutcome, Self::Error> {
        self.check_resources()?;
        if key.is_empty() {
            return Ok(CommitOutcome::Refused(StoreRefusal::InvalidKey));
        }
        if key.len() > self.limits.max_key_bytes {
            return Ok(CommitOutcome::Refused(StoreRefusal::KeyLimit));
        }
        if bytes.len() > self.limits.max_value_bytes {
            return Ok(CommitOutcome::Refused(StoreRefusal::ValueLimit));
        }
        if self.key.as_deref().is_some_and(|stored| stored != key) {
            return Ok(CommitOutcome::Refused(StoreRefusal::InvalidKey));
        }
        if self.reclamations.iter().any(|record| !record.complete) {
            return Ok(CommitOutcome::Refused(StoreRefusal::RevisionConflict {
                expected,
                actual: self.checkpoint.as_ref().map(|value| value.revision),
            }));
        }
        let actual = self.checkpoint.as_ref().map(|value| value.revision);
        if actual != expected {
            return Ok(CommitOutcome::Refused(StoreRefusal::RevisionConflict {
                expected,
                actual,
            }));
        }
        if self.generation_stamps.len() == self.limits.max_live_generations {
            return Ok(CommitOutcome::Refused(StoreRefusal::EntryLimit));
        }
        let revision = match actual {
            Some(Revision(value)) => match value.checked_add(1) {
                Some(next) => Revision(next),
                None => return Ok(CommitOutcome::Refused(StoreRefusal::RevisionExhausted)),
            },
            None => Revision(0),
        };
        let record = encode_record(key, revision, bytes, self.limits.record_limits())
            .map_err(GenerationalDirectoryErrorV1::from)?;
        let generation_path = self.path.join(generation_name(revision.0));
        let mut generation = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&generation_path)
        {
            Ok(file) => file,
            Err(_) => {
                self.fenced.set(true);
                return Ok(CommitOutcome::Indeterminate);
            }
        };
        if generation.write_all(&record).is_err() || generation.sync_all().is_err() {
            self.fenced.set(true);
            return Ok(CommitOutcome::Indeterminate);
        }
        let pending_path = self.path.join(PENDING);
        let mut pending = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&pending_path)
        {
            Ok(file) => file,
            Err(_) => {
                self.fenced.set(true);
                return Ok(CommitOutcome::Indeterminate);
            }
        };
        if pending.write_all(&record).is_err()
            || pending.sync_all().is_err()
            || fs::rename(&pending_path, self.path.join(HEAD)).is_err()
            || self.directory.sync_all().is_err()
        {
            self.fenced.set(true);
            return Ok(CommitOutcome::Indeterminate);
        }
        let Ok(generation_metadata) = fs::symlink_metadata(&generation_path) else {
            self.fenced.set(true);
            return Ok(CommitOutcome::Indeterminate);
        };
        let Ok(head_metadata) = fs::symlink_metadata(self.path.join(HEAD)) else {
            self.fenced.set(true);
            return Ok(CommitOutcome::Indeterminate);
        };
        if validate_private_file(&generation_metadata).is_err()
            || validate_private_file(&head_metadata).is_err()
        {
            self.fenced.set(true);
            return Ok(CommitOutcome::Indeterminate);
        }
        self.generation_stamps
            .insert(revision.0, Stamp::of(&generation_metadata));
        self.head_stamp = Some(Stamp::of(&head_metadata));
        self.key = Some(key.to_owned());
        self.checkpoint = Some(Checkpoint {
            revision,
            bytes: bytes.to_vec(),
            strength: AcknowledgementStrength::Durable,
        });
        Ok(CommitOutcome::Committed(CommitReceipt {
            revision,
            mechanism: "agora-private-generational-directory-v1",
            strength: AcknowledgementStrength::Durable,
        }))
    }
}

fn generation_id(revision: u64) -> StoreGenerationIdV1 {
    let mut bytes = [0_u8; 16];
    bytes[..8].copy_from_slice(GENERATION_PREFIX);
    bytes[8..].copy_from_slice(&revision.to_be_bytes());
    StoreGenerationIdV1::new(bytes).expect("generation domain is nonzero")
}

fn generation_revision(id: StoreGenerationIdV1) -> Option<u64> {
    let bytes = id.as_bytes();
    (bytes[..8] == *GENERATION_PREFIX)
        .then(|| u64::from_be_bytes(bytes[8..].try_into().expect("fixed generation identity")))
}

fn generation_name(revision: u64) -> String {
    format!("generation-{revision:020}.bin")
}

fn parse_generation_name(name: &str) -> Option<u64> {
    let digits = name.strip_prefix("generation-")?.strip_suffix(".bin")?;
    (digits.len() == 20 && digits.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| digits.parse().ok())
        .flatten()
}

fn plan_revisions(
    plan: &StoreReclamationPlanV1,
) -> Result<Vec<u64>, GenerationalReclamationRefusalV1> {
    plan.obsolete_generations()
        .iter()
        .copied()
        .map(|id| generation_revision(id).ok_or(GenerationalReclamationRefusalV1::Conflict))
        .collect()
}

fn reclamation_outcome(
    plan: &StoreReclamationPlanV1,
    apply: GenerationalReclamationApplyV1,
) -> GenerationalReclamationOutcomeV1 {
    let evidence = plan
        .obsolete_generations()
        .iter()
        .map(|generation| {
            ReclamationEvidenceV1::new(plan, *generation, ReclamationStageV1::GenerationUnlinked)
                .expect("plan generation produces evidence")
        })
        .collect();
    GenerationalReclamationOutcomeV1 { apply, evidence }
}

fn validate_optional_stamp(
    path: &Path,
    expected: Option<Stamp>,
) -> Result<(), GenerationalDirectoryErrorV1> {
    match expected {
        Some(expected) => {
            let metadata = fs::symlink_metadata(path)
                .map_err(|_| GenerationalDirectoryErrorV1::LostResource)?;
            validate_private_file(&metadata).map_err(GenerationalDirectoryErrorV1::from)?;
            if Stamp::of(&metadata) != expected {
                return Err(GenerationalDirectoryErrorV1::LostResource);
            }
        }
        None if path_exists_no_follow(path).map_err(GenerationalDirectoryErrorV1::from)? => {
            return Err(GenerationalDirectoryErrorV1::LostResource);
        }
        None => {}
    }
    Ok(())
}

fn scan_resources(
    path: &Path,
    key: Option<&str>,
    checkpoint: Option<&Checkpoint>,
    limits: GenerationalDirectoryLimitsV1,
) -> Result<(BTreeMap<u64, Stamp>, Option<Stamp>), GenerationalDirectoryErrorV1> {
    let mut generations = BTreeMap::new();
    let mut reclamation = None;
    for entry in fs::read_dir(path).map_err(map_io)? {
        let entry = entry.map_err(map_io)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| GenerationalDirectoryErrorV1::UnsafeResource)?;
        if matches!(name.as_str(), LOCK | HEAD) {
            continue;
        }
        if name == RECLAMATION {
            let metadata = fs::symlink_metadata(entry.path()).map_err(map_io)?;
            validate_private_file(&metadata).map_err(GenerationalDirectoryErrorV1::from)?;
            reclamation = Some(Stamp::of(&metadata));
            continue;
        }
        if matches!(name.as_str(), PENDING | RECLAMATION_PENDING) {
            return Err(GenerationalDirectoryErrorV1::CrashResidue);
        }
        let revision =
            parse_generation_name(&name).ok_or(GenerationalDirectoryErrorV1::UnsafeResource)?;
        let (record_key, generation, stamp) = read_record(&entry.path(), limits.record_limits())
            .map_err(GenerationalDirectoryErrorV1::from)?;
        if generation.revision != Revision(revision)
            || key != Some(record_key.as_str())
            || checkpoint.is_none_or(|head| revision > head.revision.0)
        {
            return Err(GenerationalDirectoryErrorV1::Corrupt);
        }
        if generations.len() == limits.max_live_generations
            || generations.insert(revision, stamp).is_some()
        {
            return Err(GenerationalDirectoryErrorV1::Capacity);
        }
        if checkpoint
            .is_some_and(|head| head.revision == generation.revision && head != &generation)
        {
            return Err(GenerationalDirectoryErrorV1::Corrupt);
        }
    }
    if checkpoint.is_some_and(|head| !generations.contains_key(&head.revision.0))
        || checkpoint.is_none() && (!generations.is_empty() || reclamation.is_some())
        || generations.len() > limits.max_live_generations
    {
        return Err(GenerationalDirectoryErrorV1::Corrupt);
    }
    Ok((generations, reclamation))
}

fn validate_reclamation_state(
    records: &[ReclamationRecordV1],
    generations: &BTreeMap<u64, Stamp>,
    checkpoint: Option<&Checkpoint>,
) -> Result<(), GenerationalDirectoryErrorV1> {
    if records
        .windows(2)
        .any(|pair| pair[0].key() >= pair[1].key())
        || records.iter().filter(|record| !record.complete).count() > 1
    {
        return Err(GenerationalDirectoryErrorV1::Corrupt);
    }
    for record in records {
        if checkpoint.is_none_or(|head| record.plan.replacement_store_revision > head.revision.0) {
            return Err(GenerationalDirectoryErrorV1::Corrupt);
        }
        let revisions = record
            .plan
            .obsolete_generations()
            .iter()
            .map(|id| generation_revision(*id).ok_or(GenerationalDirectoryErrorV1::Corrupt))
            .collect::<Result<Vec<_>, _>>()?;
        if record.complete
            && revisions
                .iter()
                .any(|revision| generations.contains_key(revision))
        {
            return Err(GenerationalDirectoryErrorV1::Corrupt);
        }
    }
    Ok(())
}

fn encode_reclamations(
    records: &[ReclamationRecordV1],
    limits: GenerationalDirectoryLimitsV1,
) -> Result<Vec<u8>, GenerationalDirectoryErrorV1> {
    if records.len() > limits.max_reclamation_receipts
        || records
            .windows(2)
            .any(|pair| pair[0].key() >= pair[1].key())
    {
        return Err(GenerationalDirectoryErrorV1::Capacity);
    }
    let maximum = 17_usize
        .checked_add(
            records
                .len()
                .checked_mul(
                    61_usize
                        .checked_add(
                            16_usize
                                .checked_mul(limits.max_generations_per_reclamation)
                                .ok_or(GenerationalDirectoryErrorV1::Capacity)?,
                        )
                        .ok_or(GenerationalDirectoryErrorV1::Capacity)?,
                )
                .ok_or(GenerationalDirectoryErrorV1::Capacity)?,
        )
        .ok_or(GenerationalDirectoryErrorV1::Capacity)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(maximum)
        .map_err(|_| GenerationalDirectoryErrorV1::Capacity)?;
    bytes.extend_from_slice(RECLAMATION_MAGIC);
    bytes.push(RECLAMATION_VERSION);
    bytes.extend_from_slice(&(records.len() as u32).to_be_bytes());
    for record in records {
        bytes.push(u8::from(record.complete));
        bytes.extend_from_slice(&record.plan.source_engine_revision.to_be_bytes());
        bytes.extend_from_slice(&record.plan.source_store_revision.to_be_bytes());
        bytes.extend_from_slice(&record.plan.replacement_store_revision.to_be_bytes());
        bytes.extend_from_slice(&record.plan.replacement_checkpoint_digest.as_bytes());
        bytes.extend_from_slice(&(record.plan.obsolete_generations().len() as u32).to_be_bytes());
        for generation in record.plan.obsolete_generations() {
            bytes.extend_from_slice(&generation.as_bytes());
        }
    }
    let checksum = crc32c(&bytes);
    bytes.extend_from_slice(&checksum.to_be_bytes());
    Ok(bytes)
}

fn read_reclamations(
    path: &Path,
    limits: GenerationalDirectoryLimitsV1,
) -> Result<(Vec<ReclamationRecordV1>, Stamp), GenerationalDirectoryErrorV1> {
    let path_metadata = fs::symlink_metadata(path).map_err(map_io)?;
    validate_private_file(&path_metadata).map_err(GenerationalDirectoryErrorV1::from)?;
    let mut file = OpenOptions::new().read(true).open(path).map_err(map_io)?;
    let metadata = file.metadata().map_err(map_io)?;
    if !Stamp::of(&path_metadata).same_object(Stamp::of(&metadata)) {
        return Err(GenerationalDirectoryErrorV1::LostResource);
    }
    let maximum = 17_usize
        .checked_add(
            limits
                .max_reclamation_receipts
                .checked_mul(
                    61_usize
                        .checked_add(
                            16_usize
                                .checked_mul(limits.max_generations_per_reclamation)
                                .ok_or(GenerationalDirectoryErrorV1::Capacity)?,
                        )
                        .ok_or(GenerationalDirectoryErrorV1::Capacity)?,
                )
                .ok_or(GenerationalDirectoryErrorV1::Capacity)?,
        )
        .ok_or(GenerationalDirectoryErrorV1::Capacity)?;
    let length: usize = metadata
        .len()
        .try_into()
        .map_err(|_| GenerationalDirectoryErrorV1::Capacity)?;
    if length < 17 || length > maximum {
        return Err(GenerationalDirectoryErrorV1::Corrupt);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| GenerationalDirectoryErrorV1::Capacity)?;
    file.read_to_end(&mut bytes).map_err(map_io)?;
    let body_length = bytes
        .len()
        .checked_sub(4)
        .ok_or(GenerationalDirectoryErrorV1::Corrupt)?;
    let encoded_checksum = u32::from_be_bytes(
        bytes[body_length..]
            .try_into()
            .map_err(|_| GenerationalDirectoryErrorV1::Corrupt)?,
    );
    if bytes.get(..8) != Some(RECLAMATION_MAGIC)
        || bytes.get(8) != Some(&RECLAMATION_VERSION)
        || crc32c(&bytes[..body_length]) != encoded_checksum
    {
        return Err(GenerationalDirectoryErrorV1::Corrupt);
    }
    let mut cursor = 9;
    let count = read_u32(&bytes, &mut cursor, body_length)? as usize;
    if count > limits.max_reclamation_receipts {
        return Err(GenerationalDirectoryErrorV1::Capacity);
    }
    let mut records = Vec::new();
    records
        .try_reserve_exact(count)
        .map_err(|_| GenerationalDirectoryErrorV1::Capacity)?;
    for _ in 0..count {
        let complete = match read_byte(&bytes, &mut cursor, body_length)? {
            0 => false,
            1 => true,
            _ => return Err(GenerationalDirectoryErrorV1::Corrupt),
        };
        let source_engine_revision = read_u64(&bytes, &mut cursor, body_length)?;
        let source_store_revision = read_u64(&bytes, &mut cursor, body_length)?;
        let replacement_store_revision = read_u64(&bytes, &mut cursor, body_length)?;
        let digest = ContentDigestV1::new(read_fixed::<32>(&bytes, &mut cursor, body_length)?);
        let generation_count = read_u32(&bytes, &mut cursor, body_length)? as usize;
        if generation_count == 0 || generation_count > limits.max_generations_per_reclamation {
            return Err(GenerationalDirectoryErrorV1::Corrupt);
        }
        let mut generations = Vec::new();
        generations
            .try_reserve_exact(generation_count)
            .map_err(|_| GenerationalDirectoryErrorV1::Capacity)?;
        for _ in 0..generation_count {
            generations.push(
                StoreGenerationIdV1::new(read_fixed::<16>(&bytes, &mut cursor, body_length)?)
                    .ok_or(GenerationalDirectoryErrorV1::Corrupt)?,
            );
        }
        let plan = StoreReclamationPlanV1::new(
            source_engine_revision,
            source_store_revision,
            replacement_store_revision,
            digest,
            generations,
            limits.max_generations_per_reclamation,
        )
        .ok_or(GenerationalDirectoryErrorV1::Corrupt)?;
        records.push(ReclamationRecordV1 { plan, complete });
    }
    if cursor != body_length
        || records
            .windows(2)
            .any(|pair| pair[0].key() >= pair[1].key())
    {
        return Err(GenerationalDirectoryErrorV1::Corrupt);
    }
    Ok((records, Stamp::of(&metadata)))
}

fn read_byte(
    bytes: &[u8],
    cursor: &mut usize,
    end: usize,
) -> Result<u8, GenerationalDirectoryErrorV1> {
    let byte = *bytes
        .get(*cursor)
        .filter(|_| *cursor < end)
        .ok_or(GenerationalDirectoryErrorV1::Corrupt)?;
    *cursor += 1;
    Ok(byte)
}

fn read_fixed<const N: usize>(
    bytes: &[u8],
    cursor: &mut usize,
    end: usize,
) -> Result<[u8; N], GenerationalDirectoryErrorV1> {
    let next = cursor
        .checked_add(N)
        .ok_or(GenerationalDirectoryErrorV1::Corrupt)?;
    if next > end {
        return Err(GenerationalDirectoryErrorV1::Corrupt);
    }
    let value = bytes[*cursor..next]
        .try_into()
        .map_err(|_| GenerationalDirectoryErrorV1::Corrupt)?;
    *cursor = next;
    Ok(value)
}

fn read_u32(
    bytes: &[u8],
    cursor: &mut usize,
    end: usize,
) -> Result<u32, GenerationalDirectoryErrorV1> {
    Ok(u32::from_be_bytes(read_fixed(bytes, cursor, end)?))
}

fn read_u64(
    bytes: &[u8],
    cursor: &mut usize,
    end: usize,
) -> Result<u64, GenerationalDirectoryErrorV1> {
    Ok(u64::from_be_bytes(read_fixed(bytes, cursor, end)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_path() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "agora-generational-resume-{}-{nonce}",
            std::process::id()
        ))
    }

    fn limits() -> GenerationalDirectoryLimitsV1 {
        GenerationalDirectoryLimitsV1 {
            max_key_bytes: 16,
            max_value_bytes: 64,
            max_live_generations: 3,
            max_reclamation_receipts: 2,
            max_generations_per_reclamation: 2,
        }
    }

    fn digest(bytes: &[u8]) -> ContentDigestV1 {
        let mut output = [0_u8; 32];
        output[..bytes.len()].copy_from_slice(bytes);
        ContentDigestV1::new(output)
    }

    #[test]
    fn persisted_plan_resumes_after_partial_unlink_and_fresh_reopen() {
        let path = temporary_path();
        let mut store =
            GenerationalDirectoryCheckpointStoreV1::create_new(&path, limits()).unwrap();
        for (expected, value) in [
            (None, b"zero".as_slice()),
            (Some(Revision(0)), b"one".as_slice()),
            (Some(Revision(1)), b"two".as_slice()),
        ] {
            assert!(matches!(
                store.compare_exchange("owner", expected, value).unwrap(),
                CommitOutcome::Committed(_)
            ));
        }
        drop(store);

        let mut store =
            GenerationalDirectoryCheckpointStoreV1::open_existing(&path, limits()).unwrap();
        let plan = store.prepare_reclamation(7, 2, digest).unwrap();
        let records = vec![ReclamationRecordV1 {
            plan: plan.clone(),
            complete: false,
        }];
        store.persist_reclamations(&records).unwrap();
        store.reclamations = records;
        fs::remove_file(path.join(generation_name(0))).unwrap();
        store.directory.sync_all().unwrap();
        drop(store);

        let mut recovered =
            GenerationalDirectoryCheckpointStoreV1::open_existing(&path, limits()).unwrap();
        assert_eq!(recovered.live_generation_count(), 2);
        let outcome = recovered.reclaim(plan.clone(), digest).unwrap();
        assert_eq!(outcome.apply, GenerationalReclamationApplyV1::Reclaimed);
        assert_eq!(recovered.live_generation_count(), 1);
        drop(recovered);
        let mut recovered =
            GenerationalDirectoryCheckpointStoreV1::open_existing(&path, limits()).unwrap();
        assert_eq!(
            recovered.reclaim(plan, digest).unwrap().apply,
            GenerationalReclamationApplyV1::Replay
        );
        drop(recovered);
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn every_durable_reclamation_transition_recovers_on_fresh_open() {
        for target in [
            ReclamationCutV1::PlanPersisted,
            ReclamationCutV1::GenerationUnlinked(0),
            ReclamationCutV1::GenerationUnlinked(1),
            ReclamationCutV1::DirectorySynced,
            ReclamationCutV1::CompletionPersisted,
        ] {
            let path = temporary_path();
            let mut store =
                GenerationalDirectoryCheckpointStoreV1::create_new(&path, limits()).unwrap();
            for (expected, value) in [
                (None, b"zero".as_slice()),
                (Some(Revision(0)), b"one".as_slice()),
                (Some(Revision(1)), b"two".as_slice()),
            ] {
                assert!(matches!(
                    store.compare_exchange("owner", expected, value).unwrap(),
                    CommitOutcome::Committed(_)
                ));
            }
            drop(store);

            let mut store =
                GenerationalDirectoryCheckpointStoreV1::open_existing(&path, limits()).unwrap();
            let plan = store.prepare_reclamation(7, 2, digest).unwrap();
            assert_eq!(
                store.reclaim_interrupted_at(plan.clone(), digest, target),
                Err(GenerationalReclamationRefusalV1::OutcomeUnknown),
                "cut must remain uncertain at {target:?}"
            );
            assert_eq!(
                store.load("owner"),
                Err(GenerationalDirectoryErrorV1::Fenced),
                "cut owner must remain fenced at {target:?}"
            );
            drop(store);

            let mut recovered =
                GenerationalDirectoryCheckpointStoreV1::open_existing(&path, limits()).unwrap();
            assert_eq!(
                recovered.load("owner").unwrap().unwrap().bytes,
                b"two".to_vec(),
                "head must survive at {target:?}"
            );
            let expected = if target == ReclamationCutV1::CompletionPersisted {
                GenerationalReclamationApplyV1::Replay
            } else {
                GenerationalReclamationApplyV1::Reclaimed
            };
            assert_eq!(
                recovered.reclaim(plan.clone(), digest).unwrap().apply,
                expected,
                "fresh owner must converge at {target:?}"
            );
            assert_eq!(recovered.live_generation_count(), 1);
            drop(recovered);

            let mut replay =
                GenerationalDirectoryCheckpointStoreV1::open_existing(&path, limits()).unwrap();
            assert_eq!(
                replay.reclaim(plan, digest).unwrap().apply,
                GenerationalReclamationApplyV1::Replay
            );
            drop(replay);
            fs::remove_dir_all(path).unwrap();
        }
    }
}
