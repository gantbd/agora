# Requirements

## Core v0.3

- **R1 — Authoritative state:** one non-cloneable engine owns participant, room,
  membership, invitation, and retained-message state.
- **R2 — Mixed participation:** humans and software agents use the same admission,
  membership, role, and speaking rules; kind is explicit and never inferred.
- **R3 — Bounded state:** all identifiers, names, message bodies, collections, and
  retained histories have profile limits checked before publication.
- **R4 — Determinism:** commands contain all semantic inputs, including timestamps;
  identical state and command produce identical state and outcome.
- **R5 — Admission:** rooms declare open or invite-only admission. Invitation and
  moderation authority are explicit.
- **R6 — Atomic refusal:** a refused command does not change authoritative state or
  consume a message sequence number.
- **R7 — Ordered conversation:** accepted messages receive a room-local increasing
  sequence and retain the caller-supplied logical timestamp.
- **R8 — Privacy-aware observation:** message debug formatting does not disclose
  message content.
- **R9 — Effect isolation:** authentication, network I/O, storage, clocks, model
  calls, and agent processes remain adapter responsibilities.
- **R10 — Optimistic publication:** each accepted command checks an expected
  revision, advances it exactly once, and publishes one ordered domain event.
- **R11 — Bounded delivery:** a full event outbox backpressures commands before
  domain mutation; acknowledgment removes only a monotone published prefix.
- **R12 — Neutral addressing:** a message may explicitly address a bounded,
  canonical set of room members. Human and agent authors and recipients follow
  identical rules. Addressing is conversation state and never proof or a request
  that a participant executed work.
- **R13 — Replay-safe publication:** a cross-engine response key binds to one
  exact message publication. Matching replay returns the original transition;
  changed content conflicts; retained receipts are bounded.

## Explicitly open

Durable storage adapters, durable event sinks, connection/session
capabilities, bans, message
editing/deletion, threads, reactions, end-to-end encryption, federation, delivery
acknowledgment, and durable audit are not part of the v0.3 claim.

