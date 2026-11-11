use crate::{DaemonErrorV1, ServerIncarnationV1};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::unix::fs::{FileExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

const MAGIC_V1: [u8; 8] = *b"AGORALF1";
const RECORD_BYTES_V1: usize = 24;

#[derive(Clone, Copy, Eq, PartialEq)]
struct ResourceIdentityV1 {
    device: u64,
    inode: u64,
}

impl ResourceIdentityV1 {
    fn of(metadata: &fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
}

pub(crate) struct LifetimeLeaseV1 {
    parent: File,
    parent_path: PathBuf,
    parent_identity: ResourceIdentityV1,
    lock: File,
    lock_path: PathBuf,
    lock_identity: ResourceIdentityV1,
    record: [u8; RECORD_BYTES_V1],
    remove_on_drop: bool,
}

impl LifetimeLeaseV1 {
    pub(crate) fn acquire_new(
        parent_path: &Path,
        socket_path: &Path,
        server_incarnation: ServerIncarnationV1,
    ) -> Result<Self, DaemonErrorV1> {
        let parent_metadata = fs::symlink_metadata(parent_path).map_err(DaemonErrorV1::Io)?;
        if !parent_metadata.is_dir() || parent_metadata.permissions().mode() & 0o777 != 0o700 {
            return Err(DaemonErrorV1::UnsafeEndpoint);
        }
        let parent = File::open(parent_path).map_err(DaemonErrorV1::Io)?;
        let parent_identity =
            ResourceIdentityV1::of(&parent.metadata().map_err(DaemonErrorV1::Io)?);
        if parent_identity != ResourceIdentityV1::of(&parent_metadata) {
            return Err(DaemonErrorV1::UnsafeEndpoint);
        }

        let lock_path = lifetime_path(socket_path)?;
        let lock = match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&lock_path)
        {
            Ok(lock) => lock,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                return Err(classify_existing_lifetime(&lock_path));
            }
            Err(error) => return Err(DaemonErrorV1::Io(error)),
        };
        if let Err(error) = lock.try_lock() {
            let _ = fs::remove_file(&lock_path);
            return Err(DaemonErrorV1::Io(error.into()));
        }
        let lock_metadata = match lock.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                let _ = fs::remove_file(&lock_path);
                return Err(DaemonErrorV1::Io(error));
            }
        };
        if !private_regular_file(&lock_metadata) {
            let _ = fs::remove_file(&lock_path);
            return Err(DaemonErrorV1::UnsafeEndpoint);
        }
        let record = record(server_incarnation);
        if let Err(error) = lock
            .write_all_at(&record, 0)
            .and_then(|()| lock.set_len(record.len() as u64))
            .and_then(|()| lock.sync_all())
            .and_then(|()| parent.sync_all())
        {
            let _ = fs::remove_file(&lock_path);
            return Err(DaemonErrorV1::Io(error));
        }
        let lock_identity = match lock.metadata() {
            Ok(metadata) => ResourceIdentityV1::of(&metadata),
            Err(error) => {
                let _ = fs::remove_file(&lock_path);
                return Err(DaemonErrorV1::Io(error));
            }
        };
        Ok(Self {
            parent,
            parent_path: parent_path.to_path_buf(),
            parent_identity,
            lock,
            lock_path,
            lock_identity,
            record,
            remove_on_drop: true,
        })
    }

    pub(crate) fn recover_stale(
        parent_path: &Path,
        socket_path: &Path,
        previous_incarnation: ServerIncarnationV1,
        next_incarnation: ServerIncarnationV1,
    ) -> Result<Self, DaemonErrorV1> {
        Self::recover_stale_inner(
            parent_path,
            socket_path,
            previous_incarnation,
            next_incarnation,
            |_| Ok(()),
        )
    }

    fn recover_stale_inner(
        parent_path: &Path,
        socket_path: &Path,
        previous_incarnation: ServerIncarnationV1,
        next_incarnation: ServerIncarnationV1,
        mut interrupt: impl FnMut(u8) -> io::Result<()>,
    ) -> Result<Self, DaemonErrorV1> {
        if previous_incarnation.is_zero()
            || next_incarnation.is_zero()
            || previous_incarnation == next_incarnation
        {
            return Err(DaemonErrorV1::ResidueMismatch);
        }
        let parent_metadata = fs::symlink_metadata(parent_path).map_err(DaemonErrorV1::Io)?;
        if !parent_metadata.is_dir() || parent_metadata.permissions().mode() & 0o777 != 0o700 {
            return Err(DaemonErrorV1::UnsafeEndpoint);
        }
        let parent = File::open(parent_path).map_err(DaemonErrorV1::Io)?;
        let parent_identity =
            ResourceIdentityV1::of(&parent.metadata().map_err(DaemonErrorV1::Io)?);
        if parent_identity != ResourceIdentityV1::of(&parent_metadata) {
            return Err(DaemonErrorV1::UnsafeEndpoint);
        }

        let lock_path = lifetime_path(socket_path)?;
        let lock_path_metadata =
            fs::symlink_metadata(&lock_path).map_err(|_| DaemonErrorV1::ResidueMismatch)?;
        if !private_regular_file(&lock_path_metadata) {
            return Err(DaemonErrorV1::UnsafeEndpoint);
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .map_err(DaemonErrorV1::Io)?;
        let lock_metadata = lock.metadata().map_err(DaemonErrorV1::Io)?;
        if !private_regular_file(&lock_metadata)
            || ResourceIdentityV1::of(&lock_path_metadata) != ResourceIdentityV1::of(&lock_metadata)
        {
            return Err(DaemonErrorV1::UnsafeEndpoint);
        }
        match lock.try_lock() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => return Err(DaemonErrorV1::LifetimeBusy),
            Err(error) => return Err(DaemonErrorV1::Io(error.into())),
        }
        let expected = record(previous_incarnation);
        let mut actual = [0_u8; RECORD_BYTES_V1];
        if lock_metadata.len() != RECORD_BYTES_V1 as u64
            || lock.read_exact_at(&mut actual, 0).is_err()
            || actual != expected
        {
            return Err(DaemonErrorV1::ResidueMismatch);
        }

        let socket_present = match fs::symlink_metadata(socket_path) {
            Ok(socket_metadata) => {
                if !socket_metadata.file_type().is_socket()
                    || socket_metadata.permissions().mode() & 0o777 != 0o600
                    || socket_metadata.nlink() != 1
                {
                    return Err(DaemonErrorV1::UnsafeEndpoint);
                }
                true
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => false,
            Err(error) => return Err(DaemonErrorV1::Io(error)),
        };

        let next_record = record(next_incarnation);
        lock.write_all_at(&next_record, 0)
            .and_then(|()| lock.set_len(next_record.len() as u64))
            .map_err(DaemonErrorV1::Io)?;
        interrupt(1).map_err(DaemonErrorV1::Io)?;
        lock.sync_all().map_err(DaemonErrorV1::Io)?;
        interrupt(2).map_err(DaemonErrorV1::Io)?;
        if socket_present {
            fs::remove_file(socket_path).map_err(DaemonErrorV1::Io)?;
        }
        interrupt(3).map_err(DaemonErrorV1::Io)?;
        parent.sync_all().map_err(DaemonErrorV1::Io)?;
        interrupt(4).map_err(DaemonErrorV1::Io)?;
        let lock_identity = ResourceIdentityV1::of(&lock.metadata().map_err(DaemonErrorV1::Io)?);
        Ok(Self {
            parent,
            parent_path: parent_path.to_path_buf(),
            parent_identity,
            lock,
            lock_path,
            lock_identity,
            record: next_record,
            remove_on_drop: true,
        })
    }

    pub(crate) fn intact(&self) -> bool {
        let Ok(parent_path_metadata) = fs::symlink_metadata(&self.parent_path) else {
            return false;
        };
        let Ok(parent_metadata) = self.parent.metadata() else {
            return false;
        };
        if !parent_path_metadata.is_dir()
            || parent_path_metadata.permissions().mode() & 0o777 != 0o700
            || ResourceIdentityV1::of(&parent_path_metadata) != self.parent_identity
            || ResourceIdentityV1::of(&parent_metadata) != self.parent_identity
        {
            return false;
        }
        let Ok(lock_path_metadata) = fs::symlink_metadata(&self.lock_path) else {
            return false;
        };
        let Ok(lock_metadata) = self.lock.metadata() else {
            return false;
        };
        if !private_regular_file(&lock_path_metadata)
            || !private_regular_file(&lock_metadata)
            || ResourceIdentityV1::of(&lock_path_metadata) != self.lock_identity
            || ResourceIdentityV1::of(&lock_metadata) != self.lock_identity
            || lock_metadata.len() != RECORD_BYTES_V1 as u64
        {
            return false;
        }
        let mut actual = [0_u8; RECORD_BYTES_V1];
        self.lock.read_exact_at(&mut actual, 0).is_ok() && actual == self.record
    }

    pub(crate) fn preserve(&mut self) {
        self.remove_on_drop = false;
    }

    pub(crate) fn release(&mut self) -> bool {
        self.remove_on_drop = false;
        self.intact() && fs::remove_file(&self.lock_path).is_ok() && self.parent.sync_all().is_ok()
    }
}

fn record(incarnation: ServerIncarnationV1) -> [u8; RECORD_BYTES_V1] {
    let mut record = [0_u8; RECORD_BYTES_V1];
    record[..8].copy_from_slice(&MAGIC_V1);
    record[8..].copy_from_slice(&incarnation.as_bytes());
    record
}

impl Drop for LifetimeLeaseV1 {
    fn drop(&mut self) {
        if self.remove_on_drop && self.intact() {
            let _ = fs::remove_file(&self.lock_path);
            let _ = self.parent.sync_all();
        }
    }
}

fn lifetime_path(socket_path: &Path) -> Result<PathBuf, DaemonErrorV1> {
    let parent = socket_path.parent().ok_or(DaemonErrorV1::UnsafeEndpoint)?;
    let mut name = socket_path
        .file_name()
        .ok_or(DaemonErrorV1::UnsafeEndpoint)?
        .to_os_string();
    name.push(".owner.lock");
    Ok(parent.join(name))
}

fn classify_existing_lifetime(path: &Path) -> DaemonErrorV1 {
    let Ok(path_metadata) = fs::symlink_metadata(path) else {
        return DaemonErrorV1::UnsafeEndpoint;
    };
    if !private_regular_file(&path_metadata) {
        return DaemonErrorV1::UnsafeEndpoint;
    }
    let Ok(lock) = OpenOptions::new().read(true).write(true).open(path) else {
        return DaemonErrorV1::UnsafeEndpoint;
    };
    let Ok(open_metadata) = lock.metadata() else {
        return DaemonErrorV1::UnsafeEndpoint;
    };
    if !private_regular_file(&open_metadata)
        || ResourceIdentityV1::of(&path_metadata) != ResourceIdentityV1::of(&open_metadata)
    {
        return DaemonErrorV1::UnsafeEndpoint;
    }
    match lock.try_lock() {
        Ok(()) => DaemonErrorV1::CrashResidue,
        Err(fs::TryLockError::WouldBlock) => DaemonErrorV1::LifetimeBusy,
        Err(error) => DaemonErrorV1::Io(error.into()),
    }
}

fn private_regular_file(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_file()
        && metadata.permissions().mode() & 0o777 == 0o600
        && metadata.nlink() == 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::DirBuilderExt;
    use std::os::unix::net::UnixListener;
    use std::process::{Command, Stdio};
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};

    static PROCESS_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn paths() -> (PathBuf, PathBuf, PathBuf) {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let parent = std::env::temp_dir().join(format!(
            "agora-lifetime-interruption-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&parent).unwrap();
        let socket = parent.join("agora.sock");
        let lock = lifetime_path(&socket).unwrap();
        (parent, socket, lock)
    }

    fn prepare_residue(socket: &Path, lock: &Path, incarnation: ServerIncarnationV1) {
        let listener = UnixListener::bind(socket).unwrap();
        fs::set_permissions(socket, fs::Permissions::from_mode(0o600)).unwrap();
        drop(listener);
        fs::write(lock, record(incarnation)).unwrap();
        fs::set_permissions(lock, fs::Permissions::from_mode(0o600)).unwrap();
    }

    #[test]
    fn every_rotation_interruption_is_exact_and_resumable() {
        let _process_test_guard = PROCESS_TEST_LOCK.lock().unwrap();
        let previous = ServerIncarnationV1::new([0xa5; 16]);
        let published = ServerIncarnationV1::new([0xb6; 16]);
        let resumed = ServerIncarnationV1::new([0xc7; 16]);

        for selected in 1..=4 {
            let (parent, socket, lock) = paths();
            prepare_residue(&socket, &lock, previous);
            let result = LifetimeLeaseV1::recover_stale_inner(
                &parent,
                &socket,
                previous,
                published,
                |point| {
                    if point == selected {
                        Err(io::Error::from(io::ErrorKind::Interrupted))
                    } else {
                        Ok(())
                    }
                },
            );
            assert!(
                matches!(result, Err(DaemonErrorV1::Io(error)) if error.kind() == io::ErrorKind::Interrupted),
                "point {selected} must not report success"
            );
            assert_eq!(fs::read(&lock).unwrap(), record(published));
            assert_eq!(
                socket.exists(),
                selected <= 2,
                "socket classification mismatch at point {selected}"
            );

            let lease =
                LifetimeLeaseV1::recover_stale(&parent, &socket, published, resumed).unwrap();
            assert!(lease.intact());
            assert_eq!(fs::read(&lock).unwrap(), record(resumed));
            assert!(!socket.exists());
            drop(lease);
            assert!(!lock.exists());
            fs::remove_dir(&parent).unwrap();
        }
    }

    #[test]
    fn every_rotation_process_cut_is_exact_and_resumable() {
        let _process_test_guard = PROCESS_TEST_LOCK.lock().unwrap();
        const CHILD_PARENT: &str = "AGORA_R81_LIFETIME_PARENT";
        const CHILD_POINT: &str = "AGORA_R81_LIFETIME_POINT";
        const CUT_EXIT: i32 = 86;
        let previous = ServerIncarnationV1::new([0xa5; 16]);
        let published = ServerIncarnationV1::new([0xb6; 16]);
        let resumed = ServerIncarnationV1::new([0xc7; 16]);

        if let (Ok(parent), Ok(selected)) =
            (std::env::var(CHILD_PARENT), std::env::var(CHILD_POINT))
        {
            let parent = PathBuf::from(parent);
            let socket = parent.join("agora.sock");
            let selected: u8 = selected.parse().unwrap();
            let _ = LifetimeLeaseV1::recover_stale_inner(
                &parent,
                &socket,
                previous,
                published,
                |point| {
                    if point == selected {
                        std::process::exit(CUT_EXIT);
                    }
                    Ok(())
                },
            );
            panic!("selected process cut was not reached");
        }

        for selected in 1..=4_u8 {
            let (parent, socket, lock) = paths();
            prepare_residue(&socket, &lock, previous);
            let status = Command::new(std::env::current_exe().unwrap())
                .arg("--exact")
                .arg("lifetime::tests::every_rotation_process_cut_is_exact_and_resumable")
                .arg("--test-threads=1")
                .env(CHILD_PARENT, &parent)
                .env(CHILD_POINT, selected.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap();
            assert_eq!(
                status.code(),
                Some(CUT_EXIT),
                "point {selected} must terminate inside rotation"
            );
            assert_eq!(fs::read(&lock).unwrap(), record(published));
            assert_eq!(
                socket.exists(),
                selected <= 2,
                "socket classification mismatch at point {selected}"
            );

            let lease =
                LifetimeLeaseV1::recover_stale(&parent, &socket, published, resumed).unwrap();
            assert!(lease.intact());
            assert_eq!(fs::read(&lock).unwrap(), record(resumed));
            assert!(!socket.exists());
            drop(lease);
            assert!(!lock.exists());
            fs::remove_dir(&parent).unwrap();
        }
    }
}
