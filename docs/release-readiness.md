# Release readiness

Status: source-ready for the declared v0.3 core and volatile service v0.1 on
2026-11-09; not ready for a network service or crates.io release.

## Established

- closed bounded deterministic conversation transitions;
- participant-neutral admission, roles, speaking, and addressing;
- atomic typed refusal and optimistic revision fencing;
- bounded outbox and replay-safe plain response publication;
- canonical V2 checkpoint preparation and semantic recovery;
- immutable representative V2 compatibility fixture;
- transport-neutral protocol, serialized participant-bound service, and client
  request profile with bounded volatile replay;
- canonical bounded V1 wire codec with executable golden compatibility fixtures;
- offline format, lint, test, doctest, and documentation gate.

## Open release blockers

- no published dependency/release provenance or remote CI;
- no authenticated host, network transport, or connector conformance package;
- no independently qualified durable Agora store;
- no performance, fuzzing, mutation, or supply-chain qualification;
- no production privacy, deletion, moderation, or abuse policy.

Passing `make check` establishes only the source profile above.

