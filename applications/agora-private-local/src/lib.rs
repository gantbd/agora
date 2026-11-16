//! Standalone private-local Agora application composition.
//!
//! This application root owns deployment choices that do not belong in the
//! engine, daemon transport, or Workbench. The first closed boundary is
//! bounded create-time state plus public Aegis connection and narrow
//! same-session grant provisioning. Durable create/open and the process shell
//! compose here in later slices.

use aegis_client::{AuthorityClient, ServicePort};
use aegis_local_client::{ClientErrorV1, LimitsV1 as AegisWireLimitsV1, UnixPort};
use aegis_protocol::{
    ActionV1, EngineNamespaceV1, GrantHandleV1, OutcomeV1, RefusalV1, ReplyV1, ScopeV1,
    SessionHandleV1,
};
use agora_aegis_verifier::{AegisContextV1, AegisVerifierV1};
use agora_durable::{DurableHostConfigV2, DurableHostStartErrorV2, DurableHostV2};
use agora_engine::{
    AdmissionPolicy, CommandV2, Engine, Limits, Participant, ParticipantId, ParticipantKind,
    Refusal, RoomId,
};
use agora_host::{HostLimits, VerifiedSubjectV1, v2::HostV2};
use agora_local_daemon::{
    DaemonErrorV1, DaemonLimitsV1, DaemonV1, PrivateEndpointAuthenticatorV1, ServerIncarnationV1,
    SystemHostClockStartErrorV1, SystemHostClockV1,
};
use agora_service::v2::{ServiceLimitsV2, ServiceV2};
use agora_storage_directory::{DirectoryCheckpointStoreV1, DirectoryErrorV1, DirectoryLimitsV1};
use std::{
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapParticipantV1 {
    pub id: ParticipantId,
    pub display_name: String,
    pub kind: ParticipantKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapRoomV1 {
    pub id: RoomId,
    pub name: String,
    pub owner: ParticipantId,
    pub admission: AdmissionPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapV1 {
    pub principal: ParticipantId,
    pub participants: Vec<BootstrapParticipantV1>,
    pub rooms: Vec<BootstrapRoomV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootstrapStageV1 {
    Participant(usize),
    Room(usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BootstrapRefusalV1 {
    NoParticipants,
    ParticipantLimit,
    RoomLimit,
    PrincipalNotRegistered,
    Engine {
        stage: BootstrapStageV1,
        refusal: Refusal,
    },
}

impl BootstrapV1 {
    /// Builds the exact initial aggregate before any endpoint or durable owner
    /// is published. A refusal returns no partially initialized engine.
    pub fn build_engine(self, limits: Limits) -> Result<Engine, BootstrapRefusalV1> {
        if self.participants.is_empty() {
            return Err(BootstrapRefusalV1::NoParticipants);
        }
        if self.participants.len() > limits.max_participants {
            return Err(BootstrapRefusalV1::ParticipantLimit);
        }
        if self.rooms.len() > limits.max_rooms {
            return Err(BootstrapRefusalV1::RoomLimit);
        }
        if !self
            .participants
            .iter()
            .any(|participant| participant.id == self.principal)
        {
            return Err(BootstrapRefusalV1::PrincipalNotRegistered);
        }

        let mut engine = Engine::with_limits(limits);
        for (index, participant) in self.participants.into_iter().enumerate() {
            engine
                .apply(CommandV2::RegisterParticipant(Participant::new(
                    participant.id,
                    participant.display_name,
                    participant.kind,
                )))
                .map_err(|refusal| BootstrapRefusalV1::Engine {
                    stage: BootstrapStageV1::Participant(index),
                    refusal,
                })?;
        }
        for (index, room) in self.rooms.into_iter().enumerate() {
            engine
                .apply(CommandV2::CreateRoom {
                    room_id: room.id,
                    name: room.name,
                    owner: room.owner,
                    admission: room.admission,
                })
                .map_err(|refusal| BootstrapRefusalV1::Engine {
                    stage: BootstrapStageV1::Room(index),
                    refusal,
                })?;
        }
        Ok(engine)
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct AuthoritySelectionV1 {
    participant: ParticipantId,
    subject: [u8; 16],
    resource: [u8; 16],
}

impl AuthoritySelectionV1 {
    pub const fn new(participant: ParticipantId, subject: [u8; 16], resource: [u8; 16]) -> Self {
        Self {
            participant,
            subject,
            resource,
        }
    }
}

impl std::fmt::Debug for AuthoritySelectionV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuthoritySelectionV1")
            .field("participant_bytes", &self.participant.as_str().len())
            .field("subject", &"<redacted>")
            .field("resource", &"<redacted>")
            .finish()
    }
}

#[derive(Debug)]
pub enum AuthorityConnectErrorV1<E> {
    PrincipalMismatch,
    Connect(ClientErrorV1),
    Grant(E),
}

/// Deployment-owned policy for provisioning or selecting one grant on the
/// exact newly connected Aegis service session. Handles never cross sessions.
pub trait SessionGrantSelectorV1 {
    type Error;

    fn select(
        &mut self,
        session: SessionHandleV1,
        port: &mut UnixPort,
        resource: [u8; 16],
    ) -> Result<GrantHandleV1, Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgoraGrantProfileV1 {
    Conversation,
    ScheduledComposition,
}

impl AgoraGrantProfileV1 {
    const fn actions(self) -> u8 {
        match self {
            Self::Conversation => 1 << ActionV1::Submit as u8,
            Self::ScheduledComposition => {
                (1 << ActionV1::Submit as u8)
                    | (1 << ActionV1::Execute as u8)
                    | (1 << ActionV1::Observe as u8)
            }
        }
    }
}

/// Concrete private-local policy that issues one narrowly scoped Agora grant
/// on the exact newly connected Aegis session. The deployment must map that
/// connection to an Aegis administrator and explicitly supply epoch and time.
pub struct IssueAgoraGrantV1 {
    expected_epoch: u64,
    now: u64,
    expires_at: u64,
    profile: AgoraGrantProfileV1,
}

impl IssueAgoraGrantV1 {
    pub const fn new(
        expected_epoch: u64,
        now: u64,
        expires_at: u64,
        profile: AgoraGrantProfileV1,
    ) -> Self {
        Self {
            expected_epoch,
            now,
            expires_at,
            profile,
        }
    }

    fn issue<P: ServicePort>(
        &self,
        session: SessionHandleV1,
        port: P,
        resource: [u8; 16],
    ) -> Result<GrantHandleV1, IssueAgoraGrantErrorV1<P::Error>> {
        if self.expires_at <= self.now {
            return Err(IssueAgoraGrantErrorV1::InvalidLifetime);
        }
        let mut authority = AuthorityClient::new(session, port);
        let reply = authority
            .issue(
                self.expected_epoch,
                self.now,
                self.expires_at,
                vec![ScopeV1 {
                    engine: EngineNamespaceV1::Agora,
                    resource,
                    actions: self.profile.actions(),
                }],
            )
            .map_err(IssueAgoraGrantErrorV1::Transport)?;
        match reply {
            ReplyV1::Accepted(OutcomeV1::GrantIssued(summary))
                if summary.parent.is_none()
                    && summary.epoch == self.expected_epoch
                    && summary.expires_at == self.expires_at
                    && summary.scope_count == 1
                    && summary.delegation_depth == 0 =>
            {
                Ok(summary.handle)
            }
            ReplyV1::Refused(refusal) => Err(IssueAgoraGrantErrorV1::Refused(refusal)),
            ReplyV1::Accepted(_) => Err(IssueAgoraGrantErrorV1::InexactReply),
        }
    }
}

impl std::fmt::Debug for IssueAgoraGrantV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IssueAgoraGrantV1")
            .field("profile", &self.profile)
            .field("authority_inputs", &"<redacted>")
            .finish()
    }
}

pub enum IssueAgoraGrantErrorV1<E> {
    InvalidLifetime,
    Transport(E),
    Refused(RefusalV1),
    InexactReply,
}

impl<E> std::fmt::Debug for IssueAgoraGrantErrorV1<E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidLifetime => formatter.write_str("InvalidLifetime"),
            Self::Transport(_) => formatter.write_str("Transport(<redacted>)"),
            Self::Refused(refusal) => formatter.debug_tuple("Refused").field(refusal).finish(),
            Self::InexactReply => formatter.write_str("InexactReply"),
        }
    }
}

impl SessionGrantSelectorV1 for IssueAgoraGrantV1 {
    type Error = IssueAgoraGrantErrorV1<ClientErrorV1>;

    fn select(
        &mut self,
        session: SessionHandleV1,
        port: &mut UnixPort,
        resource: [u8; 16],
    ) -> Result<GrantHandleV1, Self::Error> {
        self.issue(session, port, resource)
    }
}

/// One live public Aegis connection plus the exact Agora context bound to its
/// server-selected session and deployment-selected session-local grant.
pub struct ConnectedAuthorityV1 {
    participant: ParticipantId,
    context: AegisContextV1,
    verifier: AegisVerifierV1<UnixPort>,
}

impl ConnectedAuthorityV1 {
    pub const fn participant(&self) -> &ParticipantId {
        &self.participant
    }

    pub fn into_parts(self) -> (AegisContextV1, AegisVerifierV1<UnixPort>) {
        (self.context, self.verifier)
    }
}

impl std::fmt::Debug for ConnectedAuthorityV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ConnectedAuthorityV1")
            .field("context", &"<redacted>")
            .field("verifier", &"<redacted>")
            .finish()
    }
}

/// Connects only after the deployment-selected authority participant matches
/// the bootstrap principal. This function discovers no resource and owns no
/// grant policy. Provisioning/selection remains an explicit deployment input
/// and runs only on the exact server-selected session.
pub fn connect_authority_v1<G: SessionGrantSelectorV1>(
    socket: impl AsRef<Path>,
    limits: AegisWireLimitsV1,
    io_timeout: Duration,
    bootstrap_principal: &ParticipantId,
    selection: AuthoritySelectionV1,
    grant_selector: &mut G,
) -> Result<ConnectedAuthorityV1, AuthorityConnectErrorV1<G::Error>> {
    if &selection.participant != bootstrap_principal {
        return Err(AuthorityConnectErrorV1::PrincipalMismatch);
    }
    let (session, mut port) = aegis_local_client::connect(socket, limits, io_timeout)
        .map_err(AuthorityConnectErrorV1::Connect)?;
    let grant = grant_selector
        .select(session, &mut port, selection.resource)
        .map_err(AuthorityConnectErrorV1::Grant)?;
    let context = AegisContextV1::new(
        selection.participant.as_str(),
        VerifiedSubjectV1::from_verifier(selection.subject),
        session,
        grant,
    );
    Ok(ConnectedAuthorityV1 {
        participant: selection.participant,
        context,
        verifier: AegisVerifierV1::new(port, selection.resource),
    })
}

pub enum DurableCompositionStartupV1 {
    Create {
        bootstrap: BootstrapV1,
        server_incarnation: ServerIncarnationV1,
    },
    Open {
        server_incarnation: ServerIncarnationV1,
    },
    RecoverResidue {
        previous_incarnation: ServerIncarnationV1,
        next_incarnation: ServerIncarnationV1,
    },
}

impl std::fmt::Debug for DurableCompositionStartupV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Create { .. } => formatter.write_str("Create(<redacted>)"),
            Self::Open { .. } => formatter.write_str("Open(<redacted>)"),
            Self::RecoverResidue { .. } => formatter.write_str("RecoverResidue(<redacted>)"),
        }
    }
}

pub struct DurableCompositionPlanV1 {
    pub store_path: PathBuf,
    pub socket_path: PathBuf,
    pub directory_limits: DirectoryLimitsV1,
    pub engine_limits: Limits,
    pub service_limits: ServiceLimitsV2,
    pub host_limits: HostLimits,
    pub durable: DurableHostConfigV2,
    pub daemon_limits: DaemonLimitsV1,
    pub startup: DurableCompositionStartupV1,
}

impl std::fmt::Debug for DurableCompositionPlanV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DurableCompositionPlanV1")
            .field("paths", &"<redacted>")
            .field("directory_limits", &self.directory_limits)
            .field("engine_limits", &self.engine_limits)
            .field("service_limits", &self.service_limits)
            .field("host_limits", &self.host_limits)
            .field("durable", &self.durable)
            .field("daemon_limits", &self.daemon_limits)
            .field("startup", &self.startup)
            .finish()
    }
}

pub enum DurableCompositionStartErrorV1 {
    PrincipalMismatch,
    Bootstrap(BootstrapRefusalV1),
    Clock(SystemHostClockStartErrorV1),
    Store(DirectoryErrorV1),
    Durable(DurableHostStartErrorV2),
    Daemon(DaemonErrorV1),
}

impl std::fmt::Debug for DurableCompositionStartErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PrincipalMismatch => formatter.write_str("PrincipalMismatch"),
            Self::Bootstrap(refusal) => formatter.debug_tuple("Bootstrap").field(refusal).finish(),
            Self::Clock(error) => formatter.debug_tuple("Clock").field(error).finish(),
            Self::Store(error) => formatter.debug_tuple("Store").field(error).finish(),
            Self::Durable(error) => formatter.debug_tuple("Durable").field(error).finish(),
            Self::Daemon(error) => formatter.debug_tuple("Daemon").field(error).finish(),
        }
    }
}

type DurableCompositionDaemonWithPortV1<P> = DaemonV1<
    DurableHostV2<AegisVerifierV1<P>, DirectoryCheckpointStoreV1>,
    PrivateEndpointAuthenticatorV1,
    SystemHostClockV1,
>;

pub type DurableCompositionDaemonV1 = DurableCompositionDaemonWithPortV1<UnixPort>;

/// Starts the production private-local durable composition in an explicit
/// create, clean-open, or exact-residue-recovery mode. Workbench may supervise
/// the returned owner but does not interpret any of these deployment inputs.
pub fn start_durable_composition_v1(
    authority: ConnectedAuthorityV1,
    plan: DurableCompositionPlanV1,
) -> Result<DurableCompositionDaemonV1, DurableCompositionStartErrorV1> {
    let ConnectedAuthorityV1 {
        participant,
        context,
        verifier,
    } = authority;
    start_durable_composition_parts_v1(participant, context, verifier, plan)
}

fn start_durable_composition_parts_v1<P>(
    participant: ParticipantId,
    context: AegisContextV1,
    verifier: AegisVerifierV1<P>,
    plan: DurableCompositionPlanV1,
) -> Result<DurableCompositionDaemonWithPortV1<P>, DurableCompositionStartErrorV1>
where
    AegisVerifierV1<P>: agora_host::ScheduledPrincipalVerifier<Context = AegisContextV1>,
{
    let DurableCompositionPlanV1 {
        store_path,
        socket_path,
        directory_limits,
        engine_limits,
        service_limits,
        host_limits,
        durable,
        daemon_limits,
        startup,
    } = plan;
    let clock = SystemHostClockV1::try_new().map_err(DurableCompositionStartErrorV1::Clock)?;
    let authenticator = PrivateEndpointAuthenticatorV1::new(context);

    match startup {
        DurableCompositionStartupV1::Create {
            bootstrap,
            server_incarnation,
        } => {
            if bootstrap.principal != participant {
                return Err(DurableCompositionStartErrorV1::PrincipalMismatch);
            }
            let engine = bootstrap
                .build_engine(engine_limits)
                .map_err(DurableCompositionStartErrorV1::Bootstrap)?;
            let host = HostV2::new(
                ServiceV2::new(engine, service_limits),
                verifier,
                host_limits,
            );
            let store = DirectoryCheckpointStoreV1::create_new(store_path, directory_limits)
                .map_err(DurableCompositionStartErrorV1::Store)?;
            let durable = DurableHostV2::initialize(host, store, durable)
                .map_err(DurableCompositionStartErrorV1::Durable)?;
            DaemonV1::bind_new_durable_composition(
                socket_path,
                durable,
                authenticator,
                clock,
                server_incarnation,
                daemon_limits,
            )
            .map_err(DurableCompositionStartErrorV1::Daemon)
        }
        DurableCompositionStartupV1::Open { server_incarnation } => {
            let store = DirectoryCheckpointStoreV1::open_existing(store_path, directory_limits)
                .map_err(DurableCompositionStartErrorV1::Store)?;
            let durable = DurableHostV2::recover(store, durable, verifier)
                .map_err(DurableCompositionStartErrorV1::Durable)?;
            DaemonV1::bind_new_durable_composition(
                socket_path,
                durable,
                authenticator,
                clock,
                server_incarnation,
                daemon_limits,
            )
            .map_err(DurableCompositionStartErrorV1::Daemon)
        }
        DurableCompositionStartupV1::RecoverResidue {
            previous_incarnation,
            next_incarnation,
        } => {
            let store = DirectoryCheckpointStoreV1::open_existing(store_path, directory_limits)
                .map_err(DurableCompositionStartErrorV1::Store)?;
            let durable = DurableHostV2::recover(store, durable, verifier)
                .map_err(DurableCompositionStartErrorV1::Durable)?;
            DaemonV1::recover_stale_durable_composition(
                socket_path,
                durable,
                authenticator,
                clock,
                previous_incarnation,
                next_incarnation,
                daemon_limits,
            )
            .map_err(DurableCompositionStartErrorV1::Daemon)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableProcessTerminalV1 {
    InvalidState,
    Ownership,
    Transport,
    Fenced,
    Durability,
    Capacity,
}

pub struct DurableProcessFailureV1 {
    pub terminal: DurableProcessTerminalV1,
    pub stopped: agora_local_daemon::DaemonStoppedV1,
}

impl std::fmt::Debug for DurableProcessFailureV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DurableProcessFailureV1")
            .field("terminal", &self.terminal)
            .field("stopped", &self.stopped)
            .finish()
    }
}

/// Requests shutdown and wakes one blocked private-local accept. Repeated
/// requests are idempotent; the daemon still owns all drain and cleanup facts.
pub fn request_durable_process_shutdown_v1(stop: &AtomicBool, socket: impl AsRef<Path>) -> bool {
    let first = !stop.swap(true, Ordering::AcqRel);
    let _ = UnixStream::connect(socket);
    first
}

/// Serves sequential connections until an explicit shutdown request, then
/// irreversibly drains and returns the daemon's exact terminal cleanup receipt.
pub fn serve_durable_process_v1(
    daemon: DurableCompositionDaemonV1,
    stop: &AtomicBool,
) -> Result<agora_local_daemon::DaemonStoppedV1, DurableProcessFailureV1> {
    serve_durable_process_with_port_v1(daemon, stop)
}

fn serve_durable_process_with_port_v1<P>(
    mut daemon: DurableCompositionDaemonWithPortV1<P>,
    stop: &AtomicBool,
) -> Result<agora_local_daemon::DaemonStoppedV1, DurableProcessFailureV1>
where
    P: ServicePort,
    AegisVerifierV1<P>: agora_host::ScheduledPrincipalVerifier<Context = AegisContextV1>,
{
    loop {
        if stop.load(Ordering::Acquire) {
            if !daemon.begin_drain() && !daemon.health().drain_complete() {
                let stopped = daemon.stop();
                return Err(DurableProcessFailureV1 {
                    terminal: DurableProcessTerminalV1::InvalidState,
                    stopped,
                });
            }
            return Ok(daemon.stop());
        }
        match daemon.serve_next() {
            Ok(())
            | Err(
                DaemonErrorV1::Authentication
                | DaemonErrorV1::Handshake
                | DaemonErrorV1::Host(_)
                | DaemonErrorV1::ConnectionRefused
                | DaemonErrorV1::FrameLimit
                | DaemonErrorV1::Route
                | DaemonErrorV1::RequestLimit,
            ) => {}
            Err(error) => {
                let terminal = match error {
                    DaemonErrorV1::InvalidLimits | DaemonErrorV1::Draining => {
                        DurableProcessTerminalV1::InvalidState
                    }
                    DaemonErrorV1::UnsafeEndpoint
                    | DaemonErrorV1::LifetimeBusy
                    | DaemonErrorV1::CrashResidue
                    | DaemonErrorV1::ResidueMismatch => DurableProcessTerminalV1::Ownership,
                    DaemonErrorV1::Io(_) => DurableProcessTerminalV1::Transport,
                    DaemonErrorV1::Fenced => DurableProcessTerminalV1::Fenced,
                    DaemonErrorV1::Durability => DurableProcessTerminalV1::Durability,
                    DaemonErrorV1::ReplayEpochExhausted => DurableProcessTerminalV1::Capacity,
                    DaemonErrorV1::Authentication
                    | DaemonErrorV1::Handshake
                    | DaemonErrorV1::Host(_)
                    | DaemonErrorV1::ConnectionRefused
                    | DaemonErrorV1::FrameLimit
                    | DaemonErrorV1::Route
                    | DaemonErrorV1::RequestLimit => unreachable!("handled connection refusal"),
                };
                let stopped = daemon.stop();
                return Err(DurableProcessFailureV1 { terminal, stopped });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aegis_protocol::{CommandV1, GrantSummaryV1};
    use agora_host::v2::persistence::{HostCheckpointScopeV1, HostRecoveryLimitsV2};
    use agora_service::v2::persistence::ServiceRecoveryLimitsV2;
    use std::fs;
    use std::os::unix::{fs::PermissionsExt, net::UnixListener};
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };

    struct Port {
        reply: Result<ReplyV1, &'static str>,
        calls: Vec<(SessionHandleV1, CommandV1)>,
    }

    impl Port {
        fn returning(reply: ReplyV1) -> Self {
            Self {
                reply: Ok(reply),
                calls: Vec::new(),
            }
        }
    }

    impl ServicePort for Port {
        type Error = &'static str;

        fn execute(
            &mut self,
            session: SessionHandleV1,
            command: CommandV1,
        ) -> Result<ReplyV1, Self::Error> {
            self.calls.push((session, command));
            self.reply.clone()
        }
    }

    fn issued(handle: u64, epoch: u64, expires_at: u64) -> ReplyV1 {
        ReplyV1::Accepted(OutcomeV1::GrantIssued(GrantSummaryV1 {
            handle: GrantHandleV1(handle),
            parent: None,
            epoch,
            expires_at,
            scope_count: 1,
            delegation_depth: 0,
        }))
    }

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "agora-private-local-{}-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            Self(path)
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn durable_config() -> DurableHostConfigV2 {
        const MAXIMUM: usize = 1 << 20;
        DurableHostConfigV2::new(
            "agora-private-local-host",
            HostCheckpointScopeV1::new([0x41; 16]),
            MAXIMUM,
            HostRecoveryLimitsV2 {
                max_snapshot_bytes: MAXIMUM,
                max_service_snapshot_bytes: MAXIMUM,
                maximum_host_limits: HostLimits::default(),
                service: ServiceRecoveryLimitsV2 {
                    max_snapshot_bytes: MAXIMUM,
                    max_engine_snapshot_bytes: MAXIMUM,
                    maximum_service_limits: ServiceLimitsV2::default(),
                    maximum_engine_limits: Limits::default(),
                },
            },
        )
    }

    fn composition_plan(
        root: &TempRoot,
        startup: DurableCompositionStartupV1,
    ) -> DurableCompositionPlanV1 {
        DurableCompositionPlanV1 {
            store_path: root.0.join("store"),
            socket_path: root.0.join("agora.sock"),
            directory_limits: DirectoryLimitsV1::default(),
            engine_limits: Limits::default(),
            service_limits: ServiceLimitsV2::default(),
            host_limits: HostLimits::default(),
            durable: durable_config(),
            daemon_limits: DaemonLimitsV1::default(),
            startup,
        }
    }

    fn authority_parts(
        participant: &str,
    ) -> (ParticipantId, AegisContextV1, AegisVerifierV1<Port>) {
        (
            ParticipantId::new(participant),
            AegisContextV1::new(
                participant,
                VerifiedSubjectV1::from_verifier([0x31; 16]),
                SessionHandleV1(7),
                GrantHandleV1(9),
            ),
            AegisVerifierV1::new(
                Port::returning(ReplyV1::Refused(RefusalV1::NotAuthorized)),
                [0x51; 16],
            ),
        )
    }

    #[test]
    fn conversation_issues_exact_session_resource_and_submit_scope() {
        let selector = IssueAgoraGrantV1::new(4, 10, 20, AgoraGrantProfileV1::Conversation);
        let mut port = Port::returning(issued(9, 4, 20));

        assert_eq!(
            selector
                .issue(SessionHandleV1(7), &mut port, [3; 16])
                .unwrap(),
            GrantHandleV1(9)
        );
        assert_eq!(port.calls.len(), 1);
        assert_eq!(port.calls[0].0, SessionHandleV1(7));
        assert_eq!(
            port.calls[0].1,
            CommandV1::Issue {
                expected_epoch: 4,
                now: 10,
                expires_at: 20,
                scopes: vec![ScopeV1 {
                    engine: EngineNamespaceV1::Agora,
                    resource: [3; 16],
                    actions: 1 << ActionV1::Submit as u8,
                }],
            }
        );
    }

    #[test]
    fn scheduled_profile_adds_only_execute_and_observe() {
        let selector = IssueAgoraGrantV1::new(4, 10, 20, AgoraGrantProfileV1::ScheduledComposition);
        let mut port = Port::returning(issued(9, 4, 20));

        selector
            .issue(SessionHandleV1(7), &mut port, [3; 16])
            .unwrap();
        let CommandV1::Issue { scopes, .. } = &port.calls[0].1 else {
            panic!("expected issue")
        };
        assert_eq!(
            scopes[0].actions,
            (1 << ActionV1::Submit as u8)
                | (1 << ActionV1::Execute as u8)
                | (1 << ActionV1::Observe as u8)
        );
    }

    #[test]
    fn invalid_lifetime_refuses_without_authority_io() {
        let selector = IssueAgoraGrantV1::new(4, 20, 20, AgoraGrantProfileV1::Conversation);
        let mut port = Port::returning(issued(9, 4, 20));

        assert!(matches!(
            selector.issue(SessionHandleV1(7), &mut port, [3; 16]),
            Err(IssueAgoraGrantErrorV1::InvalidLifetime)
        ));
        assert!(port.calls.is_empty());
    }

    #[test]
    fn refusal_transport_and_inexact_success_remain_distinct_and_redacted() {
        let selector = IssueAgoraGrantV1::new(4, 10, 20, AgoraGrantProfileV1::Conversation);
        let mut refused = Port::returning(ReplyV1::Refused(RefusalV1::NotAdministrator));
        assert!(matches!(
            selector.issue(SessionHandleV1(7), &mut refused, [3; 16]),
            Err(IssueAgoraGrantErrorV1::Refused(RefusalV1::NotAdministrator))
        ));

        let mut inexact = Port::returning(issued(9, 5, 20));
        assert!(matches!(
            selector.issue(SessionHandleV1(7), &mut inexact, [3; 16]),
            Err(IssueAgoraGrantErrorV1::InexactReply)
        ));

        let mut failed = Port {
            reply: Err("private transport detail"),
            calls: Vec::new(),
        };
        let error = selector
            .issue(SessionHandleV1(7), &mut failed, [3; 16])
            .unwrap_err();
        let rendered = format!("{error:?}");
        assert!(matches!(error, IssueAgoraGrantErrorV1::Transport(_)));
        assert!(!rendered.contains("private transport detail"));
    }

    #[test]
    fn durable_factory_creates_opens_and_rotates_exact_endpoint_residue() {
        let root = TempRoot::new();
        let first = ServerIncarnationV1::new([0xa1; 16]);
        let second = ServerIncarnationV1::new([0xb2; 16]);
        let bootstrap = BootstrapV1 {
            principal: ParticipantId::new("alice"),
            participants: vec![BootstrapParticipantV1 {
                id: ParticipantId::new("alice"),
                display_name: "Alice".to_owned(),
                kind: ParticipantKind::Human,
            }],
            rooms: vec![BootstrapRoomV1 {
                id: RoomId::new("general"),
                name: "General".to_owned(),
                owner: ParticipantId::new("alice"),
                admission: AdmissionPolicy::InviteOnly,
            }],
        };
        let (participant, context, verifier) = authority_parts("alice");
        let daemon = start_durable_composition_parts_v1(
            participant,
            context,
            verifier,
            composition_plan(
                &root,
                DurableCompositionStartupV1::Create {
                    bootstrap,
                    server_incarnation: first,
                },
            ),
        )
        .unwrap();
        assert!(daemon.health().ready());
        assert!(
            daemon
                .host()
                .host()
                .unwrap()
                .service()
                .engine()
                .room(&RoomId::new("general"))
                .is_some()
        );
        assert_eq!(
            daemon.stop().cleanup,
            agora_local_daemon::DaemonCleanupV1::Clean
        );

        let (participant, context, verifier) = authority_parts("alice");
        let reopened = start_durable_composition_parts_v1(
            participant,
            context,
            verifier,
            composition_plan(
                &root,
                DurableCompositionStartupV1::Open {
                    server_incarnation: second,
                },
            ),
        )
        .unwrap();
        assert!(
            reopened
                .host()
                .host()
                .unwrap()
                .service()
                .engine()
                .room(&RoomId::new("general"))
                .is_some()
        );
        assert_eq!(
            reopened.stop().cleanup,
            agora_local_daemon::DaemonCleanupV1::Clean
        );

        let socket = root.0.join("agora.sock");
        let stale = UnixListener::bind(&socket).unwrap();
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
        drop(stale);
        let lifetime = root.0.join("agora.sock.owner.lock");
        let mut record = b"AGORALF1".to_vec();
        record.extend_from_slice(&second.as_bytes());
        fs::write(&lifetime, record).unwrap();
        fs::set_permissions(&lifetime, fs::Permissions::from_mode(0o600)).unwrap();

        let third = ServerIncarnationV1::new([0xc3; 16]);
        let (participant, context, verifier) = authority_parts("alice");
        let recovered = start_durable_composition_parts_v1(
            participant,
            context,
            verifier,
            composition_plan(
                &root,
                DurableCompositionStartupV1::RecoverResidue {
                    previous_incarnation: second,
                    next_incarnation: third,
                },
            ),
        )
        .unwrap();
        assert!(recovered.health().ready());
        assert!(
            recovered
                .host()
                .host()
                .unwrap()
                .service()
                .engine()
                .room(&RoomId::new("general"))
                .is_some()
        );
        assert_eq!(
            recovered.stop().cleanup,
            agora_local_daemon::DaemonCleanupV1::Clean
        );
    }

    #[test]
    fn durable_create_refuses_authority_principal_mismatch_before_store_or_endpoint() {
        let root = TempRoot::new();
        let bootstrap = BootstrapV1 {
            principal: ParticipantId::new("alice"),
            participants: vec![BootstrapParticipantV1 {
                id: ParticipantId::new("alice"),
                display_name: "Alice".to_owned(),
                kind: ParticipantKind::Human,
            }],
            rooms: Vec::new(),
        };
        let (participant, context, verifier) = authority_parts("bob");
        let result = start_durable_composition_parts_v1(
            participant,
            context,
            verifier,
            composition_plan(
                &root,
                DurableCompositionStartupV1::Create {
                    bootstrap,
                    server_incarnation: ServerIncarnationV1::new([0xa1; 16]),
                },
            ),
        );

        assert!(matches!(
            result,
            Err(DurableCompositionStartErrorV1::PrincipalMismatch)
        ));
        assert!(!root.0.join("store").exists());
        assert!(!root.0.join("agora.sock").exists());
        assert!(!root.0.join("agora.sock.owner.lock").exists());
    }

    #[test]
    fn explicit_shutdown_wakes_accept_drains_and_returns_exact_cleanup_receipt() {
        let root = TempRoot::new();
        let socket = root.0.join("agora.sock");
        let bootstrap = BootstrapV1 {
            principal: ParticipantId::new("alice"),
            participants: vec![BootstrapParticipantV1 {
                id: ParticipantId::new("alice"),
                display_name: "Alice".to_owned(),
                kind: ParticipantKind::Human,
            }],
            rooms: Vec::new(),
        };
        let (participant, context, verifier) = authority_parts("alice");
        let daemon = start_durable_composition_parts_v1(
            participant,
            context,
            verifier,
            composition_plan(
                &root,
                DurableCompositionStartupV1::Create {
                    bootstrap,
                    server_incarnation: ServerIncarnationV1::new([0xd4; 16]),
                },
            ),
        )
        .unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let server_stop = Arc::clone(&stop);
        let server =
            std::thread::spawn(move || serve_durable_process_with_port_v1(daemon, &server_stop));

        assert!(request_durable_process_shutdown_v1(&stop, &socket));
        assert!(!request_durable_process_shutdown_v1(&stop, &socket));
        let receipt = server.join().unwrap().unwrap();
        assert_eq!(
            receipt.prior_lifecycle,
            agora_local_daemon::DaemonLifecycleV1::Draining
        );
        assert_eq!(
            receipt.health.lifecycle,
            agora_local_daemon::DaemonLifecycleV1::Stopped
        );
        assert_eq!(receipt.health.active_connections, 0);
        assert_eq!(receipt.cleanup, agora_local_daemon::DaemonCleanupV1::Clean);
        assert!(!socket.exists());
        assert!(!root.0.join("agora.sock.owner.lock").exists());
    }
}
