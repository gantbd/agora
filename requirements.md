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

## Volatile service v0.1

- **R14 — Participant-bound requests:** a connected client is bound to one
  registered participant. Speaking derives its author from that binding and
  room snapshots require membership. Human and agent participants follow the
  same service path.
- **R15 — Replay-safe service calls:** request identity is scoped by client;
  matching replay returns the retained reply, changed content conflicts, and a
  full bounded receipt index refuses before engine mutation.
- **R16 — Layered dependencies:** protocol values do not depend on engine or
  service internals, client interpretation depends only on protocol, and the
  serialized service is the only operational layer that owns an engine.
- **R17 — Canonical bounded wire:** protocol V1 has one closed binary encoding
  with explicit version and tag refusal, strict UTF-8, checked preflight sizing,
  independent frame/text/field/list ceilings, and complete-input decoding.

## Authenticated host v0.1

- **R18 — External authentication:** a trusted verifier authenticates the exact
  Connect participant claim and revalidates the current transport/credential
  context before every later frame. Per-session host time cannot regress. Agora
  retains no credential material.
- **R19 — Dual binding:** a server-owned host session binds one opaque verified
  subject, participant, and service client. A frame naming another client retires
  only the offending session and cannot operate on the named target.
- **R20 — Revocation-safe cleanup:** failed revalidation and transport loss release
  service clients even when retirement windows are disabled or saturated.
- **R21 — Disclosure-safe authentication:** unauthenticated participant claims do
  not reveal registration state, and unknown/revoked/mismatched sessions share one
  external refusal class. Host diagnostics redact subjects, sessions, and frames.

## SDK distribution facades v0.1

- **R22 — Audience-specific facades:** the trusted-host SDK must be sufficient to
  construct the engine/service/host stack, while the remote-client SDK exposes
  only client/protocol/wire concerns. Both are reexport-only, unpublished
  distribution labels that add no transport, credentials, authority, storage,
  orchestration, retry, or runtime policy.

## Bounded core observation v0.1

- **R23 — Revision-bound pages:** a registered participant may observe only rooms
  in which it is a current member, and a current member may page that room's
  members and retained messages. Every page is nonempty-limit bounded; a
  continuation is opaque, bound to observer, room where applicable, and exact
  engine revision; foreign and stale continuations refuse distinctly. Message
  pages expose the retained sequence interval rather than implying complete
  history. Observation and its refusals do not mutate engine state, and debug
  output discloses no identifiers, display names, room names, or message bodies.

## Volatile service V2 source profile

- **R24 — Participant-bound functional parity:** a distinct V2 service derives
  every create, invite, join, leave, role, and speak actor from its connected
  participant and projects the bounded core room/member/message pages through
  client-bound opaque cursor handles. V2 requests retain the same exact replay,
  changed-input conflict, capacity-before-execution, disconnect cleanup, and
  participant-kind neutrality laws as V1. Page cursors are bounded, client-bound,
  one-shot on successful continuation, reclaimed on disconnect, and refuse
  unknown/cross-client/type/room use without disclosing cursor ownership. A V2
  client owns a separate monotone request namespace.
- **R25 — Canonical Wire V2:** Protocol V2 has a separate version-2 complete-frame
  grammar covering every command, outcome, refusal, page, cursor, role, kind, and
  admission value. Encoding preflights exact bounded size; decoding bounds before
  reconstruction, consumes all input, and refuses V1, unknown nested tags,
  malformed text, overflow, truncation, and trailing bytes. Canonical V2 fixtures
  must not change without compatibility review.
- **R26 — Authenticated Host V2:** a separately typed V2 host applies the same
  exact Connect authentication, per-frame current-context revalidation, monotone
  host-time, dual host-session/service-client binding, client-substitution fence,
  non-enumerating external refusal, and unconditional transport-loss cleanup laws
  to Wire/Service V2. V1 frames refuse before authentication and V2 failures
  cannot operate on another session.

## Explicitly open

Durable storage adapters, durable event sinks, credential/Aegis verifier adapters,
network transports, durable sessions, bans, message
editing/deletion, threads, reactions, end-to-end encryption, federation, delivery
acknowledgment, and durable audit are not part of the v0.3 claim.
Credential/Aegis adapters, transport, and durable V2 recovery remain open;
Protocol V1 bytes and semantics are unchanged.

