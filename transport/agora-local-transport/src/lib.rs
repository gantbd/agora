//! Authority-free private Unix transport mechanics for Agora.
//!
//! This package owns endpoint lifetime, Unix socket resources, byte framing,
//! stream timeouts, and server-incarnation entropy. It deliberately knows
//! nothing about principals, authorization, conversations, services, hosts,
//! durability, or application routing.

#[cfg(not(unix))]
compile_error!("agora-local-transport currently qualifies only Unix hosts");

mod incarnation;
mod lifetime;

pub use agora_wire::local_handshake::ServerIncarnationV1;
pub use incarnation::{
    INCARNATION_GENERATION_ATTEMPTS_V1, IncarnationGenerationErrorV1, OsServerIncarnationEntropyV1,
    ServerIncarnationEntropyV1, generate_distinct_server_incarnation_v1,
    generate_server_incarnation_v1,
};

use lifetime::LifetimeLeaseV1;
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::ops::Range;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug)]
pub enum LocalTransportErrorV1 {
    UnsafeEndpoint,
    Io(io::Error),
    LifetimeBusy,
    CrashResidue,
    ResidueMismatch,
    Fenced,
    FrameLimit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalTransportCleanupV1 {
    Clean,
    ResiduePreserved,
    Incomplete,
}

/// One owned local endpoint with a pinned socket and lifetime record.
pub struct LocalEndpointV1 {
    listener: UnixListener,
    socket_path: PathBuf,
    socket_device: u64,
    socket_inode: u64,
    lifetime: LifetimeLeaseV1,
}

impl LocalEndpointV1 {
    pub fn bind_new(
        socket_path: impl AsRef<Path>,
        server_incarnation: ServerIncarnationV1,
    ) -> Result<Self, LocalTransportErrorV1> {
        let socket_path = socket_path.as_ref().to_path_buf();
        let parent = socket_path
            .parent()
            .ok_or(LocalTransportErrorV1::UnsafeEndpoint)?;
        let lifetime = LifetimeLeaseV1::acquire_new(parent, &socket_path, server_incarnation)?;
        Self::bind_owned(socket_path, lifetime)
    }

    pub fn recover_stale(
        socket_path: impl AsRef<Path>,
        previous_incarnation: ServerIncarnationV1,
        next_incarnation: ServerIncarnationV1,
    ) -> Result<Self, LocalTransportErrorV1> {
        let socket_path = socket_path.as_ref().to_path_buf();
        let parent = socket_path
            .parent()
            .ok_or(LocalTransportErrorV1::UnsafeEndpoint)?;
        let lifetime = LifetimeLeaseV1::recover_stale(
            parent,
            &socket_path,
            previous_incarnation,
            next_incarnation,
        )?;
        Self::bind_owned(socket_path, lifetime)
    }

    fn bind_owned(
        socket_path: PathBuf,
        lifetime: LifetimeLeaseV1,
    ) -> Result<Self, LocalTransportErrorV1> {
        match fs::symlink_metadata(&socket_path) {
            Ok(_) => {
                return Err(LocalTransportErrorV1::Io(io::Error::new(
                    io::ErrorKind::AddrInUse,
                    "socket path exists",
                )));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(LocalTransportErrorV1::Io(error)),
        }
        let listener = UnixListener::bind(&socket_path).map_err(LocalTransportErrorV1::Io)?;
        if let Err(error) = fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600)) {
            let _ = fs::remove_file(&socket_path);
            return Err(LocalTransportErrorV1::Io(error));
        }
        let metadata = match fs::symlink_metadata(&socket_path) {
            Ok(metadata) => metadata,
            Err(error) => {
                let _ = fs::remove_file(&socket_path);
                return Err(LocalTransportErrorV1::Io(error));
            }
        };
        if !metadata.file_type().is_socket()
            || metadata.permissions().mode() & 0o777 != 0o600
            || metadata.nlink() != 1
        {
            let _ = fs::remove_file(&socket_path);
            return Err(LocalTransportErrorV1::Io(io::Error::other(
                "unsafe socket resource",
            )));
        }
        Ok(Self {
            listener,
            socket_path,
            socket_device: metadata.dev(),
            socket_inode: metadata.ino(),
            lifetime,
        })
    }

    pub fn accept(&self) -> Result<LocalStreamV1, LocalTransportErrorV1> {
        self.listener
            .accept()
            .map(|(stream, _)| LocalStreamV1 { stream })
            .map_err(LocalTransportErrorV1::Io)
    }

    pub fn check_ownership(&mut self) -> Result<(), LocalTransportErrorV1> {
        let socket_owned = fs::symlink_metadata(&self.socket_path).is_ok_and(|metadata| {
            metadata.file_type().is_socket()
                && metadata.permissions().mode() & 0o777 == 0o600
                && metadata.nlink() == 1
                && metadata.dev() == self.socket_device
                && metadata.ino() == self.socket_inode
        });
        if socket_owned && self.lifetime.intact() {
            return Ok(());
        }
        self.lifetime.preserve();
        Err(LocalTransportErrorV1::Fenced)
    }

    pub fn teardown(&mut self) -> LocalTransportCleanupV1 {
        let endpoint_owned = fs::symlink_metadata(&self.socket_path).is_ok_and(|metadata| {
            metadata.file_type().is_socket()
                && metadata.nlink() == 1
                && metadata.dev() == self.socket_device
                && metadata.ino() == self.socket_inode
        });
        if !endpoint_owned || !self.lifetime.intact() {
            self.lifetime.preserve();
            return LocalTransportCleanupV1::ResiduePreserved;
        }
        if fs::remove_file(&self.socket_path).is_err() {
            self.lifetime.preserve();
            return LocalTransportCleanupV1::Incomplete;
        }
        if self.lifetime.release() {
            LocalTransportCleanupV1::Clean
        } else {
            LocalTransportCleanupV1::Incomplete
        }
    }
}

/// One accepted local byte stream. The application may authenticate the peer,
/// but socket resource operations remain encapsulated here.
pub struct LocalStreamV1 {
    stream: UnixStream,
}

impl LocalStreamV1 {
    /// Borrows the accepted OS stream for connection authentication (for
    /// example, peer-credential inspection). Ownership, framing, deadlines,
    /// and endpoint lifecycle remain transport responsibilities.
    pub fn as_unix_stream(&self) -> &UnixStream {
        &self.stream
    }

    pub fn set_io_timeout(&self, timeout: Duration) -> Result<(), LocalTransportErrorV1> {
        self.stream
            .set_read_timeout(Some(timeout))
            .and_then(|()| self.stream.set_write_timeout(Some(timeout)))
            .map_err(LocalTransportErrorV1::Io)
    }

    pub fn read_frame(
        &mut self,
        max_frame_bytes: usize,
    ) -> Result<Option<Vec<u8>>, LocalTransportErrorV1> {
        let mut header = [0_u8; 4];
        match self.stream.read(&mut header[..1]) {
            Ok(0) => return Ok(None),
            Ok(1) => {}
            Ok(_) => unreachable!(),
            Err(error) => return Err(LocalTransportErrorV1::Io(error)),
        }
        self.stream
            .read_exact(&mut header[1..])
            .map_err(LocalTransportErrorV1::Io)?;
        let length =
            validate_outer_frame_length(u32::from_be_bytes(header) as usize, max_frame_bytes)
                .map_err(|_| LocalTransportErrorV1::FrameLimit)?;
        let mut payload = vec![0_u8; length];
        self.stream
            .read_exact(&mut payload)
            .map_err(LocalTransportErrorV1::Io)?;
        Ok(Some(payload))
    }

    pub fn write_frame(
        &mut self,
        payload: &[u8],
        max_frame_bytes: usize,
    ) -> Result<(), LocalTransportErrorV1> {
        let frame = encode_outer_frame_v1(payload, max_frame_bytes)
            .map_err(|_| LocalTransportErrorV1::FrameLimit)?;
        self.stream
            .write_all(&frame)
            .and_then(|()| self.stream.flush())
            .map_err(LocalTransportErrorV1::Io)
    }
}

impl Read for LocalStreamV1 {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buffer)
    }
}

impl Write for LocalStreamV1 {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.stream.write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OuterFrameErrorV1 {
    Incomplete,
    Empty,
    Limit,
}

/// One decoded length-prefixed frame and the unconsumed stream suffix.
#[derive(Clone, Eq, PartialEq)]
pub struct OuterFrameV1<'a> {
    input: &'a [u8],
    payload: Range<usize>,
}

impl OuterFrameV1<'_> {
    pub fn payload(&self) -> &[u8] {
        &self.input[self.payload.clone()]
    }

    pub fn remainder(&self) -> &[u8] {
        &self.input[self.payload.end..]
    }
}

impl fmt::Debug for OuterFrameV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OuterFrameV1")
            .field("payload_bytes", &self.payload.len())
            .field("remainder_bytes", &self.remainder().len())
            .finish()
    }
}

pub fn decode_outer_frame_prefix_v1(
    input: &[u8],
    max_frame_bytes: usize,
) -> Result<OuterFrameV1<'_>, OuterFrameErrorV1> {
    let header: [u8; 4] = input
        .get(..4)
        .ok_or(OuterFrameErrorV1::Incomplete)?
        .try_into()
        .expect("four-byte slice");
    let length = validate_outer_frame_length(u32::from_be_bytes(header) as usize, max_frame_bytes)?;
    let end = 4_usize
        .checked_add(length)
        .ok_or(OuterFrameErrorV1::Limit)?;
    if input.len() < end {
        return Err(OuterFrameErrorV1::Incomplete);
    }
    Ok(OuterFrameV1 {
        input,
        payload: 4..end,
    })
}

pub fn encode_outer_frame_v1(
    payload: &[u8],
    max_frame_bytes: usize,
) -> Result<Vec<u8>, OuterFrameErrorV1> {
    validate_outer_frame_length(payload.len(), max_frame_bytes)?;
    let length = u32::try_from(payload.len()).map_err(|_| OuterFrameErrorV1::Limit)?;
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(payload);
    Ok(frame)
}

fn validate_outer_frame_length(
    length: usize,
    max_frame_bytes: usize,
) -> Result<usize, OuterFrameErrorV1> {
    if length == 0 {
        return Err(OuterFrameErrorV1::Empty);
    }
    if max_frame_bytes == 0 || length > max_frame_bytes || length > u32::MAX as usize {
        return Err(OuterFrameErrorV1::Limit);
    }
    Ok(length)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outer_frames_preserve_suffix_and_hide_content_in_debug() {
        let mut bytes = encode_outer_frame_v1(b"secret", 8).unwrap();
        bytes.extend_from_slice(b"tail");
        let frame = decode_outer_frame_prefix_v1(&bytes, 8).unwrap();
        assert_eq!(frame.payload(), b"secret");
        assert_eq!(frame.remainder(), b"tail");
        assert!(!format!("{frame:?}").contains("secret"));
    }
}
