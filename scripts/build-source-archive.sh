#!/usr/bin/env bash
set -euo pipefail

if (( $# != 2 )) || [[ -z "$1" || -z "$2" || "$2" == / || "$2" == *[[:space:]]* ]]; then
  echo 'usage: build-source-archive.sh CANDIDATE_DIRECTORY OUTPUT.tar' >&2
  exit 1
fi
candidate=$1
output=$2
checksum=$output.sha256
if [[ -e "$output" || -e "$checksum" ]]; then
  echo 'source archive or checksum output already exists' >&2
  exit 1
fi
mkdir -p "$(dirname "$output")"
work=$(mktemp "$(dirname "$output")/.agora-source-archive.XXXXXX")
rm -f "$work"
cleanup() {
  rm -f "$work" "$work.sha256"
}
trap cleanup EXIT
node "$(dirname "$0")/build-source-archive.mjs" "$candidate" "$work"
digest=$(shasum -a 256 "$work" | awk '{ print $1 }')
printf '%s  %s\n' "$digest" "$(basename "$output")" >"$work.sha256"
mv "$work.sha256" "$checksum"
mv "$work" "$output"
trap - EXIT
echo "Agora source archive and SHA-256 built at $output"
