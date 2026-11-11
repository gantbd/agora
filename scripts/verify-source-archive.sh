#!/usr/bin/env bash
set -euo pipefail

if (( $# != 1 )) || [[ -z "$1" || "$1" == *[[:space:]]* ]]; then
  echo 'usage: verify-source-archive.sh ARCHIVE.tar' >&2
  exit 1
fi
archive=$1
checksum=$archive.sha256
if [[ ! -f "$archive" || -L "$archive" || ! -f "$checksum" || -L "$checksum" ]]; then
  echo 'source archive or checksum is absent, nonregular, or symbolic' >&2
  exit 1
fi
expected=$(shasum -a 256 "$archive" | awk '{ print $1 }')
if [[ $(cat "$checksum") != "$expected  $(basename "$archive")" ]]; then
  echo 'source archive checksum is malformed or does not match' >&2
  exit 1
fi
node "$(dirname "$0")/validate-source-archive.mjs" "$archive"
