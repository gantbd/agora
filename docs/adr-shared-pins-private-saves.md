# ADR: shared pins and private saves

Status: adopted; implementation pending

Date: 2026-11-14

## Context

Conversation products commonly collapse pins, bookmarks, saved items, copied
message previews, and attachment shortcuts into one feature. Agora cannot do so:
a pin is shared room state and a save is sensitive participant-owned state.
Copying message or artifact payload into either relation would also create a
second retention and redaction authority.

## Decision

Both features target one stable room-local `MessageId`. Neither relation copies
message body, revisions, addressing, attachment descriptors, Vellum artifact
targets, or external object identifiers.

### Shared pins

A room owns at most 256 pin records. A record retains the message identity,
original sequence, and actor that established it. Any current member may pin or
unpin; the host/Aegis action vocabulary may impose narrower product policy.
Pinning requires the message to be retained in that room. Unpinning an existing
record remains possible after message-history eviction. Setting the already
current state refuses without a revision or event.

Pin and unpin are ordinary revisioned room commands. Each accepted change emits
one room-visible event containing the room, actor, message identity, original
sequence, and resulting Boolean state. Current members may observe the bounded
pin set. Membership loss gates observation and mutation but does not erase the
shared room fact.

Withdrawal or moderation redaction leaves the pin relation in place and exposes
only a tombstoned availability state. Message-history eviction leaves the stable
pin identity and original sequence in place with an evicted availability state.
The relation never preserves scrubbed message or artifact metadata.

### Private saves

Agora owns at most 4,096 save records globally, keyed by participant, room, and
message identity. A current member may save a retained message, observe its own
saves, or remove an existing save. Save/remove is guarded by the exact observed
engine revision but does not advance that revision and emits no room event,
change-feed item, search document, or notification fact.

Reversible membership loss gates access but retains private state. A retained
save remains observable and removable after message-history eviction, with only
the stable identity, original sequence, and availability classification.
Another participant, moderator, room feed, diagnostic, or agent harness cannot
observe it.

### Availability and artifact references

Observation classifies a referenced message as `Available`, `Tombstoned`, or
`Evicted`. This classification is not a copy of message state and grants no
authority to retrieve payload. Pins and saves refer to the containing message,
not to an attachment or Vellum artifact independently. When tombstoning scrubs
those relations, pins and saves reveal no former descriptor, document target,
revision target, label, object key, or integrity value. External object
retention and authorization remain adapter/owner concerns.

## Acceptance

- Core authority, privacy, tombstone, eviction, membership-loss, capacity, and
  human/agent symmetry cases.
- Canonical recovery that rejects foreign identities, duplicate/default rows,
  invalid ordering, and legacy-profile reinterpretation.
- Closed protocol/wire tags, frozen frames, service/durable/Host/client/SDK and
  real private-transport binding.
- Bounded model and adversarial mutation evidence, compatibility/fuzz closure,
  ordinary gate, and commit-pinned release qualification.

## Nonclaims

A pin is not an approval, moderation hold, legal hold, retention override,
workflow assignment, or delivery request. A save is not a read receipt,
attention fact, private copy, export, offline cache, or device synchronization
claim. Neither relation authorizes access to referenced external bytes.
