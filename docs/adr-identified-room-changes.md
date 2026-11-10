# ADR: additive identity-bearing room changes

Status: accepted for implementation

Date: 2026-11-10

## Context

The engine's authoritative `DomainEventV2::MessageAccepted` contains the stable
`MessageId`, but the frozen Protocol/Wire V2 `RoomEventV2::MessageAccepted`
grammar predates message identity. Adding a field to event tag `06` would change
already frozen bytes. Threads, reactions, edits, withdrawal, and redaction need
one stable reference vocabulary across history, mutation receipts, and room
changes.

## Decision

Add an `IdentifiedRoomChangesV1` capability with parallel
`OpenIdentifiedChanges` and `IdentifiedChanges` commands. The open result may
reuse the existing revision/cursor scalar shape, but continuation is owned by a
new outer service cursor kind. A legacy change cursor and an identified change
cursor are never substitutable, even though both nest the same private engine
`RoomChangePosition` checkpoint.

The pull result uses new `IdentifiedRoomChangePageV1`,
`IdentifiedRoomChangeV1`, and `IdentifiedRoomEventV1` values. The event vocabulary
is closed and mirrors the current room-event set. Its message-accepted variant
contains `IdentifiedMessageV1`; withdrawal and moderator-redaction variants
retain their existing message ID, actor, author where applicable, reason, and
causation evidence. All other variants preserve their current meaning.

Wire V2 assigns new root command, outcome, and capability tags. Within the new
outcome only, nested event tag `06` encodes the identified message grammar. The
legacy Changes outcome, its nested tags, all existing fixtures, and legacy
cursor behavior remain byte-identical.

Opening remains snapshot-aligned at an exact engine revision. Pull remains
exclusive-after, bounded, room-filtered, membership-rechecked, replayable, and
subject to the same retention and leave/rejoin gaps. Identity adds reference
information only; it grants no authority and changes no ordering law.

## Rejected alternatives

- Adding an ID field to legacy `MessageV2` or room-event tag `06` breaks frozen
  frames.
- An optional ID on every legacy change makes absence ambiguous and still
  changes assigned grammar.
- Deriving IDs from sequence in clients exposes an implementation detail and
  cannot remain valid across room incarnations.
- Treating the opaque ID as order or authority violates the identity contract.

## Required evidence

- Cross-profile cursor substitution refuses without consuming either cursor.
- Identified accepted messages expose the same ID through speak receipt,
  history, retained change feed, checkpoint recovery, and SDK decoding.
- Legacy change fixtures remain unchanged; a new canonical identified-change
  fixture closes the additive bytes.
- Every identified event variant round-trips; unknown nested tags, truncation,
  trailing data, limits, Debug disclosure, retention gaps, and membership-epoch
  gaps remain fail-closed.
- Service and host checkpoints advance independently if durable identified
  cursor or reply grammar is added; the engine checkpoint does not advance
  unless its authoritative schema changes.

## Nonclaims

This profile is bounded pull, not push delivery, subscription ownership,
per-client retention, transport resumption, read state, notification, or proof
that a human or agent observed or acted on an event.
