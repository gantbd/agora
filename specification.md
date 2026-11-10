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
receipt index is full. Disconnect is idempotent through a bounded retired-client
tombstone and releases that client's receipts.

All service state is volatile. Handles are correlation values rather than
credentials. Authentication, transport binding, persistence, and distributed
serialization remain host obligations.
