//! Bounded private Unix transport for an Aegis-authorized Agora Host V2.
//!
//! The listener owns byte framing and connection lifetime only. A trusted
//! authenticator supplies one Aegis context per connection; the host owns the
//! opaque session and all conversation semantics.

#[cfg(not(unix))]
compile_error!("agora-local-daemon currently qualifies only Unix hosts");

use agora_aegis_verifier::{AegisContextV1, AegisVerifierV1};
use agora_host::v2::HostV2;
use agora_host::{HostRefusal, HostTimeV1, PrincipalVerifier};
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub trait AuthenticatorV1 {
    type Error;
    fn authenticate(&mut self, stream: &UnixStream) -> Result<AegisContextV1, Self::Error>;
}

pub trait HostClockV1 {
    fn now(&mut self) -> HostTimeV1;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DaemonLimitsV1 {
    pub max_frame_bytes: usize,
    pub max_frames_after_connect: usize,
    pub io_timeout: Duration,
}

impl Default for DaemonLimitsV1 {
    fn default() -> Self {
        Self {
            max_frame_bytes: 8 * 1_024,
            max_frames_after_connect: 1_024,
            io_timeout: Duration::from_secs(5),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DaemonHealthV1 {
    pub ready: bool,
    pub active_connections: u64,
    pub accepted_connections: u64,
    pub completed_connections: u64,
    pub refused_connections: u64,
    pub transport_errors: u64,
    pub saturated: bool,
}

#[derive(Debug)]
pub enum DaemonErrorV1 {
    InvalidLimits,
    UnsafeEndpoint,
    Io(io::Error),
    Authentication,
    Host(HostRefusal),
    ConnectionRefused,
    FrameLimit,
    RequestLimit,
}

pub struct DaemonV1<P, A, C> {
    listener: UnixListener,
    socket_path: PathBuf,
    socket_device: u64,
    socket_inode: u64,
    host: HostV2<AegisVerifierV1<P>>,
    authenticator: A,
    clock: C,
    limits: DaemonLimitsV1,
    health: DaemonHealthV1,
}

impl<P, A, C> DaemonV1<P, A, C>
where
    AegisVerifierV1<P>: PrincipalVerifier<Context = AegisContextV1>,
    A: AuthenticatorV1,
    C: HostClockV1,
{
    pub fn bind_new(
        socket_path: impl AsRef<Path>,
        host: HostV2<AegisVerifierV1<P>>,
        authenticator: A,
        clock: C,
        limits: DaemonLimitsV1,
    ) -> Result<Self, DaemonErrorV1> {
        if limits.max_frame_bytes == 0
            || limits.max_frame_bytes > u32::MAX as usize
            || limits.max_frames_after_connect == 0
            || limits.io_timeout.is_zero()
        {
            return Err(DaemonErrorV1::InvalidLimits);
        }
        let socket_path = socket_path.as_ref().to_path_buf();
        let parent = socket_path.parent().ok_or(DaemonErrorV1::UnsafeEndpoint)?;
        let parent_metadata = fs::symlink_metadata(parent).map_err(DaemonErrorV1::Io)?;
        if !parent_metadata.is_dir() || parent_metadata.permissions().mode() & 0o777 != 0o700 {
            return Err(DaemonErrorV1::UnsafeEndpoint);
        }
        match fs::symlink_metadata(&socket_path) {
            Ok(_) => {
                return Err(DaemonErrorV1::Io(io::Error::new(
                    io::ErrorKind::AddrInUse,
                    "socket path exists",
                )));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(DaemonErrorV1::Io(error)),
        }
        let listener = UnixListener::bind(&socket_path).map_err(DaemonErrorV1::Io)?;
        if let Err(error) = fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600)) {
            let _ = fs::remove_file(&socket_path);
            return Err(DaemonErrorV1::Io(error));
        }
        let metadata = match fs::symlink_metadata(&socket_path) {
            Ok(metadata) => metadata,
            Err(error) => {
                let _ = fs::remove_file(&socket_path);
                return Err(DaemonErrorV1::Io(error));
            }
        };
        if !metadata.file_type().is_socket() || metadata.permissions().mode() & 0o777 != 0o600 {
            let _ = fs::remove_file(&socket_path);
            return Err(DaemonErrorV1::Io(io::Error::other(
                "unsafe socket resource",
            )));
        }
        Ok(Self {
            listener,
            socket_path,
            socket_device: metadata.dev(),
            socket_inode: metadata.ino(),
            host,
            authenticator,
            clock,
            limits,
            health: DaemonHealthV1 {
                ready: true,
                ..DaemonHealthV1::default()
            },
        })
    }

    pub const fn health(&self) -> DaemonHealthV1 {
        self.health
    }

    pub const fn host(&self) -> &HostV2<AegisVerifierV1<P>> {
        &self.host
    }

    pub fn serve_next(&mut self) -> Result<(), DaemonErrorV1> {
        let (mut stream, _) = self.listener.accept().map_err(DaemonErrorV1::Io)?;
        increment(
            &mut self.health.active_connections,
            &mut self.health.saturated,
        );
        increment(
            &mut self.health.accepted_connections,
            &mut self.health.saturated,
        );
        let result = self.serve_stream(&mut stream);
        self.health.active_connections = self.health.active_connections.saturating_sub(1);
        match &result {
            Ok(()) => increment(
                &mut self.health.completed_connections,
                &mut self.health.saturated,
            ),
            Err(DaemonErrorV1::Io(_) | DaemonErrorV1::FrameLimit | DaemonErrorV1::RequestLimit) => {
                increment(
                    &mut self.health.transport_errors,
                    &mut self.health.saturated,
                )
            }
            Err(_) => increment(
                &mut self.health.refused_connections,
                &mut self.health.saturated,
            ),
        }
        result
    }

    fn serve_stream(&mut self, stream: &mut UnixStream) -> Result<(), DaemonErrorV1> {
        stream
            .set_read_timeout(Some(self.limits.io_timeout))
            .and_then(|()| stream.set_write_timeout(Some(self.limits.io_timeout)))
            .map_err(DaemonErrorV1::Io)?;
        let context = self
            .authenticator
            .authenticate(stream)
            .map_err(|_| DaemonErrorV1::Authentication)?;
        let connect = read_frame(stream, self.limits.max_frame_bytes)?
            .ok_or_else(|| DaemonErrorV1::Io(io::Error::from(io::ErrorKind::UnexpectedEof)))?;
        let opened = self
            .host
            .open(&connect, &context, self.clock.now())
            .map_err(DaemonErrorV1::Host)?;
        let Some(session) = opened.session() else {
            write_frame(stream, opened.frame(), self.limits.max_frame_bytes)?;
            return Err(DaemonErrorV1::ConnectionRefused);
        };

        let result = (|| {
            write_frame(stream, opened.frame(), self.limits.max_frame_bytes)?;
            for _ in 0..self.limits.max_frames_after_connect {
                let Some(frame) = read_frame(stream, self.limits.max_frame_bytes)? else {
                    return Ok(());
                };
                let reply = self
                    .host
                    .handle(session, &frame, &context, self.clock.now())
                    .map_err(DaemonErrorV1::Host)?;
                write_frame(stream, &reply, self.limits.max_frame_bytes)?;
            }
            Err(DaemonErrorV1::RequestLimit)
        })();
        let _ = self.host.close(session);
        result
    }
}

impl<P, A, C> Drop for DaemonV1<P, A, C> {
    fn drop(&mut self) {
        self.health.ready = false;
        if let Ok(metadata) = fs::symlink_metadata(&self.socket_path)
            && metadata.file_type().is_socket()
            && metadata.dev() == self.socket_device
            && metadata.ino() == self.socket_inode
        {
            let _ = fs::remove_file(&self.socket_path);
        }
    }
}

fn read_frame(
    stream: &mut UnixStream,
    max_frame_bytes: usize,
) -> Result<Option<Vec<u8>>, DaemonErrorV1> {
    let mut length = [0_u8; 4];
    match stream.read(&mut length[..1]) {
        Ok(0) => return Ok(None),
        Ok(1) => {}
        Ok(_) => unreachable!(),
        Err(error) => return Err(DaemonErrorV1::Io(error)),
    }
    stream
        .read_exact(&mut length[1..])
        .map_err(DaemonErrorV1::Io)?;
    let length = u32::from_be_bytes(length) as usize;
    if length == 0 || length > max_frame_bytes {
        return Err(DaemonErrorV1::FrameLimit);
    }
    let mut frame = vec![0; length];
    stream.read_exact(&mut frame).map_err(DaemonErrorV1::Io)?;
    Ok(Some(frame))
}

fn write_frame(
    stream: &mut UnixStream,
    frame: &[u8],
    max_frame_bytes: usize,
) -> Result<(), DaemonErrorV1> {
    if frame.is_empty() || frame.len() > max_frame_bytes {
        return Err(DaemonErrorV1::FrameLimit);
    }
    let length = u32::try_from(frame.len()).map_err(|_| DaemonErrorV1::FrameLimit)?;
    stream
        .write_all(&length.to_be_bytes())
        .and_then(|()| stream.write_all(frame))
        .and_then(|()| stream.flush())
        .map_err(DaemonErrorV1::Io)
}

fn increment(value: &mut u64, saturated: &mut bool) {
    if let Some(next) = value.checked_add(1) {
        *value = next;
    } else {
        *saturated = true;
    }
}
