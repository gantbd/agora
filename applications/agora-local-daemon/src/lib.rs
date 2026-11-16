//! Aegis-authorized Agora Host V2 application composition.
//!
//! The authority-free local transport owns endpoint and byte mechanics. This
//! application selects authentication, routing, Host ownership, durability,
//! clocks, recovery policy, and lifecycle observations.

#[cfg(not(unix))]
compile_error!("agora-local-daemon currently qualifies only Unix hosts");

pub use agora_local_transport::{
    INCARNATION_GENERATION_ATTEMPTS_V1, IncarnationGenerationErrorV1, OsServerIncarnationEntropyV1,
    OuterFrameErrorV1, OuterFrameV1, ServerIncarnationEntropyV1, decode_outer_frame_prefix_v1,
    encode_outer_frame_v1, generate_distinct_server_incarnation_v1, generate_server_incarnation_v1,
};

use agora_aegis_verifier::{AegisContextV1, AegisVerifierV1};
use agora_durable::{
    DurableHostObservationV2, DurableHostPublicationV2, DurableHostSuppressionV2, DurableHostV2,
    DurableStoreObservationV2, DurableTransientHostV1, ObservableCheckpointStoreV1,
};
use agora_host::search::{SearchHostV1, SearchPortV1};
use agora_host::transient::TransientHostV1;
use agora_host::v2::{HostSessionIdV2, HostV2, OpenReplyV2};
use agora_host::{HostRefusal, HostTimeV1, HostUsage, PrincipalVerifier};
use agora_local_transport::{
    LocalEndpointV1, LocalStreamV1, LocalTransportCleanupV1, LocalTransportErrorV1,
};
pub use agora_wire::local_handshake::{
    CLIENT_HELLO_BYTES_V1, COLLABORATION_CLIENT_HELLO_BYTES_V1,
    COLLABORATION_SERVER_HELLO_BYTES_V1, COMPOSITION_CLIENT_HELLO_BYTES_V1,
    COMPOSITION_SERVER_HELLO_BYTES_V1, CollaborationServerHelloV1, CompositionServerHelloV1,
    HandshakeErrorV1, SEARCH_CLIENT_HELLO_BYTES_V1, SEARCH_SERVER_HELLO_BYTES_V1,
    SERVER_HELLO_BYTES_V1, SearchServerHelloV1, ServerHelloV1, ServerIncarnationV1,
    decode_client_hello_v1, decode_collaboration_client_hello_v1,
    decode_collaboration_server_hello_v1, decode_composition_client_hello_v1,
    decode_composition_server_hello_v1, decode_search_client_hello_v1,
    decode_search_server_hello_v1, decode_server_hello_v1, encode_client_hello_v1,
    encode_collaboration_client_hello_v1, encode_collaboration_server_hello_v1,
    encode_composition_client_hello_v1, encode_composition_server_hello_v1,
    encode_search_client_hello_v1, encode_search_server_hello_v1, encode_server_hello_v1,
};
pub use agora_wire::local_transient::{
    LOCAL_SESSION_HELLO_BYTES_V1, LocalReplayEpochV1, LocalRouteErrorV1, LocalRouteV1,
    decode_local_routed_frame_v1, decode_local_session_hello_v1, encode_local_routed_frame_v1,
    encode_local_session_hello_v1,
};
use agora_wire::scheduled::{
    ScheduledWireReplyV1, ScheduledWireRequestV1, decode_scheduled_request_v1,
    encode_scheduled_reply_v1,
};
use keystone_storage::CheckpointStore;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
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
pub enum DaemonLifecycleV1 {
    /// Construction is not yet complete and no endpoint may be advertised.
    #[default]
    Starting,
    Ready,
    Draining,
    Stopped,
    Fenced,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DaemonCleanupV1 {
    Clean,
    ResiduePreserved,
    Incomplete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DaemonStoppedV1 {
    pub prior_lifecycle: DaemonLifecycleV1,
    pub health: DaemonHealthV1,
    pub cleanup: DaemonCleanupV1,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DaemonHealthV1 {
    pub lifecycle: DaemonLifecycleV1,
    pub active_connections: u64,
    pub accepted_connections: u64,
    pub handshake_accepted: u64,
    pub handshake_refused: u64,
    pub completed_connections: u64,
    pub refused_connections: u64,
    pub transport_errors: u64,
    pub saturated: bool,
}

impl DaemonHealthV1 {
    pub const fn ready(self) -> bool {
        matches!(self.lifecycle, DaemonLifecycleV1::Ready)
    }

    pub const fn drain_complete(self) -> bool {
        matches!(self.lifecycle, DaemonLifecycleV1::Draining) && self.active_connections == 0
    }
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

/// Passive durable-daemon facts without exposing an untrustworthy fenced Host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DurableDaemonObservationV1 {
    pub transport: DaemonHealthV1,
    pub runtime: Option<DaemonObservationV1>,
    pub durability: DurableHostObservationV2,
    pub fidelity_lost: bool,
}

/// Passive daemon, durable-publication, and physical-store facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalDurableDaemonObservationV1<O> {
    pub transport: DaemonHealthV1,
    pub runtime: Option<DaemonObservationV1>,
    pub durability: DurableHostObservationV2,
    pub physical: O,
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
    ResidueMismatch,
    Fenced,
    Durability,
    Host(HostRefusal),
    ConnectionRefused,
    FrameLimit,
    Route,
    RequestLimit,
    ReplayEpochExhausted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub enum DaemonHostErrorV1 {
    Host(HostRefusal),
    Durability,
}

mod sealed {
    pub trait Sealed {}
}

/// Sealed transport-facing projection implemented only by Agora's volatile,
/// transient, durable, and combined durable/transient authenticated Host V2
/// owners. It is not a general runtime port.
#[doc(hidden)]
pub trait DaemonHostV1: sealed::Sealed {
    fn open_daemon(
        &mut self,
        frame: &[u8],
        context: &AegisContextV1,
        now: HostTimeV1,
    ) -> Result<OpenReplyV2, DaemonHostErrorV1>;
    fn handle_daemon(
        &mut self,
        session: HostSessionIdV2,
        frame: &[u8],
        context: &AegisContextV1,
        now: HostTimeV1,
    ) -> Result<Vec<u8>, DaemonHostErrorV1>;
    fn handle_transient_daemon(
        &mut self,
        session: HostSessionIdV2,
        frame: &[u8],
        context: &AegisContextV1,
        now: HostTimeV1,
    ) -> Result<Vec<u8>, DaemonHostErrorV1>;
    fn handle_search_daemon(
        &mut self,
        _session: HostSessionIdV2,
        _frame: &[u8],
        _context: &AegisContextV1,
        _now: HostTimeV1,
    ) -> Result<Vec<u8>, DaemonHostErrorV1> {
        Err(DaemonHostErrorV1::Host(HostRefusal::ProtocolViolation))
    }
    fn handle_scheduled_daemon(
        &mut self,
        _session: HostSessionIdV2,
        _frame: &[u8],
        _context: &AegisContextV1,
        _now: HostTimeV1,
    ) -> Result<Vec<u8>, DaemonHostErrorV1> {
        Err(DaemonHostErrorV1::Host(HostRefusal::ProtocolViolation))
    }
    fn close_daemon(&mut self, session: HostSessionIdV2) -> Result<bool, DaemonHostErrorV1>;
}

impl<P> sealed::Sealed for HostV2<AegisVerifierV1<P>> where
    AegisVerifierV1<P>: PrincipalVerifier<Context = AegisContextV1>
{
}

impl<P> DaemonHostV1 for HostV2<AegisVerifierV1<P>>
where
    AegisVerifierV1<P>: agora_host::ScheduledPrincipalVerifier<Context = AegisContextV1>,
{
    fn open_daemon(
        &mut self,
        frame: &[u8],
        context: &AegisContextV1,
        now: HostTimeV1,
    ) -> Result<OpenReplyV2, DaemonHostErrorV1> {
        self.open(frame, context, now)
            .map_err(DaemonHostErrorV1::Host)
    }

    fn handle_daemon(
        &mut self,
        session: HostSessionIdV2,
        frame: &[u8],
        context: &AegisContextV1,
        now: HostTimeV1,
    ) -> Result<Vec<u8>, DaemonHostErrorV1> {
        self.handle(session, frame, context, now)
            .map_err(DaemonHostErrorV1::Host)
    }

    fn close_daemon(&mut self, session: HostSessionIdV2) -> Result<bool, DaemonHostErrorV1> {
        Ok(self.close(session))
    }

    fn handle_transient_daemon(
        &mut self,
        _session: HostSessionIdV2,
        _frame: &[u8],
        _context: &AegisContextV1,
        _now: HostTimeV1,
    ) -> Result<Vec<u8>, DaemonHostErrorV1> {
        Err(DaemonHostErrorV1::Host(HostRefusal::ProtocolViolation))
    }

    fn handle_scheduled_daemon(
        &mut self,
        session: HostSessionIdV2,
        frame: &[u8],
        context: &AegisContextV1,
        now: HostTimeV1,
    ) -> Result<Vec<u8>, DaemonHostErrorV1> {
        handle_scheduled_host(self, session, frame, context, now)
    }
}

impl<P, S> sealed::Sealed for DurableHostV2<AegisVerifierV1<P>, S>
where
    AegisVerifierV1<P>: PrincipalVerifier<Context = AegisContextV1>,
    S: CheckpointStore,
{
}

impl<P, S> DaemonHostV1 for DurableHostV2<AegisVerifierV1<P>, S>
where
    AegisVerifierV1<P>: agora_host::ScheduledPrincipalVerifier<Context = AegisContextV1>,
    S: CheckpointStore,
{
    fn open_daemon(
        &mut self,
        frame: &[u8],
        context: &AegisContextV1,
        now: HostTimeV1,
    ) -> Result<OpenReplyV2, DaemonHostErrorV1> {
        durable_value(self.open(frame, context, now))?.map_err(DaemonHostErrorV1::Host)
    }

    fn handle_daemon(
        &mut self,
        session: HostSessionIdV2,
        frame: &[u8],
        context: &AegisContextV1,
        now: HostTimeV1,
    ) -> Result<Vec<u8>, DaemonHostErrorV1> {
        durable_value(self.handle(session, frame, context, now))?.map_err(DaemonHostErrorV1::Host)
    }

    fn close_daemon(&mut self, session: HostSessionIdV2) -> Result<bool, DaemonHostErrorV1> {
        durable_value(self.close(session))
    }

    fn handle_transient_daemon(
        &mut self,
        _session: HostSessionIdV2,
        _frame: &[u8],
        _context: &AegisContextV1,
        _now: HostTimeV1,
    ) -> Result<Vec<u8>, DaemonHostErrorV1> {
        Err(DaemonHostErrorV1::Host(HostRefusal::ProtocolViolation))
    }

    fn handle_scheduled_daemon(
        &mut self,
        session: HostSessionIdV2,
        frame: &[u8],
        context: &AegisContextV1,
        now: HostTimeV1,
    ) -> Result<Vec<u8>, DaemonHostErrorV1> {
        let request = decode_scheduled_request_v1(
            frame,
            self.host()
                .ok_or(DaemonHostErrorV1::Durability)?
                .limits()
                .wire,
        )
        .map_err(|_| DaemonHostErrorV1::Host(HostRefusal::ProtocolViolation))?;
        let reply = match request {
            ScheduledWireRequestV1::Publish(request) => ScheduledWireReplyV1::Published(
                durable_value(self.publish_scheduled(session, request, context, now))?,
            ),
            ScheduledWireRequestV1::Observe(request) => ScheduledWireReplyV1::Observed(
                durable_value(self.observe_scheduled(session, request, context, now))?,
            ),
        };
        let limits = self
            .host()
            .ok_or(DaemonHostErrorV1::Durability)?
            .limits()
            .wire;
        encode_scheduled_reply_v1(&reply, limits)
            .map_err(|_| DaemonHostErrorV1::Host(HostRefusal::Unavailable))
    }
}

fn handle_scheduled_host<P>(
    host: &mut HostV2<AegisVerifierV1<P>>,
    session: HostSessionIdV2,
    frame: &[u8],
    context: &AegisContextV1,
    now: HostTimeV1,
) -> Result<Vec<u8>, DaemonHostErrorV1>
where
    AegisVerifierV1<P>: agora_host::ScheduledPrincipalVerifier<Context = AegisContextV1>,
{
    let limits = host.limits().wire;
    let request = decode_scheduled_request_v1(frame, limits)
        .map_err(|_| DaemonHostErrorV1::Host(HostRefusal::ProtocolViolation))?;
    let reply = match request {
        ScheduledWireRequestV1::Publish(request) => {
            ScheduledWireReplyV1::Published(host.publish_scheduled(session, request, context, now))
        }
        ScheduledWireRequestV1::Observe(request) => {
            ScheduledWireReplyV1::Observed(host.observe_scheduled(session, request, context, now))
        }
    };
    encode_scheduled_reply_v1(&reply, limits)
        .map_err(|_| DaemonHostErrorV1::Host(HostRefusal::Unavailable))
}

impl<P> sealed::Sealed for TransientHostV1<AegisVerifierV1<P>> where
    AegisVerifierV1<P>: PrincipalVerifier<Context = AegisContextV1>
{
}

impl<P> DaemonHostV1 for TransientHostV1<AegisVerifierV1<P>>
where
    AegisVerifierV1<P>: PrincipalVerifier<Context = AegisContextV1>,
{
    fn open_daemon(
        &mut self,
        frame: &[u8],
        context: &AegisContextV1,
        now: HostTimeV1,
    ) -> Result<OpenReplyV2, DaemonHostErrorV1> {
        self.open(frame, context, now)
            .map_err(DaemonHostErrorV1::Host)
    }

    fn handle_daemon(
        &mut self,
        session: HostSessionIdV2,
        frame: &[u8],
        context: &AegisContextV1,
        now: HostTimeV1,
    ) -> Result<Vec<u8>, DaemonHostErrorV1> {
        self.handle(session, frame, context, now)
            .map_err(DaemonHostErrorV1::Host)
    }

    fn handle_transient_daemon(
        &mut self,
        session: HostSessionIdV2,
        frame: &[u8],
        context: &AegisContextV1,
        now: HostTimeV1,
    ) -> Result<Vec<u8>, DaemonHostErrorV1> {
        self.handle_transient(session, frame, context, now)
            .map_err(DaemonHostErrorV1::Host)
    }

    fn close_daemon(&mut self, session: HostSessionIdV2) -> Result<bool, DaemonHostErrorV1> {
        Ok(self.close(session))
    }
}

impl<P, S> sealed::Sealed for DurableTransientHostV1<AegisVerifierV1<P>, S>
where
    AegisVerifierV1<P>: PrincipalVerifier<Context = AegisContextV1>,
    S: CheckpointStore,
{
}

impl<P, S> DaemonHostV1 for DurableTransientHostV1<AegisVerifierV1<P>, S>
where
    AegisVerifierV1<P>: PrincipalVerifier<Context = AegisContextV1>,
    S: CheckpointStore,
{
    fn open_daemon(
        &mut self,
        frame: &[u8],
        context: &AegisContextV1,
        now: HostTimeV1,
    ) -> Result<OpenReplyV2, DaemonHostErrorV1> {
        durable_value(self.open(frame, context, now))?.map_err(DaemonHostErrorV1::Host)
    }

    fn handle_daemon(
        &mut self,
        session: HostSessionIdV2,
        frame: &[u8],
        context: &AegisContextV1,
        now: HostTimeV1,
    ) -> Result<Vec<u8>, DaemonHostErrorV1> {
        durable_value(self.handle(session, frame, context, now))?.map_err(DaemonHostErrorV1::Host)
    }

    fn handle_transient_daemon(
        &mut self,
        session: HostSessionIdV2,
        frame: &[u8],
        context: &AegisContextV1,
        now: HostTimeV1,
    ) -> Result<Vec<u8>, DaemonHostErrorV1> {
        durable_value(self.handle_transient(session, frame, context, now))?
            .map_err(DaemonHostErrorV1::Host)
    }

    fn close_daemon(&mut self, session: HostSessionIdV2) -> Result<bool, DaemonHostErrorV1> {
        durable_value(self.close(session))
    }
}

impl<P, Q> sealed::Sealed for SearchHostV1<AegisVerifierV1<P>, Q>
where
    AegisVerifierV1<P>: PrincipalVerifier<Context = AegisContextV1>,
    Q: SearchPortV1,
{
}

impl<P, Q> DaemonHostV1 for SearchHostV1<AegisVerifierV1<P>, Q>
where
    AegisVerifierV1<P>: PrincipalVerifier<Context = AegisContextV1>,
    Q: SearchPortV1,
{
    fn open_daemon(
        &mut self,
        frame: &[u8],
        context: &AegisContextV1,
        now: HostTimeV1,
    ) -> Result<OpenReplyV2, DaemonHostErrorV1> {
        self.open(frame, context, now)
            .map_err(DaemonHostErrorV1::Host)
    }

    fn handle_daemon(
        &mut self,
        session: HostSessionIdV2,
        frame: &[u8],
        context: &AegisContextV1,
        now: HostTimeV1,
    ) -> Result<Vec<u8>, DaemonHostErrorV1> {
        self.handle(session, frame, context, now)
            .map_err(DaemonHostErrorV1::Host)
    }

    fn handle_transient_daemon(
        &mut self,
        _session: HostSessionIdV2,
        _frame: &[u8],
        _context: &AegisContextV1,
        _now: HostTimeV1,
    ) -> Result<Vec<u8>, DaemonHostErrorV1> {
        Err(DaemonHostErrorV1::Host(HostRefusal::ProtocolViolation))
    }

    fn handle_search_daemon(
        &mut self,
        session: HostSessionIdV2,
        frame: &[u8],
        context: &AegisContextV1,
        now: HostTimeV1,
    ) -> Result<Vec<u8>, DaemonHostErrorV1> {
        self.handle_search(session, frame, context, now)
            .map_err(DaemonHostErrorV1::Host)
    }

    fn close_daemon(&mut self, session: HostSessionIdV2) -> Result<bool, DaemonHostErrorV1> {
        Ok(self.close(session))
    }
}

fn durable_value<T>(publication: DurableHostPublicationV2<T>) -> Result<T, DaemonHostErrorV1> {
    match publication {
        DurableHostPublicationV2::Published { value, .. } => Ok(value),
        DurableHostPublicationV2::Suppressed(
            DurableHostSuppressionV2::Fenced
            | DurableHostSuppressionV2::Checkpoint(_)
            | DurableHostSuppressionV2::StoreRefused(_)
            | DurableHostSuppressionV2::OutcomeUnknown
            | DurableHostSuppressionV2::NotDurable
            | DurableHostSuppressionV2::InvalidReceipt,
        ) => Err(DaemonHostErrorV1::Durability),
    }
}

pub struct DaemonV1<H, A, C> {
    endpoint: LocalEndpointV1,
    host: H,
    authenticator: A,
    clock: C,
    limits: DaemonLimitsV1,
    server_incarnation: ServerIncarnationV1,
    health: DaemonHealthV1,
    transport_profile: DaemonTransportProfileV1,
    next_connection_epoch: u128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DaemonTransportProfileV1 {
    Conversation,
    Collaboration,
    Search,
    Composition,
}

impl<H, A, C> DaemonV1<H, A, C>
where
    H: DaemonHostV1,
    A: AuthenticatorV1,
    C: HostClockV1,
{
    pub fn bind_new(
        socket_path: impl AsRef<Path>,
        host: H,
        authenticator: A,
        clock: C,
        server_incarnation: ServerIncarnationV1,
        limits: DaemonLimitsV1,
    ) -> Result<Self, DaemonErrorV1> {
        validate_daemon_configuration(server_incarnation, limits)?;
        let endpoint = LocalEndpointV1::bind_new(socket_path, server_incarnation)
            .map_err(map_transport_error)?;
        Self::bind_owned(
            endpoint,
            host,
            authenticator,
            clock,
            server_incarnation,
            limits,
            DaemonTransportProfileV1::Conversation,
        )
    }

    /// Replaces exact unlocked crash residue with a distinct declared lifetime.
    ///
    /// This does not recover or adopt the prior volatile Host. The caller must
    /// supply a fresh Host and prove the expected prior incarnation exactly.
    #[allow(clippy::too_many_arguments)]
    pub fn recover_stale(
        socket_path: impl AsRef<Path>,
        host: H,
        authenticator: A,
        clock: C,
        previous_incarnation: ServerIncarnationV1,
        next_incarnation: ServerIncarnationV1,
        limits: DaemonLimitsV1,
    ) -> Result<Self, DaemonErrorV1> {
        validate_daemon_configuration(next_incarnation, limits)?;
        let endpoint =
            LocalEndpointV1::recover_stale(socket_path, previous_incarnation, next_incarnation)
                .map_err(map_transport_error)?;
        Self::bind_owned(
            endpoint,
            host,
            authenticator,
            clock,
            next_incarnation,
            limits,
            DaemonTransportProfileV1::Conversation,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn bind_owned(
        endpoint: LocalEndpointV1,
        host: H,
        authenticator: A,
        clock: C,
        server_incarnation: ServerIncarnationV1,
        limits: DaemonLimitsV1,
        transport_profile: DaemonTransportProfileV1,
    ) -> Result<Self, DaemonErrorV1> {
        Ok(Self {
            endpoint,
            host,
            authenticator,
            clock,
            limits,
            server_incarnation,
            health: DaemonHealthV1 {
                lifecycle: DaemonLifecycleV1::Ready,
                ..DaemonHealthV1::default()
            },
            transport_profile,
            next_connection_epoch: 1,
        })
    }

    pub const fn health(&self) -> DaemonHealthV1 {
        self.health
    }

    /// Irreversibly fences this sequential daemon from accepting more work.
    ///
    /// The mutable borrow means no connection can be in flight through this
    /// owner when the transition occurs. The endpoint remains owned until teardown.
    pub fn begin_drain(&mut self) -> bool {
        match self.health.lifecycle {
            DaemonLifecycleV1::Ready => {
                self.health.lifecycle = DaemonLifecycleV1::Draining;
                true
            }
            DaemonLifecycleV1::Starting
            | DaemonLifecycleV1::Draining
            | DaemonLifecycleV1::Stopped
            | DaemonLifecycleV1::Fenced => false,
        }
    }

    pub fn serve_next(&mut self) -> Result<(), DaemonErrorV1> {
        match self.health.lifecycle {
            DaemonLifecycleV1::Ready => {}
            DaemonLifecycleV1::Draining | DaemonLifecycleV1::Stopped => {
                return Err(DaemonErrorV1::Draining);
            }
            DaemonLifecycleV1::Starting | DaemonLifecycleV1::Fenced => {
                return Err(DaemonErrorV1::Fenced);
            }
        }
        self.check_ownership()?;
        let mut stream = self.endpoint.accept().map_err(map_transport_error)?;
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
        self.endpoint.check_ownership().map_err(|error| {
            self.health.lifecycle = DaemonLifecycleV1::Fenced;
            map_transport_error(error)
        })
    }

    fn serve_stream(&mut self, stream: &mut LocalStreamV1) -> Result<(), DaemonErrorV1> {
        stream
            .set_io_timeout(self.limits.io_timeout)
            .map_err(map_transport_error)?;
        self.exchange_handshake(stream)?;
        increment(
            &mut self.health.handshake_accepted,
            &mut self.health.saturated,
        );
        let context = self
            .authenticator
            .authenticate(stream.as_unix_stream())
            .map_err(|_| DaemonErrorV1::Authentication)?;
        let connect = stream
            .read_frame(self.limits.max_frame_bytes)
            .map_err(map_transport_error)?
            .ok_or_else(|| DaemonErrorV1::Io(io::Error::from(io::ErrorKind::UnexpectedEof)))?;
        let (connect_route, connect_payload) = self.decode_profile_frame(&connect, true)?;
        let opened = match self
            .host
            .open_daemon(connect_payload, &context, self.clock.now())
        {
            Ok(opened) => opened,
            Err(error) => return Err(self.map_host_error(error)),
        };
        let Some(session) = opened.session() else {
            self.write_profile_frame(stream, connect_route, opened.frame())?;
            return Err(DaemonErrorV1::ConnectionRefused);
        };

        let result = (|| {
            let replay_epoch = if self.transport_profile != DaemonTransportProfileV1::Conversation {
                Some(self.allocate_replay_epoch()?)
            } else {
                None
            };
            self.write_profile_frame(stream, connect_route, opened.frame())?;
            if let Some(epoch) = replay_epoch {
                let hello = encode_local_session_hello_v1(epoch);
                self.write_profile_frame(stream, LocalRouteV1::Control, &hello)?;
            }
            for _ in 0..self.limits.max_frames_after_connect {
                let Some(frame) = stream
                    .read_frame(self.limits.max_frame_bytes)
                    .map_err(map_transport_error)?
                else {
                    return Ok(());
                };
                let (route, payload) = self.decode_profile_frame(&frame, false)?;
                let reply = match route {
                    LocalRouteV1::Conversation => {
                        self.host
                            .handle_daemon(session, payload, &context, self.clock.now())
                    }
                    LocalRouteV1::Transient => self.host.handle_transient_daemon(
                        session,
                        payload,
                        &context,
                        self.clock.now(),
                    ),
                    LocalRouteV1::Search => {
                        self.host
                            .handle_search_daemon(session, payload, &context, self.clock.now())
                    }
                    LocalRouteV1::Scheduled => self.host.handle_scheduled_daemon(
                        session,
                        payload,
                        &context,
                        self.clock.now(),
                    ),
                    LocalRouteV1::Control => return Err(DaemonErrorV1::Route),
                };
                let reply = match reply {
                    Ok(reply) => reply,
                    Err(error) => return Err(self.map_host_error(error)),
                };
                self.write_profile_frame(stream, route, &reply)?;
            }
            Err(DaemonErrorV1::RequestLimit)
        })();
        match self.host.close_daemon(session) {
            Ok(_) => result,
            Err(error) => Err(self.map_host_error(error)),
        }
    }

    fn exchange_handshake(&mut self, stream: &mut LocalStreamV1) -> Result<(), DaemonErrorV1> {
        let server = match self.transport_profile {
            DaemonTransportProfileV1::Conversation => {
                let mut client = [0; CLIENT_HELLO_BYTES_V1];
                stream.read_exact(&mut client).map_err(DaemonErrorV1::Io)?;
                if decode_client_hello_v1(&client).is_err() {
                    increment(
                        &mut self.health.handshake_refused,
                        &mut self.health.saturated,
                    );
                    return Err(DaemonErrorV1::Handshake);
                }
                encode_server_hello_v1(
                    self.server_incarnation,
                    u32::try_from(self.limits.max_frame_bytes)
                        .map_err(|_| DaemonErrorV1::Handshake)?,
                    u32::try_from(self.limits.max_frames_after_connect)
                        .map_err(|_| DaemonErrorV1::Handshake)?,
                )
                .map_err(|_| DaemonErrorV1::Handshake)?
                .to_vec()
            }
            DaemonTransportProfileV1::Collaboration => {
                let mut client = [0; COLLABORATION_CLIENT_HELLO_BYTES_V1];
                stream.read_exact(&mut client).map_err(DaemonErrorV1::Io)?;
                if decode_collaboration_client_hello_v1(&client).is_err() {
                    increment(
                        &mut self.health.handshake_refused,
                        &mut self.health.saturated,
                    );
                    return Err(DaemonErrorV1::Handshake);
                }
                encode_collaboration_server_hello_v1(
                    self.server_incarnation,
                    u32::try_from(self.limits.max_frame_bytes)
                        .map_err(|_| DaemonErrorV1::Handshake)?,
                    u32::try_from(self.limits.max_frames_after_connect)
                        .map_err(|_| DaemonErrorV1::Handshake)?,
                )
                .map_err(|_| DaemonErrorV1::Handshake)?
                .to_vec()
            }
            DaemonTransportProfileV1::Search => {
                let mut client = [0; SEARCH_CLIENT_HELLO_BYTES_V1];
                stream.read_exact(&mut client).map_err(DaemonErrorV1::Io)?;
                if decode_search_client_hello_v1(&client).is_err() {
                    increment(
                        &mut self.health.handshake_refused,
                        &mut self.health.saturated,
                    );
                    return Err(DaemonErrorV1::Handshake);
                }
                encode_search_server_hello_v1(
                    self.server_incarnation,
                    u32::try_from(self.limits.max_frame_bytes)
                        .map_err(|_| DaemonErrorV1::Handshake)?,
                    u32::try_from(self.limits.max_frames_after_connect)
                        .map_err(|_| DaemonErrorV1::Handshake)?,
                )
                .map_err(|_| DaemonErrorV1::Handshake)?
                .to_vec()
            }
            DaemonTransportProfileV1::Composition => {
                let mut client = [0; COMPOSITION_CLIENT_HELLO_BYTES_V1];
                stream.read_exact(&mut client).map_err(DaemonErrorV1::Io)?;
                if decode_composition_client_hello_v1(&client).is_err() {
                    increment(
                        &mut self.health.handshake_refused,
                        &mut self.health.saturated,
                    );
                    return Err(DaemonErrorV1::Handshake);
                }
                encode_composition_server_hello_v1(
                    self.server_incarnation,
                    u32::try_from(self.limits.max_frame_bytes)
                        .map_err(|_| DaemonErrorV1::Handshake)?,
                    u32::try_from(self.limits.max_frames_after_connect)
                        .map_err(|_| DaemonErrorV1::Handshake)?,
                )
                .map_err(|_| DaemonErrorV1::Handshake)?
                .to_vec()
            }
        };
        stream.write_all(&server).map_err(DaemonErrorV1::Io)?;
        stream.flush().map_err(DaemonErrorV1::Io)
    }

    fn decode_profile_frame<'a>(
        &self,
        frame: &'a [u8],
        connect: bool,
    ) -> Result<(LocalRouteV1, &'a [u8]), DaemonErrorV1> {
        match self.transport_profile {
            DaemonTransportProfileV1::Conversation => Ok((LocalRouteV1::Conversation, frame)),
            DaemonTransportProfileV1::Collaboration
            | DaemonTransportProfileV1::Search
            | DaemonTransportProfileV1::Composition => {
                let decoded = decode_local_routed_frame_v1(frame, self.limits.max_frame_bytes)
                    .map_err(|_| DaemonErrorV1::Route)?;
                if decoded.route() == LocalRouteV1::Control
                    || (connect && decoded.route() != LocalRouteV1::Conversation)
                    || (self.transport_profile == DaemonTransportProfileV1::Collaboration
                        && matches!(
                            decoded.route(),
                            LocalRouteV1::Search | LocalRouteV1::Scheduled
                        ))
                    || (self.transport_profile == DaemonTransportProfileV1::Search
                        && matches!(
                            decoded.route(),
                            LocalRouteV1::Transient | LocalRouteV1::Scheduled
                        ))
                    || (self.transport_profile == DaemonTransportProfileV1::Composition
                        && matches!(
                            decoded.route(),
                            LocalRouteV1::Transient | LocalRouteV1::Search
                        ))
                {
                    return Err(DaemonErrorV1::Route);
                }
                Ok((decoded.route(), decoded.payload()))
            }
        }
    }

    fn write_profile_frame(
        &self,
        stream: &mut LocalStreamV1,
        route: LocalRouteV1,
        payload: &[u8],
    ) -> Result<(), DaemonErrorV1> {
        match self.transport_profile {
            DaemonTransportProfileV1::Conversation => stream
                .write_frame(payload, self.limits.max_frame_bytes)
                .map_err(map_transport_error),
            DaemonTransportProfileV1::Collaboration
            | DaemonTransportProfileV1::Search
            | DaemonTransportProfileV1::Composition => {
                let encoded =
                    encode_local_routed_frame_v1(route, payload, self.limits.max_frame_bytes)
                        .map_err(|_| DaemonErrorV1::FrameLimit)?;
                stream
                    .write_frame(&encoded, self.limits.max_frame_bytes)
                    .map_err(map_transport_error)
            }
        }
    }

    fn allocate_replay_epoch(&mut self) -> Result<LocalReplayEpochV1, DaemonErrorV1> {
        let sequence = self.next_connection_epoch;
        let Some(next) = sequence.checked_add(1) else {
            self.health.lifecycle = DaemonLifecycleV1::Fenced;
            return Err(DaemonErrorV1::ReplayEpochExhausted);
        };
        self.next_connection_epoch = next;
        let mut bytes = [0; 32];
        bytes[..16].copy_from_slice(&self.server_incarnation.as_bytes());
        bytes[16..].copy_from_slice(&sequence.to_be_bytes());
        LocalReplayEpochV1::new(bytes).ok_or_else(|| {
            self.health.lifecycle = DaemonLifecycleV1::Fenced;
            DaemonErrorV1::ReplayEpochExhausted
        })
    }

    fn map_host_error(&mut self, error: DaemonHostErrorV1) -> DaemonErrorV1 {
        match error {
            DaemonHostErrorV1::Host(error) => DaemonErrorV1::Host(error),
            DaemonHostErrorV1::Durability => {
                self.health.lifecycle = DaemonLifecycleV1::Fenced;
                DaemonErrorV1::Durability
            }
        }
    }
}

impl<P, A, C> DaemonV1<HostV2<AegisVerifierV1<P>>, A, C>
where
    AegisVerifierV1<P>: agora_host::ScheduledPrincipalVerifier<Context = AegisContextV1>,
    A: AuthenticatorV1,
    C: HostClockV1,
{
    pub fn bind_new_composition(
        socket_path: impl AsRef<Path>,
        host: HostV2<AegisVerifierV1<P>>,
        authenticator: A,
        clock: C,
        server_incarnation: ServerIncarnationV1,
        limits: DaemonLimitsV1,
    ) -> Result<Self, DaemonErrorV1> {
        validate_daemon_configuration(server_incarnation, limits)?;
        let endpoint = LocalEndpointV1::bind_new(socket_path, server_incarnation)
            .map_err(map_transport_error)?;
        Self::bind_owned(
            endpoint,
            host,
            authenticator,
            clock,
            server_incarnation,
            limits,
            DaemonTransportProfileV1::Composition,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn recover_stale_composition(
        socket_path: impl AsRef<Path>,
        host: HostV2<AegisVerifierV1<P>>,
        authenticator: A,
        clock: C,
        previous_incarnation: ServerIncarnationV1,
        next_incarnation: ServerIncarnationV1,
        limits: DaemonLimitsV1,
    ) -> Result<Self, DaemonErrorV1> {
        validate_daemon_configuration(next_incarnation, limits)?;
        let endpoint =
            LocalEndpointV1::recover_stale(socket_path, previous_incarnation, next_incarnation)
                .map_err(map_transport_error)?;
        Self::bind_owned(
            endpoint,
            host,
            authenticator,
            clock,
            next_incarnation,
            limits,
            DaemonTransportProfileV1::Composition,
        )
    }

    pub fn observation(&self) -> DaemonObservationV1 {
        observation(self.health, &self.host)
    }

    pub const fn host(&self) -> &HostV2<AegisVerifierV1<P>> {
        &self.host
    }
}

impl<H, A, C> DaemonV1<H, A, C> {
    /// Consumes the exclusive sequential owner and returns terminal teardown facts.
    ///
    /// Because the owner is mutably inaccessible while `serve_next` runs, a
    /// caller can obtain this receipt only with no admitted connection in flight.
    pub fn stop(mut self) -> DaemonStoppedV1 {
        let prior_lifecycle = self.health.lifecycle;
        debug_assert_eq!(self.health.active_connections, 0);
        let cleanup = self.teardown();
        self.health.lifecycle = DaemonLifecycleV1::Stopped;
        DaemonStoppedV1 {
            prior_lifecycle,
            health: self.health,
            cleanup,
        }
    }

    fn teardown(&mut self) -> DaemonCleanupV1 {
        match self.endpoint.teardown() {
            LocalTransportCleanupV1::Clean => DaemonCleanupV1::Clean,
            LocalTransportCleanupV1::ResiduePreserved => DaemonCleanupV1::ResiduePreserved,
            LocalTransportCleanupV1::Incomplete => DaemonCleanupV1::Incomplete,
        }
    }
}

impl<P, A, C> DaemonV1<TransientHostV1<AegisVerifierV1<P>>, A, C>
where
    AegisVerifierV1<P>: PrincipalVerifier<Context = AegisContextV1>,
    A: AuthenticatorV1,
    C: HostClockV1,
{
    pub fn bind_new_collaboration(
        socket_path: impl AsRef<Path>,
        host: TransientHostV1<AegisVerifierV1<P>>,
        authenticator: A,
        clock: C,
        server_incarnation: ServerIncarnationV1,
        limits: DaemonLimitsV1,
    ) -> Result<Self, DaemonErrorV1> {
        validate_daemon_configuration(server_incarnation, limits)?;
        let endpoint = LocalEndpointV1::bind_new(socket_path, server_incarnation)
            .map_err(map_transport_error)?;
        Self::bind_owned(
            endpoint,
            host,
            authenticator,
            clock,
            server_incarnation,
            limits,
            DaemonTransportProfileV1::Collaboration,
        )
    }

    pub fn observation(&self) -> DaemonObservationV1 {
        observation(self.health, self.host.host())
    }

    pub const fn host(&self) -> &TransientHostV1<AegisVerifierV1<P>> {
        &self.host
    }
}

impl<P, Q, A, C> DaemonV1<SearchHostV1<AegisVerifierV1<P>, Q>, A, C>
where
    AegisVerifierV1<P>: PrincipalVerifier<Context = AegisContextV1>,
    Q: SearchPortV1,
    A: AuthenticatorV1,
    C: HostClockV1,
{
    pub fn bind_new_search(
        socket_path: impl AsRef<Path>,
        host: SearchHostV1<AegisVerifierV1<P>, Q>,
        authenticator: A,
        clock: C,
        server_incarnation: ServerIncarnationV1,
        limits: DaemonLimitsV1,
    ) -> Result<Self, DaemonErrorV1> {
        validate_daemon_configuration(server_incarnation, limits)?;
        let endpoint = LocalEndpointV1::bind_new(socket_path, server_incarnation)
            .map_err(map_transport_error)?;
        Self::bind_owned(
            endpoint,
            host,
            authenticator,
            clock,
            server_incarnation,
            limits,
            DaemonTransportProfileV1::Search,
        )
    }

    /// Rebinds exact unlocked endpoint residue around a caller-reconstructed
    /// Host and a fresh volatile search runtime.
    #[allow(clippy::too_many_arguments)]
    pub fn recover_stale_search(
        socket_path: impl AsRef<Path>,
        host: SearchHostV1<AegisVerifierV1<P>, Q>,
        authenticator: A,
        clock: C,
        previous_incarnation: ServerIncarnationV1,
        next_incarnation: ServerIncarnationV1,
        limits: DaemonLimitsV1,
    ) -> Result<Self, DaemonErrorV1> {
        validate_daemon_configuration(next_incarnation, limits)?;
        let endpoint =
            LocalEndpointV1::recover_stale(socket_path, previous_incarnation, next_incarnation)
                .map_err(map_transport_error)?;
        Self::bind_owned(
            endpoint,
            host,
            authenticator,
            clock,
            next_incarnation,
            limits,
            DaemonTransportProfileV1::Search,
        )
    }

    pub fn observation(&self) -> DaemonObservationV1 {
        observation(self.health, self.host.host())
    }

    pub const fn host(&self) -> &SearchHostV1<AegisVerifierV1<P>, Q> {
        &self.host
    }
}

impl<P, S, A, C> DaemonV1<DurableHostV2<AegisVerifierV1<P>, S>, A, C>
where
    AegisVerifierV1<P>: agora_host::ScheduledPrincipalVerifier<Context = AegisContextV1>,
    S: CheckpointStore,
    A: AuthenticatorV1,
    C: HostClockV1,
{
    pub fn bind_new_durable_composition(
        socket_path: impl AsRef<Path>,
        host: DurableHostV2<AegisVerifierV1<P>, S>,
        authenticator: A,
        clock: C,
        server_incarnation: ServerIncarnationV1,
        limits: DaemonLimitsV1,
    ) -> Result<Self, DaemonErrorV1> {
        validate_daemon_configuration(server_incarnation, limits)?;
        let endpoint = LocalEndpointV1::bind_new(socket_path, server_incarnation)
            .map_err(map_transport_error)?;
        Self::bind_owned(
            endpoint,
            host,
            authenticator,
            clock,
            server_incarnation,
            limits,
            DaemonTransportProfileV1::Composition,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn recover_stale_durable_composition(
        socket_path: impl AsRef<Path>,
        host: DurableHostV2<AegisVerifierV1<P>, S>,
        authenticator: A,
        clock: C,
        previous_incarnation: ServerIncarnationV1,
        next_incarnation: ServerIncarnationV1,
        limits: DaemonLimitsV1,
    ) -> Result<Self, DaemonErrorV1> {
        validate_daemon_configuration(next_incarnation, limits)?;
        let endpoint =
            LocalEndpointV1::recover_stale(socket_path, previous_incarnation, next_incarnation)
                .map_err(map_transport_error)?;
        Self::bind_owned(
            endpoint,
            host,
            authenticator,
            clock,
            next_incarnation,
            limits,
            DaemonTransportProfileV1::Composition,
        )
    }

    pub fn observation(&self) -> DurableDaemonObservationV1 {
        durable_observation(
            self.health,
            self.host.host().map(|host| observation(self.health, host)),
            self.host.observation(),
        )
    }

    pub fn physical_observation(&self) -> PhysicalDurableDaemonObservationV1<S::Observation>
    where
        S: ObservableCheckpointStoreV1,
    {
        physical_durable_observation(
            self.health,
            self.host.host().map(|host| observation(self.health, host)),
            self.host.layered_observation(),
        )
    }

    pub const fn host(&self) -> &DurableHostV2<AegisVerifierV1<P>, S> {
        &self.host
    }
}

impl<P, S, A, C> DaemonV1<DurableTransientHostV1<AegisVerifierV1<P>, S>, A, C>
where
    AegisVerifierV1<P>: PrincipalVerifier<Context = AegisContextV1>,
    S: CheckpointStore,
    A: AuthenticatorV1,
    C: HostClockV1,
{
    pub fn bind_new_durable_collaboration(
        socket_path: impl AsRef<Path>,
        host: DurableTransientHostV1<AegisVerifierV1<P>, S>,
        authenticator: A,
        clock: C,
        server_incarnation: ServerIncarnationV1,
        limits: DaemonLimitsV1,
    ) -> Result<Self, DaemonErrorV1> {
        validate_daemon_configuration(server_incarnation, limits)?;
        let endpoint = LocalEndpointV1::bind_new(socket_path, server_incarnation)
            .map_err(map_transport_error)?;
        Self::bind_owned(
            endpoint,
            host,
            authenticator,
            clock,
            server_incarnation,
            limits,
            DaemonTransportProfileV1::Collaboration,
        )
    }

    /// Rebinds an explicitly recovered durable collaboration owner after exact
    /// unlocked endpoint residue validation.
    ///
    /// The caller recovers and selects the durable owner; this path only
    /// rotates the daemon lifetime and preserves collaboration handshake
    /// profile 2. It never resumes a prior transport session.
    #[allow(clippy::too_many_arguments)]
    pub fn recover_stale_durable_collaboration(
        socket_path: impl AsRef<Path>,
        host: DurableTransientHostV1<AegisVerifierV1<P>, S>,
        authenticator: A,
        clock: C,
        previous_incarnation: ServerIncarnationV1,
        next_incarnation: ServerIncarnationV1,
        limits: DaemonLimitsV1,
    ) -> Result<Self, DaemonErrorV1> {
        validate_daemon_configuration(next_incarnation, limits)?;
        let endpoint =
            LocalEndpointV1::recover_stale(socket_path, previous_incarnation, next_incarnation)
                .map_err(map_transport_error)?;
        Self::bind_owned(
            endpoint,
            host,
            authenticator,
            clock,
            next_incarnation,
            limits,
            DaemonTransportProfileV1::Collaboration,
        )
    }

    pub fn observation(&self) -> DurableDaemonObservationV1 {
        durable_observation(
            self.health,
            self.host
                .host()
                .map(|host| observation(self.health, host.host())),
            self.host.observation(),
        )
    }

    pub fn physical_observation(&self) -> PhysicalDurableDaemonObservationV1<S::Observation>
    where
        S: ObservableCheckpointStoreV1,
    {
        physical_durable_observation(
            self.health,
            self.host
                .host()
                .map(|host| observation(self.health, host.host())),
            self.host.layered_observation(),
        )
    }

    pub const fn host(&self) -> &DurableTransientHostV1<AegisVerifierV1<P>, S> {
        &self.host
    }
}

fn observation<V: PrincipalVerifier>(
    transport: DaemonHealthV1,
    host_owner: &HostV2<V>,
) -> DaemonObservationV1 {
    let host = host_owner.usage();
    let usage = host_owner.service().usage();
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
        transport,
        host,
        service,
        fidelity_lost: observation_fidelity_lost(transport, host, service),
    }
}

fn durable_observation(
    transport: DaemonHealthV1,
    runtime: Option<DaemonObservationV1>,
    durability: DurableHostObservationV2,
) -> DurableDaemonObservationV1 {
    DurableDaemonObservationV1 {
        transport,
        runtime,
        durability,
        fidelity_lost: transport.saturated
            || runtime.is_some_and(|observation| observation.fidelity_lost)
            || durability.saturated,
    }
}

fn physical_durable_observation<O: Copy>(
    transport: DaemonHealthV1,
    runtime: Option<DaemonObservationV1>,
    store: DurableStoreObservationV2<O>,
) -> PhysicalDurableDaemonObservationV1<O> {
    PhysicalDurableDaemonObservationV1 {
        transport,
        runtime,
        durability: store.durability,
        physical: store.physical,
        fidelity_lost: transport.saturated
            || runtime.is_some_and(|observation| observation.fidelity_lost)
            || store.fidelity_lost,
    }
}

fn validate_daemon_configuration(
    server_incarnation: ServerIncarnationV1,
    limits: DaemonLimitsV1,
) -> Result<(), DaemonErrorV1> {
    if limits.max_frame_bytes == 0
        || limits.max_frame_bytes > u32::MAX as usize
        || limits.max_frames_after_connect == 0
        || limits.max_frames_after_connect > u32::MAX as usize
        || limits.io_timeout.is_zero()
        || server_incarnation.is_zero()
    {
        return Err(DaemonErrorV1::InvalidLimits);
    }
    Ok(())
}

impl<H, A, C> Drop for DaemonV1<H, A, C> {
    fn drop(&mut self) {
        if self.health.lifecycle != DaemonLifecycleV1::Stopped {
            let _ = self.teardown();
            self.health.lifecycle = DaemonLifecycleV1::Stopped;
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

fn map_transport_error(error: LocalTransportErrorV1) -> DaemonErrorV1 {
    match error {
        LocalTransportErrorV1::UnsafeEndpoint => DaemonErrorV1::UnsafeEndpoint,
        LocalTransportErrorV1::Io(error) => DaemonErrorV1::Io(error),
        LocalTransportErrorV1::LifetimeBusy => DaemonErrorV1::LifetimeBusy,
        LocalTransportErrorV1::CrashResidue => DaemonErrorV1::CrashResidue,
        LocalTransportErrorV1::ResidueMismatch => DaemonErrorV1::ResidueMismatch,
        LocalTransportErrorV1::Fenced => DaemonErrorV1::Fenced,
        LocalTransportErrorV1::FrameLimit => DaemonErrorV1::FrameLimit,
    }
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
        DaemonHealthV1, DaemonLifecycleV1, HostUsage, OuterFrameErrorV1, ServiceObservationV1,
        decode_outer_frame_prefix_v1, durable_observation, encode_outer_frame_v1, increment,
        observation_fidelity_lost, physical_durable_observation,
    };
    use agora_durable::{DurableHostObservationV2, DurableOwnerStateV2, DurableStoreObservationV2};

    #[test]
    fn default_health_is_starting_and_cannot_claim_readiness_or_drain_completion() {
        let health = DaemonHealthV1::default();
        assert_eq!(health.lifecycle, DaemonLifecycleV1::Starting);
        assert!(!health.ready());
        assert!(!health.drain_complete());
    }

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
    fn fenced_durability_remains_observable_without_runtime_projection() {
        let durability = DurableHostObservationV2 {
            state: DurableOwnerStateV2::Fenced,
            store_revision: 4,
            offers: 6,
            committed: 4,
            previously_committed: 0,
            checkpoint_suppressed: 0,
            store_refused: 1,
            outcome_unknown: 1,
            not_durable: 0,
            invalid_receipt: 0,
            fenced: 0,
            saturated: true,
        };
        let observation = durable_observation(DaemonHealthV1::default(), None, durability);
        assert!(observation.runtime.is_none());
        assert_eq!(observation.durability, durability);
        assert!(observation.fidelity_lost);
    }

    #[test]
    fn physical_store_fidelity_loss_reaches_the_aggregate() {
        let durability = DurableHostObservationV2 {
            state: DurableOwnerStateV2::Active,
            store_revision: 4,
            offers: 4,
            committed: 4,
            previously_committed: 0,
            checkpoint_suppressed: 0,
            store_refused: 0,
            outcome_unknown: 0,
            not_durable: 0,
            invalid_receipt: 0,
            fenced: 0,
            saturated: false,
        };
        let observation = physical_durable_observation(
            DaemonHealthV1::default(),
            None,
            DurableStoreObservationV2 {
                durability,
                physical: (),
                fidelity_lost: true,
            },
        );
        assert_eq!(observation.durability, durability);
        assert!(observation.fidelity_lost);
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
