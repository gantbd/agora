# ADR: bounded polls

Status: accepted; implementation and release qualification pending

Date: 2026-11-14

## Context

Polls are structured conversation facts, but common implementations blur four
independent concerns: who may vote, whether a ballot is disclosed, whether one
or several choices are allowed, and whether a result means approval. Agora
needs deterministic replay and participant privacy without becoming a workflow,
task, identity-directory, or wall-clock engine.

## Decision

Agora owns a stable room-local poll identity, its bounded prompt and options,
an immutable eligible cohort, one private ballot per eligible participant, a
public aggregate, and an explicit open/closed lifecycle. A poll is not a
message and does not copy or reference message, attachment, or Vellum content.

Creation is an ordinary revisioned room mutation by a current member. It
supplies a nonempty prompt of at most 1,024 UTF-8 bytes, 2 through 32 ordered
options with distinct nonempty labels of at most 256 UTF-8 bytes, a strictly
increasing cohort of 1 through 256 distinct registered current members, and a
maximum choice count from 1 through the option count. The cohort, option order,
labels, and maximum choice count are immutable. Eligibility is an Agora-local
snapshot, not a directory group or continuing membership rule. It grants no
room membership. Creation refuses before mutation if any proposed voter is not
a current member.

An eligible current member owns one ballot containing a canonical strictly
increasing set of option indices. `SetPollBallot` replaces that ballot; an
empty set retracts it. A nonempty ballot must contain no more than the immutable
maximum choice count. Repeating the exact current ballot refuses without a
revision or event. Human and agent participants obey identical laws.

Ballots are always private in this first profile. The caller may observe only
its own exact ballot. Other participants, moderators, feeds, diagnostics, and
SDK surfaces receive only per-option totals, total ballots cast, and lifecycle
state. The engine retains participant keys solely to enforce one ballot per
eligible participant and exact replacement. A room-visible ballot-change event
contains the poll identity and complete aggregate after the change, but neither
the voter nor the changed selection. Timing or aggregate deltas can still leak
inferences; this profile does not claim cryptographic anonymity.

Every accepted creation, ballot replacement/retraction, close, and reopen is a
single revisioned room mutation and emits one event. The poll creator, a current
room owner, or a current moderator may close or reopen. Closing fences ballot
changes. Reopening preserves every ballot and aggregate. Same-state lifecycle
commands refuse atomically. No trusted time, duration, automatic expiry, quorum,
winner, tie-break, or finality rule exists.

A room retains at most 256 polls. Creation at capacity evicts the oldest closed
poll by creation sequence; if no closed poll exists it refuses atomically. Open
polls are never evicted. Eviction removes poll content, ballots, and aggregates
from current observation while retained historical events remain governed by
their existing bounded journals. Poll identities and creation sequence are
never reused. Canonical recovery validates all bounds, ordering, cohort
membership identities, ballot eligibility, aggregate derivation, lifecycle,
capacity, and allocation frontiers; aggregates are derived and must exactly
match the private ballots when serialized.

## Consequences

- Poll content is a bounded shared conversation object with its own retention
  law, not a special message body or external-document reference.
- Secret ballot mappings require actor-bound commands and private observation,
  while aggregate events deliberately omit actor attribution.
- The service and Host derive the ballot owner from the authenticated client;
  no wire command nominates a voter.
- Exact request replay returns the original accepted outcome without applying a
  ballot twice; changed input under the same request identity conflicts.
- A later public-ballot, timed-poll, directory-backed, or federated profile
  requires a separately negotiated capability and cannot reinterpret these
  bytes or checkpoints.

## Required evidence

- Creation bounds, immutable option/cohort law, current-member eligibility,
  exact single/multiple choice enforcement, replacement/retraction, privacy,
  lifecycle authority, retention eviction, stale revision, and atomic refusal.
- Human/agent symmetry plus an independent bounded schedule model over vote,
  retract, close, and reopen transitions.
- Canonical recovery and legacy migration, including aggregate recomputation,
  malformed private ballot rejection, capacity/frontier closure, and retained
  event continuity.
- Closed protocol/wire tags, frozen frames, all-feed aggregate events, service
  replay/recovery, durable commit-before-publication, Host actor binding,
  client uncertainty, both SDK facades, private Unix routing, compatibility,
  fuzz closure, ordinary gate, and commit-pinned release qualification.

## Nonclaims

A ballot or aggregate is not approval, authorization, consensus, quorum,
election, identity proof, task admission, completion, priority, prediction,
delivery acknowledgment, or evidence that a participant understood a prompt.
Agora does not provide cryptographic anonymity, coercion resistance, audit
secrecy, automatic deadlines, weighted voting, delegated voting, or external
eligibility. Chronos remains the owner of time and scheduling; Moira remains the
owner of workflow meaning.
