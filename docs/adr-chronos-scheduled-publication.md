# ADR: Chronos scheduled-publication composition

Status: accepted; implementation open

Date: 2026-11-14

## Context

Chronos owns tasks, eligibility time, cancellation, leases, attempts, retry, and
uncertain-effect reconciliation. Agora owns authenticated participants, current
room authority, message admission, ordered events, and a recoverable keyed
publication primitive. A scheduled chat message needs both engines, but neither
engine should import the other's state machine or pretend their commits are one
transaction.

Chronos task identity alone is not an Agora idempotency key: task names are
bounded strings, incarnations are scoped to one Chronos service, and truncating
or hashing that pair would either lose scope or introduce an unstated collision
law. Agora request IDs also cannot solve the problem because they are scoped to
one service client and are intentionally reset by disconnect. The composition
therefore needs its own durable correlation identity and recovery record.

## Decision

R102 is implemented by a separate `agora-chronos-adapter` project. Agora and
Chronos keep their existing dependency directions and public ownership. The
adapter composes their qualified ports and owns only the cross-engine intent,
correlation, and reconciliation record.

### Schedule publication identity

Before Chronos admission, the adapter allocates one opaque nonzero 128-bit
`ScheduledPublicationIdV1`. It is never reused. A durable adapter record binds
that ID to all of the following:

- one configured opaque Chronos service scope;
- the original Chronos task reference, including incarnation, once admitted;
- one authenticated Agora participant binding;
- one canonical bounded Agora plain-room message intent; and
- the adapter state and exact retained outcomes needed for reconciliation.

The ID is generated independently; it is not a truncated task name, task
incarnation, request ID, timestamp, or content digest. The Chronos task and its
eligibility remain the scheduled object. The adapter record is correlation and
payload custody, not a second scheduler. Chronos stores no Agora message body.

### Trusted Agora publication port

Agora adds a transport-neutral trusted scheduled-publication port above Service
V2. The port accepts the publication ID and canonical intent, derives the author
from the authenticated Agora session, and supplies current trusted Host time for
the ordinary message-admission rules. The caller cannot nominate a different
author or provide enforcement time.

First acceptance evaluates current membership, room existence, posting
restriction, minimum-message interval, content bounds, capacity, and every
ordinary message invariant at the actual admission instant. Schedule creation
does not reserve membership, bypass moderation, pin a room revision, or grant
future speaking authority. The initial profile publishes only a plain room
message and addresses nobody.

An accepted publication durably binds the ID to the canonical intent and exact
Agora transition. Matching replay returns that transition without another
message, even after later room revisions or adapter restart. Reusing the ID for
different room, author, logical time, or body conflicts. A known pre-mutation
refusal does not fabricate a retained success and may be retried only under the
adapter's explicit Chronos policy. Receipt capacity backpressures before message
mutation.

The port is not part of the ordinary participant Wire V2 command set. A normal
client can speak immediately but cannot assert truthful schedule provenance.
The authenticated composition root exposes the trusted port only to the
scheduled-publication adapter under a distinct Aegis action and resource policy.

### Durable adapter state

The adapter uses a canonical receipt-gated store record with a monotone state:

1. `Prepared`: canonical Agora intent and publication ID are durable, but no
   Chronos allocation is claimed.
2. `Scheduled`: the exact original Chronos task allocation is durable.
3. `Dispatching`: the adapter durably records that an Agora offer may exist.
4. `Published`: the exact accepted Agora transition is durable.
5. `Cancelled`: cancellation became final before any Agora offer could exist.
6. `ReconciliationRequired`: an outcome cannot safely be classified without
   querying/replaying the exact existing identities.

Every transition is exact-predecessor receipt-gated. Weak, malformed, failed,
or outcome-unknown persistence fences the adapter. Recovery never invents a
Chronos task reference, Agora success, or cancellation result.

### Cancellation races

Cancellation before durable `Dispatching` may become `Cancelled`; the adapter
then never calls Agora. Once `Dispatching` is durable, cancellation cannot prove
that no Agora mutation occurred. The adapter must replay the identical Agora
publication ID and intent or inspect its retained result before classifying the
schedule.

If Agora accepted, publication wins the cross-engine race: later Chronos
cancellation prevents future work but does not retract, redact, or erase the
message. If Agora is known not to have accepted and Chronos is terminally
cancelled, the adapter may close as `Cancelled`. Conflicting or unavailable
evidence enters `ReconciliationRequired`; it is never resolved by issuing a new
publication ID or a new Chronos task.

### Late execution and unavailable Agora

Chronos and the adapter policy decide whether an eligible task is dispatched,
skipped, coalesced, or considered too late. Agora interprets no scheduled time,
lateness, deadline, or catch-up rule. An unavailable Agora endpoint is not
message acceptance. A definitely-not-sent result may follow Chronos retry
policy; an outcome-unknown result must reconcile the exact publication ID before
another offer. All retries re-evaluate current Agora authority while preserving
accepted-result replay.

### Provenance and observation

The accepted Agora message remains an ordinary attributed participant message.
The adapter receipt exposes the schedule publication ID and original Chronos
task reference to authorized operators without copying credentials, leases,
grants, or message content into diagnostics. The initial room event does not
publish Chronos task identity to room members. A future visible “scheduled”
badge requires a separately versioned privacy and federation contract.

## Consequences

- Both engines retain single-writer authority over their own facts.
- Exact Agora replay closes lost replies without treating Chronos retry as an
  idempotency mechanism.
- Current Agora authority wins over authority observed when the schedule was
  created.
- The adapter must durably retain sensitive bounded message intent until its
  retention policy permits removal.
- Cross-engine completion is a reconciled composition result, not an atomic
  transaction or task/message equivalence.

## Rejected alternatives

- Importing Chronos into Agora would turn scheduling into conversation state.
- Importing Agora into Chronos would make task admission depend on room policy
  and message payloads.
- Deriving a 128-bit key by truncating a task reference creates silent aliases.
- Using Agora service request IDs fails across client retirement and reconnect.
- Treating Chronos cancellation as proof of message absence loses the
  cancellation-versus-publication race.
- Publishing with the schedule-time room revision or role snapshot grants
  stale future authority.
- Automatically deleting a message after cancellation invents a moderation
  transition and erasure claim.

## Required evidence

- independent ID allocation, non-reuse, canonical intent equality, and
  conflict refusal;
- authenticated actor binding and current membership/moderation/admission-time
  enforcement without participant-kind or role bypass;
- matching replay before and after Agora/adapter recovery with one message;
- every cancellation cut around prepare, Chronos admission, dispatch record,
  Agora acceptance, adapter receipt, and reply delivery;
- definitely-not-sent versus outcome-unknown Agora failures and exact
  reconciliation without new identities;
- Chronos original-allocation recovery after task retirement/name reuse;
- unavailable and late-dispatch policy cases without importing clock semantics
  into Agora;
- bounded canonical adapter storage, content-safe diagnostics, capacity
  refusal, corruption/truncation closure, and receipt-strength validation;
- Aegis action separation, dependency-direction gates, both SDK facades, and
  commit-pinned release qualification in the adapter and affected siblings.

## Nonclaims

R102 does not provide calendars, recurrence, time zones, reminders, message
editing, scheduled attachments, scheduled Vellum references, addressing,
approval, workflow completion, agent execution, atomic cross-engine commits,
automatic moderation bypass, message retraction, delivery, federation, or a
network transport. Chronos cancellation is not Agora erasure, and Agora
acceptance is not proof that every Chronos bookkeeping write succeeded.
