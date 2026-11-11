# Changelog

All notable changes to Agora will be recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Released versions will
follow Semantic Versioning once a public release contract is selected.

## [Unreleased]

### Added

- Added offline canonical backup, verification, and restore for the private
  directory store. Export holds the source lock and synchronizes a new private
  exact-record file; restore validates before creating a new store, preserves
  revision continuity, and leaves failed post-creation work explicitly
  operator-visible rather than claiming rollback. Revision
  `4ef205466ddd434fe9f05400534c7a00b08c8abb` passed the integrated release
  profile with 10,000 soak cycles and 1,000,000 cases on each fuzz target.
- Added deterministic interruption injection at all four export and six restore
  persistence boundaries, proving no false success and classifying each visible
  result as invalid/verifiable backup, empty destination, pending residue, or
  exact post-rename recovery with preserved revision continuity. Revision
  `cd4088ea5297655b84b4caf80f31e4284be14128` passed the integrated release
  profile with 10,000 soak cycles and 1,000,000 cases on each fuzz target.
- Added explicit operator-restore recovery for durable Host and transient Host
  profiles. It preserves conversation state and identity frontiers, retires all
  recovered sessions, disconnects private Service client state, and durably
  commits the sanitized checkpoint before returning an active owner. Revision
  `25c12f3c6f8526ba98dd490c7835b8c1cca9c05a` passed the integrated release
  profile with 10,000 soak cycles and 1,000,000 cases on each fuzz target.
- Composed physical restore and durable session sanitization with bounded
  distinct-incarnation generation, exact endpoint-residue rotation, and a fresh
  collaboration handshake as an explicit non-atomic local operator procedure.
  Revision `2649605b00a33167b4edb1555720a18b0596181d` passed the integrated
  release profile with 10,000 soak cycles and 1,000,000 cases per fuzz target.
- Added real process termination after every ordered operator-restore boundary,
  exact partial-state classification, and conservative successful resume from
  each restored/sanitized/generated/endpoint-rotated class. Revision
  `33a2f61093bae785767ec9d817ae77e6e5be189c` passed the integrated release
  profile with 10,000 soak cycles and 1,000,000 cases per fuzz target.
- Reordered endpoint rotation to synchronize the next incarnation record before
  stale-socket removal, made exact-record/missing-socket residue resumable, and
  classified four injected persistence boundaries with successful exact retry.
  Revision `f2ba3a9e35ce48173e83a44a4ed1a5e1132d006e` passed the integrated release
  profile with 10,000 soak cycles and 1,000,000 cases per fuzz target.
- Added mandatory exact key/scope/revision authority before operator-restore
  sanitization, plus an Aegis `Administer` adapter that retains decision/revision
  correlation evidence without claiming atomic cross-engine recovery. Revision
  `89fec4c12a7a4ac1d4f51b92a47a5c137344816e` passed the integrated release profile with 10,000 soak cycles and
  1,000,000 cases per fuzz target.
- Added no-unwind subprocess termination after every endpoint-rotation
  persistence boundary, proving the exact record/socket residue classes unlock
  and resume to another distinct incarnation. Revision
  `eacc767bcb27560372036de940f3f98b773200bc` passed the integrated release profile with 10,000 soak cycles and
  1,000,000 cases per fuzz target.

- Added an injectable, eight-attempt server-incarnation entropy boundary and a
  validated `/dev/urandom` implementation. It rejects zero and, for recovery,
  the immediately prior value; erases source failures to a content-free kind;
  and has real-socket evidence that the generated value reaches the canonical
  lifetime record and ServerHello unchanged. Revision
  `f2db93b9bd85244901694dcc906be07449ce8104` passed the integrated release
  profile with 10,000 soak cycles and 1,000,000 cases on each fuzz target.

- Refreshed the checksum-closed source-candidate and canonical source-archive
  evidence after the stable graph expanded to seventeen components: revision
  `46098fb3cd1a6f9164365d8c49cf7edbc0957ef7` produced two byte-identical
  291-payload candidates and two byte-identical 2,450,432-byte/362-entry
  archives at SHA-256
  `4437ccfa263ff913b36679706a5585be5170cc1abd5f32869db8f4d20544c0c5`;
  direct-candidate and extracted-archive locked/offline qualification passed.

- Added collaboration handshake profile 2 for the private Unix daemon, with
  closed conversation/transient/control routing, a server-only session hello
  binding replay to the declared server incarnation and exact authenticated
  connection, frozen wire fixtures, SDK/fuzz reachability, and real-socket
  reconnect/restart privacy evidence. The conversation-only profile and durable
  capability list remain unchanged.
- Added `DurableTransientHostV1`, combining receipt-gated Host checkpoints with
  deliberately non-durable presence and typing. Store refusal restores Host
  state without leaking a signal, uncertain durability fences and retires the
  source, and physical local-daemon restart retains rooms while reconstructing
  transient state empty.
- Added explicit durable-collaboration endpoint-residue recovery: the caller
  supplies the recovered owner, exact prior incarnation, and distinct next
  incarnation; the daemon rotates only validated residue and preserves
  collaboration handshake profile 2 without resuming an old session.
- Added passive saturating durability-publication observations for both
  authenticated durable Host owners and daemon composition, including truthful
  fenced-state reporting without exposing the Host/Service projection.
- Replaced independent daemon readiness, drain, and fence flags with one closed
  sequential lifecycle and added explicit terminal stop evidence that
  distinguishes clean teardown, preserved ambiguous residue, and incomplete
  cleanup.
- Replaced the directory adapter's raw internal signals with a passive,
  content-free physical-owner observation covering revision, load/CAS outcomes,
  indeterminate publication, resource fencing, and sticky saturation.
- Made every active directory/lock/head/residue validation failure establish one
  sticky physical-owner fence before returning its specific first error; later
  operations refuse without revalidation or a duplicate fence count.
- Fenced the physical store owner on pending-publication creation I/O errors,
  closing the remaining operational path that could return a filesystem error
  while still reporting the constructed owner Active.
- Added an effect-free optional physical-store observation port and generic
  durable-host/daemon composition that preserves transport, runtime,
  publication, and physical facts as separate groups with aggregate fidelity.
  Revision `b9152c58c32481af93d58d2dfb075f79a19f5be7` passed the integrated
  release profile with 10,000 soak cycles and 1,000,000 cases on each fuzz
  target.

- Added a bounded transient-typing owner for room, retained-root,
  and stable-topic targets, with current membership/target authority, opaque
  session sources, exact snapshot cursors, atomic capacity, lifecycle cleanup,
  human/agent symmetry, generation-fidelity fencing, and restart-to-inactive
  evidence.
- Added a sealed durable-Host selection to the existing private Unix daemon and
  a real-socket directory-store restart scenario proving retained conversation
  state under a fresh authenticated connection and server incarnation; reconnect
  request replay remains explicitly unsupported.
- Added a bounded transient-collaboration owner for expiring
  presence signals, with current-membership authority, opaque session sources,
  monotone explicit time, snapshot-bound pages, lifecycle retirement, bounded
  expiry work, human/agent symmetry, and restart-to-`Unknown` evidence.
- Added a non-durable transient Host composition that derives
  presence and typing sources from authenticated sessions, uses qualified Host time,
  retires state on close/revocation/regression, reconciles leave/kick/ban before
  returning conversation replies, and fences observation on generation-fidelity
  loss without blocking cleanup.
- Added validated V2 client-state export and restoration for the client-owned
  request namespace and exact unresolved request, while leaving durable storage,
  endpoint/session correspondence, and reconnect policy caller-owned.
- Added a separately typed transient collaboration Protocol/Wire
  V1 grammar with volatile request identities; distinct presence and typing
  cursors; room, retained-root, and stable-topic typing targets; remaining-only
  lifetime projection; canonical shape/order checks; frozen frames; and bounded
  deterministic mutation closure. Durable Protocol V2 remains byte-unchanged.
- Added a command-neutral transient client state machine whose exact uncertainty
  is bound to a nonzero caller-supplied replay epoch spanning the server
  incarnation and authenticated session; reconnect/restart rebind returns old
  uncertainty and begins a fresh request namespace.
- Added a public presence/typing Host handler with authenticated
  request-scoped revalidation, one bounded exact replay receipt per session,
  explicit sequence/conflict refusal, opaque one-live-cursor pagination, and
  independently bounded snapshot age without durable receipt contamination.
- Added explicit fail-closed local endpoint residue recovery requiring the exact
  prior incarnation, a distinct next incarnation, and a fresh Host composition,
  with locked in-place record rotation and no implicit volatile-state adoption.
- Added an endpoint-scoped exclusive local-daemon lifetime lease with a frozen
  incarnation-bound record, pinned resource revalidation, live-versus-residue
  refusals, and conservative fencing and teardown under replacement.
- Added a canonical fixed-width pre-authentication local handshake selecting
  Wire V2 and reporting explicit transport budgets, capability-discovery
  support, and content-safe server-incarnation evidence, with frozen fixtures,
  SDK reachability, fuzz oracles, and real-socket refusal before downstream work.
- Added a canonical bounded USTAR source-archive profile over verified source
  candidates, with fixed metadata and ordering, an exact SHA-256 sidecar,
  platform extraction, byte-identical rearchive, and adversarial contract tests;
  reproduced and qualified the first 1,984,512-byte exact-revision archive.
- Added a deterministic CycloneDX 1.5 source SBOM for the exact sixteen-package
  stable graph, with offline canonical regeneration, closed dependency and
  claim validation, host-path exclusion, and focused adversarial mutants.
- Added a fail-closed self-contained stable source-candidate contract with exact
  local Aegis/Keystone content closure, contextual cleanliness evidence,
  complete SHA-256 inventory, adversarial verification, and isolated locked
  offline qualification; reproduced and qualified a 248-file candidate that
  includes the canonical source SBOM and archive tooling.
- Added a third pinned coverage-guided target over the local daemon's production
  outer-frame prefix codec, with exact consumed-prefix re-encoding, stream-suffix
  preservation, canonical framed seeds, ordinary codec boundary tests, and an
  integrated one-million-case release-candidate campaign.
- Added an irreversible idempotent idle drain fence for the sequential local
  daemon, refusing before accept or authority work while retaining pinned
  endpoint ownership through teardown.
- Added a passive content-free local-daemon observation snapshot with distinct
  transport, host, and service counter boundaries plus sticky aggregate
  fidelity-loss evidence.
- Added a commit-pinned integrated release-candidate profile spanning the full
  ordinary gate, all release-mode targets, a bounded 10,000-cycle recovery soak,
  and one million fixed-seed cases on each pinned fuzz target; recorded the
  first successful local Apple Silicon campaign against its exact source commit.
- Added pinned, opt-in cargo-fuzz targets for every public wire decoder and the
  engine/service/host recovery stack, with immutable canonical seeds, exact
  re-encoding oracles, a stable contract gate, and bounded campaign runner.
- Added an exhaustive four-step core lifecycle model spanning 4,096 schedules
  for both human and agent actors across speech, stale revisions, membership,
  moderation, retention, and dual event-copy invariants.
- Added a second exhaustive four-step model for replies, thread lifecycle,
  reactions, edits, withdrawal, retention eviction, and human/agent symmetry.
- Separated the lossless acknowledged delivery outbox from a bounded recent
  change journal, with independent eviction evidence, participant-bound pull
  gaps, canonical profile-14/16/16 recovery, a closed capability, and frozen
  Wire V2 gap bytes.
- Added a bounded private local Unix transport composition over Aegis-authorized
  Host V2 with a required mode-0700 parent, mode-0600 new-path socket,
  preallocation frame ceiling,
  deadlines, request budget, per-frame reauthorization, pinned teardown,
  unconditional cleanup, and content-free health.
- Added a separate Aegis verifier adapter that maps exact public session/grant
  authorization to Agora host admission and revalidation with failure erasure.
- Accepted the private monotone read-position architecture: self-scoped room,
  thread, and topic markers are durable private projection state, never public
  events or evidence of delivery, attention, or execution.
- Added the engine tranche for bounded private room/thread/topic markers,
  including revision and scope validation, monotone/idempotent advancement,
  membership privacy, lifecycle retention, and canonical profile-13/15/15
  engine/service/host recovery.
- Completed participant-bound private read-position service execution, closed
  protocol and canonical wire values, SDK reachability, exact receipt recovery,
  and durable commit-before-reply/lost-reply qualification.
- Added a finite exhaustive read-position schedule model across both participant
  kinds, all scope variants, and every three-step ordering of three targets.
- Added a 216-schedule Service V2 model covering human/agent symmetry, stale
  revisions, membership authority changes, disconnects, and reconnects.
- Added an opt-in 5,000-cycle service soak for replay, paging, retention,
  leave/kick/rejoin lifecycle, reconnect, and checkpoint recovery.
- Added audience-specific embedded and remote-client SDK facades.
- Added exact package and canonical-fixture compatibility inventories with
  adversarial drift checks.
- Added reproducible bounded mutation over every canonical V1/V2 wire fixture,
  requiring exact re-encoding for every accepted single-bit mutation.
- Added fixed-seed service and host checkpoint mutation campaigns that require
  fail-closed recovery or exact canonical re-encoding.
- Added a pinned Rust toolchain, source release contract, security policy, dual
  license files, and conservative locked-component supply inventory.
- Added membership-filtered, revision-bound room, member, and retained-message
  pages with opaque cursors and explicit retention-gap evidence.
- Added a Protocol/Service/Client V2 profile with participant-bound
  room lifecycle parity, bounded client cursor handles, and exact volatile replay.
- Added a canonical bounded Wire V2 grammar with golden create-room, room-page,
  and retained-message-page fixtures.
- Added a separately typed authenticated Host V2 with current-context
  revalidation, client-substitution fencing, and revocation-safe cleanup.
- Added complete scope-bound Service V2 and private cursor checkpoints plus a
  receipt-gated durable owner with rollback, uncertainty fencing, and exact
  replay recovery over the Keystone storage port.
- Added a V2 client uncertainty fence that distinguishes not-sent from
  outcome-unknown, permits only exact replay, and exposes abandonment explicitly.
- Added canonical authenticated Host V2 recovery and a durable combined
  host/service gate so session binding, service receipt, and security cleanup
  commit as one pre-reply unit.
- Added an exclusive private local Unix directory checkpoint adapter with a
  frozen checksummed record, synchronized atomic replace, resource pinning,
  fail-closed corruption/residue handling, and authenticated-host restart proof.
- Added interruption injection at every directory publication boundary, proving
  pre-rename residue refusal and post-rename candidate recovery with fencing.
- Added a snapshot-aligned bounded room change feed with stable event identity,
  exclusive replay-safe positions, explicit retention and membership-epoch gaps,
  durable Service V2 cursors, and a closed canonical Wire V2 projection.
- Added participant-neutral kick, ban, and unban with role-scoped authority,
  bounded reason and causation evidence, current-member feed projection, durable
  recovery, and canonical moderation wire/checkpoint fixtures.
- Added owner-bound room admission-policy changes as ordered room facts with
  prior/current policy, participant-bound service projection, and frozen wire evidence.
- Added authority-scoped bounded ban-list pages with opaque revision-bound,
  client-owned cursors and durable continuation recovery.
- Added replayable closed capability discovery with canonical wire evidence;
  advertised support is explicitly non-authorizing.
- Added a parallel identified-message profile with opaque 128-bit room-local
  references, identified acceptance/history, isolated cursors, and recovery.
- Added replay-safe author withdrawal for retained messages, with stable
  tombstones, scrubbed retained feed content, and an attributable room event.
- Added role-scoped moderator message redaction as a distinct action with
  bounded reason, opaque causation, author evidence, tombstones, and recovery.
- Added an identity-bearing room-change profile with isolated durable cursors,
  stable message IDs across receipt/history/feed/recovery, and canonical wire
  and SDK evidence without changing the legacy change grammar.
- Added one-level threads with immutable root/reply identity, independent bounded
  pages, isolated durable cursors, tombstone-safe references, a parallel threaded
  change feed, canonical wire fixtures, and SDK evidence.
- Added explicit retained-thread archive/reopen lifecycle with root-author and
  moderator authority, current status observation, reply admission fencing,
  attributable events across all change profiles, and exact recovery.
- Added bounded participant-to-message reactions with explicit set transitions,
  canonical count/caller aggregates, tombstone-safe state, all-feed attribution,
  exact replay/recovery, and closed protocol, wire, SDK, and fixture evidence.
- Added bounded immutable message revision history with original-author
  authority, 64-record append-only numbering, client-bound observation,
  all-feed attribution, tombstone scrubbing, and exact replay/recovery.
- Added stable room-local topics with opaque identity, role-bound kind-neutral
  creation, immutable message relation and reply inheritance, bounded pages, a
  parallel topic-aware feed, and canonical whole-stack recovery.

### Fixed

- Closed the fuzz workspace lock over every current target dependency, changed
  the ordinary fuzz contract from metadata-only inspection to locked offline
  compilation of all targets, and made release qualification refuse tracked
  drift after execution as well as before it. Revision
  `1ca5bb741f646dce7fe428482c49eb9e97454c7d` passed the corrected profile with
  10,000 soak cycles and 1,000,000 cases on each fuzz target.

### Changed

- Made every workspace package explicitly non-publishable.
- Advanced the engine, service, and host checkpoint records to profiles 12, 14,
  and 14 respectively; earlier profile bytes remain fail-closed.

### Security

- Documented private vulnerability reporting and the boundary between Agora
  conversation facts, authentication, authority, orchestration, and execution.

[Unreleased]: https://github.com/gantbd/agora/commits/main
