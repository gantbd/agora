# Changelog

All notable changes to Agora will be recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Released versions will
follow Semantic Versioning once a public release contract is selected.

## [Unreleased]

### Added

- Added audience-specific embedded and remote-client SDK facades.
- Added exact package and canonical-fixture compatibility inventories with
  adversarial drift checks.
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

### Changed

- Made every workspace package explicitly non-publishable.
- Advanced the engine, service, and host checkpoint records to profiles 8, 10,
  and 10 respectively; earlier profile bytes remain fail-closed.

### Security

- Documented private vulnerability reporting and the boundary between Agora
  conversation facts, authentication, authority, orchestration, and execution.

[Unreleased]: https://github.com/gantbd/agora/commits/main
