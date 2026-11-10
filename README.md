# Agora

Agora is a headless Rust conversation engine in which humans and software agents
are equal, explicitly identified participants. It owns participants, rooms,
membership, roles, messages, and participant-neutral addressing. It does not own
authentication, orchestration, tasks, harnesses, storage, transports, UI, model
behavior, or wall-clock access.

The v0.3 core provides bounded deterministic transitions, typed atomic refusal,
room-local message ordering and retention, optimistic revisions, an ordered
bounded outbox, replay-safe response publication, and canonical profile-4 checkpoints.
Messages may explicitly address a canonical bounded list of current room members.
Participant kind never changes admission, role, speaking, or addressing behavior.

The core also provides membership-filtered, bounded room, member, and retained-
message pages. Their opaque cursors are bound to the exact observer, room where
applicable, and engine revision; message pages expose the retained sequence
interval rather than claiming complete history. A snapshot-aligned bounded room
change feed continues from a stable event identity and returns explicit retention
or leave/rejoin gaps requiring resnapshot. Protocol V2 projects these values;
Protocol V1 remains frozen.

An additive Protocol V2 now has a separately typed serialized service, client,
and canonical version-2 complete-frame codec. It closes participant-bound room creation, invitation, joining,
leaving, role change, kick/ban/unban moderation, speaking, and bounded page projection with client-scoped
opaque cursors and exact volatile request replay. It deliberately has no V2
credential adapter or network transport yet. Its separately typed authenticated
host applies current-context revalidation and exact session/client binding. V1
remains unchanged.

Room owners can also change Open/InviteOnly admission as an ordered room fact.
The event records prior and new policy without rewriting earlier invitations or
admissions; current members observe it through the bounded room-change feed.

An independently versioned Service V2 checkpoint preserves the engine, pending
outbox, client namespace, exact request replies, and live cursor consequences.
`agora-durable` gates changed service state on a durable compare-and-exchange
receipt, restores known refusals, and fences indeterminate outcomes.
`agora-storage-directory` supplies one separately qualified physical profile:
an exclusive private local Unix directory with a synchronized atomic checkpoint
replace. Its claim does not extend to hardware power-loss behavior, network
filesystems, backup/restore, encryption, or multi-host availability.

The V2 client separately fences an outcome-unknown transmission: only the exact
same command may reuse the retained request identity until a reply resolves it
or the caller explicitly abandons and accounts for the unresolved request.

For authenticated deployments, the host and service also have one combined
checkpoint and pre-reply gate. Session subject/participant/client/time bindings
recover with the exact service receipt and must pass current verifier
revalidation before reuse.

The repository also contains an initial transport-neutral operational profile:

- `agora-protocol` owns closed request, reply, snapshot, and refusal values;
- `agora-wire` owns their canonical bounded V1 binary representation;
- `agora-service` serially owns one engine, binds each connected client to one
  registered participant, and provides bounded replay-safe requests;
- `agora-durable` owns the store-neutral pre-reply durability and uncertainty
  fence for complete Service V2 checkpoints;
- `agora-storage-directory` owns the one qualified local Unix checkpoint-store
  mechanism and no semantic state;
- `agora-host` binds exact wire frames and server-owned clients to a current
  externally verified principal/transport context;
- `agora-client` owns a monotone request namespace over an abstract service port.

These crates add no authentication, network transport, durable execution, or
agent behavior. See the [service architecture](docs/service-architecture.md).

`agora-host` performs authentication composition but does not authenticate any
credential itself. A trusted injected verifier may consume Aegis decision
evidence or another credential system. See the
[host architecture](docs/host-architecture.md).

## SDK facades

`agora-sdk-embedded` is the trusted-host, zero-authority distribution facade for
applications that embed the Agora engine, serialized service, and authenticated
host. `agora-sdk-client` is the remote-client distribution facade for tools that
speak Agora's client, protocol, and wire contracts. Neither facade chooses a
transport, credential verifier, persistence adapter, clock, or runtime policy.
See the [SDK taxonomy](docs/sdk-taxonomy.md) for the dependency closures and
explicit non-claims.

The wire grammar and tag assignments are frozen in the
[wire protocol](docs/wire-protocol.md) and executable golden fixtures. A connect
frame's participant ID remains an unauthenticated claim until a host verifies it.

## Boundary

The engine accepts `RevisionedCommandV2` and returns `TransitionV2`. The
convenience `apply` method binds a `CommandV2` to the current revision. It performs
no I/O, reads no clock, starts no participant, and authenticates no principal.

Addressing is conversation data only. Agora never turns an addressed participant
into a task or execution request. An adapter may offer accepted messages to Moira;
Moira decides whether workflow exists, Chronos schedules it, and Daimon executes
already-authorized work. The qualified local boundary is recorded in the
[Moira integration profile](docs/moira-integration.md).

Commands publish one event and advance one revision. Concurrent hosts use
`apply_revisioned`; the core still requires one serialized publication owner and
makes no consensus claim. Adapters must deliver and acknowledge the outbox rather
than infer delivery from command acceptance.

`PublicationCommandV2` binds one exact plain response to an opaque idempotency
key. Matching replay returns the original transition without another message or
revision. Changed content conflicts. The bounded volatile receipt index becomes
durable only when a host persists it atomically with Agora state.

The `AGORACP` version-4 codec validates bounds, canonical order, ownership/references,
message and event frontiers, addressed membership, bans, moderation evidence, publication attribution, and
the exact unacknowledged outbox suffix. V1 bytes are refused because their agent
activation state has no neutral interpretation. The codec proves neither origin
authentication nor storage durability. Earlier profile bytes are refused.

The service's `ClientIdV1` and request IDs are correlation handles, not
credentials. A host must authenticate `connect`, bind the returned handle to its
transport context, and never trust a client-supplied participant identity.

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
