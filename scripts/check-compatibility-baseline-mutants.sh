#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

task_dir=$(mktemp -d "${TMPDIR:-/tmp}/agora-baseline-mutants.XXXXXX")
cleanup() {
  rm -f "$task_dir/baseline.txt" "$task_dir/output.txt"
  rmdir "$task_dir"
}
trap cleanup EXIT

expect_refusal() {
  local label=$1
  if AGORA_COMPATIBILITY_BASELINE="$task_dir/baseline.txt" \
    ./scripts/check-compatibility-baseline.sh >"$task_dir/output.txt" 2>&1; then
    echo "compatibility mutant unexpectedly passed: $label" >&2
    cat "$task_dir/output.txt" >&2
    exit 1
  fi
}

awk '!/^package\|sdk\/client\// { print }' \
  docs/compatibility-baseline.txt > "$task_dir/baseline.txt"
expect_refusal 'missing package'

sed 's/f9a95e870e010262492833781d204a18a0126d2019ae3752761012fea390ef03/09a95e870e010262492833781d204a18a0126d2019ae3752761012fea390ef03/' \
  docs/compatibility-baseline.txt > "$task_dir/baseline.txt"
expect_refusal 'changed fixture digest'

awk '!/^fixture\|/' docs/compatibility-baseline.txt > "$task_dir/baseline.txt"
expect_refusal 'empty fixture inventory'

echo 'Agora compatibility baseline rejects package, fixture, and empty-inventory mutants'
