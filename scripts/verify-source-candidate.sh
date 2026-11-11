#!/usr/bin/env bash
set -euo pipefail

if (( $# != 1 )); then
  echo 'usage: verify-source-candidate.sh CANDIDATE_DIRECTORY' >&2
  exit 1
fi
root=$1
if [[ ! -d "$root" || -L "$root" ]]; then
  echo 'source candidate is absent or symbolic' >&2
  exit 1
fi
for required in SOURCE-MANIFEST.txt SHA256SUMS agora/Cargo.toml agora/Cargo.lock \
  aegis/crates/aegis-client/Cargo.toml aegis/crates/aegis-protocol/Cargo.toml \
  keystone/Cargo.toml keystone/crates/keystone-codec/Cargo.toml \
  keystone/crates/keystone-storage/Cargo.toml; do
  if [[ ! -f "$root/$required" || -L "$root/$required" ]]; then
    echo "required candidate file is absent or symbolic: $required" >&2
    exit 1
  fi
done
if find "$root" -type l -print -quit | grep -q .; then
  echo 'source candidate contains a symbolic link' >&2
  exit 1
fi
if find "$root" ! -type d ! -type f -print -quit | grep -q .; then
  echo 'source candidate contains a special file' >&2
  exit 1
fi
if find "$root" -type f -print | LC_ALL=C grep -q '[[:space:]]'; then
  echo 'source candidate contains an unsupported whitespace path' >&2
  exit 1
fi
if ! rg -q '^schema=agora-source-candidate-v1$' "$root/SOURCE-MANIFEST.txt" \
  || ! rg -q '^agora_revision=[0-9a-f]{40}$' "$root/SOURCE-MANIFEST.txt" \
  || ! rg -q '^aegis_context_revision=[0-9a-f]{40}$' "$root/SOURCE-MANIFEST.txt" \
  || ! rg -q '^keystone_context_revision=[0-9a-f]{40}$' "$root/SOURCE-MANIFEST.txt" \
  || ! rg -q '^stable_component_count=19$' "$root/SOURCE-MANIFEST.txt" \
  || ! rg -q '^provenance=exact-local-content-closure$' "$root/SOURCE-MANIFEST.txt" \
  || ! rg -q '^claim=self-contained-stable-local-source$' "$root/SOURCE-MANIFEST.txt" \
  || ! rg -q '^publication=denied$' "$root/SOURCE-MANIFEST.txt"; then
  echo 'source candidate manifest is malformed or overclaims provenance' >&2
  exit 1
fi

actual=$(mktemp "${TMPDIR:-/tmp}/agora-source-files.XXXXXX")
declared=$(mktemp "${TMPDIR:-/tmp}/agora-source-declared.XXXXXX")
cleanup() {
  rm -f "$actual" "$declared"
}
trap cleanup EXIT
(
  cd "$root"
  find . -type f ! -name SHA256SUMS -print | LC_ALL=C sort >"$actual"
  awk '{ print $2 }' SHA256SUMS | LC_ALL=C sort >"$declared"
  if ! diff -u "$actual" "$declared"; then
    echo 'source candidate checksum inventory is incomplete or stale' >&2
    exit 1
  fi
  shasum -a 256 -c SHA256SUMS >/dev/null
)
echo 'Agora source candidate checksum closure verified'
