#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

wire_seed_dir=fuzz/seeds/wire_canonical
checkpoint_seed_dir=fuzz/seeds/checkpoint_canonical
outer_seed_dir=fuzz/seeds/outer_frame_canonical
mkdir -p "$wire_seed_dir" "$checkpoint_seed_dir" "$outer_seed_dir"

for fixture in crates/agora-wire/tests/fixtures/*.hex; do
  name=$(basename "$fixture" .hex)
  rm -f "$wire_seed_dir/$name.bin" "$outer_seed_dir/$name.bin"
  xxd -r -p "$fixture" "$wire_seed_dir/$name.bin"
  if [[ "$name" == local-client-hello-v1 \
    || "$name" == local-server-hello-v1 \
    || "$name" == local-collaboration-client-hello-v1 \
    || "$name" == local-collaboration-server-hello-v1 \
    || "$name" == local-session-hello-v1 ]]; then
    continue
  fi
  bytes=$(wc -c <"$wire_seed_dir/$name.bin" | tr -d ' ')
  printf '%08x' "$bytes" | xxd -r -p >"$outer_seed_dir/$name.bin"
  xxd -r -p "$fixture" >>"$outer_seed_dir/$name.bin"
done

rm -f "$checkpoint_seed_dir/engine-v16.bin" \
  "$checkpoint_seed_dir/service-v18.bin" \
  "$checkpoint_seed_dir/host-v18.bin"
xxd -r -p tests/fixtures/checkpoint-v16.hex \
  "$checkpoint_seed_dir/engine-v16.bin"
xxd -r -p crates/agora-service/tests/fixtures/service-checkpoint-v18.hex \
  "$checkpoint_seed_dir/service-v18.bin"
xxd -r -p crates/agora-host/tests/fixtures/host-checkpoint-v18.hex \
  "$checkpoint_seed_dir/host-v18.bin"

echo 'Refreshed Agora wire, outer-frame, and checkpoint fuzz seeds from canonical fixtures'
