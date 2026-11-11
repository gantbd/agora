# ADR: stable topic rename

Status: implemented

Date: 2026-11-11

## Context

`TopicsV1` established opaque room-local topic identity, role-authorized
creation, immutable message relations, bounded observation, and durable
recovery. Its display name was intentionally not identity, but the profile did
not define how a name could change without retargeting references, losing
history, or silently widening authority.

## Decision

Add the separately advertised `TopicRenameV1` profile. A rename names one
existing `TopicId`, an expected room revision, and a bounded nonempty new name.
The service derives the actor from the connected client. Only a current room
owner or moderator may rename; human and agent participants follow the same
role law.

The topic ID, creator, message-topic relations, reply topology, reactions,
revision history, and read-position scopes do not change. The current topic
registry replaces only the display name. A rename to the current name or to
another current topic's name is refused atomically. Unknown topics, stale
revisions, invalid names, and unauthorized actors likewise produce no identity,
revision, event, receipt, or relation change.

An accepted rename emits one ordered `TopicRenamed` fact containing room,
actor, topic ID, previous name, and new name. All four room-change projections
carry the same attribution and names. The mutation receipt returns the stable
topic ID. Topic pages expose the new current name; retained messages and topic
message pages continue to expose the same relation.

`TopicRenameV1` is distinct from `TopicsV1` because capability discovery is a
closed semantic contract. Supporting topic creation does not imply support for
rename. Knowledge of the capability or topic ID grants no participant
authority.

## Compatibility and persistence

Command tag `2d`, outcome tag `2b`, capability tag `12`, and room-event tag
`12` were previously unassigned. The command, receipt, and topic-aware change
have frozen canonical fixtures. Existing assigned variant encodings are
unchanged; the canonical capability fixture intentionally gains the new tag.

The retained event adds a new checkpoint grammar, so engine/service/host
profiles advance to 15/17/17. Their representative fixtures contain the rename
fact, current renamed state, and exact service/host receipt consequences.
Earlier profiles fail closed rather than omit or reinterpret rename history.

## Rejected alternatives

- Allocating a new topic ID would break stable references and falsely resemble
  a move or copy.
- Rewriting historical creation or message events would destroy attributable
  history and checkpoint determinism.
- Letting any member rename would make a room-wide namespace mutation follow
  posting authority rather than topic-management authority.
- Folding rename into `TopicsV1` would make closed capability discovery
  ambiguous for older implementations.

## Nonclaims

Rename does not establish aliases, name history pages, move, merge, resolve,
deletion, tags, subscription, notification, unread aggregation, workflow state,
connector synchronization, task creation, agent activation, or execution.
