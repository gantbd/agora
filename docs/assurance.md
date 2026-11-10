# Assurance plan

Status: active for the v0.3 core and volatile service v0.1.

Agora uses a requirements-to-scenario-to-invariant evidence chain. A behavior is
claimed only when its requirement, closed public value, adversarial scenario, and
traceability row agree. Refusal scenarios assert the revision and event frontier
remain unchanged. Persistence scenarios compare canonical bytes after recovery.

The active `make check` gate contains:

1. formatting and warning-denied lint;
2. all unit, integration, and bounded-model tests offline;
3. doctests and warning-denied documentation.

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

The gate does not establish authentication, transport security, multi-process
serialization, durable storage, event delivery, distributed availability, or
orchestrator correctness. Those claims require separate adapters and evidence.

