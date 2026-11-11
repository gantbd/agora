# Release readiness

Status: source-ready for the declared v0.3 core, volatile service v0.1,
authenticated host profile v0.1, store-neutral durable service/host boundary,
single-owner local Unix directory-store profile, and bounded private local Unix
transport composition on 2026-11-10;
not ready for a network service or crates.io release.

## Established

- closed bounded deterministic conversation transitions;
- participant-neutral admission, roles, speaking, and addressing;
- atomic typed refusal and optimistic revision fencing;
- bounded lossless delivery outbox, independently bounded recent-change journal,
  and replay-safe plain response publication;
- canonical V2 checkpoint preparation and semantic recovery;
- immutable representative V2 compatibility fixture;
- transport-neutral protocol, serialized participant-bound service, and client
  request profile with bounded replay plus validated export/restore of its exact
  pending request namespace; persistence and reconnect correspondence remain
  host-owned;
- canonical bounded V1 wire codec with executable golden compatibility fixtures;
- canonical fixed-width local ClientHello/ServerHello profile selecting Wire V2
  before authentication, with explicit server limits, capability-discovery
  profile, and caller-supplied nonzero server-incarnation evidence;
- endpoint-scoped exclusive daemon lifetime lease with a frozen
  incarnation-bound record, pinned parent/record/socket identities, typed live
  owner versus crash-residue refusal, and fail-closed replacement fencing;
- explicit exact-incarnation endpoint-residue recovery with locked lifetime
  rotation, fresh Host composition, and next-incarnation handshake evidence;
- an additive private local collaboration handshake with bounded closed routing,
  server-only replay-epoch control, authenticated connection binding, and
  reconnect/restart privacy evidence;
- a bounded transient presence owner with current-membership
  checks, opaque sources, monotone explicit time, snapshot cursors, bounded
  expiry work, lifecycle retirement, and restart-to-`Unknown` evidence;
- a bounded transient typing owner with current membership plus
  room/retained-root/stable-topic authority, opaque session sources, bounded
  target fan-out and expiry, exact snapshot pages, lifecycle cleanup, and
  restart-to-inactive projection;
- a non-durable authenticated Host wrapper for transient presence and typing,
  including close/revocation/regression source retirement, membership
  reconciliation before conversation replies, fail-safe cleanup under sticky
  fidelity loss, and wrapper reconstruction to `Unknown`;
- a separate transient presence/typing Protocol/Wire grammar with
  canonical state and target shapes, distinct opaque cursors, remaining-only
  lifetimes, four frozen frames, and deterministic bounded mutation closure;
- a replay-epoch-bound transient client with exact uncertainty,
  validated restoration, and explicit reconnect/restart rebind;
- an authenticated transient Host request handler with one bounded
  replay receipt and one age-bounded opaque page cursor per session;
- transport-neutral authenticated host composition with bounded volatile
  sessions, current-context revalidation, and revocation-safe cleanup;
- unpublished trusted-host and remote-client SDK facades with executable
  facade-only construction and closed dependency evidence;
- exact non-publication and Rust 1.95.0 source-contract enforcement across all
  twelve packages;
- conservative package and canonical-fixture inventory closure over twelve
  stable workspace packages, one standalone fuzz package, and thirty-nine
  fixtures;
- repository license texts, private security reporting, a changelog, and an
  explicit source-distribution policy;
- conservative supply inventory closure over sixteen locked local components with
  missing, changed-version, and empty-inventory mutants;
- deterministic CycloneDX 1.5 stable-source SBOM closure over the same sixteen
  components and their dependency graph, with exact license/publication facts,
  offline byte-for-byte regeneration, and adversarial schema/claim mutants;
- canonical bounded USTAR source-archive construction and verification,
  including SHA-256 closure, platform listing/extraction, exact rearchive, and
  checksum/framing/traversal/link mutants;
- a checksum-closed self-contained stable source directory for revision
  `651374a3e9cf4b51cb21da7c7af03bb746604d25`, including the canonical source
  SBOM and archive tooling, reproduced byte-identically twice across 248 payload files and qualified
  locked/offline from its isolated sixteen-component local content closure while
  truthfully marking both sibling selections unclean;
- two byte-identical canonical USTAR archives for that revision, each 1,984,512
  bytes and 316 entries with SHA-256 `41e22b596d32c6bd44578afde4421c4834d2b87c45b4d4a7dd3a5637976f4ad2`;
  one passed validation, platform extraction, exact rearchive, and complete
  locked/offline extracted-source qualification;
- membership-filtered, revision-bound in-process room/member/message pages with
  opaque cursors and explicit retained-history intervals;
- separately typed volatile Protocol/Service/Client V2 source profile closing
  participant-bound room lifecycle commands and bounded cursor projection;
- exhaustive 216-schedule Service V2 model covering human/agent symmetry,
  current and stale mutations, leave/join authority, retired clients, and
  reconnect namespace reset;
- exhaustive 4,096-schedule, four-step core lifecycle model repeated for human
  and agent actors across speaking, stale revisions, leave/rejoin,
  kick/ban/unban, atomic refusal, ordering, and message retention;
- exhaustive 4,096-schedule, four-step conversation-relation model repeated for
  human and agent root authors across replies, thread lifecycle, reactions,
  edits, withdrawal, retention eviction, and dual event-copy atomicity;
- opt-in deterministic 5,000-cycle Service V2 soak covering exact replay,
  bounded paging and retention, leave/kick/rejoin lifecycle, reconnect, and
  repeated canonical checkpoint recovery;
- canonical bounded Wire V2 complete-frame grammar with four additional golden
  fixtures and closed nested vocabularies;
- deterministic bounded mutation of every canonical V1/V2 wire fixture across
  all strict prefixes, trailing framing, and every single-bit change, with exact
  re-encoding required for every accepted candidate;
- separately pinned coverage-guided smoke targets for all public wire decoders,
  engine/service/host recovery, and the production outer-frame prefix codec,
  with immutable canonical seeds and exact re-encoding oracles;
- commit-pinned integrated local qualification of revision
  `c4cad484d6dec38123b7270f7f11848519d65fd3` on Darwin 25.6.0 arm64 / macOS
  26.6.2 across the ordinary gate, all release-mode targets, 10,000 deterministic
  soak cycles, and 1,000,000 fixed-seed cases on each of the three fuzz targets,
  with locked fuzz-target compilation and unchanged tracked bytes afterward;
- fixed-seed bounded mutation of canonical service and host recovery records,
  requiring refusal or exact byte-preserving recovery and re-encoding;
- bounded reaction set transitions and aggregates with participant-bound
  authority, tombstone/archive preservation, retention cleanup, exact recovery,
  all-feed attribution, and two canonical wire fixtures;
- bounded immutable message revisions with original-author authority, stable
  identity and relationships, fixed history ceilings, participant-bound pages,
  all-feed attribution, tombstone scrubbing, and exact replay/recovery;
- stable room-local topics with role-bound kind-neutral creation, immutable
  message relation and reply inheritance, bounded participant-bound pages, a
  parallel topic-aware feed, and profile-14/16/16 canonical recovery;
- independent delivery and pull retention: acknowledgment drains only the
  backpressured outbox, journal eviction carries its own exact gap, and both
  suffixes recover canonically with live service cursors;
- private monotone room/thread/topic read positions with participant-derived
  authority, closed public values and canonical bytes, exact receipt recovery,
  zero room revision/event effect, and durable lost-reply replay;
- exhaustive finite three-step read-position schedules across both participant
  kinds and all room/thread/topic scope variants against a monotone reference;
- authenticated transport-neutral Host V2 with distinct session/client binding,
  per-frame revalidation, substitution fencing, and cleanup;
- separate Aegis public-service verifier adapter with exact session/grant/time/
  resource mapping, fixed Agora Submit admission, failure erasure, revocation
  cleanup, and closed dependency direction;
- bounded sequential private Unix transport composition with required mode-0700
  parent, mode-0600 new-path endpoint, preallocation length bound, deadlines,
  fixed request budget, server-owned Host V2 session, per-frame Aegis
  reauthorization, unconditional cleanup, pinned teardown, and content-free
  health;
- passive content-free local-daemon observations preserving separate transport,
  host, and service measurement boundaries with sticky saturation/fidelity-loss
  evidence and no semantic or authority effect;
- explicit irreversible idle drain fencing that withdraws readiness and refuses
  before accept or authority work while retaining endpoint ownership until
  pinned teardown;
- independently versioned, scope-bound Service V2 checkpoints preserving engine,
  outbox, recent-change journal, clients, retired tombstones, exact request
  replies, and live cursor consequences across semantic recovery;
- `agora-durable` pre-reply compare-and-exchange gating with durable-strength
  receipt validation, known-refusal rollback, replay-without-store, and
  indeterminate-outcome fencing;
- V2 client uncertainty fencing with exact-request replay, not-sent versus
  outcome-unknown distinction, different-command suppression, and explicit
  abandonment evidence;
- canonical authenticated Host V2 recovery and durable combined host/service
  publication, preserving session/client/subject/time bindings and committing
  revocation cleanup before its external refusal;
- exclusive private local Unix directory storage with a frozen checksummed
  record, process lock, pinned resource identities, synchronized atomic replace,
  fail-closed residue/corruption handling, all five publication interruption
  points, and real authenticated-host restart;
- snapshot-aligned bounded room pull changes with stable event identity,
  exclusive continuation, explicit retention/leave-rejoin gaps, durable service
  cursors, closed Wire V2 values, and a frozen representative frame;
- stable message references and author-only retained-message withdrawal with
  attributable reference-preserving tombstones and exact recovery;
- role-scoped moderator redaction with bounded evidence, stable tombstones, and
  exact recovery, distinct from author withdrawal;
- identity-bearing bounded room changes with a cursor profile isolated from the
  legacy feed and stable IDs across receipt, history, feed, and recovery;
- authoritative one-level roots/replies with independent bounded pages,
  root/profile-bound durable cursors, relationship-preserving tombstones,
  exact replay/recovery, and a parallel closed threaded change profile;
- explicit retained-thread archive/reopen lifecycle with root-author and
  moderator authority, current status, reply fencing, attributed events across
  every change profile, and exact replay/recovery;
- separately qualified local Moira ingress/publication composition in which
  addressed events remain passive facts until explicit workflow admission;
- offline format, lint, test, doctest, and documentation gate.

## Open release blockers

- R62 is qualified for an additive private local volatile profile: handshake
  profile 2 multiplexes conversation and transient frames, supplies an exact
  connection/server replay epoch, retires sources on close, and restarts empty.
  R64 additionally qualifies a combined durable-conversation plus volatile
  collaboration owner with suppression cleanup and clean physical restart.
  Network transport and a production credential authenticator remain open and
  are not inferred from these profiles.

- no remote origin provenance or remote CI; the qualified local source candidate
  is content-addressed, but its selected Aegis and Keystone inputs were unclean
  and are not proven to correspond to remotely available commits;
- SDK facades are not approved or configured for registry publication;
- no qualified credential/Unix-peer authenticator, network transport, or
  connector conformance package; Aegis context provenance remains trusted
  wiring;
- no qualified server-incarnation generator or automatic/crash-coupled restart;
  clean durable Host/directory-store restart is qualified, but there is no
  concurrent admission, in-flight drain,
  signal/socket-activation/crash-residue qualification, reconnect resumption, or
  cross-disconnect replay; recovered Aegis and Agora state do not share one
  atomic commit boundary;
- no qualified store beyond the single-owner local Unix directory profile; no
  hardware power-loss, network-filesystem, backup/restore, encryption, or
  multi-host availability qualification;
- no performance qualification, coverage threshold, repeated/remote fuzz
  campaign, advisory lookup, legal compatibility review, binary SBOM, or
  artifact provenance;
- no production privacy, deletion, moderation, or abuse policy.

Passing `make check` establishes only the ordinary source profile above. The
recorded integrated qualification establishes only its exact revision, host,
and bounded numeric profile; it does not qualify a distributable artifact.
The Moira composition evidence runs in that repository and does not establish a
durable Agora service or broaden this repository's release claim.

