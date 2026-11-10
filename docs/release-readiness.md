# Release readiness

Status: source-ready for the declared v0.3 in-process profile on 2026-11-09; not
ready for a standalone service or crates.io release.

## Established

- closed bounded deterministic conversation transitions;
- participant-neutral admission, roles, speaking, and addressing;
- atomic typed refusal and optimistic revision fencing;
- bounded outbox and replay-safe plain response publication;
- canonical V2 checkpoint preparation and semantic recovery;
- offline format, lint, test, doctest, and documentation gate.

## Open release blockers

- no published dependency/release provenance or remote CI;
- no authenticated service, wire protocol, or connector conformance package;
- no independently qualified durable Agora store;
- no immutable compatibility fixtures for V2;
- no performance, fuzzing, mutation, or supply-chain qualification;
- no production privacy, deletion, moderation, or abuse policy.

Passing `make check` establishes only the source profile above.

