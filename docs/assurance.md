# Assurance plan

Status: active for the v0.3 in-process core.

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

The gate does not establish authentication, transport security, multi-process
serialization, durable storage, event delivery, distributed availability, or
orchestrator correctness. Those claims require separate adapters and evidence.

