#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

validate_only=false
case ${1:-} in
  --validate-profile-only)
    validate_only=true
    shift
    ;;
  '') ;;
  *)
    echo 'usage: qualify-release-candidate.sh [--validate-profile-only]' >&2
    exit 1
    ;;
esac
if (( $# != 0 )); then
  echo 'usage: qualify-release-candidate.sh [--validate-profile-only]' >&2
  exit 1
fi

soak_cycles=${AGORA_RELEASE_SOAK_CYCLES:-10000}
fuzz_runs=${AGORA_RELEASE_FUZZ_RUNS:-1000000}
if [[ ! "$soak_cycles" =~ ^[1-9][0-9]*$ ]] ||
  (( ${#soak_cycles} > 5 )) ||
  (( ${#soak_cycles} == 5 && 10#$soak_cycles > 10000 )); then
  echo 'AGORA_RELEASE_SOAK_CYCLES must be a positive integer no greater than 10000' >&2
  exit 1
fi
if [[ ! "$fuzz_runs" =~ ^[1-9][0-9]*$ ]] ||
  (( ${#fuzz_runs} > 7 )) ||
  (( ${#fuzz_runs} == 7 && 10#$fuzz_runs > 1000000 )); then
  echo 'AGORA_RELEASE_FUZZ_RUNS must be a positive integer no greater than 1000000' >&2
  exit 1
fi
if [[ "$validate_only" == true ]]; then
  echo "Agora release-candidate profile validated only ($soak_cycles soak cycles, $fuzz_runs fuzz runs per target); qualification did not run"
  exit 0
fi
if ! git diff --quiet || ! git diff --cached --quiet; then
  echo 'release-candidate qualification requires a commit-pinned tracked tree' >&2
  exit 1
fi
revision=$(git rev-parse --verify HEAD)

make check
cargo test --workspace --all-targets --release --locked --offline
AGORA_SOAK_CYCLES="$soak_cycles" make soak-release
AGORA_FUZZ_RUNS="$fuzz_runs" make fuzz-smoke

if ! git diff --quiet || ! git diff --cached --quiet; then
  echo 'release-candidate qualification mutated its commit-pinned tracked tree' >&2
  exit 1
fi

echo "Agora release candidate qualified at $revision ($soak_cycles soak cycles, $fuzz_runs fuzz runs per target)"
