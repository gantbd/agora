#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
source_root=$PWD
task_dir=$(mktemp -d "${TMPDIR:-/tmp}/agora-release-mutants.XXXXXX")
fixture_root="$task_dir/repository"
cleanup() {
  rm -rf "$task_dir"
}
trap cleanup EXIT

mkdir -p "$fixture_root/docs" "$fixture_root/engine/agora-engine" \
  "$fixture_root/protocol/agora-wire" \
  "$fixture_root/adapters/agora-aegis-verifier" \
  "$fixture_root/client/agora-client" "$fixture_root/host/agora-host" \
  "$fixture_root/protocol/agora-protocol" "$fixture_root/service/agora-service" \
  "$fixture_root/adapters/agora-storage-directory" \
  "$fixture_root/ports/agora-storage-observation" \
  "$fixture_root/sdk/client" "$fixture_root/sdk/embedded"

for file in CHANGELOG.md Cargo.lock LICENSE-APACHE LICENSE-MIT README.md SECURITY.md \
  model.md package-classification.json requirements.md specification.md rust-toolchain.toml; do
  cp "$file" "$fixture_root/$file"
done
for file in assurance.md compatibility.md project-boundaries.md release-policy.md release-qualification.md release-readiness.md \
  source-archive.md source-candidate.md supply-chain.md threat-model.md traceability.md; do
  cp "docs/$file" "$fixture_root/docs/$file"
done
cp docs/agora-source.cdx.json "$fixture_root/docs/agora-source.cdx.json"
for manifest in Cargo.toml engine/agora-engine/Cargo.toml protocol/agora-wire/Cargo.toml adapters/agora-aegis-verifier/Cargo.toml client/agora-client/Cargo.toml \
  host/agora-host/Cargo.toml protocol/agora-protocol/Cargo.toml \
  service/agora-service/Cargo.toml adapters/agora-storage-directory/Cargo.toml \
  ports/agora-storage-observation/Cargo.toml \
  sdk/client/Cargo.toml sdk/embedded/Cargo.toml; do
  cp "$manifest" "$fixture_root/$manifest"
done

run_gate() {
  AGORA_RELEASE_ROOT="$fixture_root" "$source_root/scripts/check-release-contract.sh"
}

if ! run_gate >"$task_dir/output.txt" 2>&1; then
  echo 'valid isolated release-contract fixture was refused' >&2
  cat "$task_dir/output.txt" >&2
  exit 1
fi

expect_refusal() {
  local label=$1
  if run_gate >"$task_dir/output.txt" 2>&1; then
    echo "release-contract mutant unexpectedly passed: $label" >&2
    cat "$task_dir/output.txt" >&2
    exit 1
  fi
}

sed 's/^publish = false$/publish = true/' "$source_root/engine/agora-engine/Cargo.toml" \
  > "$fixture_root/engine/agora-engine/Cargo.toml"
expect_refusal 'engine publication enabled'
cp "$source_root/engine/agora-engine/Cargo.toml" "$fixture_root/engine/agora-engine/Cargo.toml"

sed 's/channel = "1.95.0"/channel = "stable"/' "$source_root/rust-toolchain.toml" \
  > "$fixture_root/rust-toolchain.toml"
expect_refusal 'unpinned toolchain'
cp "$source_root/rust-toolchain.toml" "$fixture_root/rust-toolchain.toml"

rm -f "$fixture_root/SECURITY.md"
expect_refusal 'missing security policy'

echo 'Agora release contract rejects publication, toolchain, and policy mutants'
