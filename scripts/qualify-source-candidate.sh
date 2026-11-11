#!/usr/bin/env bash
set -euo pipefail

if (( $# != 1 )); then
  echo 'usage: qualify-source-candidate.sh CANDIDATE_DIRECTORY' >&2
  exit 1
fi
root=$1
"$(dirname "$0")/verify-source-candidate.sh" "$root"

target=$(mktemp -d "${TMPDIR:-/tmp}/agora-source-target.XXXXXX")
cleanup() {
  rm -rf "$target"
}
trap cleanup EXIT
CARGO_TARGET_DIR="$target" cargo test --manifest-path "$root/agora/Cargo.toml" \
  --workspace --all-targets --locked --offline
CARGO_TARGET_DIR="$target" cargo test --manifest-path "$root/agora/Cargo.toml" \
  --workspace --doc --locked --offline
RUSTDOCFLAGS='-D warnings' CARGO_TARGET_DIR="$target" \
  cargo doc --manifest-path "$root/agora/Cargo.toml" --workspace --no-deps --locked --offline
echo 'Agora self-contained stable source candidate qualified offline'
