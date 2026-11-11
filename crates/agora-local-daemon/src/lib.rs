//! Bounded private Unix transport for an Aegis-authorized Agora Host V2.
//!
//! The listener owns byte framing and connection lifetime only. A trusted
//! authenticator supplies one Aegis context per connection; the host owns the
//! opaque session and all conversation semantics.

#[cfg(not(unix))]
compile_error!("agora-local-daemon currently qualifies only Unix hosts");

mod lifetime;

use agora_aegis_verifier::{AegisContextV1, AegisVerifierV1};
use agora_host::v2::HostV2;
use agora_host::{HostRefusal, HostTimeV1, HostUsage, PrincipalVerifier};
pub use agora_wire::local_handshake::{
    CLIENT_HELLO_BYTES_V1, HandshakeErrorV1, SERVER_HELLO_BYTES_V1, ServerHelloV1,
    ServerIncarnationV1, decode_client_hello_v1, decode_server_hello_v1, encode_client_hello_v1,
    encode_server_hello_v1,
};
use lifetime::LifetimeLeaseV1;
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{fmt, ops::Range};

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
    pub draining: bool,
    pub fenced: bool,
    pub active_connections: u64,
    pub accepted_connections: u64,
    pub handshake_accepted: u64,
    pub handshake_refused: u64,
    pub completed_connections: u64,
    pub refused_connections: u64,
    pub transport_errors: u64,
    pub saturated: bool,
}

/// Content-free, point-in-time operational facts from each owned runtime layer.
///
/// Reading this value is passive: it performs no I/O, invokes no authority,
/// changes no counter, and cannot affect a conversation result. Counters remain
/// separated by measurement boundary so callers cannot mistake a transport
/// acceptance for host execution or service acceptance.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DaemonObservationV1 {
    pub transport: DaemonHealthV1,
    pub host: HostUsage,
    pub service: ServiceObservationV1,
    /// Sticky loss of counter fidelity in any represented layer.
    pub fidelity_lost: bool,
}

/// Service-boundary facts copied into the daemon's public observation profile.
/// This keeps the transport package from acquiring a direct service dependency.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ServiceObservationV1 {
    pub connect_offers: u64,
    pub connected: u64,
    pub request_offers: u64,
    pub accepted: u64,
    pub refused: u64,
    pub replayed: u64,
    pub conflicts: u64,
    pub disconnected: u64,
    pub overflowed: bool,
}

#[derive(Debug)]
pub enum DaemonErrorV1 {
    InvalidLimits,
    Draining,
    UnsafeEndpoint,
    Io(io::Error),
    Authentication,
    Handshake,
    LifetimeBusy,
    CrashResidue,
    Fenced,
    Host(HostRefusal),
    ConnectionRefused,
    FrameLimit,
    RequestLimit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OuterFrameErrorV1 {
    Incomplete,
    Empty,
    Limit,
}

/// One decoded length-prefixed frame and the unconsumed stream suffix.
///
/// Debug output is content-free because a frame may contain conversation data.
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

/// Decodes exactly one canonical big-endian length-prefixed frame prefix.
/// Any later bytes are returned as stream remainder rather than reinterpreted.
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

/// Encodes one canonical outer frame without interpreting its payload.
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

pub struct DaemonV1<P, A, C> {
    listener: UnixListener,
    socket_path: PathBuf,
    socket_device: u64,
    socket_inode: u64,
    lifetime: LifetimeLeaseV1,
    host: HostV2<AegisVerifierV1<P>>,
    authenticator: A,
    clock: C,
    limits: DaemonLimitsV1,
    server_incarnation: ServerIncarnationV1,
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
        server_incarnation: ServerIncarnationV1,
        limits: DaemonLimitsV1,
    ) -> Result<Self, DaemonErrorV1> {
        if limits.max_frame_bytes == 0
            || limits.max_frame_bytes > u32::MAX as usize
            || limits.max_frames_after_connect == 0
            || limits.max_frames_after_connect > u32::MAX as usize
            || limits.io_timeout.is_zero()
            || server_incarnation.is_zero()
        {
            return Err(DaemonErrorV1::InvalidLimits);
        }
        let socket_path = socket_path.as_ref().to_path_buf();
        let parent = socket_path.parent().ok_or(DaemonErrorV1::UnsafeEndpoint)?;
        let lifetime = LifetimeLeaseV1::acquire_new(parent, &socket_path, server_incarnation)?;
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
            lifetime,
            host,
            authenticator,
            clock,
            limits,
            server_incarnation,
            health: DaemonHealthV1 {
                ready: true,
                ..DaemonHealthV1::default()
            },
        })
    }

    pub const fn health(&self) -> DaemonHealthV1 {
        self.health
    }

    pub fn observation(&self) -> DaemonObservationV1 {
        let host = self.host.usage();
        let usage = self.host.service().usage();
        let service = ServiceObservationV1 {
            connect_offers: usage.connect_offers,
            connected: usage.connected,
            request_offers: usage.request_offers,
            accepted: usage.accepted,
            refused: usage.refused,
            replayed: usage.replayed,
            conflicts: usage.conflicts,
            disconnected: usage.disconnected,
            overflowed: usage.overflowed,
        };
        DaemonObservationV1 {
            transport: self.health,
            host,
            service,
            fidelity_lost: observation_fidelity_lost(self.health, host, service),
        }
    }

    pub const fn host(&self) -> &HostV2<AegisVerifierV1<P>> {
        &self.host
    }

    /// Irreversibly fences this sequential daemon from accepting more work.
    ///
    /// The mutable borrow means no connection can be in flight through this
    /// owner when the transition occurs. The endpoint remains owned until drop.
    pub fn begin_drain(&mut self) -> bool {
        if self.health.draining {
            return false;
        }
        self.health.ready = false;
        self.health.draining = true;
        true
    }

    pub fn serve_next(&mut self) -> Result<(), DaemonErrorV1> {
        if self.health.draining {
            return Err(DaemonErrorV1::Draining);
        }
        self.check_ownership()?;
        let (mut stream, _) = self.listener.accept().map_err(DaemonErrorV1::Io)?;
        increment(
            &mut self.health.active_connections,
            &mut self.health.saturated,
        );
        increment(
            &mut self.health.accepted_connections,
            &mut self.health.saturated,
        );
        let result = self
            .check_ownership()
            .and_then(|()| self.serve_stream(&mut stream));
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

    fn check_ownership(&mut self) -> Result<(), DaemonErrorV1> {
        let socket_owned = fs::symlink_metadata(&self.socket_path).is_ok_and(|metadata| {
            metadata.file_type().is_socket()
                && metadata.permissions().mode() & 0o777 == 0o600
                && metadata.dev() == self.socket_device
                && metadata.ino() == self.socket_inode
        });
        if socket_owned && self.lifetime.intact() {
            return Ok(());
        }
        self.health.ready = false;
        self.health.fenced = true;
        self.lifetime.preserve();
        Err(DaemonErrorV1::Fenced)
    }

    fn serve_stream(&mut self, stream: &mut UnixStream) -> Result<(), DaemonErrorV1> {
        stream
            .set_read_timeout(Some(self.limits.io_timeout))
            .and_then(|()| stream.set_write_timeout(Some(self.limits.io_timeout)))
            .map_err(DaemonErrorV1::Io)?;
        let mut client_hello = [0_u8; CLIENT_HELLO_BYTES_V1];
        stream
            .read_exact(&mut client_hello)
            .map_err(DaemonErrorV1::Io)?;
        if decode_client_hello_v1(&client_hello).is_err() {
            increment(
                &mut self.health.handshake_refused,
                &mut self.health.saturated,
            );
            return Err(DaemonErrorV1::Handshake);
        }
        let server_hello = encode_server_hello_v1(
            self.server_incarnation,
            u32::try_from(self.limits.max_frame_bytes).map_err(|_| DaemonErrorV1::Handshake)?,
            u32::try_from(self.limits.max_frames_after_connect)
                .map_err(|_| DaemonErrorV1::Handshake)?,
        )
        .map_err(|_| DaemonErrorV1::Handshake)?;
        stream
            .write_all(&server_hello)
            .and_then(|()| stream.flush())
            .map_err(DaemonErrorV1::Io)?;
        increment(
            &mut self.health.handshake_accepted,
            &mut self.health.saturated,
        );
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
        let endpoint_owned = fs::symlink_metadata(&self.socket_path).is_ok_and(|metadata| {
            metadata.file_type().is_socket()
                && metadata.dev() == self.socket_device
                && metadata.ino() == self.socket_inode
        });
        if endpoint_owned && self.lifetime.intact() {
            if fs::remove_file(&self.socket_path).is_err() {
                self.lifetime.preserve();
            }
        } else {
            self.lifetime.preserve();
        }
    }
}

const fn observation_fidelity_lost(
    transport: DaemonHealthV1,
    host: HostUsage,
    service: ServiceObservationV1,
) -> bool {
    transport.saturated || host.overflowed || service.overflowed
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
    let length = validate_outer_frame_length(u32::from_be_bytes(length) as usize, max_frame_bytes)
        .map_err(|_| DaemonErrorV1::FrameLimit)?;
    let mut frame = vec![0; length];
    stream.read_exact(&mut frame).map_err(DaemonErrorV1::Io)?;
    Ok(Some(frame))
}

fn write_frame(
    stream: &mut UnixStream,
    frame: &[u8],
    max_frame_bytes: usize,
) -> Result<(), DaemonErrorV1> {
    validate_outer_frame_length(frame.len(), max_frame_bytes)
        .map_err(|_| DaemonErrorV1::FrameLimit)?;
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

#[cfg(test)]
mod tests {
    use super::{
        DaemonHealthV1, HostUsage, OuterFrameErrorV1, ServiceObservationV1,
        decode_outer_frame_prefix_v1, encode_outer_frame_v1, increment, observation_fidelity_lost,
    };

    #[test]
    fn counter_saturation_is_sticky_and_value_preserving() {
        let mut value = u64::MAX;
        let mut saturated = false;
        increment(&mut value, &mut saturated);
        assert_eq!(value, u64::MAX);
        assert!(saturated);
        increment(&mut value, &mut saturated);
        assert_eq!(value, u64::MAX);
        assert!(saturated);
    }

    #[test]
    fn every_layer_contributes_to_aggregate_fidelity_loss() {
        assert!(!observation_fidelity_lost(
            DaemonHealthV1::default(),
            HostUsage::default(),
            ServiceObservationV1::default()
        ));
        assert!(observation_fidelity_lost(
            DaemonHealthV1 {
                saturated: true,
                ..DaemonHealthV1::default()
            },
            HostUsage::default(),
            ServiceObservationV1::default()
        ));
        assert!(observation_fidelity_lost(
            DaemonHealthV1::default(),
            HostUsage {
                overflowed: true,
                ..HostUsage::default()
            },
            ServiceObservationV1::default()
        ));
        assert!(observation_fidelity_lost(
            DaemonHealthV1::default(),
            HostUsage::default(),
            ServiceObservationV1 {
                overflowed: true,
                ..ServiceObservationV1::default()
            }
        ));
    }

    #[test]
    fn outer_frame_codec_is_bounded_prefix_exact_and_content_safe() {
        for prefix in [b"".as_slice(), b"\0", b"\0\0", b"\0\0\0"] {
            assert_eq!(
                decode_outer_frame_prefix_v1(prefix, 4),
                Err(OuterFrameErrorV1::Incomplete)
            );
        }
        assert_eq!(
            decode_outer_frame_prefix_v1(&[0, 0, 0, 0], 4),
            Err(OuterFrameErrorV1::Empty)
        );
        assert_eq!(
            decode_outer_frame_prefix_v1(&[0, 0, 0, 5], 4),
            Err(OuterFrameErrorV1::Limit)
        );
        assert_eq!(
            decode_outer_frame_prefix_v1(&[0, 0, 0, 4, 1, 2, 3], 4),
            Err(OuterFrameErrorV1::Incomplete)
        );

        let encoded = encode_outer_frame_v1(&[1, 2, 3, 4], 4).unwrap();
        let mut stream = encoded.clone();
        stream.extend_from_slice(&[9, 8]);
        let decoded = decode_outer_frame_prefix_v1(&stream, 4).unwrap();
        assert_eq!(decoded.payload(), [1, 2, 3, 4]);
        assert_eq!(decoded.remainder(), [9, 8]);
        assert_eq!(
            &stream[..4 + decoded.payload().len()],
            encoded,
            "accepted prefix must re-encode exactly"
        );
        let diagnostic = format!("{decoded:?}");
        assert!(!diagnostic.contains("1, 2, 3, 4"));
        assert_eq!(encode_outer_frame_v1(&[], 4), Err(OuterFrameErrorV1::Empty));
        assert_eq!(
            encode_outer_frame_v1(&[0; 5], 4),
            Err(OuterFrameErrorV1::Limit)
        );
    }
}
