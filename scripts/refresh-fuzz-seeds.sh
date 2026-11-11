#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

wire_seed_dir=fuzz/seeds/wire_canonical
checkpoint_seed_dir=fuzz/seeds/checkpoint_canonical
mkdir -p "$wire_seed_dir" "$checkpoint_seed_dir"

for fixture in crates/agora-wire/tests/fixtures/*.hex; do
  name=$(basename "$fixture" .hex)
  xxd -r -p "$fixture" "$wire_seed_dir/$name.bin"
done

xxd -r -p tests/fixtures/checkpoint-v14.hex \
  "$checkpoint_seed_dir/engine-v14.bin"
xxd -r -p crates/agora-service/tests/fixtures/service-checkpoint-v16.hex \
  "$checkpoint_seed_dir/service-v16.bin"
xxd -r -p crates/agora-host/tests/fixtures/host-checkpoint-v16.hex \
  "$checkpoint_seed_dir/host-v16.bin"

echo 'Refreshed Agora fuzz seeds from canonical fixtures'
