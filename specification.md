# Core specification

For the configured limits, an admitted state and a revisioned command define a total
deterministic transition:

```text
(State, RevisionedCommandV1) -> (State, Result<TransitionV1, Refusal>)
```

Expected invalidity is a `Refusal`; it is not a panic. Refusal is the identity
state transition. The mutable Rust owner is an implementation detail of this
functional contract.

Every accepted command increments the engine revision and event sequence exactly
once and appends exactly one event. Revision conflict, outbox saturation, or
counter exhaustion is checked before the domain command runs. The unrevisioned
`apply` convenience method binds the command to the current in-process revision.

## State

An engine contains a participant registry and room map. A room contains its
admission policy, members and roles, pending invitations, retained messages, the
next room-local message sequence, and its agent activation policy. The engine also
contains revision and event cursors plus a bounded event outbox. Domain collections
are ordered maps/sets so observation and activation order are stable.

## Authority

- A room creator becomes its owner and first member.
- Owners and moderators may invite registered participants.
- Only owners may promote or demote moderators.
- Open rooms admit any registered participant with remaining capacity.
- Invite-only rooms admit a registered participant only while an invitation is
  pending; successful joining consumes it.
- An owner cannot leave. Ownership transfer is deliberately absent in v0.2.
- Any member may speak. Participant kind grants no special authority.

The host must authenticate command actors. The engine checks room authority but
does not establish that a caller owns a supplied participant ID.

## Agent activation

Rooms begin with `Disabled` activation. An owner may select:

- `Disabled`: activation requests are refused;
- `Explicit`: a human room member may request one admitted agent for a retained
  causal message;
- `OnHumanMessage`: accepting a human message emits all admitted agent members as
  activation targets in stable ID order.

Automatic activation never follows an agent-authored message. Therefore the core
cannot create an unbounded agent-to-agent wakeup chain. Fan-out is checked against
`max_agent_activations_per_message` before accepting the message. An activation
event means requested work only; connector completion and delivery remain external.

## Event outbox

Accepted commands append one `EventEnvelopeV1` containing the event sequence,
accepted revision, and typed domain event. Saturation refuses before state change.
Acknowledgment may advance only through the published edge and cannot regress.
Acknowledged prefixes are removed. The in-memory outbox is not durable evidence;
an adapter must atomically persist state and events to claim crash recovery.

## Message retention

Each accepted message gets the current room-local sequence; checked increment
precedes publication. When retention is full, the oldest retained message is
evicted before the new message is appended. A zero retention limit refuses
speaking. Eviction affects retrieval only, not sequence continuity.

## Bounds

`Limits` bounds participant count, room count, members/invitations/messages per
room, pending events, activation fan-out, and UTF-8 byte lengths for IDs, display
names, room names, and bodies.
Empty identifiers/names/bodies are refused. Arithmetic overflow is refused.

## Replay-safe response publication

`PublicationCommandV1` is an adapter profile over the closed v0.2 `Speak`
command, not an expansion of `CommandV1`. It binds one opaque 16-byte key to the
room, author, logical time, body, and original accepted transition. A matching
replay observes the retained transition and does not re-run optimistic revision
checking. Changed semantic content under the same key is an idempotency conflict.
New keys backpressure before message mutation when
`max_publication_receipts` is full. The in-memory receipt map is not durable
evidence until a host persists it atomically with engine state.

