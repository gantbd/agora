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

### Changed

- Made every workspace package explicitly non-publishable.

### Security

- Documented private vulnerability reporting and the boundary between Agora
  conversation facts, authentication, authority, orchestration, and execution.

[Unreleased]: https://github.com/gantbd/agora/commits/main
