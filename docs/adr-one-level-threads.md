# ADR: one-level reply and thread profile

Status: implemented

Date: 2026-11-10

## Context

Agora now has stable opaque message identity across acceptance, retained
history, room changes, recovery, and SDK decoding. That identity is the required
reference substrate for replies, but existing legacy and identified message
grammars are frozen and carry no relationship field. A reply must remain a
conversation fact: it cannot imply notification, subscription, workflow
admission, agent activation, task creation, delivery, reading, or attention.

## Decision

Add a `ThreadsV1` capability whose first profile supports exactly one-level
root/reply relationships. `SpeakReply` names a retained root message in the same
room. The engine derives the author from the service binding, rechecks current
membership and addressing, and refuses an unknown target or a target that is
itself a reply. Replies cannot nest in this profile.

The authoritative message record gains an optional `reply_to_root` message ID.
`None` is a root; `Some(id)` is a reply to that root. The relation is immutable,
room-local, and independent of sequence and event order. Withdrawal or
moderator redaction clears only the body and preserves the relationship. A
tombstoned retained root remains a valid reply target. Once retention evicts a
root, new replies to it refuse; already-retained replies keep the opaque root
reference and may outlive the root body or record.

Add bounded `ThreadRoots` and `ThreadReplies` observations. Roots and the
replies to one root page independently through profile-bound cursors carrying
observer, room, exact engine revision, selector, and exclusive sequence
position. Pages expose the global retained sequence interval so absence of a
root or reply is never presented as complete historical proof. Every use
rechecks current membership.

Add `ThreadedMessageV1`, which is the identified message grammar plus an
optional root ID, and a parallel `ThreadedRoomChangesV1` open/pull profile. Its
closed event vocabulary mirrors the current identified room-event vocabulary;
only nested event tag `06` uses the threaded message grammar. Existing legacy
and identified history and change profiles continue to project a reply as a
normal accepted message without its relationship. Their assigned bytes and
cursor kinds do not change.

`ReplyAccepted` returns revision, message ID, sequence, and root ID. Knowledge of
either ID grants no authority. Lifecycle is a separately advertised additive
profile; reactions, edits, quotes, forwards, read state, notification, and
subscription ownership are not part of `ThreadsV1`.

Wire V2 allocates command tags `0x18` through `0x1c` to `ThreadRoots`,
`ThreadReplies`, `SpeakReply`, `OpenThreadedChanges`, and `ThreadedChanges`.
Outcome tags `0x18` through `0x1b` are `ThreadRoots`, `ThreadReplies`,
`ReplyAccepted`, and `ThreadedChanges`. Capability tag `0x0b` is `ThreadsV1`.
The service allocates outer cursor kinds `8`, `9`, and `10` to root pages, reply
pages, and threaded changes respectively.

Because the authoritative message and engine checkpoint grammar change, the
engine checkpoint advances from profile 7 to 8. Service and host checkpoints
advance from profile 9 to 10 because they embed the engine and add cursor/reply
grammar. Public Wire V2 remains version 2 because every allocation is additive
and previously unassigned.

## Rejected alternatives

- Adding an optional relationship to `MessageV2` or `IdentifiedMessageV1`
  changes frozen assigned grammar.
- Treating the immediately preceding message as a parent confuses order with
  identity and is unstable under paging and retention.
- Recursive reply trees make bounds, observation, lifecycle, and tombstone laws
  materially harder before the one-level profile is qualified.
- Keeping thread relation only in clients permits contradictory authoritative
  histories and loses the relation across clients and recovery.
- Accepting replies to an evicted root would require a separate durable root
  registry and retention contract not present in this profile.
- Making a reply imply notification, subscription, or agent execution crosses
  Agora's ownership boundary.

## Required evidence

- Root acceptance, reply acceptance, non-recursion refusal, cross-room/unknown
  refusal, addressing validation, kind neutrality, and refusal atomicity.
- Independent bounded root/reply pagination; selector, client, room, revision,
  and profile cursor isolation; retention interval disclosure.
- Root and reply withdrawal/redaction preserve IDs and relationship while
  tombstoning bodies; a tombstoned retained root remains replyable.
- The same reply/root IDs survive receipt, pages, threaded feed, exact replay,
  engine/service/host recovery, and SDK decoding.
- All threaded event variants and optional-root tags round-trip; unknown tags,
  truncation, trailing data, limits, and Debug disclosure fail closed.
- New canonical thread fixtures close additive bytes while every legacy and
  identified wire fixture remains byte-identical except the intentionally
  expanded capability inventory.
- Engine/service/host profile advances are canonical, bounded, scope-bound, and
  represented in compatibility, assurance, threat, traceability, and release
  evidence.

## Implementation closure

- [x] Authoritative one-level relationship and bounded engine observations.
- [x] Closed Protocol/Service commands, outcomes, events, cursor isolation, and
      exact replay.
- [x] Engine/service/host checkpoint recovery and profile advance.
- [x] Canonical Wire V2 codec, fixtures, adversarial closure, and SDK evidence.
- [x] Documentation, compatibility baseline, assurance, and release gate.

## Nonclaims

This profile alone does not establish recursive threads or lifecycle; the
separate retained lifecycle profile adds archive/reopen without changing these
bytes. Neither profile establishes thread membership, subscriptions,
notification, unread state, presence,
delivery, attention, task or workflow admission, agent activation, reactions,
edits, search indexing, external connector parity, or physical erasure.
