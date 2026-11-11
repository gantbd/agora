# Assurance plan

Status: active for the v0.3 core, volatile service v0.1, store-neutral durable
Service/Host V2 boundary, exclusive local Unix directory-store profile, and
bounded private local Unix transport composition.

Agora uses a requirements-to-scenario-to-invariant evidence chain. A behavior is
claimed only when its requirement, closed public value, adversarial scenario, and
traceability row agree. Refusal scenarios assert the revision and event frontier
remain unchanged. Persistence scenarios compare canonical bytes after recovery.

The active `make check` gate contains:

1. fail-closed release-contract, source-candidate contract, conservative
   compatibility-baseline, and locked local-component supply checks with
   adversarial inventory mutants;
2. dependency, effect-boundary, and immutable fuzz-corpus contract checks;
3. formatting and warning-denied lint;
4. all unit, integration, and bounded-model tests offline;
5. doctests and warning-denied documentation.

The release-contract gate requires every current package to deny registry
publication, pins Rust 1.95.0 with Clippy and rustfmt, and preserves the current
explicit sibling-Aegis/Keystone source-layout assumption. The compatibility gate
closes over the thirteen-package stable workspace, the standalone fuzz package,
and forty-five canonical fixtures.
These are source-drift controls: they do not establish artifact provenance, registry
readiness, semantic compatibility, or a self-contained source archive.
The release-contract adversarial fixture separately proves that enabling root
publication, weakening the toolchain pin, or removing the security policy is
refused.
The supply gate closes over seventeen local locked components and exact local-path
dependency declarations. It also regenerates and strictly validates the
canonical CycloneDX 1.5 source SBOM, including exact component and dependency
closure, licenses, publication denial, deterministic ordering, host-path
exclusion, and five adversarial mutants. It makes no advisory, legal
compatibility, binary-content, binary-SBOM, remote source proof, signature,
attestation, or artifact provenance claim.

The source-candidate contract pins a reduced two-package Keystone workspace,
requires a clean tracked Agora revision, separates sibling contextual revisions
from selected-subtree cleanliness, and closes every staged regular file through
one SHA-256 inventory. Its adversarial fixture rejects changed or undeclared
content, symbolic links, and a claim that upgrades local content closure into
commit provenance. This ordinary gate validates the mechanism; actual candidate
construction and isolated qualification remain separately recorded evidence.

The source-archive contract composes only after candidate verification. Its
dependency-free USTAR codec fixes paths, order, ownership, time, modes, padding,
and terminal framing under explicit entry and byte ceilings, while the adjacent
SHA-256 record closes payload changes. An independent fixture proves equal
construction, system `tar` listing/extraction, and refusal of checksum, framing,
valid-header traversal, and symbolic-input mutants. Actual exact-revision
archive qualification remains separate evidence.

Revision `46098fb3cd1a6f9164365d8c49cf7edbc0957ef7` then produced two
byte-identical candidates containing 291 checksummed payload files, including
the canonical seventeen-component source SBOM, archive tooling, and
storage-observation port. The manifest digest was
`21aae72df2634c215fc1075161d51e4046a66c5f06e341ab73e623cc15ad521f` and
the checksum inventory digest was
`3c5c1781b9592a92e700e8023b46f6096a547205328adbdfb9de1e087679c8f3`.
Both selected sibling trees were recorded as unclean, so only their staged byte
digests—not contextual revisions—carry the claim. One isolated candidate passed
every stable workspace target, doctest, and warning-denied documentation build
with the lock file, offline mode, and a fresh target directory. This proves
self-contained local source content for the stable graph, not remote origin,
clean-room dependency availability, an archive, binary reproducibility, or
publication readiness.

Those two candidates produced byte-identical 2,450,432-byte canonical USTAR
archives with 362 entries and SHA-256
`4437ccfa263ff913b36679706a5585be5170cc1abd5f32869db8f4d20544c0c5`.
One archive passed pre-extraction checksum/canonical validation, platform
extraction, exact rearchive, and the complete isolated source qualification.

Every frozen V1 and V2 wire frame is also a deterministic mutation corpus. The
offline suite checks every strict prefix, a trailing-byte framing violation, and
every single-bit mutation. A refused candidate remains closed; any accepted
candidate must re-encode to the exact candidate bytes. This bounded campaign
proves reproducibility and canonical acceptance for that corpus, not coverage
completeness or parser correctness for every possible byte string.
The frozen service and host checkpoints additionally receive 512 fixed-seed
single-bit mutations per profile. Each candidate must refuse recovery or recover
and re-encode byte-for-byte, extending the no-silent-normalization law through
nested private state, receipts, cursors, and authenticated sessions.

The transient owner has focused pure-state evidence for human/agent
symmetry, current-member-only observation, explicit expiry to `Unknown`, no
conversation revision effect, capacity refusal without eviction, bounded expiry
reclamation, monotone-clock refusal, exact as-of/generation cursor binding,
source and membership-transition retirement, redacted opaque handles, and an
empty restart projection. A saturation unit proves that generation overflow
sticks fidelity loss and fences publication/observation without blocking source
cleanup.

The separate typing owner has six integration scenarios and two
saturation units covering room, retained-root, and stable-topic targets;
human/agent symmetry; current membership and target authority; no conversation
revision effect; per-source target and total-signal ceilings; atomic refusal;
bounded expiry; clock regression; observer/room/target/generation/as-of cursor
binding; engine-revision staleness; source and participant retirement;
membership reconciliation; restart-to-inactive projection; and content-safe
diagnostics. Generation exhaustion fences publication and observation before
mutation while lifecycle cleanup remains available.

Ten Host-composition scenarios bind both owners to authenticated sessions and
qualified Host time. They cover participant-neutral publication and observation,
exact public replay/conflict/sequence, close/revocation/time-regression retirement
before refusal, leave reconciliation before its accepted reply and no rejoin
resurrection, bounded expiry, wrapper reconstruction to `Unknown`/inactive, and
session/room/target/snapshot/age cursor binding. The presence/typing Protocol/Wire grammar
has exhaustive value round trips, canonical shape and ordering checks, four
frozen frames, content-safe Debug, and deterministic
truncation/trailing/single-bit mutation closure.

The additive local collaboration transport has three more frozen control
fixtures and canonical fuzz oracles. Its real-socket scenario multiplexes
ordinary and transient frames through one authenticated Host session, proves
presence and typing publication, source retirement on EOF, `Unknown`/inactive
projection after reconnect, a distinct replay epoch for each connection, and
empty transient ownership plus a distinct epoch after daemon reconstruction
under a new server incarnation. The transport-supplied epoch bytes are directly
accepted by the command-neutral client facade. This closes the volatile R62
profile; R64 separately composes the same transient boundary with durable
conversation ownership. Network transport remains outside the evidence.

The combined durable-collaboration owner has focused suppression evidence. A
known store refusal after an authenticated transient offer restores the Host,
resets the volatile request namespace, and exposes no signal; an indeterminate
post-commit outcome fences the owner and also retires the signal. A twelfth
real-socket scenario composes the physical directory store, commits a room and
publishes both signal families, then reconstructs under a distinct server
incarnation through exact unlocked endpoint-residue rotation: the room remains
while presence is `Unknown`, typing is inactive, and the replay epoch differs.

The same focused schedule checks exact passive durability counts across four
commits, one known store refusal, and one outcome-unknown fence. Saturation unit
evidence proves counters preserve `u64::MAX` and set sticky fidelity loss; daemon
unit evidence proves durability and transport facts remain available after a
fence while the Host/Service runtime projection is absent. Successful durable
socket scenarios check the exact commit count including terminal cleanup.

The idle lifecycle scenario covers every externally relevant sequential state:
construction returns Ready, drain reaches exact zero-obligation completion,
later service refuses before accept, and explicit stop returns Stopped plus
Clean only after both pinned resources disappear. The ownership-replacement
scenario proves Fenced cannot be relabeled Draining and explicit stop returns
ResiduePreserved while all ambiguous endpoint evidence remains in place. A unit
check fixes default pre-construction health at Starting.

Six transient-client scenarios prove exact equal-command retry after an
outcome-unknown failure, definitely-not-sent progression, same-epoch state
restoration, malformed-tail and changed-epoch refusal, explicit reconnect/restart
rebind with returned uncertainty, fresh request identity, and content-safe
diagnostics. The remote-client facade constructs the client and codec without a
direct crate dependency; typing commands obey the same uncertainty fence. The
private collaboration daemon supplies that binding after authenticated connect.
Other transports must still bind the opaque replay epoch to both server
incarnation and exact authenticated session themselves.

Four additional public-Host scenarios drive the canonical frames through the
authenticated wrapper. They prove exact semantic replay without another domain
mutation, changed-input conflict, skipped-ID refusal, participant observation
without conversation revision change, bounded snapshot continuation at a later
Host time, cursor expiry, cross-session cursor refusal, and the rule that even a
refused newer-time offer fences another session's clock regression before
publication. The complete focused Host suite now has nine scenarios, and the
embedded facade constructs the profile without direct crate dependencies.

The standalone opt-in fuzz profile pins cargo-fuzz 0.13.2,
nightly-2026-11-01, and libfuzzer-sys 0.4.13. Its committed binary seed corpus
is regenerated from and checked against all thirty-four wire and fixed local
control fixtures plus the current engine, service, and host checkpoint fixtures.
`wire_canonical` drives all six public conversation/transient request/reply
decoders, five fixed-width handshake/session-control decoders, and the local
route envelope;
`checkpoint_canonical` drives all three semantic
recovery layers;
`outer_frame_canonical` drives the daemon's production length-prefix codec over
the twenty-nine complete-frame protocol seeds (the five fixed-width local
control fixtures are deliberately excluded). An accepted
candidate must re-encode exactly, and an accepted frame prefix must preserve its
unconsumed stream suffix. The qualified smoke ran 20,000 fixed-seed cases on all
three targets. The latest integrated local release-candidate campaign, for
revision `eacc767bcb27560372036de940f3f98b773200bc`, ran 1,000,000 fixed-seed
cases on each of those three targets. All campaigns use a 64-KiB input,
10-second per-input, and 2-GiB memory ceiling on Apple Silicon. This is bounded
coverage-guided counterexample search, not a coverage threshold, parser proof,
partial-kernel-read test, physical-storage test, or migration claim. The
ordinary offline gate verifies pins and seeds but does not misreport an opt-in
campaign as executed. It now also compiles every fuzz target against the
committed offline lock, and release qualification refuses any tracked-tree
mutation after the campaign as well as before it.

The addressing model exhausts every author, recipient subset, and human/agent
kind assignment for a three-participant room. This establishes symmetry only for
that finite profile; it is executable counterexample search, not an unbounded
proof. Checkpoint mutation tests cover truncation, trailing data, profile tags,
host ceilings, exhaustive engine-fixture bit changes, and fixed-seed enclosing
service/host changes.

The separately qualified local Moira receiver preserves the same participant
neutrality while projecting pending addressed events as passive facts. Its gate
proves explicit admission, changed-mapping conflict, restart and retention-
eviction recovery, and replay after a lost admission reply. That evidence belongs
to Moira and establishes composition behavior, not an Agora orchestration API.

The service scenarios exercise participant-derived authorship, member-only
snapshots, per-client request namespaces, matching retry, changed-request
conflict, disconnect tombstones, and receipt saturation before engine mutation.
The canonical V2 fixture detects accidental checkpoint drift and is recovered
and re-encoded during the gate.

A finite Service V2 schedule model enumerates all 216 three-step schedules over
owner speaking, actor speaking, deliberately stale speaking, leave, join, and
disconnect/reconnect. Each schedule runs with the second participant represented
once as human and once as agent. Replies, revision progression, event counts,
membership authority, stale fencing, retired-client refusal, and reconnect
namespace reset must match the explicit reference state after every step. This
is serialized bounded exploration, not thread-parallel linearizability proof.

A separate core lifecycle model enumerates all 4,096 four-step schedules over
owner/member speaking, deliberately stale member speaking, leave, rejoin, kick,
ban, and unban, then repeats each schedule with the member represented as human
and agent. After every command it compares exact normalized acceptance/refusal,
membership, ban state, revision, event count/order, message sequence, and the
two-message retained suffix against an independent model. Refusals additionally
preserve the complete room view plus both retained event copies. This covers
only the represented two-participant open-room vocabulary and schedule depth;
it is not an unbounded state or concurrency proof.

A second core model enumerates the same 4,096 four-step schedule space over
reply admission, archive/reopen, one reaction toggle, author edit, author
withdrawal, and unrelated speech under a three-message retention ceiling. It
repeats every schedule for human and agent root authors and checks exact
acceptance/refusal, refusal nonmutation, revision/event order, message suffix,
revision tombstones, thread state, reaction aggregate, and cleanup when the root
is evicted. This is limited to one root, one reaction key, two participants, and
depth four; it does not cover pagination, recovery, or concurrent execution.

`make soak` runs an opt-in deterministic 5,000-cycle Service V2 campaign. Every
cycle performs identified speaking plus exact replay, bounded history paging,
leave or owner kick, open rejoin, and retention checks; actor clients reconnect
periodically and the complete service checkpoint is recovered and required to
re-encode exactly every seventeen cycles. The default campaign completed under
the qualified toolchain. `AGORA_SOAK_CYCLES` may select a different positive
cycle count for diagnostics. This is repeated serialized behavior, not a
latency/throughput benchmark, crash-durability proof, or concurrency proof.

`make qualify-release-candidate` refuses staged or unstaged tracked drift and
records the exact HEAD before running the complete ordinary gate, all workspace
targets in release mode, the soak in release mode, and all three pinned fuzz
targets and verifies the tracked tree is unchanged afterward. Revision
`eacc767bcb27560372036de940f3f98b773200bc` completed the expanded profile
with 10,000 soak cycles and 1,000,000 fuzz cases on each target on Darwin
25.6.0 arm64 / macOS 26.6.2. A pass qualifies one source revision on
one host. It does not create
or qualify a distributable archive, checksum, signature, attestation,
reproducible binary, performance threshold, remote workflow, or other
architecture.

Wire scenarios freeze representative request/reply bytes, round-trip every
closed protocol variant, refuse every truncation and trailing byte, and exercise
frame/text/field/list ceilings, strict UTF-8, unsupported versions, and unknown
root and nested tags. This establishes codec behavior, not transport security.

Host scenarios exercise exact verified Connect, per-frame context revalidation,
service-derived authorship, matching replay, non-enumerating authentication
failure, client-handle substitution, context revocation, transport close,
host-time regression, capacity/malformed preflight, and tombstone-pressure
cleanup. The verifier is a trusted test double; no credential system or Aegis
adapter is qualified.

SDK facade scenarios compile and construct an engine/service/host stack using
only `agora-sdk-embedded`, and a protocol client plus canonical frame using only
`agora-sdk-client`. The dependency gate requires the embedded facade's direct
closure to be exactly engine/service/host and the client facade's direct closure
to be exactly client/protocol/wire. These checks establish packaging and audience
separation only; they add no runtime, transport, or release-provenance claim.

The bounded observation scenarios concatenate room, member, and retained-message
pages; distinguish foreign from stale cursors; enforce membership filtering and
positive configured limits; preserve human/agent kind as non-authoritative data;
surface the retained message interval after eviction; and assert content-safe
Debug output. This establishes an in-process core primitive only. Protocol V1
remains unchanged, and no service, wire, transport, durable cursor, or cross-
restart claim follows.

Protocol/Service V2 scenarios exercise the complete participant-bound room
lifecycle through create, failed uninvited join, invite, join, role delegation,
moderator invite, human join, agent-authored addressed message, leave, and member
observation. Separate scenarios prove exact replay, changed-input conflict,
client-bound one-shot cursors, cross-client non-enumerating refusal, staleness,
cursor saturation, validation before receipt consumption, and disconnect
reclamation. Protocol and page Debug tests reject conversation identifiers and
contents. The client test fixes the independent monotone V2 request namespace.
No V2 authenticated host, persistence, transport, or uncertainty behavior is
claimed.

Wire V2 freezes eighteen canonical frames and round-trips every V2 command,
outcome, and refusal. It independently exercises every truncation of
representative request and reply frames, trailing data, V1/version mismatch,
unknown command and cursor tags, text/list ceilings, and content-safe Debug
surfaces. These fixtures establish only complete-frame codec behavior; Host V2
and the bounded local transport evidence are separate below.
The remote-client facade-only scenario also constructs a V2 client request and
round-trips a V2 Connect frame using only the facade's existing three-package
closure.

Host V2 scenarios authenticate and bind a participant-derived CreateRoom actor,
prove exact replay, erase registered versus unknown authentication failure,
retire only a substituting or revoked caller, preflight capacity and V1/version
mismatch before verifier invocation, and exercise both protocol disconnect and
unconditional transport-close cleanup. The embedded-facade scenario constructs
the complete V2 engine/service/host stack without direct package dependencies.
The verifier remains a trusted test double in the host scenarios. The separate
Aegis adapter scenarios map the exact public session, grant, time, namespace,
resource, and action; reject participant mismatch before authority I/O; erase
refusal, expiry, malformed success, and port failure; retire host/service state
on revoked revalidation; and keep diagnostics content-safe.

The exceptional operator-restore constructors additionally require an injected
authority port after checkpoint and transient-limit validation but before the
sanitizing CAS. Deterministic store evidence proves the request carries the
exact key, scope, and loaded revision, and that refusal is one content-free
class with zero mutation and no owner. The Aegis restore adapter maps that port
to `Administer`, prebinds key/scope, rejects mismatch without authority I/O,
erases refusal/expiry/wrong-action/port failure, and retains the accepted
decision plus Agora revision as non-atomic correlation evidence.

Thirteen real Unix-socket daemon scenarios compose that adapter with Host V2. They
verify mode-0700 parent enforcement and mode-0600 endpoint creation, Aegis
authorization on Connect and every later frame, exact pre-authentication
handshake values, incompatible-profile refusal with zero downstream work, exact
replay, EOF cleanup, shared participant-mismatch refusal
without authority I/O, erased authenticator failure detail, post-reply
request-budget cleanup, oversized outer-length
refusal before allocation or host admission, existing-path refusal, pinned
replacement preservation, and content-free counters. I/O deadlines are present
in the configured profile; the tests do not claim timing determinism. Credential
and Unix-peer authentication, hostile same-UID parent defense, incarnation
uniqueness, concurrency, operational drain/crash recovery,
cross-disconnect replay, and network transport remain
unqualified. Physical storage is separate and narrowly qualified below.

The fixed-width handshake codec is owned by `agora-wire`, not the daemon. Five
canonical fixtures close both ClientHello/ServerHello pairs and the
session-control hello; ordinary tests
reject length, magic, profile, zero-limit, and zero-incarnation drift and require
exact reconstruction. The client SDK facade constructs and decodes the profile.
The pinned wire fuzz target now applies exact re-encoding oracles to all five
local control decoders, the route envelope, and all complete-frame decoders. This establishes
closed profile selection and visible server-lifetime evidence, not credential
authentication, compatibility-range negotiation, incarnation provenance,
resume, or replay safety.

The endpoint-scoped lifetime lease adds one exact mode-0600 record containing
the declared incarnation beneath the already trusted mode-0700 parent. The
daemon holds an exclusive kernel lock, pins the parent and record identities,
checks exact record bytes and endpoint ownership before and after accept, and
fences readiness before authentication if any resource is replaced. A live
owner and unlocked residue are distinct typed refusals. Exact clean teardown
removes socket and lease; uncertain or replaced resources are preserved. The
frozen 24-byte record and real-filesystem scenario cover canonical bytes, live
exclusion, stale residue, replacement fencing, zero downstream work, and
non-destructive teardown. A separate injectable generator validates and reads
the Unix `/dev/urandom` character device, rejects zero and the immediately prior
value within a fixed eight-read bound, and erases source failures to their I/O
kind. Pure retry/exhaustion/failure cases plus one real socket prove the exact
generated value reaches the record and ServerHello. This does not prove
uniqueness, recover service state, or establish multi-process handoff.

Explicit endpoint recovery is a separate constructor and evidence path. Its
real-filesystem scenario refuses a live owner and equal, wrong, or malformed
prior incarnation without mutation; then locks exact residue, removes only the
validated stale socket, rotates to a distinct record, binds a fresh zero-use
Host, advertises the next incarnation, and cleans up normally. The evidence does
not cover every I/O interruption point or automatic restart; a failed mutating
recovery remains operator-visible rather than a claimed success. Durable Host
state is a separately selected owner, not something the endpoint-residue path
infers or adopts. Its clean teardown/reopen evidence is tracked independently
and does not establish reconnect replay.

The daemon's composed operational snapshot preserves transport, host, and
service counters as three named measurement boundaries. The successful socket
scenario proves exact connect/execute/replay/disconnect counts, identical
repeated reads, and content-safe Debug output; a unit scenario forces counter
saturation and proves the sticky fidelity indicator without wraparound. This is
passive process-local diagnostic evidence, not durable audit, metric export,
latency measurement, concurrency evidence, or an SLO.

The idle-drain scenario uses a real bound endpoint and proves that the first
transition clears readiness, records draining, and is irreversible; repeated
transition is inert; and every later serve attempt returns before socket accept,
authenticator/Aegis invocation, Host V2 open, or Service V2 connect. The daemon
retains its pinned endpoint until explicit stop removes it and returns clean
terminal evidence. Exclusive sequential ownership excludes concurrent in-flight
drain, deadlines, signals, and service-manager behavior from this evidence.

Service V2 persistence scenarios recover byte-identical engine state and pending
outbox, active clients, retained exact replies, and live room-page continuation.
They replay a committed mutation without advancing the engine, preserve
changed-input conflict, continue mutation from the recovered revision, and
refuse every truncation plus foreign scope, reduced host ceiling, future profile,
and trailing data. The service record and its private cursor subrecords have
content-safe Debug surfaces. A representative complete service checkpoint is
frozen as a canonical fixture.

The store-neutral durable boundary scenarios use a deterministic
checkpoint-store fault model.
They prove a fresh mutation is not published before a durable next-revision
receipt, a committed lost reply recovers as exact replay without another event
or store call, a known store refusal reconstructs the pre-offer service, and
indeterminate-before versus indeterminate-after commitment both fence until load
resolves the store. Weak acknowledgement and store error never publish. This is
store-port and crash-point evidence. The local directory profile below is a
separate, deliberately narrow physical composition qualification.

The directory adapter injects interruption after pending creation, completed
write, pending sync, head rename, and directory sync. Each injected caller is
fenced with an indeterminate result. Reopening the first three states refuses
the surviving `.pending` residue; reopening either post-rename state recovers
the exact candidate revision. This closes the adapter's process-boundary state
machine, not controller-cache or hardware power-loss behavior.

The same real-filesystem suite qualifies a passive physical-owner observation.
Reopen begins with the recovered revision and zero process-local counters;
validated loads, compare-and-exchange offers, commits, known refusals,
indeterminate publications, and integrity fences advance at their exact
boundaries. Repeated reads are equal and perform no filesystem operation. A
forced `u64::MAX` offer proves sticky saturation without wraparound, while
Debug output excludes the directory path, key, checkpoint content, and record
size. These facts reset on reopen and do not establish durable metrics or
current resource integrity beyond the last operation.

A four-case active-resource schedule mutates the live directory mode, removes
the pinned lock, creates pending residue, or removes the current head. Each
first operation returns its exact unsafe/lost/residue error, changes the passive
state to Fenced, and records exactly one resource fence. A second operation
returns Fenced without another filesystem-validation count. Existing head
replacement evidence covers stamp substitution under the same law.

A deterministic pending-open fault passes active validation, fails the exact
exclusive-create boundary with PermissionDenied, and checks the specific I/O
error, absent pending/head files, Fenced observation, one offer, zero commits,
zero indeterminate publications, and one resource fence. A later load returns
Fenced without another fence count. Existing injected write/sync/rename and
post-rename cases cover the indeterminate half of the operational-I/O law.

The effect-free storage-observation port is dependency-closed over only the
Keystone checkpoint-store contract. Directory evidence reads the same passive
snapshot through both the concrete and generic ports and propagates saturation.
Two real-socket daemon scenarios cover ordinary durable and combined
durable/transient owners: after initialization, mutation, cleanup, reopen, and
recovery they assert separate equal runtime/durability views, exact physical
load/offer/commit/revision facts, repeated-read passivity, and aggregate
fidelity. A pure unit case proves physical fidelity loss reaches the daemon even
when transport and durable-publication counters remain exact.

Client V2 scenarios distinguish definitely-not-sent from outcome-unknown,
retain and resend the exact request identity only for an equal command, fence a
different command without transport access, keep uncertainty through a later
not-sent retry, and return the unresolved request on explicit abandonment or
port extraction. They establish no automatic retry, durable client state, or
session reconnection behavior.

Host checkpoint scenarios freeze the complete authenticated record, recover the
exact session/service receipt and continue only after fresh verifier
revalidation, compare canonical bytes, and refuse every truncated prefix plus a
foreign scope. Durable-host fault scenarios prove authenticated Connect and
service client allocation commit together, indeterminate-after-commit recovery
replays the same request without a duplicate room event, known store refusal
rolls back both namespaces, and revoked-session cleanup is durable before the
Unavailable result is visible. A canonical fixture pins the combined
record.

The private-directory scenarios exercise a real Unix filesystem: initialization,
two CAS revisions, close/reopen recovery, wrong-revision and capacity refusal,
exclusive locking, private modes, lock symlinks, crash residue, header corruption,
and live head replacement. A fixture freezes the `AGDIRCP` bytes. A full
`DurableHostV2` composition then closes and reopens the store, recovers the
authenticated host/service unit, and proves same-time exact replay adds neither
an engine event nor a store offer. The adapter's durable claim is limited to
successful temporary-file write and sync, atomic rename, and directory sync on a
single-owner local Unix profile. No hardware power-loss, controller cache,
network filesystem, encryption, hostile same-UID process, or multi-host
availability claim follows. Two additional offline scenarios acquire the source
lock, prove the backup is byte-identical to the canonical head, verify mode 0600
and checksum closure, restore revision 1 into a new private directory, and
advance it by CAS to revision 2. They refuse a live or empty source, existing
output/destination, corruption, and unsafe backup mode before widening state.
The evidence establishes local exact-record backup/restore, not encrypted or
remote transport, origin authentication, rollback resistance, scheduling,
retention, or disaster-recovery objectives.
Two further deterministic schedules inject interruption after each of four
export and six restore persistence boundaries. Export leaves either a corrupt
new private file or an independently verifiable exact record while returning
only `Interrupted`. Restore leaves an empty reopenable destination, one of three
explicit `.pending` residue states, or an exact post-rename revision-1 checkpoint
that can advance by CAS to revision 2. This closes process-visible residue
classification without claiming automatic remediation or device-level crash
durability.
Host-level operator-restore evidence starts from an authenticated session with a
committed room mutation, preserves the conversation revision and event count,
invalidates the old session without verifier I/O, continues Host and Service
identity frontiers, and refuses activation for known refusal, indeterminate
before/after commit, weak acknowledgement, and store error. The transient
variant proves invalid configuration performs no store offer. A real directory
composition exports, restores, commits the sanitized checkpoint, reopens it,
proves the old session remains inert after another process restart, and admits a
fresh higher-identity connection.
The local operator-startup composition adds an exact backup/restore, retained
pre-restore session, durable transient Host sanitization, deterministic bounded
distinct-incarnation generation, exact unlocked endpoint-residue rotation, and
a real collaboration handshake. It proves the old session causes no authority
I/O, the new client identity advances, the replay epoch embeds the generated
incarnation, and the retained room remains readable. It does not inject failure
between these independently recoverable steps or establish cross-resource
atomicity.
A four-case subprocess campaign exits without Rust cleanup immediately after
physical restore, Host sanitization, incarnation generation, or endpoint
rotation. It checks exact store revision/client activity and exact endpoint
record/socket residue after every exit. Conservative retry either repeats the
safe sanitization or recovers it ordinarily, rotates from the observed prior
incarnation, completes a fresh handshake, advances client identity, and reads
the retained room in all four cases. This is inter-step process evidence, not
intra-operation filesystem fault injection.
Endpoint-rotation evidence injects `Interrupted` after the next record is
written, after it is synchronized, after stale-socket removal, and after parent
synchronization. No injection reports success. Every record is exactly the
declared next incarnation; the first two states retain the validated old socket
and the latter two have no socket. All four retry under that recorded
incarnation to another distinct exact record. A public daemon test binds from
exact unlocked record plus missing socket, serves a fresh handshake and
connection with zero prior Host/Service offers, and tears down cleanly.

Room change-feed scenarios bind a position to an exact completed snapshot,
filter the independent recent-change journal by room, replay the same exclusive
prefix, and advance without overlap. Delivery acknowledgment is proven not to
consume pull history, including across canonical recovery; only journal eviction
surfaces the exact change-retention gap. Leave/rejoin remains a distinct
visibility discontinuity requiring resnapshot. Foreign client/room use,
invalid limits, debug redaction, every truncated private position, canonical
engine recovery, durable service-cursor recovery, every wire command/outcome/
room-event/gap variant, nested unknown tags, and frozen legacy-change fixture
close the represented pull path. They do not establish push delivery, per-client
retained copies, read receipts, presence, or transport reconnection policy.

The additive identified-change scenarios prove cursor-profile isolation without
consumption, one stable ID across acceptance, history, feed, checkpoint
recovery, and SDK decoding, membership-gap continuity, content-safe Debug, and
all-event closed wire decoding. A separate canonical fixture freezes the new
accepted-message grammar while the legacy change fixture remains byte-identical.

The one-level thread scenarios prove human/agent-neutral reply acceptance,
nested/unknown/evicted-target atomic refusal, independent root/reply pagination,
root/profile cursor isolation without consumption, stable receipt/page/feed IDs,
and relationship preservation through withdrawal, redaction, checkpoint, and
exact replay. All threaded event variants and optional-root tags round-trip;
semantic page-shape checks, nested unknown tags, every truncation, Debug safety,
SDK construction, and two canonical wire fixtures close the public profile.
Canonical profile-15 engine and profile-18 service/host fixtures close recovery.

Thread lifecycle scenarios prove root-author and owner authority, foreign-author
refusal, archive reply fencing, reopen admission, current status, exact replay,
all-profile feed evidence, tombstone independence, and recovery. Closed command,
outcome, state, refusal, capability, and archive/reopen event tags are exercised
with truncation, unknown-tag, Debug, SDK, and two new canonical frame fixtures.

Reaction scenarios prove human/agent-neutral set semantics, current-member actor
binding, repeat/absent transition atomicity, fixed key and distinct-aggregate
bounds, canonical counts plus caller membership, tombstone preservation,
retention cleanup, exact replay, all-profile feed attribution, and checkpoint
continuation. Closed command, outcome, capability, status, and event tags are
round-tripped; Debug/SDK evidence and two canonical frames close the public
surface without claiming authority, acknowledgement, delivery, or completion.

Message revision scenarios prove original-author authority for humans and
agents, append-only numbering through a fixed saturation ceiling, immutable
identity/addressing/reply/reaction relations, atomic refusal, revision-bound
pagination, exact receipt replay, every-feed attribution, recovery continuation,
and tombstone scrubbing of retained revision and event bodies. Closed protocol,
wire, Debug, SDK, two canonical frames, and profile-16/18/18 checkpoints close
the retained short-message correction surface without claiming collaborative
document editing or physical erasure.

Topic scenarios prove opaque deterministic identity, owner/moderator creation
authority without participant-kind privilege, bounded canonical naming and
capacity, immutable message relation and reply inheritance, topic/profile-bound
pagination, exact replay, four-profile projection, and cursor continuation after
recovery. Closed protocol/wire tags, malformed relation checks, Debug/SDK
construction, expanded capability bytes, and profile-16/18/18 checkpoints close
the retained topic surface without claiming subscriptions, notifications,
unread state, workflow resolution, or execution authority.

Topic-rename scenarios prove owner/moderator and human/agent-neutral authority,
atomic same-name/collision/unknown/bounds refusals, stable identity and retained
message relation, actor derivation, exact replay/conflict behavior, and prior/
new-name attribution in all four change profiles. Frozen command, receipt, and
change frames plus profile-16/18/18 engine/service/host fixtures close canonical
wire and recovery consequences without claiming aliases, move, merge,
resolution, deletion, subscription, or connector synchronization.

Topic-resolution scenarios prove owner/moderator and human/agent-neutral
authority, atomic same-state/unknown/stale refusals, new-root and inherited-reply
fencing, reversible admission, and stable identity/history/read-scope
preservation. Current state is projected by a separate participant-bound status
observation; actor/prior/new state
is retained in all four feeds. Frozen resolve, receipt, and change frames,
closed capability/refusal tags, SDK construction, and profile-16/18/18 nested
checkpoints close the bounded profile. Revision
`956bb2308f7324a61e36b956cacbdb8edc97f600` passed the complete R53
qualification without claiming task completion, approval, notification,
connector synchronization, move, merge, or deletion.

Private read-position core scenarios prove room/thread/topic scope validation,
human/agent-neutral self-state, monotone and idempotent advancement, atomic
stale/regression/target refusal, membership inaccessibility, retention survival,
and zero domain-revision or outbox effect. Canonical profile-14 engine and
profile-18 service/host fixtures retain exact markers through nested recovery;
truncation, profile, scope, ceiling, and canonical-state checks remain closed.
An exhaustive finite model additionally runs every length-three schedule over
three ordered targets for both participant kinds and all three scope variants,
checking the monotone reference state plus revision/outbox invariance after each
operation. It is bounded counterexample search, not an unbounded proof.
Participant-bound public service, wire, SDK, and durable lost-reply evidence is
covered by actor-derived service execution and replay tests, exhaustive closed
wire round trips and a frozen marker fixture, facade-only SDK construction,
exact receipt recovery, and durable commit-before-reply recovery without a
second store write.

The gate does not establish a credential adapter, transport security,
multi-host serialization, physical-device power-loss behavior, event delivery,
distributed availability, or orchestrator correctness. Those claims require
separate adapters and evidence.

