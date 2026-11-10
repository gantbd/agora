# ADR: private monotone read positions

Status: accepted for implementation

Date: 2026-11-10

## Context

Agora now exposes stable message, thread, and topic identities together with
bounded history and resumable room-change observation. Clients need a durable
way to remember how far one participant has read without confusing that fact
with message delivery, attention, presence, notification state, or work
execution. Slack, IRCv3, Matrix, and Zulip provide useful precedents, but public
receipts and product-specific unread policy do not belong in Agora's first
profile.

The profile must remain participant-neutral, private by default, bounded,
durable, and independent of the authoritative room event stream.

## Decision

Add a separately advertised `PrivateReadPositionsV1` profile. A read position is
private current-state projection keyed by one participant, room, and closed
`ReadScopeV1` selector:

- `Room` covers every retained message in the room.
- `Thread(root_message_id)` covers one retained root and its direct replies.
- `Topic(topic_id)` covers messages carrying one stable topic identity.

Thread and topic scopes are alternatives in V1. The profile does not define a
combined topic-and-thread selector, arbitrary predicates, or recursive scopes.

An advance names an exact retained target `MessageId`, the engine revision at
which the caller observed it, and the scope. The target must belong to the
selected room and scope at that exact current revision. The actor must be a
current member. Knowledge of an identifier grants no observation or mutation
authority.

Each marker records the target identity, its room-local message sequence, and
the observed engine revision. A later target must have a strictly greater
sequence. Repeating the current target is idempotent; moving backward refuses
atomically. Sequence is used only for monotonicity inside one room and is not a
delivery, attention, or event-order claim.

Positions use a separately bounded private registry. Creating a key checks
capacity before mutation; advancing an existing key requires no new slot. A
marker survives message retention eviction, leave, kick, and ban so those
operations do not silently rewrite private history. It is inaccessible while
the participant is not a current member. A future forget operation, room
deletion law, or privacy-retention profile must decide reclamation. Advancing
from an evicted prior target remains possible when a newer retained target can
be validated because the retained sequence proves monotonicity.

The engine exposes self-scoped advance and current-position operations rather
than adding read commands to `RevisionedCommandV2`. A successful advance does
not increment the authoritative domain revision and does not publish a room
event or durable-outbox entry. The enclosing service checkpoint still changes,
so the durable owner must commit private state and the exact request receipt
before publishing success. Matching replay returns the original outcome;
changed input conflicts under the existing service law.

Protocol and service commands derive the participant from the authenticated
client binding. Neither advance nor observation may nominate another
participant. Outcomes return only the caller's marker. No room feed, history
page, member page, moderator surface, or capability response reveals another
participant's marker.

## Compatibility and persistence

Capability, command, outcome, scope, marker, and refusal values use previously
unassigned additive wire tags. Existing frames remain byte-identical except the
intentionally expanded capability inventory.

Engine persistence adds the bounded private registry after topic state. Service
and host checkpoints carry the engine state and exact request receipts; no
service cursor kind is needed because a read position is current state rather
than a continuation. Persistence profiles advance together and canonical
fixtures include room-, thread-, and topic-scoped markers.

## Rejected alternatives

- Publishing a room event for every advance would disclose private client state,
  consume shared retention, and invite false delivery or attention claims.
- Reusing a room-change cursor would conflate observation continuity with a
  participant's chosen read boundary and inherit the wrong retention law.
- Storing only sequence loses stable identity; storing only identity weakens the
  monotonicity proof.
- Accepting an arbitrary position without a retained target lets clients invent
  boundaries Agora cannot validate.
- Moderator inspection would create surveillance policy without consent or a
  product-level decision.
- Clearing on leave, kick, or ban silently destroys private state as a side
  effect of a different lifecycle transition.
- Engine unread counts would obscure retention gaps and import notification and
  aggregation policy.

## Required evidence

- Scope validation, exact revision binding, monotone/idempotent advancement, and
  regression, unknown, evicted-target, and capacity refusal atomicity.
- Current-member self-authority and identical human/agent behavior, including
  leave/rejoin and kick/ban inaccessibility without silent loss.
- No domain revision or outbox change and no disclosure through public surfaces
  or content-safe diagnostics.
- Participant-bound service execution, exact replay/conflict, durable
  commit-before-reply, checkpoint recovery, and lost-reply recovery.
- Closed SDK, protocol, wire, malformed-input, unknown-tag, truncation, fixture,
  compatibility, release-contract, and documentation evidence.

## Nonclaims

`PrivateReadPositionsV1` does not establish public receipts, delivery,
attention, comprehension, consent, availability, presence, typing,
notifications, subscriptions, unread-count policy, draft recovery, device sync,
agent activation, work admission, or task completion. It does not prove the
participant personally caused an advance; authentication and device policy
remain host and product responsibilities.
