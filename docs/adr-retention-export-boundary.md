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
state transitions and class-specific policy application remain separate work;
the later sections record qualified export materialization, authority
composition, and physical adapter reclamation slices.
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

Service V2 now also owns a persisted monotone receipt-state revision. It starts
absent, advances exactly once when a new exact request/refusal receipt is
retained, when disconnect removes one or more retained receipts, or when one
atomic retirement batch replaces receipts with tombstones. Exact replay,
changed-input conflict, inert refusal, connection changes without receipt
removal, and policy installation do not advance it. Canonical Service profile
32 stores the revision and an explicit exhaustion fence; Host profile 29 nests
that Service profile without interpreting the revision. Reaching `u64::MAX`
does not wrap; the next receipt-state mutation permanently makes exact receipt
export unavailable while still permitting cleanup. Profiles 24 through 31
recover with no receipt-source revision and preserve their historical bytes;
the native receipt source remains unavailable until a post-migration
receipt-state mutation establishes the new domain. This revision is source
identity substrate, not by itself an export, authorization, or custody grant.

`RequestReceiptSourceV1` now turns that identity domain into one narrow
owner-native `RequestReceipts` snapshot record. Canonical `AGARRC1` bytes contain
only ordered live V2 request/reply wire receipts and ordered receipt-retirement
tombstones. The record ordinal is the receipt revision; its frontier marks all
earlier receipt generations superseded without claiming that external copies
were erased. Recovery revalidates bounds, ordering, outer keys against decoded
wire identities, canonical wire re-encoding, tombstone class/reason/bindings,
framing, and exact whole-record re-encoding. Missing legacy identity, disabled
policy, saturation, wire failure, and record limits refuse before a source
fragment escapes. The administrative materializer composes it with the Engine
authoritative-change source only at one engine and policy revision. Authority,
visibility, custody, and physical reclamation remain enclosing obligations.

`RoomProjectionSourceV1` supplies a separate Engine-owned generation for the
current public room projection. Canonical `AGAROM1` bytes contain only the
referenced participant identities, display names, and human/agent kinds plus
current room metadata, membership and admission controls, retained messages and
revisions, thread lifecycle, reactions, shared pins, topics, and public poll
aggregates. Individual ballots, read positions, attention preferences, private
saves, reports, admission-clock observations, publication receipts, delivery
journals, and Service receipts are absent. One record represents the complete
generation: its ordinal is the engine revision successor and its frontier marks
all earlier generations superseded. Recovery revalidates bounds, canonical
ordering, participant references, aggregate shape, framing, and exact
re-encoding. An empty engine has a frozen generation, and equal public poll
aggregates remain byte-identical when only the private ballot owner differs.
This administrative source grants no room visibility or custody authority.

The Engine now also owns a persisted monotone private-state revision, separate
from the room/domain revision. Effective read-position, attention, private-save,
poll-ballot, message-report, and report-review mutations advance it exactly
once; idempotent observations, invalid changes, and ordinary room mutations do
not. Poll eviction advances it only when private ballots are actually removed.
Canonical Engine profile 151 stores the revision and its explicit exhaustion
fence while preserving profiles 20 through 150 byte-for-byte. Legacy recovery
has no private source identity until the next effective private mutation.
Reaching `u64::MAX` never wraps: the next mutation permanently fences an exact
private-state source while remaining available for cleanup and ordinary use.
This identity domain is prerequisite substrate, not itself an export or an
authorization decision.

`PrivateParticipantStateSourceV1` turns that identity domain into one narrow
Engine-owned `PrivateParticipantState` generation. Canonical `AGAPRV1` bytes
contain only ordered read positions, non-default attention preferences, private
saves, individual poll ballots, and private message reports and reviews. They
exclude the public room projection, request and publication receipts, delivery
and change journals, federation state, clocks, and operational copies. The
record ordinal is the persisted private-state revision; its frontier marks all
earlier private generations superseded without claiming that any external copy
was erased. Recovery revalidates bounded framing, canonical key and selection
ordering, report/review consistency, source revisions, and exact whole-record
re-encoding. Missing legacy identity, disabled policy, saturation, and byte
limits refuse before a fragment escapes. The source is administrative-only:
participant-private export still requires that exact participant or a
separately defined administrative authority, and custody and physical
reclamation remain enclosing obligations.

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

The first owner-native application primitive applies only the current policy's
`AuthoritativeChanges` bound to the Engine-owned retained-change journal. It
preflights the exact engine revision and all record identities, then prunes the
oldest excess entries and returns the same ordinal frontier exposed by
`AuthoritativeChangeSourceV1`. Disabled retains none; equal reapplication is an
inert replay; stale policy is nonmutating; canonical Engine recovery preserves
the new discard edge. It does not acknowledge or prune the delivery outbox,
advance the domain revision, apply another class, or keep the bound continuously
enforced across later domain writes. Service V2 now binds application to its
exact installed policy and engine revision. Host revalidates the administrative
session and obtains a fresh authority decision over the class bound, prior
discard ordinal, and retained count before mutation. Durable Host commit-gates
the complete checkpoint: known refusal restores the journal, while committed
restart preserves the exact gap and still-pending delivery outbox. Later-write
reapplication and the other owner-native application profiles remain open.

The engine export foundation now materializes one complete canonical record
inventory into bounded `AGEXCH1` chunks and an `AGEXMF1` manifest body. It
requires canonical record order, exact included-class/frontier agreement,
nonempty bounded payloads, whole-record chunk framing, and a named SHA-256
digest profile. Chunk payload diagnostics are redacted. The digest function is
injected into the no-I/O engine. The pure `agora-sha256` adapter supplies that
named profile and is qualified by standard vectors, padding-boundary coverage,
exact Agora materialization/recovery, and payload-mutation refusal. Equal inputs
produce byte-identical chunks and manifest evidence. `ServiceV2::export_source`
now supplies one narrow projection: the exact canonical complete Service V2
checkpoint becomes one administrative `OperationalCopies` record only when the
current policy enables that class. Its ordinal is the current engine revision's
successor, its frontier is exact, and the embedded checkpoint remains independently
scope-bound and recoverable. Nested checkpoint content is not reclassified as
room, change, receipt, private, or federation records. Other source projections,
visibility authorization and concrete provider composition remain open. The
trusted embedded SDK names the effect-free retention-authority and
export-custody contracts; the participant protocol and remote-client SDK remain
deliberately free of administrative export operations.
Owner-native source composition is now a closed engine law rather than an
application convention. `ExportSourceFragmentV1` binds exactly one class, its
records and frontier, and one engine/policy revision. Composition requires
canonical class order, refuses duplicate ownership and cross-revision input,
and derives the manifest class set and empty frontiers rather than trusting the
caller to restate them. The existing administrative operational-copy source
uses this path. This substrate permits future Engine, Service, federation, and
operator-owned projections to compose without importing one another's storage
or relabeling a complete checkpoint; it does not itself create the remaining
class-specific projections.
The first narrower owner-native projection is `AuthoritativeChangeSourceV1`.
It reads only the engine's independent retained-change journal, maps event
sequence `n` to nonzero record ordinal `n + 1`, translates the journal's exact
discard edge into the class frontier, and binds the current engine and policy
revisions. Each `AGACHG1` payload reuses the canonical event grammar under
minimal exact feature bits and recovers with sequence/revision, bound,
truncation, trailing-byte, and canonical re-encoding checks. A frozen human/
agent registration case and deterministic single-bit mutation closure cover the
record. This source is administrative-only and performs no room visibility,
authority, materialization custody, or physical retention action. Pending
delivery events are not a second source: the retained-change journal is the
authoritative export owner for this class.
An engine-owned bounded export registry now gives the materialized result exact
request-identity semantics: equal retry replays the complete manifest and chunk
bytes, changed materialization conflicts, saturation refuses before mutation,
and recovery accepts only canonical request order. The registry is volatile
substrate until Service and Host checkpoint profiles durably compose it.
Recovery of one materialization now rechecks canonical chunk indices, declared
lengths, framing domains, every chunk digest, and the manifest digest through
the same named digest implementation. A structurally valid payload mutation is
therefore a digest refusal rather than silently recovered export state.

Export custody is a separate service owner, not a field in the
conversation `ServiceV2` checkpoint. An export is a newly retained copy with a
different access and retention lifecycle; folding it into the authoritative
room checkpoint would make conversation recovery retain export payloads and
would erase that distinction. The adopted `agora-export-service` reuses the
engine registry and materialization recovery laws with its own capacity and
checkpoint scope. It binds admission to exact current engine/policy revisions,
persists canonical request-ordered materializations behind a whole-checkpoint
digest, and recovers only after revalidating nested chunk and manifest digests.
The separate `agora-export-durable` owner gives that checkpoint its own store
key and revision line. Exact durable receipts commit custody, equal replay
suppresses another offer, known refusal rolls back, and unknown or weak outcomes
fence until fresh recovery. The service also owns bounded export retirement
independently from live custody capacity. One exact manifest-derived identity
atomically replaces its live materialization with a request non-reuse
tombstone; equal retry replays, changed identity conflicts, missing live custody
refuses, and saturation leaves the live copy intact. Additive checkpoint V2
persists the retirement limit and canonical tombstones while V1 recovery and
unchanged empty V1 emission remain compatible. Other class-specific source
projections, authorization, and digest-provider composition remain enclosing
obligations.
The participant protocol remains free of administrative export commands.
The separate `agora-export-local` application now closes the bounded private
local digest/custody/store composition: it selects `agora-sha256`, gives export
custody its own private-directory register and revision line, verifies recovery
after dropping every live owner, and proves exact retry is replay rather than a
second copy. It additionally commits retirement, drops every live owner,
reopens the tombstone, replays without another store offer, and refuses request
reuse. This qualifies custody persistence and logical retirement only. The
single-head directory mechanism remains ineligible for physical reclamation
evidence.
The same application now also exposes a separately typed generational custody
composition. Integrated evidence commits the initial empty custody checkpoint,
one exact export, and its logical-retirement tombstone as three immutable
generations; drops every owner; reopens and digest-validates revision 2; then
prepares and journals a bounded plan naming revisions 0 and 1 before unlink.
Both returned facts are exactly `GenerationUnlinked`, exact replay performs no
second unlink, and a later fresh owner observes only the validated successor.
This composes qualified custody and physical reclamation ordering without
turning logical retirement into reclamation authority or unlink evidence into
secure erasure.
Revision `1b48be4ee16d60c37aa74d18b5b8fce72fec6b0a` commit-pins that integrated
slice on Darwin 25.6.0 arm64 / macOS 26.6.2: the ordinary gate, every
release-mode workspace target, 10,000 deterministic soak cycles, and 1,000,000
fixed-seed cases on each of the three fuzz targets completed with unchanged
tracked bytes. That campaign qualifies the represented private-local path, not
the still-open class-specific source profiles, federation-copy retention,
broader process/power-loss/storage-hardware campaigns, or secure erasure.
The effect-free `agora-export-custody-port` now separates Host from that durable
implementation. `HostV2` revalidates the administrative session, checks the
prepared manifest against the current engine and installed-policy revisions,
and obtains a fresh authority decision over the exact manifest identity before
admission. Stale local bindings stop before authority; denied authority stops
before custody. The durable owner implements the port without exporting
Keystone vocabulary, preserving exact commit/replay, definite noncommit,
unknown outcome, and fence meaning.
The port exposes retirement separately from admission, including missing and
retirement-capacity refusals. `HostV2` revalidates the administrative session,
requires the retirement source and policy to remain current, obtains a fresh
authority decision over the complete digest-bound retirement identity, and only
then calls custody. Humans and agents remain symmetric. No layer translates
this logical transition into reclamation or erasure evidence.

The `agora-storage-directory` V1 profile is not a reclamation implementation.
Its compare-exchange renames a new record over the sole
head, unlinking the prior directory entry before a separately reopened successor
can be validated. It remains unchanged and must not be relabelled as
checkpoint-before-reclaim evidence.

The same adapter package now supplies a separately typed generational profile.
Every successful CAS retains an immutable checksummed generation in addition to
the synchronized head. Live-generation capacity backpressures publication
instead of silently deleting recovery material. Reclamation is unavailable to
the publishing owner: it requires closing and reopening the exact current head,
then prepares an engine-owned bounded plan naming only older generation IDs and
the digest of that reopened replacement. A private canonical plan journal is
synchronized before any unlink; partial progress resumes from the same plan
after a fresh reopen; the completed receipt makes exact retry replayable. Only
after every named unlink and directory sync does the adapter return
`GenerationUnlinked` evidence. Resource replacement, journal corruption,
foreign/stale plans, saturation, and unvalidated successors fail closed. This
is local Unix unlink evidence only, never provider attestation or secure
erasure.

## Nonclaims

R107 does not provide legal advice, records classification, legal hold,
eDiscovery, data-subject workflow, guaranteed deletion, secure overwrite,
remote-recipient deletion, backup expiry, key destruction, jurisdiction policy,
cross-engine atomic retention, task scheduling, or agent harness behavior.
