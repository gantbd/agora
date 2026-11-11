# ADR: bounded topic supersession

Status: implemented; qualification pending

Date: 2026-11-11

## Context

Stable topics, rename, and reversible resolution preserve room-local identity,
history, and admission state. Product interfaces still need an honest way to
say that discussion should continue in another topic. Rewriting message-topic
relations would destroy stable references; deleting the source would erase
current explanation; and implicit posting redirects would make the addressed
topic differ from the authoritative accepted relation.

## Decision

Add a separately advertised `TopicSupersessionV1` profile. A current owner or
moderator may attach one successor topic to a resolved source topic in the same
room. Human and agent participants follow the same role law. The source and
successor must be distinct existing identities.

The relation is depth one. A source with a successor cannot itself be the
successor of another topic, and a successor cannot itself have a successor.
Multiple resolved sources may share one successor. This permits a bounded
"merged into" projection without introducing chains, cycles, traversal
ceilings, or ambiguous canonicalization.

Setting, replacing, and clearing the relation are explicit transitions.
Repeating the current relation is an atomic refusal. A source cannot reopen
while it has a successor; clearing the relation leaves it resolved and requires
a separate reopen command. The successor's resolution state is independent.

Supersession never moves or copies a message, reply, reaction, revision, read
position, or retained event. `SpeakInTopic` and replies are never silently
retargeted. Current state is exposed through a separate participant-bound
status observation, preserving established topic-page bytes. Every accepted
transition retains actor, source, prior successor, and new successor in every
change-feed profile.

## Consequences

- Historical topic references always continue to name the exact admitted
  topic.
- Clients can offer navigation or a merge-like projection from an explicit
  bounded fact without treating display names as identity.
- Clearing supersession is reversible metadata change, not implicit reopen.
- Cross-room move, physical deletion, alias names, automatic routing, search
  canonicalization, connector synchronization, and workflow meaning remain
  separate decisions.

## Rejected alternatives

- Rewriting retained message relations would contradict the qualified stable
  topic law and make recovery history-dependent.
- Unbounded successor chains require traversal and cycle policy at every read,
  write, checkpoint, and connector boundary.
- Automatically posting to the successor would make the caller's requested
  topic differ from the authoritative stored relation.
- Treating supersession as deletion would conflate navigation with privacy and
  retention obligations.

## Nonclaims

Supersession is not move, copy, deletion, erasure, name aliasing, task
completion, approval, notification, subscription, connector state, workflow
truth, agent activation, or execution evidence.
