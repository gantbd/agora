#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

work=$(mktemp -d "${TMPDIR:-/tmp}/agora-source-mutants.XXXXXX")
cleanup() {
  rm -rf "$work"
}
trap cleanup EXIT

fixture=$work/valid
mkdir -p \
  "$fixture/agora" \
  "$fixture/aegis/crates/aegis-client" \
  "$fixture/aegis/crates/aegis-protocol" \
  "$fixture/keystone/crates/keystone-codec" \
  "$fixture/keystone/crates/keystone-storage"
printf '[workspace]\nmembers = []\n' >"$fixture/agora/Cargo.toml"
printf 'version = 4\n' >"$fixture/agora/Cargo.lock"
printf '[package]\nname = "aegis-client"\n' >"$fixture/aegis/crates/aegis-client/Cargo.toml"
printf '[package]\nname = "aegis-protocol"\n' >"$fixture/aegis/crates/aegis-protocol/Cargo.toml"
printf '[workspace]\nmembers = []\n' >"$fixture/keystone/Cargo.toml"
printf '[package]\nname = "keystone-codec"\n' >"$fixture/keystone/crates/keystone-codec/Cargo.toml"
printf '[package]\nname = "keystone-storage"\n' >"$fixture/keystone/crates/keystone-storage/Cargo.toml"
{
  echo 'schema=agora-source-candidate-v1'
  echo 'agora_revision=1111111111111111111111111111111111111111'
  echo 'aegis_context_revision=2222222222222222222222222222222222222222'
  echo 'aegis_selected_tree_clean=false'
  echo 'keystone_context_revision=3333333333333333333333333333333333333333'
  echo 'keystone_selected_tree_clean=false'
  echo 'stable_component_count=19'
  echo 'provenance=exact-local-content-closure'
  echo 'claim=self-contained-stable-local-source'
  echo 'publication=denied'
} >"$fixture/SOURCE-MANIFEST.txt"

checksums() {
  local root=$1
  (
    cd "$root"
    find . -type f ! -name SHA256SUMS -print | LC_ALL=C sort | while IFS= read -r file; do
      hash=$(shasum -a 256 "$file" | awk '{ print $1 }')
      printf '%s  %s\n' "$hash" "$file"
    done >SHA256SUMS
  )
}
checksums "$fixture"
./scripts/verify-source-candidate.sh "$fixture" >/dev/null

mutant=$work/changed
cp -R "$fixture" "$mutant"
printf '\nchanged\n' >>"$mutant/agora/Cargo.toml"
if ./scripts/verify-source-candidate.sh "$mutant" >/dev/null 2>&1; then
  echo 'source-candidate verifier accepted changed content' >&2
  exit 1
fi

mutant=$work/extra
cp -R "$fixture" "$mutant"
printf 'extra\n' >"$mutant/extra.txt"
if ./scripts/verify-source-candidate.sh "$mutant" >/dev/null 2>&1; then
  echo 'source-candidate verifier accepted an undeclared file' >&2
  exit 1
fi

mutant=$work/symbolic
cp -R "$fixture" "$mutant"
ln -s Cargo.toml "$mutant/agora/alias.toml"
if ./scripts/verify-source-candidate.sh "$mutant" >/dev/null 2>&1; then
  echo 'source-candidate verifier accepted a symbolic link' >&2
  exit 1
fi

mutant=$work/overclaim
cp -R "$fixture" "$mutant"
sed -i.bak 's/provenance=exact-local-content-closure/provenance=commit-provenance/' \
  "$mutant/SOURCE-MANIFEST.txt"
rm "$mutant/SOURCE-MANIFEST.txt.bak"
checksums "$mutant"
if ./scripts/verify-source-candidate.sh "$mutant" >/dev/null 2>&1; then
  echo 'source-candidate verifier accepted a provenance overclaim' >&2
  exit 1
fi

echo 'Agora source-candidate verifier rejects content, inventory, link, and claim mutants'
