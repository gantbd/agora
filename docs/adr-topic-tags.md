# ADR: bounded topic tags

Status: accepted; implementation pending

Date: 2026-11-11

## Context

Stable topic identity, lifecycle, supersession, and aliases now support durable
conversation navigation and presentation. Forum-style clients also need a small
set of display/index labels on a topic. Importing a server/category/forum
hierarchy would make product organization part of Agora authority, while an
unbounded extension-property bag would escape the closed protocol and recovery
model.

## Decision

Add a separately advertised `TopicTagsV1` profile. A current owner or moderator
may explicitly add or remove a tag from an existing topic. Humans and agents
follow the same role law. A topic may retain at most eight tags, each nonempty
and bounded by the existing topic-name byte ceiling.

Tags are byte-exact display/index metadata, not stable identity. They form a
canonical set within one topic; duplicate add and absent remove are atomic
refusals. The same tag may appear on multiple topics, and a tag may equal a
topic name or alias because those vocabularies have distinct meanings. Agora
performs no case folding, normalization, hierarchy, implication, or inheritance.

Commands continue to select the target solely by stable `TopicId`. A separate
participant-bound status returns the canonical tag set without changing the
established topic page. Every accepted transition records actor, stable topic
ID, tag, and add/remove state in every change-feed profile. Exact request replay
returns the original result.

## Consequences

- Clients can build bounded forum-style filters or derived indexes without
  Agora owning a forum, category, server, team, or tenant hierarchy.
- The fixed count and byte ceilings bound state, checkpoint, feed, connector,
  and indexing work.
- Byte-exact set semantics remain deterministic across wire and recovery.
- Cross-topic tag discovery remains a derived index or client projection until
  a separately specified observation law is justified.

## Rejected alternatives

- A room-wide mutable tag catalog introduces stable tag identity, rename,
  retirement, pagination, and referential laws that the initial use case does
  not require.
- Hierarchical or implied tags import product taxonomy and ambiguous inherited
  authority into the conversation engine.
- An unbounded property map escapes capability negotiation, schema closure,
  compatibility review, and resource ceilings.
- Treating tag strings as command selectors would conflate display metadata
  with stable topic identity.

## Nonclaims

Tags are not topic identity, aliases, lookup authority, routing, redirects,
search results, subscriptions, notifications, permissions, workflow state,
agent activation, execution evidence, conversation collections, or tenant
hierarchy.
