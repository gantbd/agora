# Agora

Agora is a Rust chat-room engine in which humans and software agents are equal,
explicitly identified participants. It is a library, not a server: authentication,
agent execution, storage, transports, user interfaces, and wall-clock access live
outside the engine.

The v0.2 core provides:

- bounded registration for human and agent participants;
- bounded room creation with an owner and open or invite-only admission;
- explicit invitation and membership transitions;
- owner, moderator, and member roles;
- deterministic, caller-timestamped messages with per-room FIFO retention;
- a closed, versioned command vocabulary and typed outcomes/refusals;
- atomic refusal: invalid commands leave authoritative state unchanged;
- content-redacted `Debug` output for messages;
- optimistic engine revisions that reject stale writers before mutation;
- a bounded, ordered domain-event outbox with monotone prefix acknowledgment;
- a bounded replay-safe response-publication profile with opaque idempotency
  keys and exact-content conflict detection;
- bounded canonical V1 semantic checkpoints preserving retention, replay receipts,
  pending events and acknowledgement cursors, with host recovery ceilings;
- room-scoped agent activation policies with bounded fan-out;
- automatic activation by human messages without agent-to-agent wakeup loops.

## Boundary

The engine accepts `RevisionedCommandV1` values and returns `TransitionV1` values.
The convenience `apply` method binds a `CommandV1` to the current revision. It performs
no I/O, reads no clock, starts no agent, and authenticates no principal. A host is
responsible for mapping an authenticated connection to a `ParticipantId`; passing
an ID to this library is not proof of identity.

Commands publish exactly one event and advance exactly one revision. Concurrent
hosts use `apply_revisioned` for optimistic exclusion; the core still requires a
single serialized publication owner and makes no distributed-consensus claim.
Adapters must deliver and acknowledge the outbox rather than infer delivery from
accepted commands.

Cross-engine response adapters use `PublicationCommandV1`. First acceptance runs
the ordinary closed `Speak` transition and retains its exact transition under an
opaque fixed-width key. Matching replay returns that transition without adding a
message or advancing revision, even when the original expected revision is now
stale. Reusing the key with changed semantic content is refused. This receipt
index is bounded and volatile; durable lost-reply recovery still requires a host
to persist it atomically with Agora state.

`Engine::prepare_checkpoint` and `Engine::recover_checkpoint` now provide that
authority-free data boundary. Recovery validates configured bounds, canonical
ordering, room ownership/references, message/revision/event frontiers, retained
publication attribution and the exact unacknowledged outbox suffix. Event and
receipt validation use indexed lookups rather than repeated history scans.
This native codec adds no runtime dependencies, I/O, clocks or authentication;
it does not authenticate checkpoint origin or itself establish durability.
The separate `../moira/adapters/moira-sqlite` receiver persists this checkpoint and
stable source/room correspondence in a single-owner local SQLite profile. Its
storage, failure-boundary and restart claims belong to that adapter.

Agent activation is a request for an external connector, never proof that an agent
ran. Rooms default to disabled activation. Owners may select explicit human
requests or automatic activation of agent members on human messages. Agent-authored
messages never automatically activate other agents.

## Rigor

The project follows the useful discipline established by the neighboring Chronos
engine: closed data, explicit bounds, deterministic transitions, effect isolation,
typed refusal, scenario tests, and honest traceability. See
[requirements](requirements.md), [specification](specification.md), and the
[traceability ledger](docs/traceability.md).

## Development

Agora requires Rust 1.95 or newer.

```sh
make check
```

