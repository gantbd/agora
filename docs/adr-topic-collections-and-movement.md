# ADR: room-local topic collections and movement boundary

Status: accepted and implemented; commit-pinned release qualification pending

Date: 2026-11-14

## Context

Agora already owns stable room-local topics, immutable message/topic relations,
rename and lifecycle facts, aliases, tags, and depth-one supersession. Product
precedents also expose “move,” “copy,” “merge,” forum, and collection concepts,
but those labels hide incompatible laws. Rewriting a retained message's topic
would contradict its admitted history. Copying another participant's content as
a new message would counterfeit authorship. Treating a category, server, team,
or forum as a room would silently introduce membership inheritance.

R100 must close those ambiguities before adding an indexed collection surface.

## Decision

The room remains the sole authority and membership boundary. Its bounded topic
registry is the first conversation collection; R100 adds no collection identity,
collection membership, inherited role, cross-room selector, or nested hierarchy.
The collection is a participant-bound derived index over existing room-local
topic facts, not a second mutable registry.

Add a separately advertised `TopicCollectionsV1` observation profile. A caller
selects one room and a closed filter containing lifecycle (`Any`, `Active`, or
`Resolved`) plus zero through eight canonical distinct tags. Tag matching is
conjunctive. Results are ordered by stable `TopicId` and each entry contains the
topic's identity, current name, creator, lifecycle state, canonical aliases and
tags, and optional depth-one successor. Pages are bounded by the room's existing
256-topic ceiling and cursors bind the client, observer, room, exact filter,
profile, engine revision, and exclusive stable-ID position. The index adds no
state, revision, event, subscription, unread marker, or search authority.

Existing message/topic relations remain immutable admission provenance. Agora
does not add `MoveMessage`, `MoveTopic`, or `CopyMessage` commands. A caller may
author a new ordinary message, but Agora will not attribute copied content to a
different participant or claim a source relation that it does not retain. A
future explicit copy profile requires a stable reference/provenance object,
retention and redaction laws, amplification bounds, and separate authorization.

Topic “merge” and redirect remain exactly the qualified depth-one
`TopicSupersessionV1` fact: a resolved source may name one successor, history is
not rewritten, and posting is never silently retargeted. Collection entries
project that relation but do not traverse or canonicalize it.

Private read positions remain scoped to the exact original topic identity.
Rename, resolve, supersession, and collection filtering never migrate, combine,
advance, or delete a marker. Clients may derive a combined view but cannot
publish it as Agora read truth.

## Consequences

- Forum-like room views can page exact current topic metadata without adding a
  product hierarchy or duplicating authority.
- “Move,” “copy,” and “merge” now have explicit non-equivalence. Only the
  already-qualified supersession relation has merge/redirect semantics.
- The derived index is bounded and canonically recoverable from existing state;
  no new engine persistence is needed beyond cursor/receipt recovery in the
  service and Host.
- Cross-room collections require a later profile with explicit visibility and
  reference laws. Federation does not follow from collection membership.

## Rejected alternatives

- Rewriting message topic IDs destroys immutable admission provenance and makes
  replay depend on later administrative choices.
- Copying retained bodies under their original author creates speech without
  that participant's mutation; copying under the operator loses provenance.
- A generic collection object without a demonstrated cross-room law duplicates
  product categories and invites accidental membership inheritance.
- Implicitly following supersession during writes or reads makes the named
  target differ from authoritative stored identity.
- Reusing derived search imports index freshness and ranking into authoritative
  collection observation.

## Required evidence

- Closed filter vocabulary, canonical tag conjunction, stable-ID order, page
  bounds, cursor isolation, stale revision, membership loss, and Debug safety.
- Exact projection of lifecycle, aliases, tags, and supersession without state,
  revision, event, read-position, or search side effects.
- Protocol/wire fixtures, service receipt/cursor recovery, authenticated Host,
  client uncertainty rules, both SDK facades, private Unix routing,
  compatibility/fuzz closure, and commit-pinned release qualification.
- Model/adversarial evidence that filtering never changes topic/message/read
  identity and human/agent participants observe under identical laws.

## Nonclaims

This profile does not move or copy messages, rewrite history, merge read state,
create folders/categories/spaces/teams/servers, inherit membership or roles,
span rooms, rank or search content, subscribe participants, deliver
notifications, establish workflow state, activate agents, federate rooms, or
provide deletion/erasure evidence.
