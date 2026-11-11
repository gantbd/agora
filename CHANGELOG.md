# Changelog

All notable changes to Agora will be recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Released versions will
follow Semantic Versioning once a public release contract is selected.

## [Unreleased]

### Added

- Added pinned, opt-in cargo-fuzz targets for every public wire decoder and the
  engine/service/host recovery stack, with immutable canonical seeds, exact
  re-encoding oracles, a stable contract gate, and bounded campaign runner.
- Added an exhaustive four-step core lifecycle model spanning 4,096 schedules
  for both human and agent actors across speech, stale revisions, membership,
  moderation, retention, and dual event-copy invariants.
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

### Changed

- Made every workspace package explicitly non-publishable.
- Advanced the engine, service, and host checkpoint records to profiles 12, 14,
  and 14 respectively; earlier profile bytes remain fail-closed.

### Security

- Documented private vulnerability reporting and the boundary between Agora
  conversation facts, authentication, authority, orchestration, and execution.

[Unreleased]: https://github.com/gantbd/agora/commits/main
