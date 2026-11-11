# ADR: bounded immutable attachment references

Status: implementation in progress; core admission, attachment-aware retained
history, canonical wire, replay, actor binding, and recovery implemented;
attachment-aware changes and capability qualification open

Date: 2026-11-11

## Context

Agora messages need to discuss files without making the conversation engine a
blob store, upload service, content scanner, or download authority. Slack,
Discord, Teams, Matrix, and IRC extensions all demonstrate that file sharing
quickly accumulates provider-specific upload sessions, mutable URLs, previews,
malware state, retention policy, and authorization behavior. Importing those
concerns into the message grammar would make an external effect part of every
conversation transition and would confuse possession of a locator with access.

Agora nevertheless must retain enough immutable metadata to preserve what a
participant attached to an accepted message. The metadata must be bounded,
canonical, content-safe in diagnostics, recoverable with the message, and
honest about which integrity facts Agora has and has not verified.

## Decision

Add a separately advertised `ImmutableAttachmentReferencesV1` conversation
profile. Existing speak commands, wire tags, messages, feeds, and checkpoints
remain unchanged. The additive command accepts one message body, canonical
addressing, one closed message target (`Room`, `Reply`, or `Topic`), and at most
eight attachment descriptors. This one target vocabulary avoids a separate
attachment variant for every existing speak command while preserving the same
membership, topic, one-level reply, resolved-topic, revision, retention,
publication, and human/agent-neutral authority laws.

Each descriptor contains:

- an opaque 128-bit attachment identity stable only within the room;
- an opaque 128-bit store namespace and opaque 256-bit object key;
- a nonzero declared byte length under a configured ceiling;
- a canonical lowercase ASCII `type/subtype` media type without parameters;
- a bounded nonempty UTF-8 display label with no path semantics; and
- one closed integrity claim, initially SHA-256 with exactly 32 digest bytes.

The store namespace and object key are logical references, never URLs, paths,
credentials, bearer tokens, or proof that an object exists. Attachment IDs and
object references must be unique within one accepted message. Descriptor order
is caller-significant presentation order and is retained exactly; it confers no
priority or authority. Debug and operational observations disclose only counts,
lengths, the integrity algorithm, and lifecycle classes.

Acceptance is atomic with the message. Agora validates descriptor shape and
ceilings before consuming message identity, sequence, revision, outbox, change
journal, or publication-receipt capacity. Agora performs no object-store I/O
and does not verify the declared length, media type, label, digest, malware
state, or availability. A composing product must upload and verify before the
Agora command and must treat an upload whose message is refused or uncertain as
an explicit orphan/reconciliation case.

Accepted attachment descriptors are immutable. Message body revision does not
replace, add, or remove them. Every attachment-aware message page and change
profile carries the same descriptors. Existing profiles intentionally omit
them and remain byte-identical. Reply topology and topic relation remain message
facts, not attachment facts. Search V1 indexes only retained message bodies and
does not receive attachment labels, media types, keys, or digests.

Author withdrawal and moderator redaction tombstone every descriptor together
with all message bodies: stable attachment identities and their original count
remain attributable, while store namespace, object key, length, media type,
label, and digest are no longer disclosed by Agora. This is logical
non-disclosure, not object deletion. Retention eviction removes the descriptors
with the message but neither operation instructs an external store.

Checkpoint recovery retains descriptors, attachment-history cursors, and exact
request receipts. Durable
publication commits the message, descriptors, outbox, and receipt as one Agora
unit before success is visible. It does not make the prior blob upload atomic
with Agora. The protocol, canonical wire, service replay, Host actor binding,
client uncertainty, and both SDK facades now close for admission and
participant-bound retained history. The attachment-aware change profile, frozen
attachment wire fixtures, explicit local-transport evidence, and capability
discovery must still close before the profile is advertised.

## Consequences

- Agora owns the historical fact that bounded references accompanied a message,
  but never owns or serves the referenced bytes.
- Knowledge of any attachment, store, object, digest, message, or room identity
  grants no download authority.
- A resolver or download service must rederive current participant authority,
  revalidate current message visibility, verify returned bytes against length
  and digest, and express unavailable, refused, corrupt, and outcome-unknown
  states independently.
- Existing clients remain compatible and simply do not negotiate or observe the
  new profile.
- Typed Vellum artifact references remain a later profile because a revisioned
  document identity and witness are not an immutable blob descriptor.

## Rejected alternatives

- Embedding bytes in messages couples blob size, storage, scanning, and
  transport cost to the authoritative conversation transition.
- Storing HTTP URLs or filesystem paths leaks topology and often turns a
  retained message into a long-lived credential.
- Treating a content digest as the attachment identity creates unwanted
  cross-message correlation and deduplication semantics.
- Adding attachments after message acceptance permits partially visible message
  state and requires a second mutable lifecycle before a useful first profile.
- Reusing message edits for attachment replacement rewrites a different
  authority and retention problem under body-revision semantics.
- A generic metadata map escapes closed vocabulary, canonical encoding, bounded
  work, privacy review, and compatibility evidence.

## Required evidence

- Human/agent-neutral admission, target inheritance, exact descriptor order,
  uniqueness, bounds, and pre-mutation atomic refusal.
- Exact replay/conflict, outbox/change-feed projection, retention cleanup,
  withdrawal/redaction scrubbing, and canonical engine/service/host recovery.
- Closed protocol and wire round trips, malformed media type and descriptor
  rejection, frozen fixtures, deterministic mutation closure, SDK construction,
  authenticated Host binding, client uncertainty, and private local routing.
- Content-safe Debug and passive observation surfaces with no object key,
  digest, label, or message body disclosure.
- Explicit adapter evidence before any future resolver claims availability,
  byte integrity, malware status, download authorization, or deletion.

## Nonclaims

This decision does not establish upload, download, previews, thumbnails,
streaming, resumable transfer, deduplication, encryption, malware scanning,
quarantine, content moderation, external retention, deletion, legal hold,
erasure, connector parity, attachment search, Vellum document composition, or
cross-engine atomicity. Declared metadata is not verified truth, a digest is not
authorship, and an accepted reference is not proof of delivery, reading,
attention, consent, safety, execution, or task completion.
