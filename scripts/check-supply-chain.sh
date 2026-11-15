#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

baseline=${AGORA_SUPPLY_BASELINE:-docs/supply-chain-baseline.txt}
if [[ ! -f "$baseline" ]]; then
  echo 'supply-chain baseline is missing' >&2
  exit 1
fi

actual=$(mktemp "${TMPDIR:-/tmp}/agora-supply.XXXXXX")
expected=$(mktemp "${TMPDIR:-/tmp}/agora-expected-supply.XXXXXX")
cleanup() {
  rm -f "$actual" "$expected"
}
trap cleanup EXIT

awk -F '|' '$1 == "component" { print }' "$baseline" | LC_ALL=C sort > "$expected"
if [[ ! -s "$expected" ]]; then
  echo 'supply-chain baseline has an empty component inventory' >&2
  exit 1
fi

awk '
  /^\[\[package\]\]$/ {
    if (name != "") print "component|" name "|" version
    name = ""
    version = ""
    next
  }
  /^name = / { value = $0; sub(/^name = "/, "", value); sub(/"$/, "", value); name = value }
  /^version = / { value = $0; sub(/^version = "/, "", value); sub(/"$/, "", value); version = value }
  END { if (name != "") print "component|" name "|" version }
' Cargo.lock | LC_ALL=C sort > "$actual"

if ! diff -u "$expected" "$actual"; then
  echo 'locked component inventory drifted; review the supply-chain baseline' >&2
  exit 1
fi

if rg -q '^(source|checksum) = ' Cargo.lock; then
  echo 'registry or Git source entered the local-only locked profile' >&2
  exit 1
fi

while IFS= read -r declaration; do
  if [[ "$declaration" != *'path = '* || "$declaration" != *'version = "='* ]]; then
    echo "dependency is not bound to an exact version and local path: $declaration" >&2
    exit 1
  fi
  if [[ "$declaration" == *'git = '* || "$declaration" == *'registry = '* ]]; then
    echo "dependency selects an unqualified external source: $declaration" >&2
    exit 1
  fi
done < <(rg --no-filename '^[A-Za-z0-9_-]+ = \{.*\}$' \
  engine/*/Cargo.toml protocol/*/Cargo.toml client/*/Cargo.toml \
  service/*/Cargo.toml host/*/Cargo.toml ports/*/Cargo.toml \
  adapters/*/Cargo.toml applications/*/Cargo.toml sdk/*/Cargo.toml \
  | LC_ALL=C sort)

echo 'Agora supply profile closes over 23 locked local components'
