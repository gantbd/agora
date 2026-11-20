# ADR: Chronos scheduled-publication composition

Status: accepted; Agora owner and private-local composition transport implemented

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

The trusted scheduled-publication contract also has a separate read-only
observation facet. It accepts the exact publication identity and canonical
intent, binds the current authenticated actor, and returns the retained
acceptance or authoritative absence without offering a message. `Absent` is a
valid owner fact because Agora never evicts accepted publication receipts and
backpressures before message mutation when the bounded receipt index is full;
it must not be generalized to a cache miss. Host revalidation is durably
committed before either result escapes. Ordinary participant Wire V2 still
does not expose this facet.

### Deployed private-local composition

Agora's private local daemon exposes the trusted facets through composition
handshake profile 4 and the dedicated scheduled route. This profile is distinct
from ordinary participant Wire V2: the session still begins with an
authenticated actor-bound conversation connect, but only the composition
profile admits scheduled publish and observe frames. Conversation,
collaboration, and search profiles reject that route.

The owner does not rely on the caller or adapter to enforce this distinction.
Opening and revalidating the Host session requires the ordinary Agora `Submit`
action; each scheduled operation then requires a second, operation-specific
decision. Publish maps to Aegis `Execute`, while retained observation maps to
`Observe`. A missing, refused, expired, failed, or mismatched decision fails
closed and retires the session. Adapter-side `Dispatch` and `InspectAgora`
authorization remains a separate cross-engine boundary and cannot substitute
for these owner-side checks.

`ScheduledLocalClientV1` is the public concrete client for this profile. It
performs endpoint hardening checks, negotiates profile 4, opens the normal
actor-bound session, validates the replay-epoch control hello, and exposes the
transport-neutral scheduled client trait. The same client can be selected by a
standalone process or by Workbench; neither deployment gets a privileged domain
API. Pre-send failures are distinguishable from post-offer uncertainty, while
malformed or wrong-operation replies never become success.

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
remote/network transport. Chronos cancellation is not Agora erasure, and Agora
acceptance is not proof that every Chronos bookkeeping write succeeded.

## Implementation status

Agora now provides the zero-dependency `agora-scheduled-publication-port`, an
embedded-SDK reexport, actor-bound Service/Host dispatch and observation,
current Host-time admission, exact keyed replay/conflict, authoritative retained
absence, and receipt-gated durable Host publication/observation. The contract
remains absent from ordinary participant Wire V2. The separate scheduled wire
codec, composition handshake profile 4, scheduled local route, and
`agora-local-client` expose publish and retained observation only through the
private-local composition profile. The daemon independently enforces `Execute`
for publication and `Observe` for observation in addition to ordinary session
`Submit` authorization. Tests cover
nonzero opaque identity, content-safe diagnostics, current interval and posting
restriction enforcement, authenticated actor binding, revocation, exact replay,
changed-intent conflict, recovery after an indeterminate committed store
outcome, closed profile/route decoding, exact operation authority, transport
certainty, and real durable Unix publish-then-observe composition through the
public client.

The composition profile now also provides an additive exact actor-provenance
observation. It reuses the identical actor-scoped publication request and fresh
`Observe` authority, then returns the retained acceptance with the Host-derived
participant. The existing compact acceptance and observation bytes are
unchanged. Focused Host evidence proves that the publishing actor receives its
identity binding while another authenticated actor receives `Conflict`; the
durable Unix journey proves reconnect, pre-reply durability and public-client
round trip. This receipt proves Agora authorship only, not workflow identity,
task linkage, delivery, acceptance or completion.

The separate adapter now has a canonical bounded six-state journal, exact-next
durable receipt gating, original-allocation conversion, cancellation-cut and
persistence-fault evidence, an Agora transport-certainty wrapper, exact replay
reconciliation, and adapter-owned late policy. It deliberately does not combine
Chronos V3 admission with a later V5 eligibility mutation: a newly admitted task
has default claimable eligibility, so those two calls have a race. Live Chronos
composition requires a qualified atomic admission-plus-eligibility capability.
An actual Workbench host factory/UI attachment and integrated release/soak
qualification remain separate work. The Chronos workspace currently contains
an unrelated categorical rewrite and was left untouched.
