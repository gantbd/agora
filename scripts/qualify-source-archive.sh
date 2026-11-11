#!/usr/bin/env bash
set -euo pipefail

if (( $# != 1 )); then
  echo 'usage: qualify-source-archive.sh ARCHIVE.tar' >&2
  exit 1
fi
archive=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
script_root=$(cd "$(dirname "$0")" && pwd)
"$script_root/verify-source-archive.sh" "$archive"

work=$(mktemp -d "${TMPDIR:-/tmp}/agora-source-archive.XXXXXX")
cleanup() {
  rm -rf "$work"
}
trap cleanup EXIT
mkdir "$work/extracted"
tar -xf "$archive" -C "$work/extracted"
candidate=$work/extracted/agora-source
"$script_root/verify-source-candidate.sh" "$candidate"
node "$script_root/build-source-archive.mjs" "$candidate" "$work/rebuilt.tar"
cmp "$archive" "$work/rebuilt.tar"
"$script_root/qualify-source-candidate.sh" "$candidate"
echo 'Agora canonical source archive extracted and qualified offline'
