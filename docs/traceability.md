# Requirement-to-evidence traceability

This ledger records current evidence. Open work is not a pass.

| Requirement | Implementation | Evidence | Status |
| --- | --- | --- | --- |
| R1 authoritative state | non-`Clone` `Engine`, private maps | behavior tests through public observations | Satisfied for in-process v0.3 |
| R2 mixed participation | `ParticipantKind`, common command paths | human/agent conversation scenario | Satisfied |
| R3 bounded state | `Limits`, pre-publication validation | boundary and capacity scenarios | Satisfied for represented domains |
| R4 determinism | `CommandV2`, `LogicalTime`, ordered collections | deterministic transcript scenario | Satisfied for serial execution |
| R5 admission | `AdmissionPolicy`, invitations, roles | open/invite-only/authority scenarios | Satisfied |
| R6 atomic refusal | validate-before-mutate transitions | state snapshot assertions around refusals | Satisfied for tested commands |
| R7 ordered conversation | checked room-local sequence, FIFO | ordering/retention scenarios | Satisfied |
| R8 privacy | custom content-free message `Debug` | debug redaction test | Satisfied for `Message` |
| R9 effect isolation | dependency-free library core | source boundary and build graph | Satisfied for core; adapters open |
| R10 optimistic publication | `Revision`, `RevisionedCommandV2`, checked counters | stale-writer and ordered transition scenarios | Satisfied for serial in-process owner |
| R11 bounded delivery | bounded `EventEnvelopeV2` queue and monotone acknowledgment cursor | saturation, release, beyond-edge, and regression scenarios | Satisfied for volatile outbox; durability open |
| R12 neutral addressing | `SpeakAddressed`, canonical member list retained in `Message` | kind-symmetry, order, membership, capacity, redaction, checkpoint, and bounded-model scenarios | Satisfied for represented message addressing; execution intentionally excluded |
| R13 replay-safe publication | opaque fixed-width key and bounded exact-content transition index | matching lost-reply replay, key conflict, saturation atomicity, and debug-redaction scenarios | Satisfied for volatile in-process profile; durable persistence open |
| R14 participant-bound requests | service-side client-to-participant binding and member-gated snapshot | author derivation, kind symmetry, addressing, and outsider snapshot scenarios | Satisfied for trusted in-process connections |
| R15 replay-safe service calls | bounded exact request/reply receipt map | matching replay, changed-request conflict, disconnect reclamation, and receipt saturation scenarios | Satisfied for volatile service lifetime |
| R16 layered dependencies | workspace crate split and normal dependency checks | boundary script plus workspace build | Satisfied for current crates |
| R17 canonical bounded wire | `agora-wire` over `agora-protocol` and `keystone-codec` | golden fixtures, full round trips, every truncation, trailing bytes, limits, UTF-8, versions, and nested unknown tags | Satisfied for protocol V1 values |

## Verification gates

`make check` runs formatting, Clippy with warnings denied, all tests offline,
doctests, and warning-denied documentation. This establishes only the in-process
core and volatile service behavior exercised by those checks. It does not establish host authentication,
transport framing/security, persistence durability, distributed ordering, or connector
behavior.

