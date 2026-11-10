# Core specification

For configured limits, an admitted state and revisioned command define a total
deterministic transition:

```text
(State, RevisionedCommandV2) -> (State, Result<TransitionV2, Refusal>)
```

Expected invalidity is a `Refusal`; it is the identity state transition. Every
accepted command increments the engine revision and event sequence exactly once
and appends exactly one event. Revision conflict, outbox saturation, and counter
exhaustion are checked before the domain command runs.

## State and authority

An engine owns participants, rooms, revision/event cursors, a bounded event
outbox, and bounded publication receipts. A room owns its admission policy,
members and roles, invitations, retained messages, and next message sequence.
Collections with observable order are ordered.

- A room creator becomes owner and first member.
- Owners and moderators may invite registered participants.
- Only owners may promote or demote moderators.
- Open rooms admit registered participants with capacity.
- Invite-only rooms require a pending invitation, consumed on join.
- Owners cannot leave; ownership transfer is absent in v0.3.
- Any member may speak. Participant kind grants no authority.

The host authenticates command actors. Supplying a `ParticipantId` is not proof
of identity, authority outside the room, or possession of an Aegis grant.

## Participant-neutral addressing

`SpeakAddressed` atomically retains a message and its addressed participants.
The list must be strictly increasing by participant ID, contain only room members
at acceptance, and fit `max_addressed_participants_per_message`. Later membership
changes do not rewrite historical messages. Validation precedes message sequence
allocation and retention eviction. `Speak` is the exact empty-addressing case.

Participant kind is descriptive metadata and is not consulted by addressing,
admission, role, or speaking transitions. Agora does not wake a participant,
create work, select a harness, or infer that an addressed participant responded.
An adapter may offer accepted message facts to Moira; orchestration belongs there.
The qualified local profile exposes a pending event as a read-only fact and
requires a separate explicit Moira admission decision; it never projects an
Agora address into execution authority.

## Event outbox

Accepted commands append one `EventEnvelopeV2` containing the event sequence,
accepted revision, and typed domain event. Saturation refuses before state change.
Acknowledgment may advance only through the published edge and cannot regress.
Acknowledged prefixes are removed. The in-memory outbox is not durable evidence.

## Message retention

Each accepted message receives the room's current sequence and caller-supplied
logical time. Checked increment precedes publication. At capacity, the oldest
retained message is evicted before insertion. Zero retention refuses speaking.
Eviction affects retrieval, not sequence continuity.

## Bounds and observation

`Limits` bounds participants, rooms, members, invitations, messages, pending
events, publication receipts, addressed participants, and UTF-8 bytes for IDs,
names, and bodies. Empty identifiers, names, and bodies are refused. Arithmetic
overflow is refused. Message and publication debug output reports sizes and
closed metadata without message bodies or idempotency keys.

### Bounded core observation profile v0.1

`room_page` returns only rooms in which the exact observer is a current member.
`member_page` and `message_page` additionally require current membership in the
named room. Page limits are positive and cannot exceed the corresponding engine
collection ceiling.

Continuations are opaque Rust values issued only by the engine. Each binds the
exact engine revision, observer, room where applicable, and final ordering key.
Using a continuation for another observer or room refuses as foreign. Any
accepted mutation makes the continuation stale, even when the mutation affected
another room; this conservative rule prevents a page sequence from silently
mixing revisions. Page order is canonical room ID, participant ID, or retained
message sequence order. A continuation is present exactly when another visible
item existed at that revision.

Message pages expose both `oldest_retained_sequence` and
`next_message_sequence`. The half-open interval identifies retention loss even
when a page is empty; it is not a complete-history or durable-history claim.
Pages clone bounded observations only, perform no mutation, consume no engine
revision or event sequence, and publish no outbox event. Their custom Debug
representations contain only revisions, sizes, counts, scalar positions, and
continuation presence.

This is an in-process engine profile. Protocol V1 remains frozen and does not
encode these values. A later Protocol V2 must close the service, wire, client,
host, and SDK projection without reinterpreting V1 bytes.

## Replay-safe response publication

`PublicationCommandV2` is an adapter profile over plain `Speak`. One opaque
16-byte key binds to the room, author, logical time, body, and original accepted
transition. Matching replay returns the retained transition without rerunning
optimistic revision checking. Changed content conflicts. New keys backpressure
before message mutation at capacity. Publication deliberately addresses nobody;
an orchestrator must use an explicit conversation command if addressing is part
of the response semantics.

## Checkpoint profile V2

The closed `AGORACP` V2 codec captures limits, participants, rooms, membership,
invitations, retained message suffixes and addressing, complete publication
records, revision/event frontiers, acknowledgment cursor, and pending events.
Host byte/cardinality ceilings apply before checkpoint-owned limits or allocation.
Invalid tags, UTF-8, ordering, identities, references, suffixes, cursors, and
receipt attribution are refused. Successful recovery has one canonical encoding.

V1 checkpoint bytes are intentionally refused rather than reinterpreted. V1
contained agent-activation state that has no participant-neutral equivalent.
Checkpoint origin authentication, storage atomicity, and durability remain host
or adapter obligations.

## Volatile service profile V1

The service owns one engine behind a serialized mutable boundary. A successful
trusted-host connection binds a service-allocated client handle to one registered
participant. Commands cannot supply their speaking author. Snapshot access
requires current room membership.

The key `(ClientIdV1, RequestIdV1)` binds to one exact request and retained
reply. Matching replay returns the reply without invoking the engine. Changed
content conflicts. New requests backpressure before invocation when the bounded
receipt index is full. Disconnect is idempotent within a bounded recent
retired-client tombstone window and releases that client's receipts. A full or
disabled tombstone window never prevents disconnect; the oldest evidence is
evicted instead.

All service state is volatile. Handles are correlation values rather than
credentials. Authentication, transport binding, persistence, and distributed
serialization remain host obligations.

## Volatile service source profile V2

`ServiceV2` is a distinct serialized owner rather than a reinterpretation of the
V1 service. A V2 connection binds a separately typed client handle to one already
registered participant. Create-room owner, invitation and role actor, joining or
leaving participant, and message author are always derived from that binding.
No V2 command may nominate its own actor.

The profile closes the current engine lifecycle commands and bounded room,
member, and retained-message pages. The service converts engine continuations
into opaque monotone `PageCursorV2` handles retained in a bounded client-owned
index. Cross-client, unknown, wrong-page-kind, and wrong-room handles share
`InvalidCursor`. A successful continuation consumes its old handle and installs
at most one new handle. Exact request replay returns the retained historical
reply without consuming or recreating cursor state. Disconnect reclaims that
client's requests and cursors. Cursor-index saturation refuses a first page that
would need a continuation before installing one.

V2 request receipts are bounded and keyed by the separately typed
`(ClientIdV2, RequestIdV2)`. Shape validation and receipt capacity precede engine
or cursor effects. Matching replay returns the original reply; changed input
conflicts. Engine revision conflict and observation staleness share a typed
`Stale` reply with the actual revision. Participant kind never selects a command
path or authority result.

The transport-neutral `ClientV2` owns a monotone request namespace over an
abstract V2 service port. It adds no connection lifecycle, authentication,
transport, retry, cursor traversal, or uncertainty policy.

The profile makes no cross-restart cursor, request, client, or durability claim.
Frozen V1 values and bytes remain unchanged.

## Wire profile V2

`agora-wire::v2` maps only Protocol V2 passive values to a distinct version-2
complete-frame grammar. It covers every V2 command, outcome, refusal, page,
cursor, admission, role, and participant kind. Optional cursors are closed tagged
values. Page limits and counts use bounded `u64` wire scalars with checked `u32`
projection where required.

Like V1, encoding measures the complete frame with checked arithmetic before
exact reservation. Decoding bounds the frame, each text, field count, and each
list before owned reconstruction and requires complete consumption. V1 bytes,
unknown root or nested tags, invalid UTF-8, truncation, trailing data, and ceiling
violations refuse. The normative grammar is in `docs/wire-protocol-v2.md`.

This is a frame codec only. V2 stream framing, feature negotiation,
authentication, transport binding, confidentiality, replay protection beyond
service request identity, and host session lifecycle remain open.

## Wire profile V1

`agora-wire` canonically encodes passive protocol V1 requests and replies using
fixed tags, big-endian integers, length-prefixed strict UTF-8 text, and counted
lists. Encoding preflights the exact checked length and all semantic ceilings
before output allocation. Decoding bounds the complete input, each text field,
field count, and every list before owned reconstruction, then requires exact
consumption. Unknown versions and tags are closed refusals. The normative byte
grammar and tag tables are in `docs/wire-protocol.md`.

Connect carries a claimed participant ID but grants no identity. Wire client IDs
remain correlation handles. Authentication, stream framing, replay protection
outside service request identity, encryption, and transport binding remain host
obligations.

## Authenticated host profile V1

The host consumes a complete Connect frame plus a trusted adapter-owned context
and explicit host time. Its injected verifier either returns an opaque verified
subject for that exact participant claim or refuses. Authentication error detail
is not projected; refusal uses `ConnectRefused(Unavailable)` before participant
registration is queried.

Successful connection retains `(host session, verified subject, participant,
service client)`. The host session is allocated outside the wire protocol and
must be bound to transport state by the adapter. Every later frame first
revalidates the retained subject/participant against the current context, then
requires its client field to match the retained service client. Failure retires
the host session and disconnects its own client before returning the generic
Unavailable class. Host time is monotone within a session; regression retires
before verifier invocation.

Transport close is an unconditional cleanup operation. Retired-session and
retired-client evidence is a bounded recent window; saturation evicts the oldest
tombstone and cannot block cleanup. Matching Execute replay preserves the
service's existing exact request/reply semantics. The host performs neither
credential verification nor network I/O and imports no authority-engine types.

## SDK distribution profiles V1

SDK is an audience and distribution label, not a semantic layer. The
`agora-sdk-embedded` trusted-host facade directly reexports only the engine,
serialized volatile service, and authenticated host. Those exports are
sufficient to construct the embedded stack; verifier, transport, storage, time,
and runtime choices remain injected by the application.

The `agora-sdk-client` remote-client facade directly reexports only the
transport-neutral client, protocol DTOs, and canonical wire codec. It supplies
no connection lifecycle, transport implementation, credential, retry policy, or
authority. Neither facade defines new values or behavior, and neither is an
umbrella over the other.
