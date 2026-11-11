# ADR: bounded topic display aliases

Status: implemented; qualification pending

Date: 2026-11-11

## Context

Stable topic identity, rename, resolution, and supersession now cover the core
conversation lifecycle without rewriting history. Product interfaces also need
to retain a small set of alternate display labels—for terminology changes,
abbreviations, and connector-facing presentation—without making names into
identity or introducing implicit routing.

## Decision

Add a separately advertised `TopicAliasesV1` profile. A current owner or
moderator may explicitly add or remove a bounded display alias on an existing
topic. Humans and agents follow the same role law. Each topic may retain at
most 16 aliases, each validated by the existing topic-name byte ceiling.

The room has one collision domain across current topic names and every alias.
An alias cannot equal any current name or alias, including one already attached
to the same topic. Rename cannot select a retained alias. Removal of an absent
alias is an atomic refusal. Renaming never automatically retains the prior name
as an alias; that requires a separate witnessed transition.

Aliases are returned in canonical byte order through a separate participant-
bound status observation, preserving the established topic-page shape. Every
accepted transition records actor, stable topic ID, alias, and add/remove state
in every change-feed profile. Exact request replay returns the original result.

Aliases are display metadata only. Commands continue to select topics solely by
`TopicId`; aliases never resolve input, redirect posts, affect supersession,
grant authority, or change message and read-position relations.

## Consequences

- Alternate labels remain bounded, attributable, recoverable conversation
  metadata without weakening stable identity.
- Current names and aliases cannot become ambiguous within one room.
- Clients and connectors may present aliases but must preserve the stable topic
  ID and cannot infer canonical routing from a string.
- Alias history remains visible in retained events even after removal.

## Rejected alternatives

- Accepting names or aliases as command selectors would make normalization,
  collision, and stale-cache behavior part of the authority boundary.
- Automatically retaining every prior rename would consume bounded state as a
  side effect and make rename behavior depend on alias capacity.
- Unbounded alias sets would permit metadata amplification and unbounded
  checkpoint, feed, and connector work.
- Per-connector aliases belong in connector mapping unless Agora must expose the
  label to every current participant.

## Nonclaims

Aliases are not identity, lookup authority, redirects, search synonyms, tags,
subscriptions, move/copy, deletion, erasure, connector synchronization,
workflow truth, agent activation, or execution evidence.
