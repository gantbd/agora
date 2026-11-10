#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

baseline=${AGORA_COMPATIBILITY_BASELINE:-docs/compatibility-baseline.txt}
if [[ ! -f "$baseline" ]]; then
  echo 'compatibility baseline is missing' >&2
  exit 1
fi

actual_packages=$(mktemp "${TMPDIR:-/tmp}/agora-packages.XXXXXX")
expected_packages=$(mktemp "${TMPDIR:-/tmp}/agora-expected-packages.XXXXXX")
actual_fixtures=$(mktemp "${TMPDIR:-/tmp}/agora-fixtures.XXXXXX")
expected_fixtures=$(mktemp "${TMPDIR:-/tmp}/agora-expected-fixtures.XXXXXX")
cleanup() {
  rm -f "$actual_packages" "$expected_packages" "$actual_fixtures" "$expected_fixtures"
}
trap cleanup EXIT

awk -F '|' '$1 == "package" { print }' "$baseline" | LC_ALL=C sort > "$expected_packages"
awk -F '|' '$1 == "fixture" { print }' "$baseline" | LC_ALL=C sort > "$expected_fixtures"

if [[ ! -s "$expected_packages" || ! -s "$expected_fixtures" ]]; then
  echo 'compatibility baseline has an empty package or fixture inventory' >&2
  exit 1
fi

while IFS= read -r manifest; do
  package_name=$(awk -F ' *= *' '
    /^\[package\]$/ { in_package = 1; next }
    /^\[/ { in_package = 0 }
    in_package && /^name = / { gsub(/"/, "", $2); print $2; exit }
  ' "$manifest")
  package_version=$(awk -F ' *= *' '
    /^\[package\]$/ { in_package = 1; next }
    /^\[/ { in_package = 0 }
    in_package && /^version = / { gsub(/"/, "", $2); print $2; exit }
  ' "$manifest")
  if [[ -z "$package_name" || -z "$package_version" ]]; then
    echo "unable to identify package in $manifest" >&2
    exit 1
  fi
  printf 'package|%s|%s|%s\n' "$manifest" "$package_name" "$package_version"
done < <(find . -name Cargo.toml -type f \
  -not -path './target/*' \
  -not -path './.git/*' \
  -print | sed 's#^./##' | LC_ALL=C sort) > "$actual_packages"

while IFS= read -r fixture; do
  compact=$(tr -d '[:space:]' < "$fixture")
  if [[ -z "$compact" || "$compact" =~ [^0-9A-Fa-f] || $(( ${#compact} % 2 )) -ne 0 ]]; then
    echo "$fixture is not a nonempty even-length hexadecimal fixture" >&2
    exit 1
  fi
  if command -v shasum >/dev/null 2>&1; then
    digest=$(shasum -a 256 "$fixture" | awk '{ print $1 }')
  elif command -v sha256sum >/dev/null 2>&1; then
    digest=$(sha256sum "$fixture" | awk '{ print $1 }')
  else
    echo 'neither shasum nor sha256sum is available' >&2
    exit 1
  fi
  printf 'fixture|%s|%s|%s\n' "$fixture" "$(( ${#compact} / 2 ))" "$digest"
done < <(find . -path '*/tests/fixtures/*.hex' -type f \
  -not -path './target/*' \
  -not -path './.git/*' \
  -print | sed 's#^./##' | LC_ALL=C sort) > "$actual_fixtures"

if ! diff -u "$expected_packages" "$actual_packages"; then
  echo 'Cargo package inventory drifted; review and update the compatibility baseline' >&2
  exit 1
fi

if ! diff -u "$expected_fixtures" "$actual_fixtures"; then
  echo 'canonical fixture inventory or bytes drifted; review and update the compatibility baseline' >&2
  exit 1
fi

cargo metadata --no-deps --format-version 1 --locked --offline >/dev/null

echo 'Agora compatibility baseline closes over 10 packages and 13 canonical fixtures'
