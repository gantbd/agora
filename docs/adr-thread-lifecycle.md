# ADR: retained thread lifecycle profile

Status: implemented

Date: 2026-11-10

## Context

The one-level thread substrate distinguishes roots from replies but has no
authoritative lifecycle state. Product-local inference from inactivity or UI
state would let clients disagree about whether a thread accepts replies and
would not survive recovery. Archive/close must remain a conversation fact: it
cannot imply resolution, task completion, notification, subscription, or agent
execution.

## Decision

Add a separately advertised `ThreadLifecycleV1` capability. A retained root is
either active or archived. Active is the canonical default; only archived root
IDs occupy state. Archiving blocks new replies but never hides or deletes the
root, its retained replies, or their immutable relationships. Reopening permits
new replies again. Withdrawal and redaction do not change lifecycle state.
Evicting the retained root removes its lifecycle state; already-retained replies
may continue to name that historical root under the existing retention law.

The current root author, current room owner, or current moderator may archive or
reopen. All actors must be current members. A root author cannot manage another
author's thread merely by knowing its ID. The transition carries a bounded
nonempty reason and opaque 16-byte causation. Repeating the current state,
naming a reply, or naming an unknown/evicted root refuses atomically.

Add a bounded `ThreadStatus` observation for current members. It returns the
root ID and active/archived state at the exact current engine revision; the
service derives the observer from the client. This is current retained state,
not complete history.

Archive and reopen are explicit ordered room events in legacy, identified, and
threaded change profiles. Existing assigned variants retain their bytes. The
threaded profile remains the only accepted-message feed that carries reply
relationships.

Wire V2 allocates command tags `0x1d`, `0x1e`, and `0x1f` to ArchiveThread,
ReopenThread, and ThreadStatus. Outcome tags `0x1c`, `0x1d`, and `0x1e` carry
the two accepted lifecycle receipts and status. Capability tag `0x0c` is
ThreadLifecycleV1. Room-event tags `0x0d` and `0x0e` are ThreadArchived and
ThreadReopened in all V2 change profiles.

The engine checkpoint advances from profile 8 to 9. Service and host
checkpoints advance from profile 10 to 11 because they embed the engine and may
retain lifecycle requests, replies, and feed events. Public Wire V2 remains
version 2 because all allocations were previously unassigned.

## Required evidence

- Root-author and owner/moderator authority; participant-kind neutrality;
  nonmember, foreign-author, reply-target, unknown, repeated-state, malformed,
  and stale refusal atomicity.
- Archive blocks replies; reopen restores them; pages and status preserve
  topology; withdrawal/redaction do not change lifecycle state.
- Receipt, status, every change profile, exact replay, and engine/service/host
  recovery preserve root ID, actor, reason, causation, state, and order.
- All commands, outcomes, event variants, capability tags, truncations, unknown
  tags, limits, and Debug surfaces close; new canonical wire fixtures freeze
  archive/status bytes without altering existing fixtures except capability
  inventory.
- Compatibility, assurance, threat, traceability, release, and roadmap evidence
  match the implemented profile and its nonclaims.

## Implementation closure

- [x] Authoritative retained-root state, authority, reply fence, and observation.
- [x] Closed Protocol/Service commands, outcomes, refusal, events, exact replay,
      and participant binding.
- [x] Engine/service/host recovery and profile advance.
- [x] Canonical Wire V2 fixtures, adversarial closure, Debug safety, and SDK evidence.
- [x] Compatibility, assurance, threat, traceability, release, and roadmap ledger.

## Nonclaims

Archive does not mean resolved, read, delivered, acknowledged, retained forever,
or completed by a human or agent. This profile defines no recursive threads,
subscriptions, notifications, unread state, reactions, revisions, search,
workflow admission, or orchestration behavior.
