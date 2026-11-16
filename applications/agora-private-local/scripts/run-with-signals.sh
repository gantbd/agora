#!/bin/sh
set -eu

if [ "$#" -ne 1 ]; then
  echo 'usage: run-with-signals.sh CONFIG' >&2
  exit 64
fi

control_directory=$(mktemp -d "${TMPDIR:-/tmp}/agora-private-local.XXXXXX")
control_fifo="$control_directory/shutdown"
cleanup() {
  rm -f "$control_fifo"
  rmdir "$control_directory"
}
trap cleanup EXIT
mkfifo -m 600 "$control_fifo"
exec 3<>"$control_fifo"

daemon=${AGORA_PRIVATE_LOCAL_DAEMON:-agora-private-local}
"$daemon" "$1" <&3 &
child=$!
request_shutdown() {
  printf x >&3
}
trap request_shutdown INT TERM

while :; do
  set +e
  wait "$child"
  status=$?
  set -e
  if kill -0 "$child" 2>/dev/null; then
    continue
  fi
  break
done
trap - INT TERM
exec 3>&-
exit "$status"
