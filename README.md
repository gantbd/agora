# Agora

Agora is a headless Rust conversation engine in which humans and software agents
are equal, explicitly identified participants. It owns participants, rooms,
membership, roles, messages, and participant-neutral addressing. It does not own
authentication, orchestration, tasks, harnesses, storage, transports, UI, model
behavior, or wall-clock access.

The v0.3 core provides bounded deterministic transitions, typed atomic refusal,
room-local message ordering and retention, optimistic revisions, an ordered
bounded outbox, replay-safe response publication, and canonical V2 checkpoints.
Messages may explicitly address a canonical bounded list of current room members.
Participant kind never changes admission, role, speaking, or addressing behavior.

## Boundary

The engine accepts `RevisionedCommandV2` and returns `TransitionV2`. The
convenience `apply` method binds a `CommandV2` to the current revision. It performs
no I/O, reads no clock, starts no participant, and authenticates no principal.

Addressing is conversation data only. Agora never turns an addressed participant
into a task or execution request. An adapter may offer accepted messages to Moira;
Moira decides whether workflow exists, Chronos schedules it, and Daimon executes
already-authorized work.

Commands publish one event and advance one revision. Concurrent hosts use
`apply_revisioned`; the core still requires one serialized publication owner and
makes no consensus claim. Adapters must deliver and acknowledge the outbox rather
than infer delivery from command acceptance.

`PublicationCommandV2` binds one exact plain response to an opaque idempotency
key. Matching replay returns the original transition without another message or
revision. Changed content conflicts. The bounded volatile receipt index becomes
durable only when a host persists it atomically with Agora state.

The `AGORACP` V2 codec validates bounds, canonical order, ownership/references,
message and event frontiers, addressed membership, publication attribution, and
the exact unacknowledged outbox suffix. V1 bytes are refused because their agent
activation state has no neutral interpretation. The codec proves neither origin
authentication nor storage durability.

## Assurance

The active claim is defined by [requirements](requirements.md), the
[specification](specification.md), [traceability](docs/traceability.md),
[assurance plan](docs/assurance.md), [threat model](docs/threat-model.md), and
[release-readiness ledger](docs/release-readiness.md). Open work remains explicit
in the [roadmap](docs/roadmap.md).

## Development

Agora requires Rust 1.95 or newer.

```sh
make check
```
