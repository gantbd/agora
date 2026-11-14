# Agora domain model

Status: normative model through the implemented R97 private-attention profile. The
requirements and specification remain authoritative for individual command and
wire details; this document states the aggregate ownership and laws that those
details refine.

## Owned objects

- **Participant:** a stable Agora-local identity with disclosed `Human` or
  `Agent` kind. Kind is descriptive and never grants authority.
- **Room:** the authority and ordering boundary for membership, roles,
  admission policy, retained conversation, moderation, topics, and events.
- **Conversation profile:** the closed local cardinality and membership law for
  a room. `Ordinary` is owner-led; `Direct` and `GroupDirect` are ownerless
  immutable founding cohorts. A shared room is not a local profile.
- **Membership:** the current room relationship and role of one participant,
  distinct from authentication, presence, read state, and delivery.
- **Message:** a stable room-local identity, monotone room-local sequence,
  original author, accepted body or tombstone, addressing, optional reply/topic
  relation, bounded revisions, reactions, attachment references, and Vellum
  artifact references.
- **Topic:** a stable room-local identity with bounded current display state,
  lifecycle, aliases, tags, and optional depth-one supersession relation.
- **Room event:** an attributable accepted conversation fact with a monotone
  room position. Event identity and message identity are not ordering aliases.
- **Delivery outbox:** the lossless bounded suffix of accepted events awaiting
  external delivery acknowledgment.
- **Change journal:** an independently retained bounded observation copy used by
  pull feeds. Journal eviction creates an explicit observation gap and never
  acknowledges delivery.
- **Private read position:** participant-owned monotone room/thread/topic state.
  It is durable but is not a room event, delivery receipt, or attention fact.
- **Private attention preference:** participant-owned room/thread/topic intent
  comprising independent follow, mute, and notification fields. It is durable
  but is not a room event, delivery subscription, or evidence of attention.
- **Shared pin:** room-owned stable message relation and room event. It retains
  identity across tombstone/eviction but never retains message or artifact
  payload.
- **Private save:** participant-owned stable message relation. It is durable and
  retention-aware but is not a room event, private message copy, or export.
- **Presence and typing:** expiring, source-scoped transient collaboration
  signals. Restart reconstructs them as unknown.
- **Service request receipt:** bounded client/request evidence for exact replay
  or changed-input conflict. It grants no participant authority.
- **Checkpoint:** canonical bounded recovery bytes for one declared engine,
  service, or Host profile. Storage origin and durability remain adapter claims.

## State ownership

Agora keeps the following classes disjoint even where one canonical checkpoint
contains more than one class:

| State class | Owner | Examples |
| --- | --- | --- |
| Shared conversation authority | Engine | rooms, membership, roles, messages, topics, pins, moderation, outbox, change journal |
| Private durable projection | Engine behind participant-bound service operations | read positions, attention preferences, saved references |
| Serialized client coordination | Service | client namespace, exact request receipts, bounded cursors |
| Authenticated continuity | Host | verified subject/participant/client/context binding and revalidation state |
| Transient collaboration | Host/service transient profile | presence, typing, transient cursors and replay state |
| Physical persistence evidence | Store adapter and durable composition | storage revision, exact receipt, lock and synchronized replacement evidence |
| Operational observation | Owning layer | bounded content-safe counters, health and fidelity-loss signals |

No observation, cursor, diagnostic, transport session, storage receipt, Aegis
decision, Vellum reference, or participant kind can substitute for conversation
authority.

## Transition relation

For a current engine state `S` and a closed command `C` carrying the expected
revision, interpretation is total:

```text
apply(S, C) -> Accepted(S', outcome, event?) | Refused(S, reason)
```

Refusal preserves the requested semantic state. An accepted room mutation
publishes its attributable fact and advances the domain revision exactly once.
Operations explicitly modeled as private projection, acknowledgment, cleanup,
or observation follow their own monotone state and publication laws and do not
manufacture a room event.

Preparation and publication remain separate at durable boundaries. A successor
cannot escape as successful until an exact storage receipt proves the configured
commit. Known refusal restores the prior owner. Post-offer uncertainty fences
publication until recovery resolves which canonical checkpoint exists.

Matching request replay observes the original result without repeating the
transition. Reusing one request identity with different semantic input is a
conflict. A client that loses a reply after transmission may only replay the
exact retained request or explicitly abandon and account for uncertainty.

## Identity, order, and references

1. Participant, room, message, topic, event, request, session, and storage
   identities are distinct types and do not imply one another.
2. Stable identities survive recovery only within the profile that explicitly
   checkpoints their allocation frontier.
3. Room event order and message sequence are monotone and separate. Neither is a
   wall clock or global distributed order.
4. Replies reference one retained same-room root and do not nest. Topic relation
   is inherited from the root and cannot be changed by editing.
5. Withdrawal and moderator redaction retain identity, attribution, order, and
   reference integrity while scrubbing the bounded content promised by their
   profile. They do not claim physical erasure of prior copies.
6. Vellum artifact and attachment values are bounded conversation references,
   never locators, credentials, content ownership, or external authority.

## Observation algebra

Bounded pages and feeds are coalgebraic projections of a named owner at an exact
revision or continuation. A short page does not prove completion when a
continuation exists. Cursors are opaque, client-bound, profile-bound, and where
applicable observer/room/topic/root/revision-bound.

Observation can return explicit malformed, foreign, stale, expired, retention
gap, membership-epoch gap, incomplete, unavailable, or fidelity-loss outcomes.
No such outcome is silently promoted to an empty complete result. Outbox
acknowledgment, change-feed continuation, read position, attention preference,
presence, typing, transport delivery, and agent execution are independent facts.

## Participant symmetry and authority

For states that differ only by swapping a human and agent participant while
preserving memberships, roles, commands, and limits, Agora must produce the
same admission, refusal, transition shape, and authority result. Participant
kind remains observable attribution data but is absent from role and permission
decisions.

Authentication proves a principal binding, not room authority. The Host derives
the acting participant from the verified session; commands cannot nominate a
different actor. Agora owns membership and roles. Aegis may evaluate a closed
external action vocabulary, but its evidence cannot create conversation facts
without the ordinary Agora transition.

Addressing, mentioning, reacting, voting, resolving, or archiving never admits
work. Moira coordinates work, Chronos owns tasks and time, Daimon executes
already-authorized work, Hermes owns generic durable delivery, and Vellum owns
document content and revision history.

Conversation profile also grants no external authority. Direct and group-direct
cohorts are kind-neutral Agora participant sets, not contacts, principals, or
tenant memberships. “Shared room” requires a separately qualified federation
binding over an ordinary room; Aegis/directory provenance cannot be inferred
from a room label.

## Recovery and effects

The engine and semantic protocols perform no ambient filesystem, network,
process, credential, or wall-clock effects. Effects enter through explicit
ports and qualified adapters. The private local profile owns one serialized
Host and a qualified Unix directory/socket mechanism; it makes no remote
network, consensus, multi-writer, hostile same-UID, or hardware power-loss
claim.

Ordinary restart restores only the state named by the checkpoint profile.
Transient collaboration restarts unknown. Explicit operator restore first
invalidates recovered Host sessions and Service client state, commits that
sanitization, selects a fresh server incarnation, and rotates the local endpoint.
The qualified sequence is recoverable but is not an atomic cross-engine restore.

## Executable refinement evidence

The production representation is not its own proof model. Independent bounded
models and behavioral suites refine parts of this document:

- `engine/agora-engine/tests/addressing_model.rs` — participant-kind symmetry
  for addressing.
- `engine/agora-engine/tests/read_position_model.rs` — monotone private
  projection schedules.
- `engine/agora-engine/tests/conversation_relation_model.rs` — bounded
  reply/topic relation laws.
- `engine/agora-engine/tests/lifecycle_model.rs` — thread/topic lifecycle
  schedules.
- `engine/agora-engine/tests/conversation_profile_behavior.rs` — exhaustive
  three-peer kind/initiator/departure-order schedules, immutable-cohort laws,
  atomic cardinality refusal, privacy loss, and canonical zero-member recovery.
- `service/agora-service/tests/service_v2_schedule_model.rs` — multi-client
  serialized service schedules, replay, and kind symmetry.
- Engine, protocol, wire, service, Host, durable, adapter, SDK, compatibility,
  fuzz, interruption, and release gates listed in `docs/traceability.md` close
  the represented implementation profiles.

These bounded models do not prove arbitrary histories, distributed agreement,
external delivery, agent behavior, or physical durability beyond the exact
qualified mechanisms.

## Explicit nonclaims

Agora does not own credentials, organization directories, generic workflow,
task scheduling, agent harnesses, model behavior, document editing, blob bytes,
notification delivery, generic event-bus consumers, voice/video media,
multi-writer federation, end-to-end encryption, legal hold, eDiscovery, or
guaranteed erasure.
