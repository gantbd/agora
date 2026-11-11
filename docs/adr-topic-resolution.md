# ADR: reversible topic resolution

Status: implemented and qualified

Date: 2026-11-11

## Context

Stable topics and rename establish identity, current display metadata, retained
history, and immutable message relations. They do not define when a topic stops
accepting conversation, whether that state can be reversed, or what survives a
lifecycle transition. Importing task completion or orchestration semantics here
would collapse Agora into Chronos or an agent harness.

## Decision

Add the separately advertised `TopicResolutionV1` profile. Current room owners
and moderators may resolve or reopen an existing topic; humans and agents obey
the same role law. Repeating the current state is an atomic invalid lifecycle
change.

A resolved topic refuses both new topic roots and replies whose retained root
belongs to that topic. Reopening admits them again. Resolution does not change
the topic ID, creator, current name, message/reply relations, reactions,
message revisions, retained history, or private read positions. Rename remains
independent and may occur while resolved.

Every accepted transition records the actor, topic ID, prior state, and new
state in all four change-feed profiles. A separate participant-bound status
observation exposes current state without changing established topic-page bytes.
Engine/service/host checkpoints preserve both state and retained event chains
at profiles 16/18/18.

`resolved` means only that Agora currently fences new conversation in the
topic. It is not evidence of task completion, approval, delivery, attention,
agent execution, connector synchronization, or external workflow state.

## Consequences

- Stable references and historical projections survive resolve and reopen.
- Clients can render a closed/reopened conversation collection from explicit
  facts without inferring workflow truth.
- Move, merge, deletion, aliases, subscriptions, notifications, and external
  connector policy remain separate decisions.
