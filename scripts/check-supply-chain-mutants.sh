#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

task_dir=$(mktemp -d "${TMPDIR:-/tmp}/agora-supply-mutants.XXXXXX")
cleanup() {
  rm -f "$task_dir/baseline.txt" "$task_dir/output.txt"
  rmdir "$task_dir"
}
trap cleanup EXIT

expect_refusal() {
  local label=$1
  if AGORA_SUPPLY_BASELINE="$task_dir/baseline.txt" \
    ./scripts/check-supply-chain.sh >"$task_dir/output.txt" 2>&1; then
    echo "supply-chain mutant unexpectedly passed: $label" >&2
    cat "$task_dir/output.txt" >&2
    exit 1
  fi
}

awk '!/^component\|keystone-codec\|/' \
  docs/supply-chain-baseline.txt > "$task_dir/baseline.txt"
expect_refusal 'missing component'

sed 's/component|agora-engine|0.3.0/component|agora-engine|0.3.1/' \
  docs/supply-chain-baseline.txt > "$task_dir/baseline.txt"
expect_refusal 'changed component version'

awk '!/^component\|/' docs/supply-chain-baseline.txt > "$task_dir/baseline.txt"
expect_refusal 'empty component inventory'

echo 'Agora supply profile rejects missing, changed, and empty-inventory mutants'
