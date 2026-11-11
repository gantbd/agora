#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

bash -n scripts/qualify-release-candidate.sh
bash -n scripts/run-fuzz-smoke.sh

AGORA_RELEASE_SOAK_CYCLES=10000 AGORA_RELEASE_FUZZ_RUNS=1000000 \
  ./scripts/qualify-release-candidate.sh --validate-profile-only >/dev/null

for assignment in \
  'AGORA_RELEASE_SOAK_CYCLES=0' \
  'AGORA_RELEASE_SOAK_CYCLES=10001' \
  'AGORA_RELEASE_SOAK_CYCLES=99999999999999999999999999999999999999' \
  'AGORA_RELEASE_SOAK_CYCLES=invalid' \
  'AGORA_RELEASE_FUZZ_RUNS=0' \
  'AGORA_RELEASE_FUZZ_RUNS=1000001' \
  'AGORA_RELEASE_FUZZ_RUNS=99999999999999999999999999999999999999' \
  'AGORA_RELEASE_FUZZ_RUNS=invalid'; do
  if env "$assignment" \
    ./scripts/qualify-release-candidate.sh --validate-profile-only >/dev/null 2>&1; then
    echo "release-candidate qualifier accepted invalid profile: $assignment" >&2
    exit 1
  fi
done

if ./scripts/qualify-release-candidate.sh --unknown >/dev/null 2>&1; then
  echo 'release-candidate qualifier accepted an unknown argument' >&2
  exit 1
fi

echo 'Agora release-candidate profile rejects malformed and over-ceiling budgets'
