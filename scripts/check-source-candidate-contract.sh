#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

bash -n scripts/build-source-candidate.sh
bash -n scripts/verify-source-candidate.sh
bash -n scripts/qualify-source-candidate.sh
for field in \
  'schema=agora-source-candidate-v1' \
  'stable_component_count=20' \
  'provenance=exact-local-content-closure' \
  'claim=self-contained-stable-local-source' \
  'publication=denied'; do
  if ! rg -q "echo '$field'" scripts/build-source-candidate.sh \
    || ! rg -q "\\^$field\\\$" scripts/verify-source-candidate.sh; then
    echo "source-candidate contract field is not build/verify closed: $field" >&2
    exit 1
  fi
done
if ! cmp -s release/keystone-workspace.Cargo.toml <(printf '%s\n' \
  '[workspace]' \
  'members = [' \
  '    "crates/keystone-codec",' \
  '    "crates/keystone-storage",' \
  ']' \
  'resolver = "3"' \
  '' \
  '[workspace.package]' \
  'version = "0.0.1"' \
  'edition = "2024"' \
  'rust-version = "1.95"' \
  'license = "MIT OR Apache-2.0"' \
  'repository = "https://github.com/gantbd/keystone"' \
  'publish = false' \
  '' \
  '[workspace.lints.rust]' \
  'unsafe_code = "forbid"' \
  '' \
  '[workspace.lints.clippy]' \
  'all = "warn"'); then
  echo 'reduced Keystone workspace template drifted' >&2
  exit 1
fi
echo 'Agora source-candidate construction and verification contract is pinned'
