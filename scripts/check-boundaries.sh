#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

if rg -n 'AgentActivation|activate_agents|max_agent_activations' src tests; then
  echo 'agent activation semantics crossed the Agora boundary' >&2
  exit 1
fi

if rg -n 'std::(fs|net|process)::|tokio|async_std|reqwest|rusqlite' src; then
  echo 'effectful runtime dependency crossed the Agora core boundary' >&2
  exit 1
fi

normal_tree=$(cargo tree --edges normal --depth 1 --locked --offline)
if [[ $(printf '%s\n' "$normal_tree" | wc -l | tr -d ' ') != 1 ]]; then
  printf '%s\n' "$normal_tree" >&2
  echo 'Agora core gained a runtime dependency' >&2
  exit 1
fi

echo 'Agora core boundary closed; no runtime dependencies or activation semantics'
