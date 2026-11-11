# ADR: participant-bound derived search profile

Status: implementation in progress; protocol values and participant-bound service
owner implemented, wire/host/client/SDK/transport closure pending

Date: 2026-11-11

## Context

R87 established an effect-free search port and a bounded reference adapter. The
port deliberately receives no participant identity and returns identity-only
hits. Making that contract remotely useful requires an Agora owner to bind every
query to an authenticated participant, select the current authoritative room
revision, and revalidate every hit. Adding an optional index directly to the
existing `ServiceV2` or its frozen Wire V2 grammar would make a derived adapter
part of the mandatory conversation owner and recovery profile.

Search observations also have consequences of their own. Continuations must not
cross clients or authenticated sessions, changed replay under one request
identity must conflict, disconnect must retire client-owned consequences, and an
index rebuild must make prior continuations stale without mutating Agora.

## Decision

Add a separately advertised `DerivedSearchV1` profile around, not inside, the
existing conversation profile.

The service composition owns one `ServiceV2`, one injected `SearchPortV1`, a
bounded exact search request/reply receipt set, and a bounded registry mapping
client-owned opaque search cursors to adapter cursors. Ordinary conversation
requests continue through `ServiceV2` unchanged. Search requests contain a
search request identity, room, closed match profile, bounded term, optional
service cursor, and bounded result count. The service derives the participant
from the active client and derives the observed revision from the current room;
neither is accepted from the caller.

Before invoking the adapter, the service proves current room membership from the
engine. Before publishing a page, it resolves every returned stable message ID
in that room, requires the exact returned room-local sequence, and requires that
the room revision is still the revision supplied to the adapter. Any mismatch or
mid-operation authoritative change becomes unavailable and publishes no hits.
The index cannot widen visibility, restore evicted history, or decide authority.

Service continuations are opaque and bound to client, room, term, match profile,
source revision, index generation, and page limit. They are one-shot. A matching
request replay returns its exact retained reply without invoking the adapter or
consuming another cursor; changed reuse conflicts. Disconnect retires receipts,
service cursors, and their adapter cursors. The search port therefore gains an
explicit idempotent cursor-retirement hook; retirement is cleanup, not an
authority or success signal.

Search protocol and wire values are separately versioned from Wire V2. They
carry identity-only hits and the same available, incomplete, stale, unavailable,
and typed-refusal distinctions. A search-aware host wraps authenticated Host V2,
revalidates the same session context before each search frame, binds the wire
client to the server-owned session client, and retires search consequences on
close, revocation, substitution, or time regression. A search client owns a
monotone request namespace and the same definitely-not-sent versus
outcome-unknown fence as the conversation client.

Search state is derived and is not added to engine, Service V2, or Host V2
checkpoints. The initial profile is volatile: process restart loses search
receipts/cursors and the reference index, so clients must reconnect and requery.
Durable index and search-session recovery require a later separately qualified
profile. Capability discovery and the local transport handshake advertise the
profile only once protocol, wire, host, client, SDK, and transport routing are
closed together.

## Consequences

- Existing conversation commands, frames, checkpoints, and durability claims
  remain byte-for-byte independent of search availability.
- Participant and revision derivation eliminate caller assertions at the two
  authoritative boundaries.
- Exact replay and cursor ownership prevent adapter work from escaping the
  repository's normal uncertainty and cleanup discipline.
- Index rebuild, lag, incomplete coverage, unavailability, and Agora retention
  remain explicit rather than silently falling back to a scan.
- Search-aware deployments opt into an additional volatile owner and must size
  request receipts, service cursors, adapter cursors, content, and CPU work.

## Rejected alternatives

- Making `ServiceV2` generic over an index changes the mandatory conversation
  construction and recovery graph for a derived feature.
- Accepting participant or observed revision in the search request turns
  caller-controlled correlation data into authority or freshness evidence.
- Exposing adapter cursors directly permits cross-client continuation and makes
  disconnect cleanup impossible to close.
- Revalidating only room membership but not every hit permits stale projections
  to disclose evicted, withdrawn, redacted, foreign, or identity-mismatched
  facts.
- Implicit full-history scanning on index failure hides operational state and
  creates unreviewed worst-case work.

## Nonclaims

This decision does not establish durable indexing, cross-restart search replay,
ranking, stemming, normalization, typo tolerance, cross-room discovery,
attachment or Vellum artifact search, connector parity, network availability,
or a performance SLO. A result is not delivery, reading, attention, consent,
workflow, execution, or audit evidence.
