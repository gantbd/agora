#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

if (( $# != 1 )) || [[ -z "$1" || "$1" == / ]]; then
  echo 'usage: build-source-candidate.sh OUTPUT_DIRECTORY' >&2
  exit 1
fi
output=$1
if [[ -e "$output" ]]; then
  echo 'source-candidate output already exists' >&2
  exit 1
fi
if ! git diff --quiet || ! git diff --cached --quiet; then
  echo 'source-candidate construction requires a commit-pinned Agora tracked tree' >&2
  exit 1
fi

agora_revision=$(git rev-parse --verify HEAD)
aegis_root=../aegis
keystone_root=../keystone
for required in \
  "$aegis_root/crates/aegis-client/Cargo.toml" \
  "$aegis_root/crates/aegis-client/src/lib.rs" \
  "$aegis_root/crates/aegis-protocol/Cargo.toml" \
  "$aegis_root/crates/aegis-protocol/src/lib.rs" \
  "$aegis_root/LICENSE-APACHE" \
  "$aegis_root/LICENSE-MIT" \
  "$keystone_root/crates/keystone-codec/Cargo.toml" \
  "$keystone_root/crates/keystone-codec/src/lib.rs" \
  "$keystone_root/crates/keystone-storage/Cargo.toml" \
  "$keystone_root/crates/keystone-storage/src/lib.rs" \
  "$keystone_root/LICENSE-APACHE" \
  "$keystone_root/LICENSE-MIT"; do
  if [[ ! -f "$required" || -L "$required" ]]; then
    echo "required source input is absent or symbolic: $required" >&2
    exit 1
  fi
done

aegis_revision=$(git -C "$aegis_root" rev-parse --verify HEAD)
keystone_revision=$(git -C "$keystone_root" rev-parse --verify HEAD)
aegis_selected_clean=true
if [[ -n $(git -C "$aegis_root" status --porcelain --untracked-files=all -- \
  crates/aegis-client crates/aegis-protocol LICENSE-APACHE LICENSE-MIT) ]]; then
  aegis_selected_clean=false
fi
keystone_selected_clean=true
if [[ -n $(git -C "$keystone_root" status --porcelain --untracked-files=all -- \
  Cargo.toml crates/keystone-codec crates/keystone-storage LICENSE-APACHE LICENSE-MIT) ]]; then
  keystone_selected_clean=false
fi

work=$(mktemp -d "${TMPDIR:-/tmp}/agora-source-candidate.XXXXXX")
cleanup() {
  rm -rf "$work"
}
trap cleanup EXIT
root=$work/agora-source-candidate
mkdir -p "$root/agora" "$root/aegis/crates" "$root/keystone/crates"

while IFS= read -r file; do
  if [[ "$file" == *[[:space:]]* ]]; then
    echo "tracked Agora path contains unsupported whitespace: $file" >&2
    exit 1
  fi
  if [[ -L "$file" || ! -f "$file" ]]; then
    echo "tracked Agora input is absent, nonregular, or symbolic: $file" >&2
    exit 1
  fi
  mkdir -p "$root/agora/$(dirname "$file")"
  cp "$file" "$root/agora/$file"
done < <(git ls-files)

for package in aegis-client aegis-protocol; do
  mkdir -p "$root/aegis/crates/$package/src"
  cp "$aegis_root/crates/$package/Cargo.toml" "$root/aegis/crates/$package/Cargo.toml"
  cp "$aegis_root/crates/$package/src/lib.rs" "$root/aegis/crates/$package/src/lib.rs"
done
cp "$aegis_root/LICENSE-APACHE" "$root/aegis/LICENSE-APACHE"
cp "$aegis_root/LICENSE-MIT" "$root/aegis/LICENSE-MIT"

cp release/keystone-workspace.Cargo.toml "$root/keystone/Cargo.toml"
for package in keystone-codec keystone-storage; do
  mkdir -p "$root/keystone/crates/$package/src"
  cp "$keystone_root/crates/$package/Cargo.toml" "$root/keystone/crates/$package/Cargo.toml"
  cp "$keystone_root/crates/$package/src/lib.rs" "$root/keystone/crates/$package/src/lib.rs"
done
cp "$keystone_root/LICENSE-APACHE" "$root/keystone/LICENSE-APACHE"
cp "$keystone_root/LICENSE-MIT" "$root/keystone/LICENSE-MIT"

{
  echo 'schema=agora-source-candidate-v1'
  echo "agora_revision=$agora_revision"
  echo "aegis_context_revision=$aegis_revision"
  echo "aegis_selected_tree_clean=$aegis_selected_clean"
  echo "keystone_context_revision=$keystone_revision"
  echo "keystone_selected_tree_clean=$keystone_selected_clean"
  echo 'stable_component_count=20'
  echo 'provenance=exact-local-content-closure'
  echo 'claim=self-contained-stable-local-source'
  echo 'publication=denied'
} >"$root/SOURCE-MANIFEST.txt"

(
  cd "$root"
  find . -type l -print | LC_ALL=C sort >"$work/symlinks"
  if [[ -s "$work/symlinks" ]]; then
    echo 'source candidate unexpectedly contains symbolic links' >&2
    exit 1
  fi
  if find . ! -type d ! -type f -print -quit | grep -q .; then
    echo 'source candidate unexpectedly contains a special file' >&2
    exit 1
  fi
  find . -type f ! -name SHA256SUMS -print | LC_ALL=C sort | while IFS= read -r file; do
    hash=$(shasum -a 256 "$file" | awk '{ print $1 }')
    printf '%s  %s\n' "$hash" "$file"
  done >SHA256SUMS
)

mkdir -p "$(dirname "$output")"
mv "$root" "$output"
trap - EXIT
rm -rf "$work"
echo "Agora source candidate built at $output for $agora_revision"
