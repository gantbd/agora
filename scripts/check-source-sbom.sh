#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
generated=$(mktemp "${TMPDIR:-/tmp}/agora-source-sbom.XXXXXX")
rm -f "$generated"
cleanup() {
  rm -f "$generated"
}
trap cleanup EXIT
node scripts/build-source-sbom.mjs "$generated"
node scripts/validate-source-sbom.mjs docs/agora-source.cdx.json
if ! diff -u docs/agora-source.cdx.json "$generated"; then
  echo 'committed source SBOM drifted from the locked Cargo graph' >&2
  exit 1
fi
node scripts/check-source-sbom-mutants.mjs
echo 'Agora CycloneDX source SBOM closes over the locked local graph'
