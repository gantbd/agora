# ADR: conversation profiles

Status: accepted and release-qualified

Date: 2026-11-14

## Context

Agora currently has one owner-led room shape. Products also need direct,
group-direct, and cross-deployment shared conversations. Treating those labels
as presentation alone would leave cardinality, admission, departure, history,
and authority to inconsistent clients. Treating every label as a new message
engine would duplicate ordering, retention, references, moderation, recovery,
and observation semantics.

External subjects, organizations, tenants, and trust relationships are not
Agora identities. Aegis and directory owners establish those facts. Federation
is a later transport and replication composition and must not be implied by a
local room label.

## Decision

Agora has three authoritative local conversation profiles:

| Profile | Founding cardinality | Discovery | Later admission | Departure | Retained history |
| --- | ---: | --- | --- | --- | --- |
| `Ordinary` | one owner | current-member directory; broader discovery is external | existing open or invite-only room law | non-owners may leave; owner transfer remains absent | current members observe the bounded retained interval |
| `Direct` | exactly two distinct registered participants | current members only | none; the founding cohort is immutable | either current member may leave; departure is irreversible | current members observe the same bounded retained interval; departed participants lose access immediately |
| `GroupDirect` | three through the configured room-member ceiling, all distinct and registered | current members only | none; the founding cohort is immutable | any current member may leave; departure is irreversible | current members observe the same bounded retained interval; departed participants lose access immediately |

Direct and group-direct conversations are true profiles over the same room,
message, topic, event, ordering, retention, reference, recovery, and observation
substrate. They are created atomically with the complete founding cohort. Every
founder has role `Member`; neither the request initiator nor participant kind
creates an owner. Cohort order is canonical participant-ID order and has no
authority meaning.

The immutable-cohort profiles always have invite-only admission and reject
invite, join, role change, admission-policy change, kick, ban, and unban. This
first profile deliberately has no member replacement or privileged moderator.
Ordinary message withdrawal and the author-scoped parts of conversation state
remain available. Actions whose current law requires an owner or moderator
refuse normally because no such role exists.

Leaving records the ordinary attributable room event, removes current access,
and does not erase or rewrite retained history. A departed founder cannot
rejoin. A conversation with zero current members remains bounded authoritative
state until an explicit future retention profile disposes of it; knowledge of
its identifier grants no observation authority.

“Shared room” is not a fourth local profile. It is a product projection formed
by an ordinary invite-only room plus an explicit Agora-to-Agora federation
binding. R106 must define remote participant provenance, trust, membership
mapping, replication, conflicts, departure, and recovery before that projection
is advertised. A local `SharedRoom` flag would otherwise manufacture a tenant
or federation claim without evidence.

## Consequences

- Direct and group-direct creation needs a separate additive command rather
  than changing the frozen ordinary `CreateRoom` grammar.
- Room snapshots, summaries, events, checkpoints, protocol DTOs, wire frames,
  service mapping, capabilities, clients, and both SDK facades must expose the
  closed local profile without changing legacy bytes.
- Canonical recovery must retain the original founding cohort separately from
  current membership so departure remains irreversible.
- All cardinality and cohort checks precede mutation, revision allocation, and
  event publication.
- Human and agent participants use identical profile laws.

## Rejected alternatives

- Product-only direct-message labels cannot enforce cardinality or immutable
  membership and permit clients to disagree about authority.
- Giving the creator ownership makes a direct conversation needlessly
  asymmetric and imports ordinary-room moderation into a peer cohort.
- Invitation-based group-direct growth changes conversation identity and makes
  history disclosure depend on an underspecified late-join rule.
- Deleting history on departure confuses access revocation with physical
  erasure and contradicts existing bounded recovery.
- A local shared-room marker imports external identity and trust without a
  federation protocol.
- A separate direct-message engine duplicates the authoritative substrate and
  creates divergent replay and recovery laws.

## Required evidence

- Exact direct and bounded group-direct cardinality, duplicate/unknown member,
  capacity, and atomic-refusal cases.
- Initiator-order and human/agent permutation symmetry.
- Immutable-cohort refusals for every membership, role, admission, and
  moderation mutation.
- Peer departure, immediate observation loss, irreversible rejoin refusal,
  retained-history preservation, and zero-member recovery.
- Profile-bearing snapshot, summary, event, capability, protocol, wire,
  service, Host, client, and SDK evidence.
- Canonical checkpoint advance, exact replay, legacy-byte preservation,
  adversarial decode closure, bounded schedule model, fuzz corpus, and
  commit-pinned release qualification.

The engine-through-SDK implementation, authenticated Host binding, private
Unix transport route, bounded permutation/schedule model, canonical recovery,
two frozen wire frames, compatibility inventory, and fuzz seeds are present.
Revision `272913ec8561581c7afc62c87bbcfc7b2bbac514` passed the ordinary gate,
all release-mode targets, 10,000 deterministic soak cycles, and 1,000,000
fixed-seed cases for each of the three fuzz targets on Darwin 25.6.0 arm64 /
macOS 26.6.2, with unchanged tracked bytes afterward.

## Nonclaims

These profiles do not establish contact discovery, address books, external
identity, tenant membership, consent UX, encryption, notification, delivery,
read receipt, presence, workflow, task admission, agent activation, federation,
IRC interoperability, legal retention, export, or erasure. Shared-room behavior
remains entirely unimplemented until the federation profile is qualified.
