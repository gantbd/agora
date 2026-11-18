#![forbid(unsafe_code)]

use aegis_local_client::LimitsV1 as AegisWireLimitsV1;
use agora_engine::{AdmissionPolicy, Limits, ParticipantId, ParticipantKind, RoomId};
use agora_host::{
    HostLimits,
    v2::persistence::{HostCheckpointScopeV1, HostRecoveryLimitsV2},
};
use agora_local_daemon::{DaemonLimitsV1, ServerIncarnationV1};
use agora_private_local::{
    AgoraGrantProfileV1, AuthoritySelectionV1, BootstrapParticipantV1, BootstrapRoomV1,
    BootstrapV1, DurableCompositionPlanV1, DurableCompositionStartupV1, IssueAgoraGrantV1,
    connect_authority_v1, request_durable_process_shutdown_v1, serve_durable_process_v1,
    start_durable_composition_v1,
};
use agora_service::v2::{ServiceLimitsV2, persistence::ServiceRecoveryLimitsV2};
use agora_storage_directory::DirectoryLimitsV1;
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    io::{self, Read},
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
    thread,
    time::Duration,
};

const MAXIMUM_CONFIG_BYTES: usize = 64 * 1_024;
const MAXIMUM_CONFIG_FIELDS: usize = 72;
const MAXIMUM_BOOTSTRAP_PARTICIPANTS: usize = 16;
const MAXIMUM_SNAPSHOT_BYTES: usize = 8 * 1_024 * 1_024;
const DURABLE_KEY: &str = "agora-private-local-host-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Create,
    Open,
    Recover,
}

struct Config {
    mode: Mode,
    store: PathBuf,
    socket: PathBuf,
    aegis_socket: PathBuf,
    participant: String,
    subject: [u8; 16],
    resource: [u8; 16],
    authority_epoch: u64,
    grant_now: u64,
    grant_expires_at: u64,
    grant_profile: AgoraGrantProfileV1,
    host_scope: [u8; 16],
    incarnation: IncarnationSelection,
    create: Option<CreateConfig>,
}

struct CreateConfig {
    participants: Vec<BootstrapParticipantV1>,
    room_id: String,
    room_name: String,
    admission: AdmissionPolicy,
}

enum IncarnationSelection {
    One([u8; 16]),
    Recovery { previous: [u8; 16], next: [u8; 16] },
}

impl Config {
    fn read(path: PathBuf) -> Result<Self, String> {
        let bytes = fs::read(path).map_err(|_| "configuration unavailable")?;
        if bytes.is_empty() || bytes.len() > MAXIMUM_CONFIG_BYTES {
            return Err("configuration size refused".into());
        }
        let text = std::str::from_utf8(&bytes).map_err(|_| "configuration must be UTF-8")?;
        Self::parse(text)
    }

    fn parse(text: &str) -> Result<Self, String> {
        let mut fields = BTreeMap::new();
        for line in text.lines() {
            if line.is_empty() || line.trim() != line {
                return Err("configuration line is not canonical".into());
            }
            let (key, value) = line
                .split_once('=')
                .ok_or("configuration line requires key=value")?;
            if key.is_empty() || value.is_empty() || fields.len() == MAXIMUM_CONFIG_FIELDS {
                return Err("configuration field refused".into());
            }
            if fields.insert(key.to_owned(), value.to_owned()).is_some() {
                return Err("duplicate configuration field".into());
            }
        }
        let schema = match take(&mut fields, "schema")?.as_str() {
            "1" => 1,
            "2" => 2,
            _ => return Err("configuration schema refused".into()),
        };
        let mode = match take(&mut fields, "mode")?.as_str() {
            "create" => Mode::Create,
            "open" => Mode::Open,
            "recover" => Mode::Recover,
            _ => return Err("mode must be create, open, or recover".into()),
        };
        let store = PathBuf::from(take(&mut fields, "store")?);
        let socket = PathBuf::from(take(&mut fields, "socket")?);
        let aegis_socket = PathBuf::from(take(&mut fields, "aegis_socket")?);
        let participant = take(&mut fields, "participant")?;
        let subject = parse_hex(&take(&mut fields, "subject")?)?;
        let resource = parse_hex(&take(&mut fields, "resource")?)?;
        let authority_epoch = parse_u64(&take(&mut fields, "authority_epoch")?)?;
        let grant_now = parse_u64(&take(&mut fields, "grant_now")?)?;
        let grant_expires_at = parse_u64(&take(&mut fields, "grant_expires_at")?)?;
        let grant_profile = match take(&mut fields, "grant_profile")?.as_str() {
            "conversation" => AgoraGrantProfileV1::Conversation,
            "scheduled" => AgoraGrantProfileV1::ScheduledComposition,
            _ => return Err("grant_profile must be conversation or scheduled".into()),
        };
        let host_scope = parse_hex(&take(&mut fields, "host_scope")?)?;
        let (incarnation, create) = match mode {
            Mode::Create => {
                let incarnation = parse_hex(&take(&mut fields, "server_incarnation")?)?;
                let participants = match schema {
                    1 => vec![BootstrapParticipantV1 {
                        id: ParticipantId::new(&participant),
                        display_name: take(&mut fields, "display_name")?,
                        kind: parse_participant_kind(&take(&mut fields, "participant_kind")?)?,
                    }],
                    2 => parse_participant_cohort(&mut fields, &participant)?,
                    _ => unreachable!("schema is closed above"),
                };
                let room_id = take(&mut fields, "room_id")?;
                let room_name = take(&mut fields, "room_name")?;
                let admission = match take(&mut fields, "admission")?.as_str() {
                    "open" => AdmissionPolicy::Open,
                    "invite-only" => AdmissionPolicy::InviteOnly,
                    _ => return Err("admission must be open or invite-only".into()),
                };
                (
                    IncarnationSelection::One(incarnation),
                    Some(CreateConfig {
                        participants,
                        room_id,
                        room_name,
                        admission,
                    }),
                )
            }
            Mode::Open => (
                IncarnationSelection::One(parse_hex(&take(&mut fields, "server_incarnation")?)?),
                None,
            ),
            Mode::Recover => (
                IncarnationSelection::Recovery {
                    previous: parse_hex(&take(&mut fields, "previous_incarnation")?)?,
                    next: parse_hex(&take(&mut fields, "next_incarnation")?)?,
                },
                None,
            ),
        };
        if !fields.is_empty() {
            return Err("unknown configuration field".into());
        }
        if participant.is_empty() || participant.len() > 1_024 {
            return Err("participant refused".into());
        }
        Ok(Self {
            mode,
            store,
            socket,
            aegis_socket,
            participant,
            subject,
            resource,
            authority_epoch,
            grant_now,
            grant_expires_at,
            grant_profile,
            host_scope,
            incarnation,
            create,
        })
    }
}

fn take(fields: &mut BTreeMap<String, String>, key: &str) -> Result<String, String> {
    fields
        .remove(key)
        .ok_or_else(|| format!("missing configuration field {key}"))
}

fn parse_u64(value: &str) -> Result<u64, String> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err("integer must be canonical decimal u64".into());
    }
    value
        .parse()
        .map_err(|_| "integer must be canonical decimal u64".into())
}

fn parse_participant_kind(value: &str) -> Result<ParticipantKind, String> {
    match value {
        "human" => Ok(ParticipantKind::Human),
        "agent" => Ok(ParticipantKind::Agent),
        _ => Err("participant kind must be human or agent".into()),
    }
}

fn parse_participant_cohort(
    fields: &mut BTreeMap<String, String>,
    principal: &str,
) -> Result<Vec<BootstrapParticipantV1>, String> {
    let count = parse_u64(&take(fields, "participant_count")?)?
        .try_into()
        .map_err(|_| "participant_count refused")?;
    if count == 0 || count > MAXIMUM_BOOTSTRAP_PARTICIPANTS {
        return Err("participant_count refused".into());
    }
    let mut ids = BTreeSet::new();
    let mut participants = Vec::with_capacity(count);
    for index in 0..count {
        let id = take(fields, &format!("participant_{index}_id"))?;
        if id.len() > 1_024 || !ids.insert(id.clone()) {
            return Err("bootstrap participant id refused".into());
        }
        let display_name = take(fields, &format!("participant_{index}_display_name"))?;
        if display_name.len() > 1_024 {
            return Err("bootstrap participant display name refused".into());
        }
        let kind = parse_participant_kind(&take(fields, &format!("participant_{index}_kind"))?)?;
        participants.push(BootstrapParticipantV1 {
            id: ParticipantId::new(id),
            display_name,
            kind,
        });
    }
    if !ids.contains(principal) {
        return Err("authenticated principal must appear in bootstrap participant cohort".into());
    }
    Ok(participants)
}

fn parse_hex(value: &str) -> Result<[u8; 16], String> {
    if value.len() != 32
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("identifier must be 32 lowercase hexadecimal digits".into());
    }
    let mut output = [0; 16];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        output[index] = (hex(pair[0])? << 4) | hex(pair[1])?;
    }
    Ok(output)
}

fn hex(value: u8) -> Result<u8, String> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err("identifier must be lowercase hexadecimal".into()),
    }
}

fn durable_config(scope: [u8; 16]) -> agora_durable::DurableHostConfigV2 {
    agora_durable::DurableHostConfigV2::new(
        DURABLE_KEY,
        HostCheckpointScopeV1::new(scope),
        MAXIMUM_SNAPSHOT_BYTES,
        HostRecoveryLimitsV2 {
            max_snapshot_bytes: MAXIMUM_SNAPSHOT_BYTES,
            max_service_snapshot_bytes: MAXIMUM_SNAPSHOT_BYTES,
            maximum_host_limits: HostLimits::default(),
            service: ServiceRecoveryLimitsV2 {
                max_snapshot_bytes: MAXIMUM_SNAPSHOT_BYTES,
                max_engine_snapshot_bytes: MAXIMUM_SNAPSHOT_BYTES,
                maximum_service_limits: ServiceLimitsV2::default(),
                maximum_engine_limits: Limits::default(),
            },
        },
    )
}

fn bootstrap_config(principal: &str, create: CreateConfig) -> BootstrapV1 {
    BootstrapV1 {
        principal: ParticipantId::new(principal),
        participants: create.participants,
        rooms: vec![BootstrapRoomV1 {
            id: RoomId::new(create.room_id),
            name: create.room_name,
            owner: ParticipantId::new(principal),
            admission: create.admission,
        }],
    }
}

fn run(config: Config) -> Result<(), Box<dyn Error>> {
    let participant = ParticipantId::new(&config.participant);
    let selection = AuthoritySelectionV1::new(participant.clone(), config.subject, config.resource);
    let mut selector = IssueAgoraGrantV1::new(
        config.authority_epoch,
        config.grant_now,
        config.grant_expires_at,
        config.grant_profile,
    );
    let authority = connect_authority_v1(
        &config.aegis_socket,
        AegisWireLimitsV1::default(),
        Duration::from_secs(5),
        &participant,
        selection,
        &mut selector,
    )
    .map_err(|error| format!("authority startup refused: {error:?}"))?;
    let startup = match (config.mode, config.incarnation, config.create) {
        (Mode::Create, IncarnationSelection::One(incarnation), Some(create)) => {
            DurableCompositionStartupV1::Create {
                bootstrap: bootstrap_config(&config.participant, create),
                server_incarnation: ServerIncarnationV1::new(incarnation),
            }
        }
        (Mode::Open, IncarnationSelection::One(incarnation), None) => {
            DurableCompositionStartupV1::Open {
                server_incarnation: ServerIncarnationV1::new(incarnation),
            }
        }
        (Mode::Recover, IncarnationSelection::Recovery { previous, next }, None) => {
            DurableCompositionStartupV1::RecoverResidue {
                previous_incarnation: ServerIncarnationV1::new(previous),
                next_incarnation: ServerIncarnationV1::new(next),
            }
        }
        _ => return Err("configuration mode invariant refused".into()),
    };
    let socket = config.socket.clone();
    let daemon = start_durable_composition_v1(
        authority,
        DurableCompositionPlanV1 {
            store_path: config.store,
            socket_path: config.socket,
            directory_limits: DirectoryLimitsV1::default(),
            engine_limits: Limits::default(),
            service_limits: ServiceLimitsV2::default(),
            host_limits: HostLimits::default(),
            durable: durable_config(config.host_scope),
            daemon_limits: DaemonLimitsV1::default(),
            startup,
        },
    )
    .map_err(|error| format!("durable owner startup refused: {error:?}"))?;
    let stop = Arc::new(AtomicBool::new(false));
    let input_stop = Arc::clone(&stop);
    thread::Builder::new()
        .name("agora-shutdown-input".into())
        .spawn(move || {
            let mut byte = [0_u8; 1];
            let _ = io::stdin().read(&mut byte);
            request_durable_process_shutdown_v1(&input_stop, socket);
        })
        .map_err(|_| "shutdown input owner unavailable")?;
    let receipt = serve_durable_process_v1(daemon, &stop)
        .map_err(|error| format!("durable owner stopped: {error:?}"))?;
    println!(
        "{{\"schema\":1,\"priorLifecycle\":\"draining\",\"cleanup\":\"{}\",\"acceptedConnections\":{},\"completedConnections\":{},\"refusedConnections\":{}}}",
        match receipt.cleanup {
            agora_local_daemon::DaemonCleanupV1::Clean => "clean",
            agora_local_daemon::DaemonCleanupV1::ResiduePreserved => "residue-preserved",
            agora_local_daemon::DaemonCleanupV1::Incomplete => "incomplete",
        },
        receipt.health.accepted_connections,
        receipt.health.completed_connections,
        receipt.health.refused_connections,
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let path = arguments
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| usage("CONFIG required"))?;
    if arguments.next().is_some() {
        return Err(usage("unexpected argument").into());
    }
    let config = Config::read(path).map_err(|error| usage(&error))?;
    run(config)
}

fn usage(message: &str) -> String {
    format!("{message}; usage: agora-private-local CONFIG")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn common(mode: &str, tail: &str) -> String {
        common_schema("1", mode, tail)
    }

    fn common_schema(schema: &str, mode: &str, tail: &str) -> String {
        format!(
            "schema={schema}\nmode={mode}\nstore=/private/store\nsocket=/private/agora.sock\naegis_socket=/private/aegis.sock\nparticipant=alice\nsubject=01010101010101010101010101010101\nresource=02020202020202020202020202020202\nauthority_epoch=7\ngrant_now=10\ngrant_expires_at=20\ngrant_profile=conversation\nhost_scope=03030303030303030303030303030303\n{tail}"
        )
    }

    #[test]
    fn configuration_closes_create_open_and_recovery_modes() {
        let create = Config::parse(&common(
            "create",
            "server_incarnation=04040404040404040404040404040404\ndisplay_name=Alice\nparticipant_kind=human\nroom_id=general\nroom_name=General\nadmission=invite-only\n",
        ))
        .unwrap();
        assert_eq!(create.mode, Mode::Create);
        assert_eq!(create.create.unwrap().participants.len(), 1);

        let open = Config::parse(&common(
            "open",
            "server_incarnation=05050505050505050505050505050505\n",
        ))
        .unwrap();
        assert_eq!(open.mode, Mode::Open);

        let recover = Config::parse(&common(
            "recover",
            "previous_incarnation=05050505050505050505050505050505\nnext_incarnation=06060606060606060606060606060606\n",
        ))
        .unwrap();
        assert_eq!(recover.mode, Mode::Recover);
    }

    #[test]
    fn schema_two_closes_a_bounded_human_agent_bootstrap_cohort() {
        let create = Config::parse(&common_schema(
            "2",
            "create",
            "server_incarnation=04040404040404040404040404040404\nparticipant_count=2\nparticipant_0_id=alice\nparticipant_0_display_name=Alice\nparticipant_0_kind=human\nparticipant_1_id=local-agent\nparticipant_1_display_name=Local Agent\nparticipant_1_kind=agent\nroom_id=general\nroom_name=General\nadmission=invite-only\n",
        ))
        .unwrap();
        let bootstrap = bootstrap_config("alice", create.create.unwrap());
        let engine = bootstrap.build_engine(Limits::default()).unwrap();
        assert_eq!(engine.participants().len(), 2);
        assert_eq!(
            engine
                .participant(&ParticipantId::new("alice"))
                .unwrap()
                .kind(),
            ParticipantKind::Human
        );
        assert_eq!(
            engine
                .participant(&ParticipantId::new("local-agent"))
                .unwrap()
                .kind(),
            ParticipantKind::Agent
        );

        let missing_principal = common_schema(
            "2",
            "create",
            "server_incarnation=04040404040404040404040404040404\nparticipant_count=1\nparticipant_0_id=local-agent\nparticipant_0_display_name=Local Agent\nparticipant_0_kind=agent\nroom_id=general\nroom_name=General\nadmission=invite-only\n",
        );
        assert!(Config::parse(&missing_principal).is_err());

        let duplicate = common_schema(
            "2",
            "create",
            "server_incarnation=04040404040404040404040404040404\nparticipant_count=2\nparticipant_0_id=alice\nparticipant_0_display_name=Alice\nparticipant_0_kind=human\nparticipant_1_id=alice\nparticipant_1_display_name=Alias\nparticipant_1_kind=agent\nroom_id=general\nroom_name=General\nadmission=invite-only\n",
        );
        assert!(Config::parse(&duplicate).is_err());
    }

    #[test]
    fn configuration_rejects_unknown_duplicate_noncanonical_and_cross_mode_fields() {
        assert!(Config::parse(&(common("open", "server_incarnation=00\n"))).is_err());
        assert!(
            Config::parse(
                &(common(
                    "open",
                    "server_incarnation=05050505050505050505050505050505\nparticipant=duplicate\n"
                ))
            )
            .is_err()
        );
        assert!(
            Config::parse(
                &(common(
                    "open",
                    "server_incarnation=05050505050505050505050505050505\nroom_id=general\n"
                ))
            )
            .is_err()
        );
        assert!(
            Config::parse(&common(
                "open",
                "server_incarnation=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n"
            ))
            .is_err()
        );
        assert!(parse_u64("01").is_err());
    }
}
