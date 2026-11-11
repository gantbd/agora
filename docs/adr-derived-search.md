# ADR: derived message search boundary

Status: implemented through participant-bound service and canonical search wire;
authenticated host, client/SDK, and transport composition pending

Date: 2026-11-11

## Context

Agora owns retained conversation facts, current room membership, and the stable
identities of retained messages. Clients need bounded message discovery, but a
search index is a delayed, rebuildable projection. Treating it as history would
create a second authority for message existence, visibility, retention, and
content. Embedding an index in the engine would also make indexing cost and
failure part of every conversation transition.

## Decision

Introduce search in two deliberately separate layers.

`agora-search-port` will be an effect-free, transport-neutral contract. A query
selects one room, one closed match profile, one bounded nonempty UTF-8 term, an
optional opaque continuation, and a bounded result count. A result carries only
stable message identity and room-local sequence; it never carries message body,
excerpt, author, membership, or an authority decision. Every result page reports
the source room revision and an opaque index generation. The closed outcome set
distinguishes available, incomplete, stale, and unavailable observations.

`agora-search-memory` will be a bounded reference adapter for the contract. It
will accept explicit room projection replacement, retain no more than configured
room/message/content ceilings, use a specified byte-exact UTF-8 substring match,
and bind continuations to room, query, source revision, and generation. Replacing
or rebuilding a projection invalidates prior continuations. It is a semantic
reference and test adapter, not a durable or production-scale index claim.

Participant binding remains in Agora's service/host composition. Before querying,
that owner must derive the participant from its authenticated client/session and
prove current room membership from the authoritative engine. Before publishing a
hit, it must resolve the stable message identity against current retained history
and suppress anything no longer visible. The adapter receives neither a
participant identity nor credentials and cannot grant access. Adapter failure or
lag never falls back to scanning authoritative state implicitly.

Index ingestion is an explicit projection operation. A source revision states the
latest authoritative room revision incorporated by that projection. Complete
coverage means the adapter was supplied every searchable retained message for
that revision. Incomplete coverage is returned as such, including when the source
cannot prove that premise. A requested revision newer than the indexed source is
stale; an adapter that cannot safely classify its state is unavailable.

The initial port and reference adapter are qualified independently. The
participant-bound service now derives authority from its connected client,
revalidates every hit against current retained history, owns exact request
replay, and retires continuations on mutation and disconnect. A separate
canonical Search Wire V1 grammar carries those closed DTOs without changing
conversation Wire V1/V2 tags. Authenticated host, client/SDK, transport,
recovery, and release evidence remain before the whole profile is deployable.
None of these layers add search mutation commands, domain events, outbox entries,
or search state to the engine checkpoint.

## Consequences

- Search cannot create messages, prove membership, restore evicted history, or
  become a recovery source.
- Identity-only hits minimize disclosure and force current authoritative
  revalidation at the composition boundary.
- Source revision and generation make lag and rebuilds observable rather than
  silently presenting stale results as current truth.
- Port and adapter resource ceilings can be tested without adding dependencies
  or effects to the conversation engine.
- Different future index implementations may exist behind the same closed
  semantics, but each requires its own durability, privacy, and operational
  qualification.

## Rejected alternatives

- Engine-owned full-text indexes couple derived work and index failure to every
  authoritative transition.
- Returning excerpts or bodies from the index makes the projection an
  independently disclosing content store and permits retention drift to leak.
- Passing participant IDs into the adapter confuses an asserted identity with
  current Agora authority.
- Unversioned offset cursors silently continue across rebuilds and changed
  queries.
- A generic property or query language escapes closed vocabulary, canonical
  compatibility, and bounded-work review.

## Nonclaims

This decision does not establish stemming, ranking, normalization, typo
tolerance, cross-room discovery, attachments, artifact search, durable indexing,
encryption, malware policy, connector search parity, public network transport,
or participant-bound service availability. Search results are not delivery,
reading, attention, consent, workflow, execution, or audit evidence.
