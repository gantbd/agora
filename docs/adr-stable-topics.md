# ADR: stable room-local topics and topic-aware observation

Status: implemented and qualified

Date: 2026-11-10

## Context

Agora now has stable message identity, one-level threads, bounded reactions, and
immutable correction history. A useful multi-participant conversation substrate
also needs named collections within a room, but a display name cannot serve as
identity: names can collide or change, connectors rename things, and cursors
must not silently retarget. Discord forums, Slack/Teams channel-like views, and
Zulip topics offer useful precedent, but their product hierarchies and
subscription policies are not interchangeable.

The first topic profile must establish identity, message relation, bounded
history, feed continuity, recovery, and authority before adding rename, move,
merge, resolve, tags, subscriptions, unread counts, or notifications.

## Decision

Add a separately advertised `TopicsV1` profile. A topic has a stable opaque
128-bit `TopicId`, a bounded nonempty display name, an attributable creator, and
an ordered creation event. IDs are allocated deterministically within a room,
survive checkpoint recovery, carry no authority or ordering contract, and are
never derived from the display name. Topic names are unique only as current
canonical bytes within one room in this first profile; the ID remains the
reference identity.

Only a current room owner or moderator may create a topic. Human and agent
participants follow the same role law. Creation checks the expected engine
revision, room topic ceiling, name bounds, and canonical-name collision before
allocating identity or publishing an event. Rename, move, merge, resolve, and
delete commands are deliberately absent until their reference and retained
history consequences have separate decisions.

Messages gain an immutable optional topic relation. Existing `Speak`,
`SpeakIdentified`, and publication commands create unscoped messages.
`SpeakInTopic` names one existing topic and otherwise follows the same current-
member, addressing, body, sequence, identity, retention, replay, and outbox
laws. A reply inherits the exact topic relation of its root without accepting a
second caller-supplied topic, so reply topology cannot cross topics. Editing,
withdrawal, redaction, reactions, and thread lifecycle never alter the topic
relation. Retention may remove all messages from a topic without removing the
topic itself.

Current members may page topics in canonical `TopicId` order and page retained
messages for one exact topic in global room-message sequence order. Topic
message pages expose the room's retained sequence interval so filtering is not
presented as complete historical proof. Cursors bind observer, room, topic where
applicable, exact engine revision, selector/profile, and exclusive ordering
position. Invalid, foreign, stale, cross-room, cross-topic, and retention-lost
continuations fail closed without consumption.

Add a parallel topic-aware message and room-change projection. Its message DTO
is the current threaded message grammar plus an optional `TopicId`; accepted
messages in legacy, identified, and threaded profiles retain their existing
bytes and intentionally omit the relation. Topic creation and topic-scoped
message acceptance are ordered room facts in the topic-aware feed. Existing
moderation, policy, withdrawal/redaction, thread lifecycle, reaction, and edit
events project without reinterpretation. Knowledge of a topic ID grants no
membership, posting permission, subscription, notification, or workflow
authority.

## Compatibility and persistence

All command, outcome, capability, event, DTO, page, and cursor values use
previously unassigned additive tags. Existing wire fixtures remain byte-identical
except the intentionally expanded capability inventory. Engine persistence adds
the topic registry, allocator, and immutable message relation. Service and host
persistence add topic page/history/feed cursors and exact receipts. Their
profiles advance together and canonical fixtures include topic state, one
topic-scoped message, and cursor/feed consequences.

## Rejected alternatives

- Using the display name as identity makes rename and connector mapping unsafe.
- Treating topics as rooms duplicates membership, authority, admission, and
  moderation semantics before a distinct cross-room law exists.
- Reusing thread root IDs as topic IDs conflates reply topology with a named
  collection and makes a topic disappear with message retention.
- Letting a reply supply a different topic permits contradictory topology.
- Adding rename, move, merge, resolve, or deletion in the first profile would
  decide reference and history consequences without enough evidence.
- Making topic membership or subscription authoritative in Agora imports client
  notification and product policy into conversation state.

## Required evidence

- Stable room-local identity, role authority, name/capacity bounds, kind
  neutrality, deterministic allocation, collision refusal, and atomicity.
- Topic-scoped speaking, immutable relation, reply inheritance, edit/reaction/
  lifecycle preservation, tombstone preservation, and retention behavior.
- Bounded topic and topic-message pages with observer/room/topic/revision/profile
  cursor isolation and retained-interval disclosure.
- Topic-aware feed ordering, gaps, exact replay, service actor binding, recovery
  continuation, and unchanged legacy/identified/threaded projections.
- Closed Debug, SDK, wire, truncation, unknown-tag, malformed-shape, and canonical
  fixture evidence.
- Requirements, specification, compatibility, threat, assurance, traceability,
  release-readiness, and full `make check` closure.

## Nonclaims

`TopicsV1` itself does not establish rename, move, merge, resolve, deletion, tags,
categories, forums, spaces, organizations, topic-specific membership,
subscriptions, notifications, unread state, presence, delivery, attention,
workflow admission, task completion, agent activation, search, or connector
parity. Those require later profiles or product/orchestration policy.

Topic rename is now defined by the separate `TopicRenameV1` decision in
`adr-topic-rename.md`; the remaining exclusions above are unchanged.
