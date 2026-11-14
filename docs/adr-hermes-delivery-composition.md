# ADR: Hermes notification-delivery composition

Status: accepted; implementation open

Date: 2026-11-14

## Context

Agora owns accepted conversation state, participant visibility, private
attention intent, and ordered change/outbox facts. Hermes owns opaque durable
append, replay, consumer progress, retention gaps, and delivery uncertainty. A
push or webhook product needs both, but Hermes identifiers are correlation
rather than authority, and neither a Hermes cursor nor endpoint acknowledgment
is an Agora read position.

## Decision

R103 is implemented by a separate `agora-hermes-adapter` project. Neither engine
imports the other. The adapter owns subscription configuration, endpoint
bindings, delivery identities, visibility-safe projection, durable handoff, and
retry/revocation policy.

The first profile is a content-minimized wake notification. It identifies an
opaque subscription and stable Agora event/message reference but carries no
message body, attachment locator, Vellum target, private attention state,
membership list, credential, webhook secret, or Hermes authority. Rich payloads
require a later schema-specific privacy decision.

A trusted observer-bound Agora projection port derives the participant from an
authenticated adapter binding and revalidates current room membership before
projection. The adapter cannot nominate another observer. Private attention may
suppress or select a notification class but is never copied into Hermes.

Subscription setup, endpoint proof, secret custody, URL/network policy, device
token lifecycle, and revocation live in the adapter and endpoint-specific
adapters. Aegis authorizes distinct subscribe, deliver, inspect, and revoke
actions. Possession of a subscription ID, Hermes stream/consumer ID, or cursor
grants nothing. Revocation is durable before further endpoint offers and is
rechecked before every attempt. Membership loss also suppresses delivery.

Before Hermes append, the adapter durably allocates and binds one opaque 128-bit
delivery ID to the exact subscription, Agora source reference, schema, and
content-minimized payload. The same bytes become the Hermes idempotency key only
because the adapter owns their uniqueness; the ID is not truncated from an
Agora sequence or recipient identity. Lost append replies reconcile by replaying
the exact Hermes envelope. Changed reuse conflicts and replay-history loss stays
explicit.

The endpoint worker reads one subscription stream with its adapter-bound
consumer and revalidates subscription, visibility, and endpoint authority before
every offer. Definitely-not-sent may retry the same delivery; outcome-unknown
requires an endpoint-specific idempotency or receipt contract. Endpoint success
is durably recorded before the Hermes consumer cursor advances.

Hermes acknowledgment means only that this delivery consumer processed the
ledger event. It does not change Agora read position, attention state, message
acceptance, or participant activity. At-least-once endpoint delivery is the
baseline. A Hermes retention gap is an explicit delivery gap and is never
silently advanced as delivered.

## Required evidence

- observer-bound visibility, human/agent symmetry, and minimized diagnostics;
- subscription setup/proof/revocation and membership-loss cuts;
- exact source-to-delivery-ID mapping and conflict refusal;
- Hermes lost-reply replay, restart, cursor, acknowledgment, and gap conformance;
- endpoint definitely-not-sent/outcome-unknown/retry/acknowledgment cuts;
- no mutation of Agora read positions or domain state;
- bounded receipt-gated recovery, corruption closure, Aegis action separation,
  SDK boundaries, and commit-pinned qualification.

## Nonclaims

R103 does not provide email/SMS semantics, rich payloads, provider exactly-once,
message reads, presence, workflow completion, agent activation, federation,
legal retention, external-queue deletion, remote Agora transport, or an
eventual-delivery guarantee.
