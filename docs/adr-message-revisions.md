# ADR: bounded immutable message revision history

Status: implemented and qualified

## Context

Agora owns short conversation messages while Vellum owns documents and rich
editing. Agora nevertheless needs an honest edit history for retained messages:
stable message identity, attributable revisions, bounded observation, exact
replay, and defined interaction with withdrawal, moderator redaction, replies,
addressing, reactions, retention, and recovery.

The legacy message projection contains one body and must retain its existing
wire grammar. Existing tombstone laws scrub retained message and acceptance-
event bodies, so revision history cannot claim that prior content is eternally
available.

## Decision

`MessageRevisionsV1` is a separately advertised capability. Only the current
original author may edit a retained, non-tombstoned message. Human and agent
authors follow identical rules. An edit supplies a nonempty bounded replacement
body and caller-supplied logical time, checks the expected engine revision, and
must differ from the current body. It appends one revision and emits one ordered
attributable room event. A message may contain at most 64 revisions including
the original admission; saturation refuses before mutation.

Message identity, original author, original admission time, room-local sequence,
addressed participants, and optional reply root never change. An edit changes
only the current body projection. Reactions remain attached to stable message
identity and survive edits. Thread lifecycle neither blocks nor changes edits.

Revision numbers begin at zero for the admitted body and increase without gaps.
Each immutable revision record retains message ID, revision number, editor,
logical time, and content-availability state. While available, it also contains
that revision's body. Author withdrawal or moderator redaction scrubs every
retained revision body and changes availability to `Tombstoned`; it does not
erase revision number, editor, logical time, attribution, reply topology,
reaction state, or event order. Retention eviction removes the message and its
current revision history.

Current members may page one retained message's revision records in ascending
revision order. The continuation is opaque and bound to client, observer, room,
message, profile, and exact engine revision. Invalid, foreign, stale, and
retention-lost targets refuse under the existing observation laws. A short page
does not imply completion when a continuation exists.

The legacy, identified, and threaded message projections continue to show only
the current body and retain their existing grammar. A separately tagged revision
page exposes history. Edit events appear in all three change profiles and carry
actor, message ID, message revision number, logical time, and current body; the
body is scrubbed from retained events if the message is later tombstoned.

This is bounded chat-message correction, not collaborative document editing,
branching, merging, rich-text operations, or source-control composition. Those
remain Vellum concerns.

## Compatibility and persistence

New command, outcome, capability, page, cursor, and event variants use
previously unassigned tags. Existing message and feed variants retain their
bytes. Engine, Service V2, and Host V2 checkpoint profiles advance together and
canonical fixtures freeze a revision page, edit event, and representative
recovery state.

## Required evidence

- [x] Core authority, append-only numbering, bounds, topology/addressing/reaction preservation, and atomic refusal.
- [x] Withdrawal/redaction content scrubbing with metadata preservation.
- [x] Revision-bound pagination and cursor/profile isolation.
- [x] Service actor binding, replay, all-feed projection, and recovery continuation.
- [x] Closed protocol Debug, SDK, wire, fixture, and adversarial decoder evidence.
- [x] Requirements, specification, compatibility, threat, assurance, and traceability closure.
- [x] Full `make check` qualification.
