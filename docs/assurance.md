# Assurance plan

Status: active for the v0.3 core, volatile service v0.1, store-neutral durable
Service/Host V2 boundary, and exclusive local Unix directory-store profile.

Agora uses a requirements-to-scenario-to-invariant evidence chain. A behavior is
claimed only when its requirement, closed public value, adversarial scenario, and
traceability row agree. Refusal scenarios assert the revision and event frontier
remain unchanged. Persistence scenarios compare canonical bytes after recovery.

The active `make check` gate contains:

1. fail-closed release-contract, conservative compatibility-baseline, and locked
   local-component supply checks with adversarial inventory mutants;
2. dependency and effect-boundary checks;
3. formatting and warning-denied lint;
4. all unit, integration, and bounded-model tests offline;
5. doctests and warning-denied documentation.

The release-contract gate requires every current package to deny registry
publication, pins Rust 1.95.0 with Clippy and rustfmt, and preserves the current
explicit sibling-Keystone source-layout assumption. The compatibility gate
closes over the exact ten-package workspace and nineteen canonical fixtures. These
are source-drift controls: they do not establish artifact provenance, registry
readiness, semantic compatibility, or a self-contained source archive.
The release-contract adversarial fixture separately proves that enabling root
publication, weakening the toolchain pin, or removing the security policy is
refused.
The supply gate closes over twelve local locked components and exact local-path
dependency declarations. It is not an advisory scan, legal conclusion, binary
SBOM, remote source proof, or artifact provenance claim.

The addressing model exhausts every author, recipient subset, and human/agent
kind assignment for a three-participant room. This establishes symmetry only for
that finite profile; it is executable counterexample search, not an unbounded
proof. Checkpoint mutation tests cover truncation, trailing data, profile tags,
host ceilings, and accepted single-bit changes.

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

Wire V2 freezes twelve canonical frames and round-trips every V2 command,
outcome, and refusal. It independently exercises every truncation of
representative request and reply frames, trailing data, V1/version mismatch,
unknown command and cursor tags, text/list ceilings, and content-safe Debug
surfaces. These fixtures establish only complete-frame codec behavior;
Host V2 evidence
is separate below and no transport is claimed.
The remote-client facade-only scenario also constructs a V2 client request and
round-trips a V2 Connect frame using only the facade's existing three-package
closure.

Host V2 scenarios authenticate and bind a participant-derived CreateRoom actor,
prove exact replay, erase registered versus unknown authentication failure,
retire only a substituting or revoked caller, preflight capacity and V1/version
mismatch before verifier invocation, and exercise both protocol disconnect and
unconditional transport-close cleanup. The embedded-facade scenario constructs
the complete V2 engine/service/host stack without direct package dependencies.
The verifier remains a trusted test double; no credential, Aegis, or network
adapter is qualified. Physical storage is separate and narrowly qualified below.

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
network filesystem, backup/restore, encryption, hostile same-UID process, or
multi-host availability claim follows.

Room change-feed scenarios bind a position to an exact completed snapshot,
filter stable outbox events by room, replay the same exclusive prefix, advance
without overlap, and surface both acknowledged-prefix loss and leave/rejoin
visibility discontinuity as explicit resnapshot gaps. Foreign client/room use,
invalid limits, debug redaction, every truncated private position, canonical
engine recovery, durable service-cursor recovery, every wire command/outcome/
room-event/gap variant, nested unknown tags, and frozen legacy-change fixture
close the represented pull path. They do not establish push delivery, per-client
event retention, read receipts, presence, or transport reconnection policy.

The additive identified-change scenarios prove cursor-profile isolation without
consumption, one stable ID across acceptance, history, feed, checkpoint
recovery, and SDK decoding, membership-gap continuity, content-safe Debug, and
all-event closed wire decoding. A separate canonical fixture freezes the new
accepted-message grammar while the legacy change fixture remains byte-identical.

The gate does not establish a credential adapter, transport security,
multi-host serialization, physical-device power-loss behavior, event delivery,
distributed availability, or orchestrator correctness. Those claims require
separate adapters and evidence.

