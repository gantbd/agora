# ADR: bounded message reactions

Status: implemented

## Context

Agora now has stable retained message identity, one-level threads, explicit
thread lifecycle, participant-bound observation, exact request replay, and
canonical recovery. A reaction can therefore be introduced as conversation
state without pretending to be a message, acknowledgement, notification, or
work-admission signal.

The production roadmap requires set semantics, explicit removal, bounded
aggregates, caller membership, and defined behavior across edits and
redaction. The current profile has tombstones but not message revisions; this
ADR closes the reaction laws that do not depend on revision history.

## Decision

`ReactionsV1` is a separately advertised capability.

A reaction is one retained relation `(message_id, reaction_key, participant)`.
The message must be retained in the named room and the participant must be a
current member when adding or removing the relation. Human and agent
participants follow identical rules. Add and remove are explicit transitions:
adding an existing relation or removing an absent relation refuses atomically.

Reaction keys are nonempty UTF-8 values of at most 64 bytes. A retained message
may have at most 64 distinct reaction keys. These fixed profile ceilings make a
complete aggregate observation bounded without another cursor family.
Aggregates are returned in canonical key order and contain only the key, the
relation count, and whether the observing participant belongs to that set.
They do not disclose the reacting-participant set.

Withdrawal and moderator redaction replace message content but preserve its
identity and reaction relations. Reactions and their ordered events therefore
remain visible to current room members after tombstoning. Thread archive also
preserves reactions and does not block reaction transitions. Retention eviction
removes all reaction state with the evicted message; a historical event may
continue to name that opaque message identity without making it observable as
current state.

Every accepted add/remove emits one ordered, attributable room event and
advances the engine revision exactly once. Events appear in the legacy,
identified, and threaded change profiles without changing the meaning of prior
tags. Exact service replay returns the retained reply and emits no second
event.

A reaction is conversation metadata only. It grants no authority, proves no
delivery or attention, admits no execution, and implies no task completion.

## Compatibility and persistence

New command, outcome, capability, and room-event tags are appended to their
closed Wire V2 vocabularies; invalid and capacity cases use the existing closed
refusal projections. Engine, Service V2, and Host V2 checkpoint
profiles advance together. Canonical fixtures freeze a reaction change, a
reaction status result, and the new checkpoint profiles.

## Required evidence

- [x] Core set, authority, bound, tombstone, archive, retention, and kind-neutral laws.
- [x] Service actor binding, exact replay, aggregate observation, and all-feed projection.
- [x] Exact engine/service/host recovery and continuation.
- [x] Closed protocol Debug surfaces and SDK construction.
- [x] Exhaustive wire round trips, unknown-tag refusal, truncation, and canonical fixtures.
- [x] Requirements, specification, threat model, compatibility, assurance, and traceability closure.
- [x] Full `make check` qualification.
