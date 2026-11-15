//! Exclusive checkpoint register in one private local Unix directory.
//!
//! Publication is temporary-file write, file synchronization, atomic rename,
//! and directory synchronization. `Durable` means that exact mechanism
//! completed; it is not a claim about hardware power-loss behavior. The parent
//! directory and same-UID processes are trusted. Resource replacement, symlinks,
//! permissive modes, crash residue, malformed records, and ambiguous commits
//! fail closed.

#[cfg(not(unix))]
compile_error!("agora-storage-directory currently qualifies only Unix hosts");

use agora_storage_observation::{ObservableCheckpointStoreV1, ObservationFidelityV1};
use keystone_storage::{
    AcknowledgementStrength, Checkpoint, CheckpointStore, CommitOutcome, CommitReceipt, Revision,
    StoreRefusal,
};
use std::cell::{Cell, RefCell};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

pub mod generational;

const HEAD: &str = "checkpoint.bin";
const LOCK: &str = ".owner.lock";
const PENDING: &str = ".pending";
const MAGIC: &[u8; 8] = b"AGDIRCP\x01";
const HEADER_BYTES: usize = 32;

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TestCrashPoint {
    PendingCreated,
    PendingWritten,
    PendingSynced,
    HeadRenamed,
    DirectorySynced,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BackupInterruptionV1 {
    None,
    ExportCreated,
    ExportWritten,
    ExportFileSynced,
    ExportDirectorySynced,
    RestoreDirectoryCreated,
    RestorePendingCreated,
    RestorePendingWritten,
    RestorePendingSynced,
    RestoreHeadRenamed,
    RestoreDirectorySynced,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectoryLimitsV1 {
    pub max_key_bytes: usize,
    pub max_value_bytes: usize,
}

impl Default for DirectoryLimitsV1 {
    fn default() -> Self {
        Self {
            max_key_bytes: 1_024,
            max_value_bytes: 8 * 1_024 * 1_024,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectoryErrorV1 {
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

fn io(error: std::io::Error) -> DirectoryErrorV1 {
    DirectoryErrorV1::Io(error.kind())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectoryOwnerStateV1 {
    Active,
    Fenced,
}

/// Content-free evidence for one canonical offline backup record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectoryBackupSummaryV1 {
    pub revision: Revision,
}

/// Passive, content-free facts from the qualified physical store owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectoryObservationV1 {
    pub state: DirectoryOwnerStateV1,
    /// The last revision acknowledged by this owner, if one exists.
    pub revision: Option<u64>,
    pub loads: u64,
    pub offers: u64,
    pub commits: u64,
    pub refusals: u64,
    pub indeterminate: u64,
    pub resource_fences: u64,
    pub saturated: bool,
}

impl ObservationFidelityV1 for DirectoryObservationV1 {
    fn fidelity_lost(&self) -> bool {
        self.saturated
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DirectoryCountersV1 {
    loads: u64,
    offers: u64,
    commits: u64,
    refusals: u64,
    indeterminate: u64,
    resource_fences: u64,
    saturated: bool,
}

fn increment(value: u64) -> (u64, bool) {
    value
        .checked_add(1)
        .map_or((value, true), |next| (next, false))
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct Stamp {
    device: u64,
    inode: u64,
    length: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}

impl Stamp {
    fn of(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            length: metadata.len(),
            modified: (metadata.mtime(), metadata.mtime_nsec()),
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        }
    }

    fn same_object(self, other: Self) -> bool {
        self.device == other.device && self.inode == other.inode
    }
}

pub struct DirectoryCheckpointStoreV1 {
    path: PathBuf,
    directory: File,
    lock: File,
    root_stamp: Stamp,
    lock_stamp: Stamp,
    head_stamp: Option<Stamp>,
    key: Option<String>,
    checkpoint: Option<Checkpoint>,
    limits: DirectoryLimitsV1,
    fenced: Cell<bool>,
    counters: RefCell<DirectoryCountersV1>,
    #[cfg(test)]
    crash_point: Cell<Option<TestCrashPoint>>,
    #[cfg(test)]
    pending_open_error: Cell<Option<std::io::ErrorKind>>,
}

impl std::fmt::Debug for DirectoryCheckpointStoreV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DirectoryCheckpointStoreV1")
            .field("observation", &self.observation())
            .finish_non_exhaustive()
    }
}

impl DirectoryCheckpointStoreV1 {
    pub fn create_new(
        path: impl AsRef<Path>,
        limits: DirectoryLimitsV1,
    ) -> Result<Self, DirectoryErrorV1> {
        validate_limits(limits)?;
        let path = path.as_ref();
        fs::DirBuilder::new()
            .mode(0o700)
            .create(path)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    DirectoryErrorV1::AlreadyExists
                } else {
                    io(error)
                }
            })?;
        let directory = open_private_directory(path)?;
        let lock_path = path.join(LOCK);
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&lock_path)
            .map_err(io)?;
        validate_private_file(&lock.metadata().map_err(io)?)?;
        lock.try_lock().map_err(|_| DirectoryErrorV1::Busy)?;
        lock.sync_all().map_err(io)?;
        directory.sync_all().map_err(io)?;
        let absolute = fs::canonicalize(path).map_err(io)?;
        let root_stamp = Stamp::of(&directory.metadata().map_err(io)?);
        let lock_stamp = Stamp::of(&lock.metadata().map_err(io)?);
        Ok(Self {
            path: absolute,
            directory,
            lock,
            root_stamp,
            lock_stamp,
            head_stamp: None,
            key: None,
            checkpoint: None,
            limits,
            fenced: Cell::new(false),
            counters: RefCell::new(DirectoryCountersV1::default()),
            #[cfg(test)]
            crash_point: Cell::new(None),
            #[cfg(test)]
            pending_open_error: Cell::new(None),
        })
    }

    pub fn open_existing(
        path: impl AsRef<Path>,
        limits: DirectoryLimitsV1,
    ) -> Result<Self, DirectoryErrorV1> {
        validate_limits(limits)?;
        let path = path.as_ref();
        let directory = open_private_directory(path)?;
        let absolute = fs::canonicalize(path).map_err(io)?;
        let lock_path = absolute.join(LOCK);
        refuse_symlink(&lock_path)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .map_err(io)?;
        validate_private_file(&lock.metadata().map_err(io)?)?;
        lock.try_lock().map_err(|_| DirectoryErrorV1::Busy)?;
        if path_exists_no_follow(&absolute.join(PENDING))? {
            return Err(DirectoryErrorV1::CrashResidue);
        }
        let root_stamp = Stamp::of(&directory.metadata().map_err(io)?);
        let lock_stamp = Stamp::of(&lock.metadata().map_err(io)?);
        let head_path = absolute.join(HEAD);
        let (key, checkpoint, head_stamp) = if path_exists_no_follow(&head_path)? {
            refuse_symlink(&head_path)?;
            let (key, checkpoint, stamp) = read_record(&head_path, limits)?;
            (Some(key), Some(checkpoint), Some(stamp))
        } else {
            (None, None, None)
        };
        Ok(Self {
            path: absolute,
            directory,
            lock,
            root_stamp,
            lock_stamp,
            head_stamp,
            key,
            checkpoint,
            limits,
            fenced: Cell::new(false),
            counters: RefCell::new(DirectoryCountersV1::default()),
            #[cfg(test)]
            crash_point: Cell::new(None),
            #[cfg(test)]
            pending_open_error: Cell::new(None),
        })
    }

    pub fn observation(&self) -> DirectoryObservationV1 {
        let counters = *self.counters.borrow();
        DirectoryObservationV1 {
            state: if self.fenced.get() {
                DirectoryOwnerStateV1::Fenced
            } else {
                DirectoryOwnerStateV1::Active
            },
            revision: self
                .checkpoint
                .as_ref()
                .map(|checkpoint| checkpoint.revision.0),
            loads: counters.loads,
            offers: counters.offers,
            commits: counters.commits,
            refusals: counters.refusals,
            indeterminate: counters.indeterminate,
            resource_fences: counters.resource_fences,
            saturated: counters.saturated,
        }
    }

    fn check_resources(&self) -> Result<(), DirectoryErrorV1> {
        if self.fenced.get() {
            return Err(DirectoryErrorV1::Fenced);
        }
        match self.validate_resources() {
            Ok(()) => Ok(()),
            Err(error) => self.fence(error),
        }
    }

    fn validate_resources(&self) -> Result<(), DirectoryErrorV1> {
        let root = fs::symlink_metadata(&self.path).map_err(|_| DirectoryErrorV1::LostResource)?;
        validate_private_directory(&root)?;
        if !self.root_stamp.same_object(Stamp::of(&root))
            || !self
                .root_stamp
                .same_object(Stamp::of(&self.directory.metadata().map_err(io)?))
        {
            return Err(DirectoryErrorV1::LostResource);
        }
        let lock = fs::symlink_metadata(self.path.join(LOCK))
            .map_err(|_| DirectoryErrorV1::LostResource)?;
        validate_private_file(&lock)?;
        if Stamp::of(&lock) != self.lock_stamp
            || Stamp::of(&self.lock.metadata().map_err(io)?) != self.lock_stamp
        {
            return Err(DirectoryErrorV1::LostResource);
        }
        let head_path = self.path.join(HEAD);
        match self.head_stamp {
            Some(expected) => {
                let actual =
                    fs::symlink_metadata(head_path).map_err(|_| DirectoryErrorV1::LostResource)?;
                validate_private_file(&actual)?;
                if Stamp::of(&actual) != expected {
                    return Err(DirectoryErrorV1::LostResource);
                }
            }
            None if path_exists_no_follow(&head_path)? => {
                return Err(DirectoryErrorV1::LostResource);
            }
            None => {}
        }
        if path_exists_no_follow(&self.path.join(PENDING))? {
            return Err(DirectoryErrorV1::CrashResidue);
        }
        Ok(())
    }

    fn fence<T>(&self, error: DirectoryErrorV1) -> Result<T, DirectoryErrorV1> {
        self.fenced.set(true);
        let mut counters = self.counters.borrow_mut();
        let (resource_fences, saturated) = increment(counters.resource_fences);
        counters.resource_fences = resource_fences;
        counters.saturated |= saturated;
        Err(error)
    }

    fn indeterminate(&self) -> Result<CommitOutcome, DirectoryErrorV1> {
        self.fenced.set(true);
        let mut counters = self.counters.borrow_mut();
        let (indeterminate, saturated) = increment(counters.indeterminate);
        counters.indeterminate = indeterminate;
        counters.saturated |= saturated;
        Ok(CommitOutcome::Indeterminate)
    }

    fn refused(&self, refusal: StoreRefusal) -> Result<CommitOutcome, DirectoryErrorV1> {
        let mut counters = self.counters.borrow_mut();
        let (refusals, saturated) = increment(counters.refusals);
        counters.refusals = refusals;
        counters.saturated |= saturated;
        Ok(CommitOutcome::Refused(refusal))
    }

    #[cfg(test)]
    fn crash_at(&self, point: TestCrashPoint) -> bool {
        self.crash_point.get() == Some(point)
    }

    fn open_pending(&self, path: &Path) -> std::io::Result<File> {
        #[cfg(test)]
        if let Some(kind) = self.pending_open_error.take() {
            return Err(std::io::Error::from(kind));
        }
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
    }
}

/// Exports the exact current canonical record while holding the store's
/// exclusive ownership lock.
///
/// The destination must not exist and its parent must already be a private
/// mode-0700 directory. The resulting mode-0600 file contains checkpoint
/// content and therefore requires the same confidentiality controls as the
/// store itself.
pub fn export_directory_backup_v1(
    store_path: impl AsRef<Path>,
    backup_path: impl AsRef<Path>,
    limits: DirectoryLimitsV1,
) -> Result<DirectoryBackupSummaryV1, DirectoryErrorV1> {
    export_directory_backup_inner(
        store_path.as_ref(),
        backup_path.as_ref(),
        limits,
        BackupInterruptionV1::None,
    )
}

fn export_directory_backup_inner(
    store_path: &Path,
    backup_path: &Path,
    limits: DirectoryLimitsV1,
    interruption: BackupInterruptionV1,
) -> Result<DirectoryBackupSummaryV1, DirectoryErrorV1> {
    let store = DirectoryCheckpointStoreV1::open_existing(store_path, limits)?;
    store.check_resources()?;
    let key = store.key.as_deref().ok_or(DirectoryErrorV1::Empty)?;
    let checkpoint = store.checkpoint.as_ref().ok_or(DirectoryErrorV1::Empty)?;
    let record = encode_record(key, checkpoint.revision, &checkpoint.bytes, limits)?;
    write_new_private_file(backup_path, &record, interruption)?;
    Ok(DirectoryBackupSummaryV1 {
        revision: checkpoint.revision,
    })
}

/// Validates one canonical private backup without constructing a store owner.
pub fn verify_directory_backup_v1(
    backup_path: impl AsRef<Path>,
    limits: DirectoryLimitsV1,
) -> Result<DirectoryBackupSummaryV1, DirectoryErrorV1> {
    validate_limits(limits)?;
    let (_, checkpoint, _) = read_record(backup_path.as_ref(), limits)?;
    Ok(DirectoryBackupSummaryV1 {
        revision: checkpoint.revision,
    })
}

/// Restores one canonical private backup into a previously absent store path.
///
/// Success returns the exclusively locked owner at the restored revision. A
/// failure after destination creation can leave a partial fail-closed directory
/// requiring operator inspection; this function makes no automatic retry or
/// rollback claim.
pub fn restore_directory_backup_v1(
    backup_path: impl AsRef<Path>,
    store_path: impl AsRef<Path>,
    limits: DirectoryLimitsV1,
) -> Result<DirectoryCheckpointStoreV1, DirectoryErrorV1> {
    restore_directory_backup_inner(
        backup_path.as_ref(),
        store_path.as_ref(),
        limits,
        BackupInterruptionV1::None,
    )
}

fn restore_directory_backup_inner(
    backup_path: &Path,
    store_path: &Path,
    limits: DirectoryLimitsV1,
    interruption: BackupInterruptionV1,
) -> Result<DirectoryCheckpointStoreV1, DirectoryErrorV1> {
    validate_limits(limits)?;
    let (key, checkpoint, _) = read_record(backup_path, limits)?;
    let record = encode_record(&key, checkpoint.revision, &checkpoint.bytes, limits)?;
    let mut store = DirectoryCheckpointStoreV1::create_new(store_path, limits)?;
    interrupt_backup_at(interruption, BackupInterruptionV1::RestoreDirectoryCreated)?;
    let pending_path = store.path.join(PENDING);
    let mut pending = store.open_pending(&pending_path).map_err(io)?;
    interrupt_backup_at(interruption, BackupInterruptionV1::RestorePendingCreated)?;
    pending.write_all(&record).map_err(io)?;
    interrupt_backup_at(interruption, BackupInterruptionV1::RestorePendingWritten)?;
    pending.sync_all().map_err(io)?;
    interrupt_backup_at(interruption, BackupInterruptionV1::RestorePendingSynced)?;
    fs::rename(&pending_path, store.path.join(HEAD)).map_err(io)?;
    interrupt_backup_at(interruption, BackupInterruptionV1::RestoreHeadRenamed)?;
    store.directory.sync_all().map_err(io)?;
    interrupt_backup_at(interruption, BackupInterruptionV1::RestoreDirectorySynced)?;
    let metadata = fs::symlink_metadata(store.path.join(HEAD)).map_err(io)?;
    validate_private_file(&metadata)?;
    store.head_stamp = Some(Stamp::of(&metadata));
    store.key = Some(key);
    store.checkpoint = Some(checkpoint);
    Ok(store)
}

impl CheckpointStore for DirectoryCheckpointStoreV1 {
    type Error = DirectoryErrorV1;

    fn load(&self, key: &str) -> Result<Option<Checkpoint>, Self::Error> {
        self.check_resources()?;
        let mut counters = self.counters.borrow_mut();
        let (loads, saturated) = increment(counters.loads);
        counters.loads = loads;
        counters.saturated |= saturated;
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
        {
            let mut counters = self.counters.borrow_mut();
            let (offers, saturated) = increment(counters.offers);
            counters.offers = offers;
            counters.saturated |= saturated;
        }
        self.check_resources()?;
        if key.is_empty() {
            return self.refused(StoreRefusal::InvalidKey);
        }
        if key.len() > self.limits.max_key_bytes {
            return self.refused(StoreRefusal::KeyLimit);
        }
        if bytes.len() > self.limits.max_value_bytes {
            return self.refused(StoreRefusal::ValueLimit);
        }
        if self.key.as_deref().is_some_and(|stored| stored != key) {
            return self.refused(StoreRefusal::InvalidKey);
        }
        let actual = self
            .checkpoint
            .as_ref()
            .map(|checkpoint| checkpoint.revision);
        if actual != expected {
            return self.refused(StoreRefusal::RevisionConflict { expected, actual });
        }
        let revision = match actual {
            Some(actual) => match actual.0.checked_add(1) {
                Some(next) => Revision(next),
                None => return self.refused(StoreRefusal::RevisionExhausted),
            },
            None => Revision(0),
        };
        let record = encode_record(key, revision, bytes, self.limits)?;
        let pending_path = self.path.join(PENDING);
        let mut pending = match self.open_pending(&pending_path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return self.fence(DirectoryErrorV1::CrashResidue);
            }
            Err(error) => return self.fence(io(error)),
        };
        #[cfg(test)]
        if self.crash_at(TestCrashPoint::PendingCreated) {
            return self.indeterminate();
        }
        if pending.write_all(&record).is_err() {
            return self.indeterminate();
        }
        #[cfg(test)]
        if self.crash_at(TestCrashPoint::PendingWritten) {
            return self.indeterminate();
        }
        if pending.sync_all().is_err() {
            return self.indeterminate();
        }
        #[cfg(test)]
        if self.crash_at(TestCrashPoint::PendingSynced) {
            return self.indeterminate();
        }
        if fs::rename(&pending_path, self.path.join(HEAD)).is_err() {
            return self.indeterminate();
        }
        #[cfg(test)]
        if self.crash_at(TestCrashPoint::HeadRenamed) {
            return self.indeterminate();
        }
        if self.directory.sync_all().is_err() {
            return self.indeterminate();
        }
        #[cfg(test)]
        if self.crash_at(TestCrashPoint::DirectorySynced) {
            return self.indeterminate();
        }
        let head_metadata = match fs::symlink_metadata(self.path.join(HEAD)) {
            Ok(metadata) => metadata,
            Err(_) => return self.indeterminate(),
        };
        if validate_private_file(&head_metadata).is_err() {
            return self.indeterminate();
        }
        let head_stamp = Stamp::of(&head_metadata);
        self.head_stamp = Some(head_stamp);
        self.key = Some(key.to_owned());
        self.checkpoint = Some(Checkpoint {
            revision,
            bytes: bytes.to_vec(),
            strength: AcknowledgementStrength::Durable,
        });
        let mut counters = self.counters.borrow_mut();
        let (commits, saturated) = increment(counters.commits);
        counters.commits = commits;
        counters.saturated |= saturated;
        Ok(CommitOutcome::Committed(CommitReceipt {
            revision,
            mechanism: "agora-private-directory-v1",
            strength: AcknowledgementStrength::Durable,
        }))
    }
}

impl ObservableCheckpointStoreV1 for DirectoryCheckpointStoreV1 {
    type Observation = DirectoryObservationV1;

    fn physical_observation(&self) -> Self::Observation {
        DirectoryCheckpointStoreV1::observation(self)
    }
}

fn validate_limits(limits: DirectoryLimitsV1) -> Result<(), DirectoryErrorV1> {
    if limits.max_key_bytes == 0
        || limits.max_value_bytes == 0
        || HEADER_BYTES
            .checked_add(limits.max_key_bytes)
            .and_then(|size| size.checked_add(limits.max_value_bytes))
            .is_none()
    {
        Err(DirectoryErrorV1::InvalidLimits)
    } else {
        Ok(())
    }
}

fn refuse_symlink(path: &Path) -> Result<(), DirectoryErrorV1> {
    if fs::symlink_metadata(path)
        .map_err(io)?
        .file_type()
        .is_symlink()
    {
        Err(DirectoryErrorV1::UnsafeResource)
    } else {
        Ok(())
    }
}

fn path_exists_no_follow(path: &Path) -> Result<bool, DirectoryErrorV1> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io(error)),
    }
}

fn validate_private_directory(metadata: &Metadata) -> Result<(), DirectoryErrorV1> {
    if !metadata.is_dir() || metadata.file_type().is_symlink() || metadata.mode() & 0o077 != 0 {
        Err(DirectoryErrorV1::UnsafeResource)
    } else {
        Ok(())
    }
}

fn validate_private_file(metadata: &Metadata) -> Result<(), DirectoryErrorV1> {
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.mode() & 0o077 != 0 {
        Err(DirectoryErrorV1::UnsafeResource)
    } else {
        Ok(())
    }
}

fn open_private_directory(path: &Path) -> Result<File, DirectoryErrorV1> {
    let metadata = fs::symlink_metadata(path).map_err(io)?;
    validate_private_directory(&metadata)?;
    let file = File::open(path).map_err(io)?;
    if !Stamp::of(&metadata).same_object(Stamp::of(&file.metadata().map_err(io)?)) {
        return Err(DirectoryErrorV1::UnsafeResource);
    }
    Ok(file)
}

fn interrupt_backup_at(
    selected: BackupInterruptionV1,
    current: BackupInterruptionV1,
) -> Result<(), DirectoryErrorV1> {
    if selected == current {
        Err(DirectoryErrorV1::Io(std::io::ErrorKind::Interrupted))
    } else {
        Ok(())
    }
}

fn write_new_private_file(
    path: &Path,
    bytes: &[u8],
    interruption: BackupInterruptionV1,
) -> Result<(), DirectoryErrorV1> {
    let parent = path.parent().ok_or(DirectoryErrorV1::UnsafeResource)?;
    let directory = open_private_directory(parent)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                DirectoryErrorV1::AlreadyExists
            } else {
                io(error)
            }
        })?;
    interrupt_backup_at(interruption, BackupInterruptionV1::ExportCreated)?;
    file.write_all(bytes).map_err(io)?;
    interrupt_backup_at(interruption, BackupInterruptionV1::ExportWritten)?;
    file.sync_all().map_err(io)?;
    interrupt_backup_at(interruption, BackupInterruptionV1::ExportFileSynced)?;
    let path_metadata = fs::symlink_metadata(path).map_err(io)?;
    validate_private_file(&path_metadata)?;
    if !Stamp::of(&path_metadata).same_object(Stamp::of(&file.metadata().map_err(io)?)) {
        return Err(DirectoryErrorV1::LostResource);
    }
    directory.sync_all().map_err(io)?;
    interrupt_backup_at(interruption, BackupInterruptionV1::ExportDirectorySynced)
}

fn crc32c(bytes: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0x82f6_3b78_u32 & (0_u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}

fn record_checksum(bytes: &[u8]) -> Result<u32, DirectoryErrorV1> {
    if bytes.len() < HEADER_BYTES {
        return Err(DirectoryErrorV1::Corrupt);
    }
    let mut checksum_input = Vec::new();
    checksum_input
        .try_reserve_exact(bytes.len() - 4)
        .map_err(|_| DirectoryErrorV1::Capacity)?;
    checksum_input.extend_from_slice(&bytes[..28]);
    checksum_input.extend_from_slice(&bytes[HEADER_BYTES..]);
    Ok(crc32c(&checksum_input))
}

fn encode_record(
    key: &str,
    revision: Revision,
    value: &[u8],
    limits: DirectoryLimitsV1,
) -> Result<Vec<u8>, DirectoryErrorV1> {
    if key.is_empty() || key.len() > limits.max_key_bytes || value.len() > limits.max_value_bytes {
        return Err(DirectoryErrorV1::Capacity);
    }
    let total = HEADER_BYTES
        .checked_add(key.len())
        .and_then(|length| length.checked_add(value.len()))
        .ok_or(DirectoryErrorV1::Capacity)?;
    let mut payload = Vec::new();
    payload
        .try_reserve_exact(total)
        .map_err(|_| DirectoryErrorV1::Capacity)?;
    payload.extend_from_slice(MAGIC);
    payload.extend_from_slice(&revision.0.to_be_bytes());
    payload.extend_from_slice(
        &u32::try_from(key.len())
            .map_err(|_| DirectoryErrorV1::Capacity)?
            .to_be_bytes(),
    );
    payload.extend_from_slice(
        &u64::try_from(value.len())
            .map_err(|_| DirectoryErrorV1::Capacity)?
            .to_be_bytes(),
    );
    payload.extend_from_slice(&0_u32.to_be_bytes());
    payload.extend_from_slice(key.as_bytes());
    payload.extend_from_slice(value);
    let checksum = record_checksum(&payload)?;
    payload[28..32].copy_from_slice(&checksum.to_be_bytes());
    Ok(payload)
}

fn read_record(
    path: &Path,
    limits: DirectoryLimitsV1,
) -> Result<(String, Checkpoint, Stamp), DirectoryErrorV1> {
    let path_metadata = fs::symlink_metadata(path).map_err(io)?;
    validate_private_file(&path_metadata)?;
    let mut file = OpenOptions::new().read(true).open(path).map_err(io)?;
    let metadata = file.metadata().map_err(io)?;
    validate_private_file(&metadata)?;
    if !Stamp::of(&path_metadata).same_object(Stamp::of(&metadata)) {
        return Err(DirectoryErrorV1::UnsafeResource);
    }
    let maximum = HEADER_BYTES
        .checked_add(limits.max_key_bytes)
        .and_then(|length| length.checked_add(limits.max_value_bytes))
        .ok_or(DirectoryErrorV1::InvalidLimits)?;
    let length: usize = metadata
        .len()
        .try_into()
        .map_err(|_| DirectoryErrorV1::Capacity)?;
    if length < HEADER_BYTES || length > maximum {
        return Err(DirectoryErrorV1::Corrupt);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| DirectoryErrorV1::Capacity)?;
    file.read_to_end(&mut bytes).map_err(io)?;
    if bytes.len() != length || bytes.get(..8) != Some(MAGIC) {
        return Err(DirectoryErrorV1::Corrupt);
    }
    let revision = Revision(u64::from_be_bytes(
        bytes[8..16]
            .try_into()
            .map_err(|_| DirectoryErrorV1::Corrupt)?,
    ));
    let key_length = u32::from_be_bytes(
        bytes[16..20]
            .try_into()
            .map_err(|_| DirectoryErrorV1::Corrupt)?,
    ) as usize;
    let value_length: usize = u64::from_be_bytes(
        bytes[20..28]
            .try_into()
            .map_err(|_| DirectoryErrorV1::Corrupt)?,
    )
    .try_into()
    .map_err(|_| DirectoryErrorV1::Capacity)?;
    let checksum = u32::from_be_bytes(
        bytes[28..32]
            .try_into()
            .map_err(|_| DirectoryErrorV1::Corrupt)?,
    );
    if key_length == 0
        || key_length > limits.max_key_bytes
        || value_length > limits.max_value_bytes
        || HEADER_BYTES
            .checked_add(key_length)
            .and_then(|end| end.checked_add(value_length))
            != Some(bytes.len())
        || record_checksum(&bytes)? != checksum
    {
        return Err(DirectoryErrorV1::Corrupt);
    }
    let key_end = HEADER_BYTES + key_length;
    let key = std::str::from_utf8(&bytes[HEADER_BYTES..key_end])
        .map_err(|_| DirectoryErrorV1::Corrupt)?
        .to_owned();
    let value = bytes[key_end..].to_vec();
    Ok((
        key,
        Checkpoint {
            revision,
            bytes: value,
            strength: AcknowledgementStrength::Durable,
        },
        Stamp::of(&metadata),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TempDirectory(PathBuf);

    impl TempDirectory {
        fn new() -> Self {
            let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "agora-storage-directory-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            Self(path)
        }

        fn store(&self) -> PathBuf {
            self.0.join("store")
        }
    }

    impl Drop for TempDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn committed(outcome: CommitOutcome, revision: u64) {
        assert_eq!(
            outcome,
            CommitOutcome::Committed(CommitReceipt {
                revision: Revision(revision),
                mechanism: "agora-private-directory-v1",
                strength: AcknowledgementStrength::Durable,
            })
        );
    }

    #[test]
    fn compare_exchange_survives_close_and_reopen() {
        let temp = TempDirectory::new();
        let path = temp.store();
        let mut store =
            DirectoryCheckpointStoreV1::create_new(&path, DirectoryLimitsV1::default()).unwrap();
        assert_eq!(store.load("host").unwrap(), None);
        committed(store.compare_exchange("host", None, b"one").unwrap(), 0);
        assert_eq!(store.load("host").unwrap().unwrap().bytes, b"one");
        drop(store);

        let mut store =
            DirectoryCheckpointStoreV1::open_existing(&path, DirectoryLimitsV1::default()).unwrap();
        assert_eq!(
            store.observation(),
            DirectoryObservationV1 {
                state: DirectoryOwnerStateV1::Active,
                revision: Some(0),
                loads: 0,
                offers: 0,
                commits: 0,
                refusals: 0,
                indeterminate: 0,
                resource_fences: 0,
                saturated: false,
            }
        );
        let loaded = store.load("host").unwrap().unwrap();
        assert_eq!(loaded.revision, Revision(0));
        assert_eq!(loaded.strength, AcknowledgementStrength::Durable);
        assert_eq!(loaded.bytes, b"one");
        committed(
            store
                .compare_exchange("host", Some(Revision(0)), b"two")
                .unwrap(),
            1,
        );
        let observation = store.observation();
        assert_eq!(observation.revision, Some(1));
        assert_eq!(observation.loads, 1);
        assert_eq!(observation.offers, 1);
        assert_eq!(observation.commits, 1);
        drop(store);

        let store =
            DirectoryCheckpointStoreV1::open_existing(&path, DirectoryLimitsV1::default()).unwrap();
        assert_eq!(store.load("host").unwrap().unwrap().bytes, b"two");
    }

    #[test]
    fn every_publication_crash_point_is_residue_or_recoverable_commit() {
        for point in [
            TestCrashPoint::PendingCreated,
            TestCrashPoint::PendingWritten,
            TestCrashPoint::PendingSynced,
            TestCrashPoint::HeadRenamed,
            TestCrashPoint::DirectorySynced,
        ] {
            let temp = TempDirectory::new();
            let path = temp.store();
            let mut store =
                DirectoryCheckpointStoreV1::create_new(&path, DirectoryLimitsV1::default())
                    .unwrap();
            store.crash_point.set(Some(point));
            assert_eq!(
                store.compare_exchange("host", None, b"candidate").unwrap(),
                CommitOutcome::Indeterminate
            );
            assert_eq!(store.observation().indeterminate, 1);
            assert_eq!(store.observation().state, DirectoryOwnerStateV1::Fenced);
            drop(store);

            if matches!(
                point,
                TestCrashPoint::PendingCreated
                    | TestCrashPoint::PendingWritten
                    | TestCrashPoint::PendingSynced
            ) {
                assert_eq!(
                    DirectoryCheckpointStoreV1::open_existing(&path, DirectoryLimitsV1::default(),)
                        .unwrap_err(),
                    DirectoryErrorV1::CrashResidue,
                    "pre-rename crash must fail closed at {point:?}"
                );
            } else {
                let recovered =
                    DirectoryCheckpointStoreV1::open_existing(&path, DirectoryLimitsV1::default())
                        .unwrap();
                let checkpoint = recovered.load("host").unwrap().unwrap();
                assert_eq!(checkpoint.revision, Revision(0));
                assert_eq!(checkpoint.bytes, b"candidate");
                assert_eq!(checkpoint.strength, AcknowledgementStrength::Durable);
            }
        }
    }

    #[test]
    fn refusals_do_not_change_the_register() {
        let temp = TempDirectory::new();
        let mut store = DirectoryCheckpointStoreV1::create_new(
            temp.store(),
            DirectoryLimitsV1 {
                max_key_bytes: 4,
                max_value_bytes: 3,
            },
        )
        .unwrap();
        assert_eq!(
            store.compare_exchange("", None, b"x").unwrap(),
            CommitOutcome::Refused(StoreRefusal::InvalidKey)
        );
        assert_eq!(
            store.compare_exchange("large", None, b"x").unwrap(),
            CommitOutcome::Refused(StoreRefusal::KeyLimit)
        );
        assert_eq!(
            store.compare_exchange("key", None, b"four").unwrap(),
            CommitOutcome::Refused(StoreRefusal::ValueLimit)
        );
        committed(store.compare_exchange("key", None, b"one").unwrap(), 0);
        assert!(matches!(
            store.compare_exchange("key", None, b"two").unwrap(),
            CommitOutcome::Refused(StoreRefusal::RevisionConflict { .. })
        ));
        assert_eq!(
            store
                .compare_exchange("else", Some(Revision(0)), b"two")
                .unwrap(),
            CommitOutcome::Refused(StoreRefusal::InvalidKey)
        );
        assert_eq!(store.load("key").unwrap().unwrap().bytes, b"one");
    }

    #[test]
    fn ownership_is_exclusive_and_observation_is_passive_content_free_and_saturating() {
        let temp = TempDirectory::new();
        let path = temp.store();
        let mut store =
            DirectoryCheckpointStoreV1::create_new(&path, DirectoryLimitsV1::default()).unwrap();
        assert_eq!(
            DirectoryCheckpointStoreV1::open_existing(&path, DirectoryLimitsV1::default())
                .unwrap_err(),
            DirectoryErrorV1::Busy
        );
        committed(
            store
                .compare_exchange("credential-secret", None, b"message-secret")
                .unwrap(),
            0,
        );
        let observation = store.observation();
        assert_eq!(observation.state, DirectoryOwnerStateV1::Active);
        assert_eq!(observation.revision, Some(0));
        assert_eq!(observation.offers, 1);
        assert_eq!(observation.commits, 1);
        assert_eq!(observation, store.observation());
        assert_eq!(
            observation,
            ObservableCheckpointStoreV1::physical_observation(&store)
        );
        assert!(!observation.fidelity_lost());
        let debug = format!("{store:?}");
        assert!(debug.contains("DirectoryCheckpointStoreV1"));
        assert!(!debug.contains(path.to_str().unwrap()));
        assert!(!debug.contains("credential-secret"));
        assert!(!debug.contains("message-secret"));

        store.counters.borrow_mut().offers = u64::MAX;
        assert_eq!(
            store.compare_exchange("", Some(Revision(0)), b"x").unwrap(),
            CommitOutcome::Refused(StoreRefusal::InvalidKey)
        );
        let observation = store.observation();
        assert_eq!(observation.offers, u64::MAX);
        assert_eq!(observation.refusals, 1);
        assert!(observation.saturated);
        assert!(ObservableCheckpointStoreV1::physical_observation(&store).fidelity_lost());
    }

    #[test]
    fn permissive_resources_symlinks_and_crash_residue_are_refused() {
        let temp = TempDirectory::new();
        let path = temp.store();
        let store =
            DirectoryCheckpointStoreV1::create_new(&path, DirectoryLimitsV1::default()).unwrap();
        drop(store);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            DirectoryCheckpointStoreV1::open_existing(&path, DirectoryLimitsV1::default())
                .unwrap_err(),
            DirectoryErrorV1::UnsafeResource
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(path.join(PENDING), b"incomplete").unwrap();
        fs::set_permissions(path.join(PENDING), fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            DirectoryCheckpointStoreV1::open_existing(&path, DirectoryLimitsV1::default())
                .unwrap_err(),
            DirectoryErrorV1::CrashResidue
        );
        fs::remove_file(path.join(PENDING)).unwrap();
        let real_lock = path.join("real-lock");
        fs::rename(path.join(LOCK), &real_lock).unwrap();
        symlink(&real_lock, path.join(LOCK)).unwrap();
        assert_eq!(
            DirectoryCheckpointStoreV1::open_existing(&path, DirectoryLimitsV1::default())
                .unwrap_err(),
            DirectoryErrorV1::UnsafeResource
        );
    }

    #[test]
    fn every_active_resource_validation_failure_is_sticky_and_counted_once() {
        fn scenario(mutate: impl FnOnce(&Path), expected: DirectoryErrorV1) {
            let temp = TempDirectory::new();
            let path = temp.store();
            let mut store =
                DirectoryCheckpointStoreV1::create_new(&path, DirectoryLimitsV1::default())
                    .unwrap();
            committed(store.compare_exchange("key", None, b"value").unwrap(), 0);
            mutate(&path);

            assert_eq!(store.load("key").unwrap_err(), expected);
            let observation = store.observation();
            assert_eq!(observation.state, DirectoryOwnerStateV1::Fenced);
            assert_eq!(observation.resource_fences, 1);
            assert_eq!(store.load("key").unwrap_err(), DirectoryErrorV1::Fenced);
            assert_eq!(store.observation().resource_fences, 1);
        }

        scenario(
            |path| {
                fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
            },
            DirectoryErrorV1::UnsafeResource,
        );
        scenario(
            |path| {
                fs::remove_file(path.join(LOCK)).unwrap();
            },
            DirectoryErrorV1::LostResource,
        );
        scenario(
            |path| {
                fs::write(path.join(PENDING), b"residue").unwrap();
                fs::set_permissions(path.join(PENDING), fs::Permissions::from_mode(0o600)).unwrap();
            },
            DirectoryErrorV1::CrashResidue,
        );
        scenario(
            |path| {
                fs::remove_file(path.join(HEAD)).unwrap();
            },
            DirectoryErrorV1::LostResource,
        );
    }

    #[test]
    fn pending_publication_open_failure_fences_before_returning_io() {
        let temp = TempDirectory::new();
        let path = temp.store();
        let mut store =
            DirectoryCheckpointStoreV1::create_new(&path, DirectoryLimitsV1::default()).unwrap();
        store
            .pending_open_error
            .set(Some(std::io::ErrorKind::PermissionDenied));

        assert_eq!(
            store.compare_exchange("key", None, b"value").unwrap_err(),
            DirectoryErrorV1::Io(std::io::ErrorKind::PermissionDenied)
        );
        let observation = store.observation();
        assert_eq!(observation.state, DirectoryOwnerStateV1::Fenced);
        assert_eq!(observation.offers, 1);
        assert_eq!(observation.commits, 0);
        assert_eq!(observation.indeterminate, 0);
        assert_eq!(observation.resource_fences, 1);
        assert!(!path.join(PENDING).exists());
        assert!(!path.join(HEAD).exists());

        assert_eq!(store.load("key").unwrap_err(), DirectoryErrorV1::Fenced);
        assert_eq!(store.observation().resource_fences, 1);
    }

    #[test]
    fn corrupt_or_replaced_head_fails_closed() {
        let temp = TempDirectory::new();
        let path = temp.store();
        let mut store =
            DirectoryCheckpointStoreV1::create_new(&path, DirectoryLimitsV1::default()).unwrap();
        committed(store.compare_exchange("key", None, b"value").unwrap(), 0);
        let record = fs::read(path.join(HEAD)).unwrap();
        drop(store);

        let mut corrupt = record.clone();
        corrupt[8] ^= 1;
        fs::write(path.join(HEAD), corrupt).unwrap();
        fs::set_permissions(path.join(HEAD), fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            DirectoryCheckpointStoreV1::open_existing(&path, DirectoryLimitsV1::default())
                .unwrap_err(),
            DirectoryErrorV1::Corrupt
        );

        fs::write(path.join(HEAD), record).unwrap();
        fs::set_permissions(path.join(HEAD), fs::Permissions::from_mode(0o600)).unwrap();
        let store =
            DirectoryCheckpointStoreV1::open_existing(&path, DirectoryLimitsV1::default()).unwrap();
        fs::remove_file(path.join(HEAD)).unwrap();
        fs::write(path.join(HEAD), b"replacement").unwrap();
        fs::set_permissions(path.join(HEAD), fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            store.load("key").unwrap_err(),
            DirectoryErrorV1::LostResource
        );
        assert_eq!(store.load("key").unwrap_err(), DirectoryErrorV1::Fenced);
        let observation = store.observation();
        assert_eq!(observation.state, DirectoryOwnerStateV1::Fenced);
        assert_eq!(observation.resource_fences, 1);
    }

    #[test]
    fn dangling_symlink_cannot_hide_pending_or_head_resources() {
        let temp = TempDirectory::new();
        let path = temp.store();
        let store =
            DirectoryCheckpointStoreV1::create_new(&path, DirectoryLimitsV1::default()).unwrap();
        drop(store);
        symlink(path.join("missing"), path.join(PENDING)).unwrap();
        assert_eq!(
            DirectoryCheckpointStoreV1::open_existing(&path, DirectoryLimitsV1::default())
                .unwrap_err(),
            DirectoryErrorV1::CrashResidue
        );
        fs::remove_file(path.join(PENDING)).unwrap();
        symlink(path.join("missing"), path.join(HEAD)).unwrap();
        assert_eq!(
            DirectoryCheckpointStoreV1::open_existing(&path, DirectoryLimitsV1::default())
                .unwrap_err(),
            DirectoryErrorV1::UnsafeResource
        );
    }

    #[test]
    fn offline_backup_restore_preserves_exact_record_and_revision_continuity() {
        let temp = TempDirectory::new();
        let source_path = temp.store();
        let backup_path = temp.0.join("backup.bin");
        let restored_path = temp.0.join("restored");
        let mut source =
            DirectoryCheckpointStoreV1::create_new(&source_path, DirectoryLimitsV1::default())
                .unwrap();
        committed(source.compare_exchange("host", None, b"one").unwrap(), 0);
        committed(
            source
                .compare_exchange("host", Some(Revision(0)), b"two")
                .unwrap(),
            1,
        );
        let expected_record = fs::read(source_path.join(HEAD)).unwrap();
        drop(source);

        assert_eq!(
            export_directory_backup_v1(&source_path, &backup_path, DirectoryLimitsV1::default())
                .unwrap(),
            DirectoryBackupSummaryV1 {
                revision: Revision(1)
            }
        );
        assert_eq!(fs::read(&backup_path).unwrap(), expected_record);
        assert_eq!(
            fs::symlink_metadata(&backup_path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            verify_directory_backup_v1(&backup_path, DirectoryLimitsV1::default()).unwrap(),
            DirectoryBackupSummaryV1 {
                revision: Revision(1)
            }
        );

        let mut restored =
            restore_directory_backup_v1(&backup_path, &restored_path, DirectoryLimitsV1::default())
                .unwrap();
        let checkpoint = restored.load("host").unwrap().unwrap();
        assert_eq!(checkpoint.revision, Revision(1));
        assert_eq!(checkpoint.bytes, b"two");
        committed(
            restored
                .compare_exchange("host", Some(Revision(1)), b"three")
                .unwrap(),
            2,
        );
        drop(restored);

        let restored =
            DirectoryCheckpointStoreV1::open_existing(&restored_path, DirectoryLimitsV1::default())
                .unwrap();
        let checkpoint = restored.load("host").unwrap().unwrap();
        assert_eq!(checkpoint.revision, Revision(2));
        assert_eq!(checkpoint.bytes, b"three");
    }

    #[test]
    fn backup_and_restore_fail_closed_on_ownership_content_and_destination_drift() {
        let temp = TempDirectory::new();
        let source_path = temp.store();
        let backup_path = temp.0.join("backup.bin");
        let restored_path = temp.0.join("restored");
        let mut source =
            DirectoryCheckpointStoreV1::create_new(&source_path, DirectoryLimitsV1::default())
                .unwrap();
        assert_eq!(
            export_directory_backup_v1(&source_path, &backup_path, DirectoryLimitsV1::default())
                .unwrap_err(),
            DirectoryErrorV1::Busy
        );
        drop(source);
        assert_eq!(
            export_directory_backup_v1(&source_path, &backup_path, DirectoryLimitsV1::default())
                .unwrap_err(),
            DirectoryErrorV1::Empty
        );
        assert!(!backup_path.exists());

        source =
            DirectoryCheckpointStoreV1::open_existing(&source_path, DirectoryLimitsV1::default())
                .unwrap();
        committed(source.compare_exchange("host", None, b"value").unwrap(), 0);
        drop(source);
        export_directory_backup_v1(&source_path, &backup_path, DirectoryLimitsV1::default())
            .unwrap();
        assert_eq!(
            export_directory_backup_v1(&source_path, &backup_path, DirectoryLimitsV1::default())
                .unwrap_err(),
            DirectoryErrorV1::AlreadyExists
        );

        let original = fs::read(&backup_path).unwrap();
        let mut corrupt = original.clone();
        corrupt[8] ^= 1;
        fs::write(&backup_path, corrupt).unwrap();
        assert_eq!(
            verify_directory_backup_v1(&backup_path, DirectoryLimitsV1::default()).unwrap_err(),
            DirectoryErrorV1::Corrupt
        );
        assert_eq!(
            restore_directory_backup_v1(&backup_path, &restored_path, DirectoryLimitsV1::default())
                .unwrap_err(),
            DirectoryErrorV1::Corrupt
        );
        assert!(!restored_path.exists());

        fs::write(&backup_path, original).unwrap();
        fs::set_permissions(&backup_path, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            verify_directory_backup_v1(&backup_path, DirectoryLimitsV1::default()).unwrap_err(),
            DirectoryErrorV1::UnsafeResource
        );
        fs::set_permissions(&backup_path, fs::Permissions::from_mode(0o600)).unwrap();
        let existing =
            DirectoryCheckpointStoreV1::create_new(&restored_path, DirectoryLimitsV1::default())
                .unwrap();
        drop(existing);
        assert_eq!(
            restore_directory_backup_v1(&backup_path, &restored_path, DirectoryLimitsV1::default())
                .unwrap_err(),
            DirectoryErrorV1::AlreadyExists
        );
    }

    #[test]
    fn every_backup_interruption_is_inspectable_without_a_false_success() {
        for point in [
            BackupInterruptionV1::ExportCreated,
            BackupInterruptionV1::ExportWritten,
            BackupInterruptionV1::ExportFileSynced,
            BackupInterruptionV1::ExportDirectorySynced,
        ] {
            let temp = TempDirectory::new();
            let source_path = temp.store();
            let backup_path = temp.0.join("backup.bin");
            let mut source =
                DirectoryCheckpointStoreV1::create_new(&source_path, DirectoryLimitsV1::default())
                    .unwrap();
            committed(source.compare_exchange("host", None, b"value").unwrap(), 0);
            let expected_record = fs::read(source_path.join(HEAD)).unwrap();
            drop(source);

            assert_eq!(
                export_directory_backup_inner(
                    &source_path,
                    &backup_path,
                    DirectoryLimitsV1::default(),
                    point,
                )
                .unwrap_err(),
                DirectoryErrorV1::Io(std::io::ErrorKind::Interrupted),
                "interruption must never report export success at {point:?}"
            );
            assert_eq!(
                fs::symlink_metadata(&backup_path)
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
            if point == BackupInterruptionV1::ExportCreated {
                assert_eq!(
                    verify_directory_backup_v1(&backup_path, DirectoryLimitsV1::default())
                        .unwrap_err(),
                    DirectoryErrorV1::Corrupt
                );
            } else {
                assert_eq!(fs::read(&backup_path).unwrap(), expected_record);
                assert_eq!(
                    verify_directory_backup_v1(&backup_path, DirectoryLimitsV1::default())
                        .unwrap()
                        .revision,
                    Revision(0)
                );
            }
            drop(
                DirectoryCheckpointStoreV1::open_existing(
                    &source_path,
                    DirectoryLimitsV1::default(),
                )
                .unwrap(),
            );
        }
    }

    #[test]
    fn every_restore_interruption_is_empty_residue_or_exact_recovery() {
        for point in [
            BackupInterruptionV1::RestoreDirectoryCreated,
            BackupInterruptionV1::RestorePendingCreated,
            BackupInterruptionV1::RestorePendingWritten,
            BackupInterruptionV1::RestorePendingSynced,
            BackupInterruptionV1::RestoreHeadRenamed,
            BackupInterruptionV1::RestoreDirectorySynced,
        ] {
            let temp = TempDirectory::new();
            let source_path = temp.store();
            let backup_path = temp.0.join("backup.bin");
            let restored_path = temp.0.join("restored");
            let mut source =
                DirectoryCheckpointStoreV1::create_new(&source_path, DirectoryLimitsV1::default())
                    .unwrap();
            committed(source.compare_exchange("host", None, b"one").unwrap(), 0);
            committed(
                source
                    .compare_exchange("host", Some(Revision(0)), b"two")
                    .unwrap(),
                1,
            );
            drop(source);
            export_directory_backup_v1(&source_path, &backup_path, DirectoryLimitsV1::default())
                .unwrap();

            assert_eq!(
                restore_directory_backup_inner(
                    &backup_path,
                    &restored_path,
                    DirectoryLimitsV1::default(),
                    point,
                )
                .unwrap_err(),
                DirectoryErrorV1::Io(std::io::ErrorKind::Interrupted),
                "interruption must never report restore success at {point:?}"
            );

            match point {
                BackupInterruptionV1::RestoreDirectoryCreated => {
                    let restored = DirectoryCheckpointStoreV1::open_existing(
                        &restored_path,
                        DirectoryLimitsV1::default(),
                    )
                    .unwrap();
                    assert_eq!(restored.load("host").unwrap(), None);
                }
                BackupInterruptionV1::RestorePendingCreated
                | BackupInterruptionV1::RestorePendingWritten
                | BackupInterruptionV1::RestorePendingSynced => {
                    assert_eq!(
                        DirectoryCheckpointStoreV1::open_existing(
                            &restored_path,
                            DirectoryLimitsV1::default(),
                        )
                        .unwrap_err(),
                        DirectoryErrorV1::CrashResidue
                    );
                }
                BackupInterruptionV1::RestoreHeadRenamed
                | BackupInterruptionV1::RestoreDirectorySynced => {
                    let mut restored = DirectoryCheckpointStoreV1::open_existing(
                        &restored_path,
                        DirectoryLimitsV1::default(),
                    )
                    .unwrap();
                    let checkpoint = restored.load("host").unwrap().unwrap();
                    assert_eq!(checkpoint.revision, Revision(1));
                    assert_eq!(checkpoint.bytes, b"two");
                    committed(
                        restored
                            .compare_exchange("host", Some(Revision(1)), b"three")
                            .unwrap(),
                        2,
                    );
                }
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn canonical_record_profile_is_frozen() {
        let record = encode_record(
            "agora-host",
            Revision(7),
            b"checkpoint",
            DirectoryLimitsV1::default(),
        )
        .unwrap();
        let actual = record
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(
            actual,
            include_str!("../tests/fixtures/directory-record-v1.hex").trim()
        );
    }
}
