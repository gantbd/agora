# Release readiness

Status: source-ready for the declared v0.3 core, volatile service v0.1,
authenticated host profile v0.1, store-neutral durable service/host boundary,
and single-owner local Unix directory-store profile on 2026-11-10;
not ready for a network service or crates.io release.

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
  ten packages;
- conservative package and canonical-fixture inventory closure over ten
  packages and eighteen fixtures;
- repository license texts, private security reporting, a changelog, and an
  explicit source-distribution policy;
- conservative supply inventory closure over twelve locked local components with
  missing, changed-version, and empty-inventory mutants;
- membership-filtered, revision-bound in-process room/member/message pages with
  opaque cursors and explicit retained-history intervals;
- separately typed volatile Protocol/Service/Client V2 source profile closing
  participant-bound room lifecycle commands and bounded cursor projection;
- canonical bounded Wire V2 complete-frame grammar with four additional golden
  fixtures and closed nested vocabularies;
- authenticated transport-neutral Host V2 with distinct session/client binding,
  per-frame revalidation, substitution fencing, and cleanup;
- independently versioned, scope-bound Service V2 checkpoints preserving engine,
  outbox, clients, retired tombstones, exact request replies, and live cursor
  consequences across semantic recovery;
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
  fail-closed residue/corruption handling, and real authenticated-host restart;
- snapshot-aligned bounded room pull changes with stable event identity,
  exclusive continuation, explicit retention/leave-rejoin gaps, durable service
  cursors, closed Wire V2 values, and a frozen representative frame;
- stable message references and author-only retained-message withdrawal with
  attributable reference-preserving tombstones and exact recovery;
- role-scoped moderator redaction with bounded evidence, stable tombstones, and
  exact recovery, distinct from author withdrawal;
- separately qualified local Moira ingress/publication composition in which
  addressed events remain passive facts until explicit workflow admission;
- offline format, lint, test, doctest, and documentation gate.

## Open release blockers

- no self-contained dependency/release provenance or remote CI; the source
  profile still requires the documented sibling Keystone checkout;
- SDK facades are not approved or configured for registry publication;
- no qualified credential/Aegis verifier, network transport, or connector
  conformance package;
- no qualified V2 credential/Aegis adapter, capability negotiation, or network
  transport; the recovered verifier remains an injected trusted port;
- no qualified store beyond the single-owner local Unix directory profile; no
  hardware power-loss, network-filesystem, backup/restore, encryption, or
  multi-host availability qualification;
- no performance or fuzzing qualification, coverage-guided mutation, advisory
  lookup, legal compatibility review, binary SBOM, or artifact provenance;
- no production privacy, deletion, moderation, or abuse policy.

Passing `make check` establishes only the source profile above.
The Moira composition evidence runs in that repository and does not establish a
durable Agora service or broaden this repository's release claim.

