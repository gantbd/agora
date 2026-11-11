#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

toolchain=nightly-2026-11-01
expected_fuzz='cargo-fuzz 0.13.2'
runs=${AGORA_FUZZ_RUNS:-20000}
if [[ ! "$runs" =~ ^[1-9][0-9]*$ ]] ||
  (( ${#runs} > 7 )) ||
  (( ${#runs} == 7 && 10#$runs > 1000000 )); then
  echo 'AGORA_FUZZ_RUNS must be a positive integer no greater than 1000000' >&2
  exit 1
fi
actual_fuzz=$(cargo fuzz --version 2>/dev/null || true)
if [[ "$actual_fuzz" != "$expected_fuzz" ]]; then
  echo "cargo-fuzz 0.13.2 is required; found: ${actual_fuzz:-not installed}" >&2
  exit 1
fi
if ! rustup run "$toolchain" rustc --version >/dev/null 2>&1; then
  echo "$toolchain is required" >&2
  exit 1
fi

campaign=$(mktemp -d "${TMPDIR:-/tmp}/agora-fuzz.XXXXXX")
cleanup() {
  rm -rf "$campaign"
}
trap cleanup EXIT

for target in wire_canonical checkpoint_canonical outer_frame_canonical; do
  corpus="$campaign/$target"
  artifacts="$campaign/artifacts-$target/"
  mkdir -p "$corpus" "$artifacts"
  cp fuzz/seeds/"$target"/*.bin "$corpus"/
  cargo +"$toolchain" fuzz run "$target" "$corpus" -- \
    -runs="$runs" \
    -seed=424242 \
    -max_len=65536 \
    -timeout=10 \
    -rss_limit_mb=2048 \
    -verbosity=0 \
    -artifact_prefix="$artifacts"
done

echo "Agora coverage-guided fuzz campaign completed all three targets at $runs runs each"
