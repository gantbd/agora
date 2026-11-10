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
| R12 neutral addressing | `SpeakAddressed`, canonical member list retained in `Message` | kind-symmetry, order, membership, capacity, redaction, checkpoint, bounded-model scenarios, and external Moira passive-fact qualification | Satisfied for represented message addressing; execution intentionally excluded |
| R13 replay-safe publication | opaque fixed-width key and bounded exact-content transition index | matching lost-reply replay, key conflict, saturation atomicity, and debug-redaction scenarios | Satisfied for volatile in-process profile; durable persistence open |
| R14 participant-bound requests | service-side client-to-participant binding and member-gated snapshot | author derivation, kind symmetry, addressing, and outsider snapshot scenarios | Satisfied for trusted in-process connections |
| R15 replay-safe service calls | bounded exact request/reply receipt map | matching replay, changed-request conflict, disconnect reclamation, and receipt saturation scenarios | Satisfied for volatile service lifetime |
| R16 layered dependencies | workspace crate split and normal dependency checks | boundary script plus workspace build | Satisfied for current crates |
| R17 canonical bounded wire | `agora-wire` over `agora-protocol` and `keystone-codec` | golden fixtures, full round trips, every truncation, trailing bytes, limits, UTF-8, versions, and nested unknown tags | Satisfied for protocol V1 values |
| R18 external authentication | injected `PrincipalVerifier`, explicit monotone context/time, per-frame revalidation | verified connect, malformed/capacity preflight, time regression, and context revocation scenarios | Satisfied for trusted verifier composition; credential adapter open |
| R19 dual binding | opaque host session mapped to subject, participant, and service client | client substitution and unaffected target-session scenario | Satisfied for volatile host lifetime |
| R20 revocation-safe cleanup | unconditional close plus eviction-based host/service tombstone windows | revocation replacement, explicit close, zero/full tombstone scenarios | Satisfied for volatile cleanup |
| R21 disclosure-safe authentication | erased verifier errors, shared Unavailable projection, redacted debug | registered/unregistered authentication refusal equality and debug scenario | Satisfied at host API boundary |
| R22 audience-specific facades | `agora-sdk-embedded` and `agora-sdk-client` reexport-only crates with closed direct dependencies | facade-only construction tests, dependency boundary script, workspace lint/test/doc gates | Satisfied for unpublished V0.1 distributions |
| R23 revision-bound pages | engine-owned opaque room/member/message cursors and membership-filtered page methods | six bounded observation scenarios covering concatenation, visibility, participant-kind neutrality, foreign/stale cursors, retention gaps, limits, atomic refusal, and Debug redaction | Satisfied for in-process core observation v0.1; volatile V2 projection is tracked separately by R24 |
| R24 participant-bound functional parity | separately typed Protocol/Service/Client V2 source modules with actor-derived engine commands, bounded cursor registry, and exact request receipts | Protocol V2 redaction; five service scenarios covering full invite-only lifecycle, paging/replay, cursor isolation/one-shot/staleness/capacity, validation/conflict atomicity, and disconnect reclamation; client namespace scenario | Satisfied for volatile source profile; host is tracked by R26 and durability remains open |
| R25 canonical Wire V2 | separately versioned `agora-wire::v2` complete-frame codec | three golden fixtures; every command/outcome/refusal round trip; truncation, trailing, version, unknown nested-tag, limit, and Debug-redaction scenarios | Satisfied for canonical frame profile; Host V2 is tracked by R26 and transport remains open |
| R26 authenticated Host V2 | separately typed `HostV2` over Wire/Service V2 with shared trusted verifier contract and distinct session identity | five scenarios covering actor binding/replay, non-enumerating authentication, substitution/revocation isolation, capacity/version preflight, explicit disconnect, and transport close; embedded-facade construction | Satisfied for transport-neutral volatile host; credential adapter/transport/durability open |
| R27 complete service recovery | `AGORASV` service checkpoint plus private `AGORACUR` cursor checkpoint profiles | three service persistence scenarios covering exact round trip, outbox/client/receipt/cursor continuation, truncation, scope, ceiling, future profile, trailing bytes, replay/conflict, and post-recovery mutation | Satisfied for canonical semantic state; physical store qualification remains open |
| R28 receipt-gated publication | generic `agora-durable::DurableServiceV2` over the Keystone checkpoint-store port | six scenarios covering commit-before-publication, lost reply replay without duplicate event/store call, known-refusal rollback, indeterminate-before/after recovery, weak/error fencing, and content-safe diagnostics | Satisfied at store-neutral boundary; no physical adapter durability claim |

## Verification gates

`make check` runs formatting, Clippy with warnings denied, all tests offline,
doctests, and warning-denied documentation. This establishes only the in-process
core, service recovery, and receipt-gated store boundary exercised by those
checks. It does not establish a physical store's durability, transport
framing/security, distributed ordering, or connector behavior.

The external Moira qualification projects an unacknowledged `MessageAccepted`
event as a participant-neutral read-only fact, then requires explicit Moira
admission. Its restart, eviction, replay, and conflict evidence is maintained by
Moira's own gate; it is not part of Agora's `make check`.

