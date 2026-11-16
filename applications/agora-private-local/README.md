# Agora private-local application

This package is Agora's standalone composition root. The `agora-private-local`
binary accepts exactly one bounded UTF-8 `key=value` configuration file,
connects through Aegis's public Unix client, provisions one session-local Agora
grant, creates or recovers the durable owner, and serves the private endpoint.
It imports no Workbench or peer-engine policy.

The common fields are:

```text
schema=1
mode=create|open|recover
store=/private/path/store
socket=/private/path/agora.sock
aegis_socket=/private/path/aegis.sock
participant=alice
subject=32-lowercase-hex-digits
resource=32-lowercase-hex-digits
authority_epoch=7
grant_now=10
grant_expires_at=20
grant_profile=conversation|scheduled
host_scope=32-lowercase-hex-digits
```

`create` additionally requires `server_incarnation`, `display_name`,
`participant_kind=human|agent`, `room_id`, `room_name`, and
`admission=open|invite-only`. `open` requires only `server_incarnation`.
`recover` instead requires `previous_incarnation` and `next_incarnation`.
Every incarnation is exactly 32 lowercase hexadecimal digits. Unknown,
duplicate, missing, over-ceiling, non-UTF-8, and noncanonical fields refuse.

The binary reads one byte or EOF from stdin as its shutdown request. It wakes
the blocked accept, drains irreversibly, and prints one bounded JSON cleanup
receipt. Workbench can own that stdin pipe directly. For ordinary standalone
use, `scripts/run-with-signals.sh CONFIG` maps SIGINT and SIGTERM to the same
stdin request without adding a second lifecycle implementation. Set
`AGORA_PRIVATE_LOCAL_DAEMON` only to select the built binary path.

Create is deliberately the first single-principal, one-room production
profile. Richer bootstrap policy belongs in a separately versioned application
profile rather than implicit CLI behavior.
