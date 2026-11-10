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

use keystone_storage::{
    AcknowledgementStrength, Checkpoint, CheckpointStore, CommitOutcome, CommitReceipt, Revision,
    StoreRefusal,
};
use std::cell::{Cell, RefCell};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

const HEAD: &str = "checkpoint.bin";
const LOCK: &str = ".owner.lock";
const PENDING: &str = ".pending";
const MAGIC: &[u8; 8] = b"AGDIRCP\x01";
const HEADER_BYTES: usize = 32;

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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DirectorySignalsV1 {
    pub loads: u64,
    pub offers: u64,
    pub commits: u64,
    pub refusals: u64,
    pub indeterminate: u64,
    pub errors: u64,
    pub retained_bytes: usize,
    pub fenced: bool,
    pub saturated: bool,
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
    signals: RefCell<DirectorySignalsV1>,
}

impl std::fmt::Debug for DirectoryCheckpointStoreV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DirectoryCheckpointStoreV1")
            .field("signals", &self.signals())
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
            signals: RefCell::new(DirectorySignalsV1::default()),
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
            signals: RefCell::new(DirectorySignalsV1 {
                retained_bytes: head_stamp.map_or(0, |stamp| stamp.length as usize),
                ..DirectorySignalsV1::default()
            }),
        })
    }

    pub fn signals(&self) -> DirectorySignalsV1 {
        *self.signals.borrow()
    }

    fn check_resources(&self) -> Result<(), DirectoryErrorV1> {
        if self.fenced.get() {
            return Err(DirectoryErrorV1::Fenced);
        }
        let root = fs::symlink_metadata(&self.path).map_err(|_| DirectoryErrorV1::LostResource)?;
        validate_private_directory(&root)?;
        if !self.root_stamp.same_object(Stamp::of(&root))
            || !self
                .root_stamp
                .same_object(Stamp::of(&self.directory.metadata().map_err(io)?))
        {
            return self.fence(DirectoryErrorV1::LostResource);
        }
        let lock = fs::symlink_metadata(self.path.join(LOCK))
            .map_err(|_| DirectoryErrorV1::LostResource)?;
        validate_private_file(&lock)?;
        if Stamp::of(&lock) != self.lock_stamp
            || Stamp::of(&self.lock.metadata().map_err(io)?) != self.lock_stamp
        {
            return self.fence(DirectoryErrorV1::LostResource);
        }
        let head_path = self.path.join(HEAD);
        match self.head_stamp {
            Some(expected) => {
                let actual =
                    fs::symlink_metadata(head_path).map_err(|_| DirectoryErrorV1::LostResource)?;
                validate_private_file(&actual)?;
                if Stamp::of(&actual) != expected {
                    return self.fence(DirectoryErrorV1::LostResource);
                }
            }
            None if path_exists_no_follow(&head_path)? => {
                return self.fence(DirectoryErrorV1::LostResource);
            }
            None => {}
        }
        if path_exists_no_follow(&self.path.join(PENDING))? {
            return self.fence(DirectoryErrorV1::CrashResidue);
        }
        Ok(())
    }

    fn fence<T>(&self, error: DirectoryErrorV1) -> Result<T, DirectoryErrorV1> {
        self.fenced.set(true);
        let mut signals = self.signals.borrow_mut();
        signals.fenced = true;
        let (errors, saturated) = increment(signals.errors);
        signals.errors = errors;
        signals.saturated |= saturated;
        Err(error)
    }

    fn indeterminate(&self) -> Result<CommitOutcome, DirectoryErrorV1> {
        self.fenced.set(true);
        let mut signals = self.signals.borrow_mut();
        signals.fenced = true;
        let (indeterminate, saturated) = increment(signals.indeterminate);
        signals.indeterminate = indeterminate;
        signals.saturated |= saturated;
        Ok(CommitOutcome::Indeterminate)
    }

    fn refused(&self, refusal: StoreRefusal) -> Result<CommitOutcome, DirectoryErrorV1> {
        let mut signals = self.signals.borrow_mut();
        let (refusals, saturated) = increment(signals.refusals);
        signals.refusals = refusals;
        signals.saturated |= saturated;
        Ok(CommitOutcome::Refused(refusal))
    }
}

impl CheckpointStore for DirectoryCheckpointStoreV1 {
    type Error = DirectoryErrorV1;

    fn load(&self, key: &str) -> Result<Option<Checkpoint>, Self::Error> {
        self.check_resources()?;
        let mut signals = self.signals.borrow_mut();
        let (loads, saturated) = increment(signals.loads);
        signals.loads = loads;
        signals.saturated |= saturated;
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
            let mut signals = self.signals.borrow_mut();
            let (offers, saturated) = increment(signals.offers);
            signals.offers = offers;
            signals.saturated |= saturated;
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
        let mut pending = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&pending_path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return self.fence(DirectoryErrorV1::CrashResidue);
            }
            Err(error) => return Err(io(error)),
        };
        if pending.write_all(&record).is_err() || pending.sync_all().is_err() {
            return self.indeterminate();
        }
        if fs::rename(&pending_path, self.path.join(HEAD)).is_err()
            || self.directory.sync_all().is_err()
        {
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
        let mut signals = self.signals.borrow_mut();
        signals.retained_bytes = head_stamp.length as usize;
        let (commits, saturated) = increment(signals.commits);
        signals.commits = commits;
        signals.saturated |= saturated;
        Ok(CommitOutcome::Committed(CommitReceipt {
            revision,
            mechanism: "agora-private-directory-v1",
            strength: AcknowledgementStrength::Durable,
        }))
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
        drop(store);

        let store =
            DirectoryCheckpointStoreV1::open_existing(&path, DirectoryLimitsV1::default()).unwrap();
        assert_eq!(store.load("host").unwrap().unwrap().bytes, b"two");
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
    fn ownership_is_exclusive_and_debug_is_content_free() {
        let temp = TempDirectory::new();
        let path = temp.store();
        let store =
            DirectoryCheckpointStoreV1::create_new(&path, DirectoryLimitsV1::default()).unwrap();
        assert_eq!(
            DirectoryCheckpointStoreV1::open_existing(&path, DirectoryLimitsV1::default())
                .unwrap_err(),
            DirectoryErrorV1::Busy
        );
        let debug = format!("{store:?}");
        assert!(debug.contains("DirectoryCheckpointStoreV1"));
        assert!(!debug.contains(path.to_str().unwrap()));
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
        assert!(store.signals().fenced);
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
