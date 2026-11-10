# Release readiness

Status: source-ready for the declared v0.3 core, volatile service v0.1, and
authenticated host profile v0.1 on 2026-11-10; not ready for a network service
or crates.io release.

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
- transport-neutral authenticated host composition with bounded volatile
  sessions, current-context revalidation, and revocation-safe cleanup;
- unpublished trusted-host and remote-client SDK facades with executable
  facade-only construction and closed dependency evidence;
- exact non-publication and Rust 1.95.0 source-contract enforcement across all
  eight packages;
- conservative package and canonical-fixture inventory closure over eight
  packages and four fixtures;
- repository license texts, private security reporting, a changelog, and an
  explicit source-distribution policy;
- conservative supply inventory closure over nine locked local components with
  missing, changed-version, and empty-inventory mutants;
- separately qualified local Moira ingress/publication composition in which
  addressed events remain passive facts until explicit workflow admission;
- offline format, lint, test, doctest, and documentation gate.

## Open release blockers

- no self-contained dependency/release provenance or remote CI; the source
  profile still requires the documented sibling Keystone checkout;
- SDK facades are not approved or configured for registry publication;
- no qualified credential/Aegis verifier, network transport, or connector
  conformance package;
- no independently qualified durable Agora store;
- no performance or fuzzing qualification, coverage-guided mutation, advisory
  lookup, legal compatibility review, binary SBOM, or artifact provenance;
- no production privacy, deletion, moderation, or abuse policy.

Passing `make check` establishes only the source profile above.
The Moira composition evidence runs in that repository and does not establish a
durable Agora service or broaden this repository's release claim.

