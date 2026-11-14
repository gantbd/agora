# ADR: admission and moderation controls

Status: accepted; implementation complete; release qualification open

Date: 2026-11-14

## Context

Agora already owns room membership, roles, immutable message attribution, kick,
ban, unban, moderator redaction, bounded reasons, and opaque causation. It does
not yet own a rate-admission rule, a non-removal posting restriction, or private
participant reports. Product precedents commonly call these slow mode, timeout,
report, and moderation queue, but frequently mix client clocks, classifier
scores, administrator bypasses, and workflow outcomes into one policy surface.

The existing message `LogicalTime` is caller-supplied conversation metadata. It
is not trusted elapsed-time evidence and cannot safely enforce admission. Aegis
may evaluate external policy, and classifiers may offer signals, but neither may
become an alternate writer of Agora conversation facts.

## Decision

R101 is one separately advertised additive profile with three independent
parts: room admission interval, posting restriction, and private report/review
facts. Existing moderation commands and every legacy byte retain their current
meaning.

### Trusted admission time

The engine owns a non-regressing `AdmissionTimeV1` floor and issues the sole
current opaque observation for that floor. Equal instants issue a fresh sequence
and stale the prior observation. An observation is namespace-bound, sequence-
bound, content-safe in diagnostics, and invalid after recovery. Recovery retains
the floor and sequence but creates a fresh observation namespace. This follows
Chronos's explicit-evidence discipline without adding a Chronos dependency:
slow-mode admission is an immediate Agora invariant, not a scheduled task.

The authenticated Host converts its already trusted, monotone `HostTimeV1` into
engine admission-time evidence and supplies it beside a message command. The
wire request never carries trusted time. Direct trusted service and embedded
callers must supply the same explicit evidence. Existing caller-supplied
`LogicalTime` remains visible message metadata and has no admission authority.
Clock regression, foreign evidence, stale evidence, or missing evidence while a
temporal rule is active refuses before conversation mutation.

### Room admission interval

An ordinary room has one owner-controlled minimum interval from zero (disabled)
through 86,400,000 milliseconds. The rule applies identically to every current
participant and every message admission form: room, reply, topic, addressed,
attachment-bearing, and Vellum-artifact-bearing. Role and participant kind do
not bypass it. The interval is measured from that participant's last accepted
message in that room using trusted admission time. Refusal returns only the
bounded remaining interval to the actor and changes no state, receipt, revision,
event, message sequence, or last-accepted instant.

Changing the interval is a stale-guarded revisioned owner mutation with one
attributable room event containing prior and new values. Disabling the interval
does not erase the retained last-accepted instants; re-enabling therefore cannot
manufacture a more recent acceptance. Leaving or being removed makes the state
inaccessible but does not silently reset it; ordinary re-entry resumes the same
room-local participant history. Immutable direct/group-direct cohorts do not
admit this owner-governed policy.

### Posting restriction

`SetPostingRestricted` is the minimum new moderation action. It sets or clears
one indefinite room-local restriction under the existing role hierarchy:
owners may act on non-owners, moderators on members, and nobody on self. It is
available only in ordinary rooms, carries a bounded nonempty reason and opaque
causation, is stale-guarded and idempotence-refusing, and emits one attributable
room event. Restriction blocks every message admission form but does not block
leave, report creation, observation, read/attention state, or cleanup. It does
not imply ban, membership removal, silence in another room, classifier truth,
or a timed sentence. Timed restrictions require a later separately qualified
expiry law.

### Reports and reviews

A current member may create a stable room-local report against one retained
message visible through that membership. A report retains reporter, target
message and author identities, bounded nonempty reason, opaque causation,
creation order, and `Pending` state. It copies no message body, attachment, Vellum
reference, reaction, or revision content. Human and agent reporters obey the
same law. One reporter may have at most one open report for the same target.

Reports are not room events. A reporter may observe their own reports; current
owners and moderators may observe the room's bounded report set. No other member learns that
a report exists. An authorized reviewer may close an open report with exactly
`Dismissed` or `Escalated`, a bounded review reason, and opaque review
causation. `Escalated` is a review classification, not proof that kick, ban,
redaction, restriction, Aegis action, workflow, or external notification
occurred. Any moderation command remains a separate ordinary Agora transition
with its own authority, replay, event, and causation.

A room retains at most 256 reports. Creation at capacity evicts only the oldest
closed report; if every report is open it refuses atomically. Report identities
and creation order are never reused. Message tombstone or history eviction does
not rewrite the report's stable target identity, but current target availability
is reported explicitly. Membership loss gates private access without silently
deleting retained facts.

### Aegis and classifier boundary

The passive external action vocabulary adds `ConfigureAdmission`, `Report`,
`ReviewReport`, `RestrictPosting`, and `RestorePosting`. Aegis acceptance is
correlation evidence only; Agora still derives the actor from the authenticated
session and enforces every room invariant in the ordinary transition.

External classifiers have no Agora command, participant identity, write port,
or authority tag. A host may present bounded classifier evidence to Aegis or a
moderator-facing adapter, but a score cannot create a report, close a report,
restrict a participant, redact content, or change admission. Persisting typed
classifier assertions would require a later provenance, retention, privacy,
appeal, and re-evaluation contract.

## Consequences

- Enforcement time is explicit and recoverable without trusting message
  timestamps or adding ambient clock I/O to the engine.
- Slow mode and posting restriction share the message-admission choke point but
  remain distinct facts with distinct authority and refusal semantics.
- Report privacy requires isolated observation, receipt, recovery, and
  wire profiles rather than adding report variants to ordinary room feeds.
- Moderation review records human/agent participant judgment without pretending
  that review is workflow completion or an external policy decision.
- The feature expands Agora's conversation-governance substrate but adds no
  harness, agent behavior, task, scheduling, delivery, or classifier runtime.

## Rejected alternatives

- Using caller `LogicalTime` permits trivial throttle bypass and rollback.
- Exempting owners, moderators, or agents creates hidden role/kind admission
  paths; explicit policy changes are safer than implicit bypass.
- Automatically mutating from classifier output creates a second authority path
  with no participant attribution or Agora replay identity.
- Publishing reports in room feeds leaks reporters and allegations to subjects
  and unrelated members.
- Treating `Escalated` as action completion creates a false cross-transition or
  cross-engine atomicity claim.
- A timed posting restriction without explicit expiry, recovery, and clock laws
  smuggles scheduling semantics into a Boolean moderation flag.

## Required evidence

- Opaque time observation regression/foreign/stale/recovery laws; trusted Host
  derivation; no wire-supplied enforcement time; exact refusal atomicity.
- All message forms at the interval boundary, unchanged last-acceptance state on
  refusal, leave/rejoin continuity, no implicit role/kind bypass, and an
  independent bounded schedule model.
- Restriction hierarchy, all-message admission coverage, unaffected non-message
  commands, reason/causation bounds, event attribution, and canonical recovery.
- Report identity, privacy, duplicate-open rule, target availability, review
  authority/state, closed-only eviction, exact replay, recovery, and
  human/agent symmetry.
- Closed protocol/wire capabilities and tags, frozen frames, service cursor and
  receipt recovery, durable commit-before-publication, authenticated Host,
  uncertainty-safe client, both SDK facades, real private Unix routing,
  compatibility/fuzz closure, ordinary gate, and commit-pinned release
  qualification.

## Nonclaims

R101 does not provide spam classification, toxicity truth, appeal workflow,
case management, moderator assignment, audit export, legal hold, automatic
sanctions, timed restrictions, cross-room sanctions, federation, delivery,
identity reputation, task admission, or agent execution. A report is an
attributed allegation; a review is an Agora-local classification; neither is a
finding of fact.
