#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

if ! rg -q '^channel = "nightly-2026-11-01"$' fuzz/rust-toolchain.toml \
  || ! rg -q '^libfuzzer-sys = "=0\.4\.13"$' fuzz/Cargo.toml \
  || ! rg -q "^expected_fuzz='cargo-fuzz 0\.13\.2'$" scripts/run-fuzz-smoke.sh; then
  echo 'fuzz toolchain or libFuzzer dependency is not exactly pinned' >&2
  exit 1
fi

for target in wire_canonical checkpoint_canonical outer_frame_canonical; do
  if ! rg -q "^name = \"$target\"$" fuzz/Cargo.toml \
    || ! rg -q "\\b$target\\b" scripts/run-fuzz-smoke.sh; then
    echo "fuzz target is not declared and campaign-bound: $target" >&2
    exit 1
  fi
done

expected=$(mktemp -d "${TMPDIR:-/tmp}/agora-fuzz-seeds.XXXXXX")
cleanup() {
  rm -rf "$expected"
}
trap cleanup EXIT

mkdir -p "$expected/wire" "$expected/outer" "$expected/checkpoint"
for fixture in crates/agora-wire/tests/fixtures/*.hex; do
  name=$(basename "$fixture" .hex)
  xxd -r -p "$fixture" "$expected/wire/$name.bin"
  if [[ "$name" == local-client-hello-v1 \
    || "$name" == local-server-hello-v1 \
    || "$name" == local-collaboration-client-hello-v1 \
    || "$name" == local-collaboration-server-hello-v1 \
    || "$name" == local-session-hello-v1 ]]; then
    continue
  fi
  bytes=$(wc -c <"$expected/wire/$name.bin" | tr -d ' ')
  printf '%08x' "$bytes" | xxd -r -p >"$expected/outer/$name.bin"
  xxd -r -p "$fixture" >>"$expected/outer/$name.bin"
done
xxd -r -p tests/fixtures/checkpoint-v14.hex "$expected/checkpoint/engine-v14.bin"
xxd -r -p crates/agora-service/tests/fixtures/service-checkpoint-v16.hex \
  "$expected/checkpoint/service-v16.bin"
xxd -r -p crates/agora-host/tests/fixtures/host-checkpoint-v16.hex \
  "$expected/checkpoint/host-v16.bin"

if ! diff -qr "$expected/wire" fuzz/seeds/wire_canonical \
  || ! diff -qr "$expected/outer" fuzz/seeds/outer_frame_canonical \
  || ! diff -qr "$expected/checkpoint" fuzz/seeds/checkpoint_canonical; then
  echo 'committed fuzz seeds drifted from canonical fixtures' >&2
  exit 1
fi

cargo metadata --no-deps --format-version 1 \
  --manifest-path fuzz/Cargo.toml --locked --offline >/dev/null

echo 'Agora fuzz contract pins its toolchain and exact canonical seed corpus'
