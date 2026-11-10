#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

if rg -n 'AgentActivation|activate_agents|max_agent_activations' src tests; then
  echo 'agent activation semantics crossed the Agora boundary' >&2
  exit 1
fi

if rg -n 'std::(fs|net|process)::|tokio|async_std|reqwest|rusqlite' src crates/*/src; then
  echo 'effectful runtime dependency crossed an Agora semantic boundary' >&2
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

echo 'Agora dependency direction closed; no effects or activation semantics'
