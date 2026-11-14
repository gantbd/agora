#!/usr/bin/env bash
set -euo pipefail

release_root=${AGORA_RELEASE_ROOT:-"$(dirname "$0")/.."}
cd "$release_root"

required_files=(
  CHANGELOG.md
  Cargo.lock
  LICENSE-APACHE
  LICENSE-MIT
  README.md
  SECURITY.md
  model.md
  package-classification.json
  requirements.md
  specification.md
  rust-toolchain.toml
  docs/assurance.md
  docs/agora-source.cdx.json
  docs/compatibility.md
  docs/project-boundaries.md
  docs/release-policy.md
  docs/release-qualification.md
  docs/release-readiness.md
  docs/source-candidate.md
  docs/source-archive.md
  docs/supply-chain.md
  docs/threat-model.md
  docs/traceability.md
)

for required in "${required_files[@]}"; do
  if [[ ! -s "$required" ]]; then
    echo "required source contract file is absent or empty: $required" >&2
    exit 1
  fi
done

if ! rg -q '^license = "MIT OR Apache-2\.0"$' engine/agora-engine/Cargo.toml \
  || ! rg -q 'Apache License' LICENSE-APACHE \
  || ! rg -q 'Permission is hereby granted' LICENSE-MIT; then
  echo 'declared dual-license contract and repository license texts disagree' >&2
  exit 1
fi

if ! rg -q '^channel = "1\.95\.0"$' rust-toolchain.toml \
  || ! rg -q '^components = \["clippy", "rustfmt"\]$' rust-toolchain.toml; then
  echo 'Rust toolchain is not exactly pinned with the required qualification components' >&2
  exit 1
fi

while IFS= read -r manifest; do
  if ! rg -q '^\[package\]$' "$manifest"; then
    continue
  fi
  if ! awk '
    /^\[package\]$/ { in_package = 1; next }
    /^\[/ { in_package = 0 }
    in_package && /^publish = false$/ { found = 1 }
    END { exit(found ? 0 : 1) }
  ' "$manifest"; then
    echo "$manifest permits registry publication; every package must set publish = false" >&2
    exit 1
  fi
  if rg -q '^publish = (true|\[)' "$manifest"; then
    echo "$manifest names an allowed publication target" >&2
    exit 1
  fi
done < <(find . \( -type d \( -name target -o -name .git \) \) -prune \
  -o -name Cargo.toml -type f -print | LC_ALL=C sort)

if ! rg -q 'not ready for a network service' docs/release-readiness.md \
  || ! rg -q 'or crates\.io release' docs/release-readiness.md; then
  echo 'release readiness no longer carries the explicit distribution refusal' >&2
  exit 1
fi

if ! rg -q 'path = "\.\./\.\./\.\./aegis/' adapters/agora-aegis-verifier/Cargo.toml \
  || ! rg -q 'path = "\.\./\.\./\.\./keystone/' protocol/agora-wire/Cargo.toml \
  || ! rg -q 'path = "\.\./\.\./\.\./keystone/' \
    adapters/agora-storage-directory/Cargo.toml \
  || ! rg -q 'path = "\.\./\.\./\.\./keystone/' \
    ports/agora-storage-observation/Cargo.toml; then
  echo 'Aegis or Keystone source-layout assumption changed without release-contract review' >&2
  exit 1
fi

echo 'Agora release contract denies publication and pins the qualified source profile'
