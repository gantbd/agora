# ADR: private attention state

Status: adopted; implementation in progress

Date: 2026-11-14

## Context

Participants need durable preferences that help clients decide what to surface,
without turning Agora into a delivery engine or disclosing those preferences to
other room members. Room, thread, and topic targets already have stable local
identities and membership-bound observation. Private read positions establish
the relevant ownership, privacy, recovery, and revision-free mutation pattern.

“Follow”, “mute”, and “notification” are often overloaded with claims about
push delivery, ranking, urgency, presence, or whether a person actually paid
attention. Agora can safely retain intent, but it cannot make those downstream
claims.

## Decision

Agora owns one optional private attention preference per participant, room, and
closed scope. The scopes are `Room`, `Thread(root_message_id)`, and
`Topic(topic_id)`. Thread scope requires a retained root; topic scope requires a
current topic. The value is the product of three independent preferences:

- `following`: an explicit Boolean request to follow the scope;
- `muted`: an explicit Boolean request to suppress ordinary attention;
- `notification_intent`: `Default`, `AllActivity`, `AddressedOnly`, or `None`.

These fields are intentionally not normalized against each other. In
particular, a muted followed scope and a muted scope with `AllActivity` are
valid stored intent. A consumer may combine the fields according to its own
documented policy; Agora does not invent precedence or claim a delivery result.

The all-default value is absence and removes the stored entry. Mutation is
idempotent, bounded across the engine, and guarded by the caller's observed
engine revision so a scope cannot be changed against stale membership or target
state. It changes neither domain revision nor room events. Observation and
mutation require a registered current room member on every call. Retained
private state survives ordinary leave/rejoin and history eviction but is
inaccessible while membership is absent. It grants no authority and cannot
restore an immutable peer-conversation departure.

Human and agent participants have identical laws. Diagnostics reveal no
participant, room, target, or preference value.

## Consequences

- The engine needs an independent bounded private-state map and a new canonical
  checkpoint profile while preserving every legacy byte.
- Protocol and wire commands must carry the authenticated participant only by
  service binding, never as caller-selected authority.
- Replies may return only the requesting participant's exact preference.
- Service receipts, Host recovery, client uncertainty, SDK facades, and the
  private Unix route must close over the additive commands.
- No room snapshot, room history, change feed, search document, event, or
  moderation view contains attention state.

## Rejected alternatives

- A room event leaks private intent and makes a preference consume shared event
  ordering and retention.
- A single notification level cannot represent following independently from
  suppression intent.
- Deriving mute from `notification_intent = None` loses the independent input
  required by downstream policy.
- Automatically deleting preferences on membership loss silently destroys
  private state during a reversible ordinary-room departure.
- Accepting arbitrary scope strings would create an unbounded extension and
  bypass target validation.

## Required evidence

- Exact ownership, member privacy, human/agent symmetry, scope validation,
  stale-revision refusal, capacity, idempotence, default removal, and no-event /
  no-revision behavior.
- Leave/rejoin, target-history eviction, canonical recovery/migration, malformed
  checkpoint, and bounded schedule evidence.
- Closed protocol/wire tags, frozen frames, service replay/conflict, Host actor
  binding, client uncertainty, both SDK facades, and real private Unix routing.
- Compatibility, fuzz-seed, ordinary-gate, and commit-pinned release evidence.

## Nonclaims

Attention preferences are not delivery subscriptions, notifications, push
registrations, acknowledgments, read positions, presence, ranking, urgency,
workflow admission, agent activation, or evidence that a human or agent saw or
acted on content. Hermes remains the owner of delivery composition in R103.
