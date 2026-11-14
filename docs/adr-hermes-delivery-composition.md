# ADR: Hermes notification-delivery composition

Status: accepted; minimized foundation qualified; endpoint profiles open

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

The first profile is a content-minimized wake notification. The Agora-derived
projection carries only its retained source event sequence and a closed wake
class. Subscription and delivery correlation remain adapter-owned envelope
metadata. The projection carries no room or message reference, message body,
attachment locator, Vellum target, private attention state, membership list,
credential, webhook secret, or Hermes authority. Rich payloads require a later
schema-specific privacy decision.

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

## Implemented Agora slice

Agora provides a zero-dependency trusted projection port plus Service, Host,
durable Host, and embedded SDK surfaces. Projection derives the observer from
the authenticated client/session, checks current membership at the referenced
retained event, returns explicit gap/unavailable/not-visible outcomes, and does
not mutate domain state, event acknowledgments, or read positions. Host session
revalidation is receipt-gated before a projection can escape, including exact
recovery after an indeterminate commit. Ordinary Wire V2 exposes no projection
operation.

The separate adapter consumes Agora's projection vocabulary directly. Revisions
`59483d0`, `e50ab79`, `f833716`, `f1d5e6a`, and `d4558cc` provide the canonical
projection contract, strong receipt-gated subscription/delivery journals,
exact Hermes append replay and reusable inbox/outbox conformance, current-
visibility endpoint offer ordering, durable success before consumer
acknowledgment, cursor reconciliation, revocation cuts, and distinct Aegis
subscribe/deliver/inspect/revoke actions. Revision `cdb81a6` adds frozen
compatibility, exact supply and package inventories, exhaustive checkpoint
mutation closure, and deterministic recovery soak evidence; revision `6228986`
pins the passing qualification to that source commit.

Endpoint setup/proof, credential custody, URL/network policy, and provider-
specific idempotency or receipt reconciliation remain open profiles. The
generic foundation never retries an outcome-unknown endpoint offer without
that evidence and does not claim a provider delivery guarantee.

## Nonclaims

R103 does not provide email/SMS semantics, rich payloads, provider exactly-once,
message reads, presence, workflow completion, agent activation, federation,
legal retention, external-queue deletion, remote Agora transport, or an
eventual-delivery guarantee.
