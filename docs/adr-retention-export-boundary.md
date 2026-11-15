# ADR: logical retention, exact export, and store reclamation

Status: accepted architecture; closed values and receipt-retirement slice implemented

Date: 2026-11-14

## Context

Agora already has bounded retained room projections, message tombstones,
revision scrubbing, durable receipts, private read positions, ordered events,
and physical checkpoint storage. Those mechanisms have different meanings.
Evicting a message from a bounded room projection does not prove that event,
receipt, backup, replica, or old storage bytes disappeared. Withdrawing or
redacting content preserves an attributable tombstone and likewise is not an
erasure claim.

R107 needs one Agora-owned contract for logical retention and exact export,
plus honest evidence about physical store reclamation. Legal hold, eDiscovery,
regulated retention schedules, subject-rights workflows, and guaranteed erasure
require a separate compliance architecture and are not inferred from this work.

## Decision

Agora owns the meaning of its logical records and their transitions. Storage
adapters own physical replacement and reclamation mechanisms but cannot decide
which Agora facts are eligible for removal. An export adapter may encode an
authorized immutable Agora projection but cannot widen visibility or invent a
compliance interpretation.

### Separate record classes

The first profile names each retained class independently:

- current room, membership, message, thread, topic, reaction, and revision
  projection;
- ordered authoritative room changes and delivery outbox entries;
- exact request/reply receipts and retired client/session tombstones;
- participant-private read positions and other private projections;
- federation publication receipts and authoritative-home replication state;
- operational checkpoints, backups, and replica copies.

One class reaching its bound never silently deletes or acknowledges another.
Human and agent participants use the same policy and authority rules.

### Logical retention policy

A retention policy is a closed, versioned value with positive bounds or an
explicit disabled state for every supported class. It is installed only by an
authenticated Agora administrative authority supplied by the application; a
participant, moderator, peer, adapter, or transport certificate cannot install
one. The policy carries a nonzero policy revision and effective engine revision.

Policy application is deterministic and monotone at one engine transition. It
returns exact counts and new oldest-retained frontiers by class. A refusal is
inert. Lowering a bound may make records logically unavailable but does not
claim physical overwrite, recipient deletion, backup deletion, or erasure.
Increasing a bound never reconstructs discarded facts.

Records required to interpret currently retained state are protected by explicit
dependencies. A retained reply keeps its root identity even if root content is
gone; a tombstone keeps identity, ordering, actor/author attribution, and the
minimum reason/evidence fields defined by its profile. Receipt pruning cannot
make an uncertain request safe to re-execute: expired receipt identity moves to
a bounded non-reusable tombstone/frontier or the command remains fenced.

### Tombstones

Withdrawal and moderation redaction remain distinct domain transitions. They
scrub bodies and retained revision/event body copies while preserving the
already specified attribution and topology. Retention eviction is not rewritten
as withdrawal or redaction. Policy retirement tombstones carry only the minimum
identity, scope, class, last retained revision/frontier, and transition evidence
needed to prevent aliasing, unsafe replay, or false reconstruction.

No tombstone means deletion from every copy. Tombstone reclamation requires a
later policy transition whose safety preconditions are independently proven.

### Exact export

An export request binds the authenticated exporter, one visibility scope, the
exact engine revision, export profile, retention-policy revision, and a nonzero
request identity. Authorization is evaluated against current authority before
materialization. A room export is no broader than the caller's current room
visibility; participant-private state requires that exact participant or a
separately defined administrative authority.

The canonical export is a bounded, paged manifest plus content-addressed chunks.
The manifest records profile versions, source engine revision, retention
frontiers, included record classes, gaps, tombstone semantics, and cryptographic
digests. Stable ordering and canonical bytes make retries byte-identical.
Matching request replay returns the same manifest; changed input conflicts.
Export never advances read positions, acknowledges delivery, changes retention,
or proves that a human or agent observed the content.

An export with a retention gap reports the gap; it cannot fill it from logs,
backups, replicas, or adapters unless those are separately authorized sources in
a later profile. Partial materialization is not a successful export.

### Physical store reclamation

Logical compaction first produces a complete next Agora checkpoint and a
reclamation plan naming the prior store generations that are no longer needed
for Agora recovery. The storage owner must durably publish the new checkpoint,
reopen and validate it, and only then may it reclaim named generations. Unknown
publication or validation outcome fences reclamation.

Store evidence distinguishes at least `NotAttempted`, `LogicallyUnreachable`,
`GenerationUnlinked`, and `ProviderAttested`. Unlink, truncate, copy-on-write
replacement, flash translation, snapshots, backups, replicas, and remote media
have different residual-data properties. Agora will not translate any of them
into secure erasure. Reclamation is bounded, restart-safe, idempotent, and never
targets an uncommitted or sole validated recovery generation.

### Federation and copies

The room home decides authoritative logical retention. A subscriber may apply a
stricter local replica bound but cannot represent it as home deletion. Retention
gaps and tombstones replicate as authoritative facts where the negotiated schema
permits. Peer acknowledgement does not authorize home reclamation unless a later
profile explicitly makes that dependency part of the home policy.

Exports, backups, and previously authorized replicas are new copies with their
own custody and retention obligations. Revoking trust or deleting a subscription
does not prove those copies were destroyed.

## Required evidence

- closed policy, record-class, frontier, tombstone, export-manifest, chunk,
  reclamation-plan, and reclamation-evidence values;
- deterministic policy transitions, dependency preservation, refusal
  nonmutation, and human/agent-neutral outcomes;
- replay-safe receipt retirement without request-identity reuse;
- observer/admin authorization, private-scope isolation, stable paging,
  canonical bytes, exact replay/conflict, gaps, and interruption recovery;
- checkpoint-before-reclaim ordering, reopen validation, unknown-outcome
  fencing, crash schedules, bounded residue, and old-generation nonselection;
- recovery and migration through engine, Service, Host, both SDKs, qualified
  storage adapters, and native federation where applicable;
- compatibility fixtures, finite models, corruption/fuzz/soak campaigns,
  supply inventory, and commit-pinned integrated qualification.

## Implementation status

`agora_engine::retention` implements the first no-I/O foundation: the complete
record-class set, positive-or-disabled bounds, revisioned policies, canonical
frontiers and transition evidence, redacted export request/scope bindings,
ordered bounded chunk manifests, distinct retention tombstones, ordered bounded
reclamation plans, and the four non-erasure reclamation evidence stages. Engine
state transitions, canonical persistence/wire profiles, authority composition,
export materialization, and physical adapter reclamation remain open.
The effect-free `agora-retention-authority-port` reuses those
Agora-owned values and gives a configured application verifier separate,
revision-bound policy-installation and export decisions. It grants no storage,
materialization, reclamation, legal-hold, or compliance authority.
The engine foundation also includes a bounded exact receipt-retirement registry.
It accepts only request-receipt retirement tombstones, treats an identical
retry as replay, rejects changed evidence for the same identity/scope, refuses
capacity atomically, and recovers only canonical sorted checkpoints. Membership
is a non-reexecution fence, not an erasure claim.

`ServiceV2` now performs atomic administrative receipt retirement: it validates
one canonical complete selection against the current engine revision, installs
every tombstone in a cloned bounded registry, and only then removes the exact
receipts. Canonical Service profile 30 and Host profile 27 recover that fence.
`HostV2` revalidates an authenticated session and obtains a fresh decision from
the injected retention-authority port before invoking the transition;
`DurableHostV2` withholds success until the complete tombstone-bearing Host
checkpoint is durably committed, restores the prior receipt state on a known
store refusal, and preserves the non-reexecution fence across restart. Human and
agent receipt identities follow the same path. This implements receipt
retirement only; general policy application, exact export, and store reclamation
remain open.

Service V2 also owns the current installed policy metadata. Installation is
engine-revision-bound, monotonically policy-revisioned, exact-replay-safe, and
conflict detecting. Host revalidation and a fresh policy-authority decision
precede installation; Durable Host commit gating makes it recoverable in
canonical Service profile 31 and Host profile 28. Receipt retirement must bind
that exact current policy revision. Installation does not itself discard any
class; deterministic class-specific application and transition evidence remain
open.

The engine foundation now also supplies a bounded effect-free retention planner
over one complete canonical record inventory. It selects the newest configured
ordinal cohort independently per class, closes the retained set transitively
over explicit dependencies, rejects missing or noncanonical dependencies before
producing a plan, and reports exact retained/discarded identities, class counts,
and frontiers. Planning mutates nothing; authoritative record owners still need
qualified class-specific application and persistence.

## Nonclaims

R107 does not provide legal advice, records classification, legal hold,
eDiscovery, data-subject workflow, guaranteed deletion, secure overwrite,
remote-recipient deletion, backup expiry, key destruction, jurisdiction policy,
cross-engine atomic retention, task scheduling, or agent harness behavior.
