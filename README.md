# Agora

Agora is a headless Rust conversation engine in which humans and software agents
are equal, explicitly identified participants. It owns participants, rooms,
membership, roles, messages, and participant-neutral addressing. It does not own
authentication, orchestration, tasks, harnesses, storage, transports, UI, model
behavior, or wall-clock access.

The v0.3 core provides bounded deterministic transitions, typed atomic refusal,
room-local message ordering and retention, optimistic revisions, an ordered
bounded outbox, replay-safe response publication, private read-position state,
an independently retained recent-change journal, and canonical profile-14
checkpoints.
Messages may explicitly address a canonical bounded list of current room members.
Participant kind never changes admission, role, speaking, or addressing behavior.

The core also provides membership-filtered, bounded room, member, and retained-
message pages. Their opaque cursors are bound to the exact observer, room where
applicable, and engine revision; message pages expose the retained sequence
interval rather than claiming complete history. A snapshot-aligned bounded room
change feed continues from a stable event identity and returns explicit retention
or leave/rejoin gaps requiring resnapshot. Protocol V2 projects these values;
Protocol V1 remains frozen.

Delivery and pull observation now retain independent copies of the same accepted
event. Delivery acknowledgment drains only the lossless/backpressured outbox;
bounded change-journal eviction alone creates a pull-feed retention gap. Both
suffixes and their distinct edges survive canonical recovery.

An opt-in standalone fuzz profile drives all public wire decoders and the three
semantic checkpoint layers under pinned cargo-fuzz, nightly, libFuzzer, corpus,
and campaign bounds. The stable offline gate verifies that contract and exact
canonical seeds; it does not claim the coverage-guided campaign ran. See
`docs/fuzzing.md`.

An additive Protocol V2 now has a separately typed serialized service, client,
and canonical version-2 complete-frame codec. It closes participant-bound room creation, invitation, joining,
leaving, role change, kick/ban/unban moderation, speaking, and bounded page projection with client-scoped
opaque cursors and exact volatile request replay. It deliberately has no V2
credential authenticator or network transport. Its separately typed authenticated
host applies current-context revalidation and exact session/client binding, and a
separate sequential Unix adapter composes that host with the qualified Aegis
verifier. V1 remains unchanged.

Room owners can also change Open/InviteOnly admission as an ordered room fact.
The event records prior and new policy without rewriting earlier invitations or
admissions; current members observe it through the bounded room-change feed.
Owners and moderators can inspect the ban set through a bounded, revision-bound
page; ordinary members and former participants cannot enumerate it.
Connected participants can also discover a closed, versioned capability list.
Discovery is descriptive only and never grants room or action authority.
The `ReactionsV1` capability adds explicit bounded participant-to-message set
relations. Current members may add or remove their own relation and observe
canonical aggregate counts plus caller membership; reactions remain metadata,
not authority, delivery, acknowledgement, or work-completion evidence.
The `MessageRevisionsV1` capability adds author-only bounded corrections over a
stable message identity. Current members can page the immutable 64-record
history; edits preserve addressing, reply topology, and reactions, while later
withdrawal or redaction scrubs every retained revision body without erasing its
metadata or ordered event.
The `IdentifiedMessagesV1` capability adds opaque restart-stable message
references through parallel speak and history operations while preserving every
legacy V2 message frame byte-for-byte.
The `AuthorWithdrawalV1` capability lets a current member withdraw only their
own retained message by opaque ID. History and retained feed copies become a
reference-preserving empty-body tombstone; this is not a physical-erasure claim.
The distinct `ModeratorRedactionV1` capability applies the room role hierarchy
to another author's retained message and preserves bounded reason, opaque
causation, actor, author, identity, and order in an attributable event.
The `IdentifiedRoomChangesV1` capability carries that same stable identity in a
parallel bounded room-change profile. Its cursor kind is isolated from the
legacy feed, while ordering, membership checks, gaps, and authority remain
unchanged.
The `ThreadsV1` capability adds an authoritative one-level root/reply relation,
independently bounded root and reply pages, and a relationship-preserving change
profile. Replies cannot nest; withdrawn retained roots remain replyable, while
evicted roots cannot receive new replies. Legacy and identified projections stay
byte-compatible and deliberately omit the relationship.
The separately advertised `ThreadLifecycleV1` capability makes active/archived
state an attributable room fact. Root authors, owners, and moderators may
archive or reopen retained roots with bounded evidence; archive blocks new
replies without hiding topology, deleting content, or implying task completion.
The `TopicsV1` capability adds stable opaque room-local topic identity, bounded
owner/moderator creation, topic-scoped speaking, and participant-bound topic and
topic-message pages. Replies inherit their root's immutable optional topic
relation. A parallel topic-aware change profile carries that relation without
altering legacy, identified, or threaded message bytes. Topics grant no posting,
subscription, notification, workflow, or agent-execution authority.
The `PrivateReadPositionsV1` capability adds participant-private monotone room,
thread, and topic markers. The service derives the participant from its client
binding; an advance changes neither room revision nor room events, but its state
and exact receipt are durably committed before success is published. Markers do
not prove delivery, attention, presence, unread policy, or execution.
The `IndependentChangeRetentionV1` capability states that participant pull feeds
read a separately bounded recent-change journal. Delivery acknowledgment cannot
advance its discard edge, and journal eviction never acknowledges delivery.

An independently versioned Service V2 checkpoint preserves the engine, pending
outbox, recent-change journal, client namespace, exact request replies, and live
cursor consequences.
`agora-durable` gates changed service state on a durable compare-and-exchange
receipt, restores known refusals, and fences indeterminate outcomes.
`agora-storage-directory` supplies one separately qualified physical profile:
an exclusive private local Unix directory with a synchronized atomic checkpoint
replace, a passive content-free owner observation, and exact offline canonical
backup/restore into a new private directory. Its claim does not extend to
hardware power-loss behavior, network filesystems, encrypted or remote backup,
rollback resistance, disaster-recovery objectives, or multi-host availability.
`agora-aegis-verifier` is a separate leaf adapter from the Host verifier port to
the public Aegis service contract. It requests `Submit` on one configured Agora
resource through an exact Aegis session and opaque grant, while Agora retains all
room authority. It authenticates no credentials and selects no transport.
`agora-local-daemon` supplies one separately qualified transport composition: a
new mode-0600 Unix socket inside a required mode-0700 parent, with bounded
length-prefix framing, deadlines, per-frame Aegis reauthorization, a fixed connection request budget, and
unconditional session cleanup. Its connection authenticator and clock remain
trusted application inputs.

The V2 client separately fences an outcome-unknown transmission: only the exact
same command may reuse the retained request identity until a reply resolves it
or the caller explicitly abandons and accounts for the unresolved request.

For authenticated deployments, the host and service also have one combined
checkpoint and pre-reply gate. Session subject/participant/client/time bindings
recover with the exact service receipt and must pass current verifier
revalidation before reuse. A separate combined owner can place that durable
conversation Host behind the collaboration profile while deliberately
reconstructing presence, typing, transient cursors, and transient replay state
empty after restart.

The repository also contains an initial layered operational profile:

- `agora-protocol` owns closed request, reply, snapshot, and refusal values;
- `agora-wire` owns their canonical bounded V1 binary representation;
- `agora-service` serially owns one engine, binds each connected client to one
  registered participant, and provides bounded replay-safe requests;
- `agora-durable` owns the store-neutral pre-reply durability and uncertainty
  fence for complete Service V2 checkpoints;
- `agora-storage-observation` owns the effect-free optional port for copying a
  passive physical-store snapshot into generic durable compositions;
- `agora-storage-directory` owns the one qualified local Unix checkpoint-store
  mechanism and no semantic state;
- `agora-host` binds exact wire frames and server-owned clients to a current
  externally verified principal/transport context;
- `agora-aegis-verifier` maps that verifier port to bounded public Aegis
  session/grant authorization without entering conversation semantics;
- `agora-local-daemon` owns the bounded sequential private Unix socket lifetime
  around separately selected Aegis-authorized conversation, transient
  collaboration, or durable Host V2 ownership, plus bounded OS-backed lifetime
  incarnation generation, and no conversation semantics;
- `agora-client` owns a monotone request namespace over an abstract service port.

These crates add no credential authentication, network transport, durable
execution, or agent behavior. See the
[service architecture](docs/service-architecture.md) and
[local daemon profile](docs/local-daemon.md).

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

The `AGORACP` version-12 codec validates bounds, canonical order, ownership/references,
message and event frontiers, addressed membership, topics, bans, moderation evidence, publication attribution, and
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
