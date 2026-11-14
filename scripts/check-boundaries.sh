#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

if rg -n 'AgentActivation|activate_agents|max_agent_activations' \
  engine protocol client service host ports adapters transport applications sdk; then
  echo 'agent activation semantics crossed the Agora boundary' >&2
  exit 1
fi

if rg -n 'std::(fs|net|process)::|tokio|async_std|reqwest|rusqlite' \
  engine protocol client service host ports adapters transport applications sdk -g '*.rs' \
  -g '!adapters/agora-storage-directory/**' \
  -g '!transport/agora-local-transport/**' \
  -g '!applications/agora-local-daemon/**'; then
  echo 'effectful runtime dependency crossed an Agora semantic boundary' >&2
  exit 1
fi

if sed '/^#\[cfg(test)\]/,$d' applications/agora-local-daemon/src/lib.rs \
  | rg -n 'std::(fs|net|process)::|std::os::unix::fs::|UnixListener'; then
  echo 'local transport mechanics crossed into the daemon composition root' >&2
  exit 1
fi

if sed '/^#\[cfg(test)\]/,$d' applications/agora-local-daemon/src/lib.rs \
  | rg -n 'std::(net|process)::|Tcp|Udp|tokio|async_std|reqwest|rusqlite'; then
  echo 'unqualified effect entered the private local transport adapter' >&2
  exit 1
fi

if sed '/^#\[cfg(test)\]/,$d' adapters/agora-storage-directory/src/lib.rs \
  | rg -n 'std::(net|process)::|tokio|async_std|reqwest|rusqlite'; then
  echo 'unqualified effect entered the private-directory adapter' >&2
  exit 1
fi

for leaf in agora-engine agora-protocol; do
  normal_tree=$(cargo tree -p "$leaf" --edges normal --depth 1 --locked --offline)
  if [[ $(printf '%s\n' "$normal_tree" | wc -l | tr -d ' ') != 1 ]]; then
    printf '%s\n' "$normal_tree" >&2
    echo "$leaf gained a normal dependency" >&2
    exit 1
  fi
done

client_tree=$(cargo tree -p agora-client --edges normal --depth 2 --locked --offline)
if printf '%s\n' "$client_tree" | rg 'agora-(engine|service)'; then
  echo 'Agora client crossed into engine or service internals' >&2
  exit 1
fi

protocol_tree=$(cargo tree -p agora-protocol --edges normal --depth 2 --locked --offline)
if printf '%s\n' "$protocol_tree" | rg 'agora-(engine|service|client)'; then
  echo 'Agora protocol crossed into another Agora layer' >&2
  exit 1
fi

wire_tree=$(cargo tree -p agora-wire --edges normal --depth 2 --locked --offline)
if printf '%s\n' "$wire_tree" | rg 'agora-(engine|service|client)'; then
  echo 'Agora wire crossed into engine, service, or client internals' >&2
  exit 1
fi

service_tree=$(cargo tree -p agora-service --edges normal --depth 1 --locked --offline)
if ! printf '%s\n' "$service_tree" | rg -q 'agora-search-port' \
  || printf '%s\n' "$service_tree" | rg -q 'agora-search-memory'; then
  printf '%s\n' "$service_tree" >&2
  echo 'Agora service search composition escaped its port-only production edge' >&2
  exit 1
fi

host_tree=$(cargo tree -p agora-host --edges normal --depth 1 --locked --offline)
if printf '%s\n' "$host_tree" | rg 'agora-client|aegis|chronos|moira|daimon|hermes'; then
  echo 'Agora host imported a client, authority, or peer-engine implementation' >&2
  exit 1
fi

durable_tree=$(cargo tree -p agora-durable --edges normal --depth 1 --locked --offline)
if ! printf '%s\n' "$durable_tree" | rg -q 'agora-service' \
  || ! printf '%s\n' "$durable_tree" | rg -q 'agora-protocol' \
  || ! printf '%s\n' "$durable_tree" | rg -q 'agora-host' \
  || ! printf '%s\n' "$durable_tree" | rg -q 'agora-storage-observation' \
  || ! printf '%s\n' "$durable_tree" | rg -q 'keystone-storage' \
  || printf '%s\n' "$durable_tree" | rg -q 'agora-(client|wire)'; then
  printf '%s\n' "$durable_tree" >&2
  echo 'Agora durable boundary escaped its host-service-protocol-storage-port closure' >&2
  exit 1
fi

aegis_verifier_tree=$(cargo tree -p agora-aegis-verifier --edges normal --depth 1 --locked --offline)
if [[ $(printf '%s\n' "$aegis_verifier_tree" | wc -l | tr -d ' ') != 5 ]] \
  || ! printf '%s\n' "$aegis_verifier_tree" | rg -q 'aegis-client' \
  || ! printf '%s\n' "$aegis_verifier_tree" | rg -q 'aegis-protocol' \
  || ! printf '%s\n' "$aegis_verifier_tree" | rg -q 'agora-host' \
  || ! printf '%s\n' "$aegis_verifier_tree" | rg -q 'agora-durable' \
  || printf '%s\n' "$aegis_verifier_tree" | rg -q 'agora-(engine|service|wire|client)'; then
  printf '%s\n' "$aegis_verifier_tree" >&2
  echo 'Agora Aegis adapter escaped its host-durable-client-protocol closure' >&2
  exit 1
fi

local_transport_tree=$(cargo tree -p agora-local-transport --edges normal --depth 1 --locked --offline)
if [[ $(printf '%s\n' "$local_transport_tree" | wc -l | tr -d ' ') != 2 ]] \
  || ! printf '%s\n' "$local_transport_tree" | rg -q 'agora-wire' \
  || printf '%s\n' "$local_transport_tree" | rg -q 'aegis|keystone|agora-(engine|service|host|client|durable)'; then
  printf '%s\n' "$local_transport_tree" >&2
  echo 'Agora local transport gained authority, domain, host, or durability knowledge' >&2
  exit 1
fi

local_daemon_tree=$(cargo tree -p agora-local-daemon --edges normal --depth 1 --locked --offline)
if [[ $(printf '%s\n' "$local_daemon_tree" | wc -l | tr -d ' ') != 7 ]] \
  || ! printf '%s\n' "$local_daemon_tree" | rg -q 'agora-aegis-verifier' \
  || ! printf '%s\n' "$local_daemon_tree" | rg -q 'agora-durable' \
  || ! printf '%s\n' "$local_daemon_tree" | rg -q 'agora-host' \
  || ! printf '%s\n' "$local_daemon_tree" | rg -q 'agora-local-transport' \
  || ! printf '%s\n' "$local_daemon_tree" | rg -q 'agora-wire' \
  || ! printf '%s\n' "$local_daemon_tree" | rg -q 'keystone-storage' \
  || printf '%s\n' "$local_daemon_tree" | rg -q 'agora-(engine|service|client|storage)'; then
  printf '%s\n' "$local_daemon_tree" >&2
  echo 'Agora local daemon escaped its Aegis-durable-host-wire-storage-port composition closure' >&2
  exit 1
fi

observation_tree=$(cargo tree -p agora-storage-observation --edges normal --depth 1 --locked --offline)
if [[ $(printf '%s\n' "$observation_tree" | wc -l | tr -d ' ') != 2 ]] \
  || ! printf '%s\n' "$observation_tree" | rg -q 'keystone-storage'; then
  printf '%s\n' "$observation_tree" >&2
  echo 'Agora storage-observation port escaped its effect-free Keystone closure' >&2
  exit 1
fi

directory_tree=$(cargo tree -p agora-storage-directory --edges normal --depth 1 --locked --offline)
if [[ $(printf '%s\n' "$directory_tree" | wc -l | tr -d ' ') != 3 ]] \
  || ! printf '%s\n' "$directory_tree" | rg -q 'agora-storage-observation' \
  || ! printf '%s\n' "$directory_tree" | rg -q 'keystone-storage' \
  || printf '%s\n' "$directory_tree" | rg -q 'agora-(engine|host|service|wire|client)'; then
  printf '%s\n' "$directory_tree" >&2
  echo 'Agora directory adapter escaped its storage-port-only runtime closure' >&2
  exit 1
fi

search_port_tree=$(cargo tree -p agora-search-port --edges normal --depth 1 --locked --offline)
if [[ $(printf '%s\n' "$search_port_tree" | wc -l | tr -d ' ') != 1 ]]; then
  printf '%s\n' "$search_port_tree" >&2
  echo 'Agora search port gained a normal dependency' >&2
  exit 1
fi

search_memory_tree=$(cargo tree -p agora-search-memory --edges normal --depth 1 --locked --offline)
if [[ $(printf '%s\n' "$search_memory_tree" | wc -l | tr -d ' ') != 2 ]] \
  || ! printf '%s\n' "$search_memory_tree" | rg -q 'agora-search-port' \
  || printf '%s\n' "$search_memory_tree" | rg -q 'agora-(engine|protocol|service|host|wire|client)'; then
  printf '%s\n' "$search_memory_tree" >&2
  echo 'Agora search reference escaped its effect-free port-only closure' >&2
  exit 1
fi

embedded_sdk_tree=$(cargo tree -p agora-sdk-embedded --edges normal --depth 1 --locked --offline)
if [[ $(printf '%s\n' "$embedded_sdk_tree" | wc -l | tr -d ' ') != 4 ]] \
  || ! printf '%s\n' "$embedded_sdk_tree" | rg -q 'agora-engine' \
  || ! printf '%s\n' "$embedded_sdk_tree" | rg -q 'agora-host' \
  || ! printf '%s\n' "$embedded_sdk_tree" | rg -q 'agora-service' \
  || printf '%s\n' "$embedded_sdk_tree" | rg -q 'agora-(client|sdk-client)'; then
  printf '%s\n' "$embedded_sdk_tree" >&2
  echo 'Agora embedded SDK escaped its engine-service-host closure' >&2
  exit 1
fi

client_sdk_tree=$(cargo tree -p agora-sdk-client --edges normal --depth 1 --locked --offline)
if [[ $(printf '%s\n' "$client_sdk_tree" | wc -l | tr -d ' ') != 4 ]] \
  || ! printf '%s\n' "$client_sdk_tree" | rg -q 'agora-client' \
  || ! printf '%s\n' "$client_sdk_tree" | rg -q 'agora-protocol' \
  || ! printf '%s\n' "$client_sdk_tree" | rg -q 'agora-wire' \
  || printf '%s\n' "$client_sdk_tree" | rg -q 'agora-(engine|host|service)'; then
  printf '%s\n' "$client_sdk_tree" >&2
  echo 'Agora client SDK escaped its client-protocol-wire closure' >&2
  exit 1
fi

for sdk_manifest in sdk/embedded/Cargo.toml sdk/client/Cargo.toml; do
  if ! rg -q '^publish = false$' "$sdk_manifest"; then
    echo "$sdk_manifest enabled publication without release approval" >&2
    exit 1
  fi
done

echo 'Agora dependency direction closed; derived search remains port-only and effects stay in qualified adapters'
