# Transient collaboration boundary

Presence and typing are short-lived collaboration signals. They are not
conversation history, membership, authority, delivery, attention, consent, or
work-admission evidence. Agora will therefore compose them through a separate
bounded transient owner rather than storing them in the durable conversation
engine, room revision, outbox, recent-change journal, or canonical engine,
service, and Host checkpoints.

This document fixes the architecture for the first transient profile. The dark
presence and typing owners, authenticated public Host/replay composition for
presence, separate presence-only Protocol/Wire grammar, and presence client
state machine now exist, but no capability is advertised and the public
composition is not yet complete.

## Owned facts

The transient owners accept only closed values:

- presence state: `Available`, `Away`, or `Busy`;
- typing state: active or explicitly cleared;
- privacy scope: current members of one room;
- source: one opaque active Host session, never a participant kind or a display
  label;
- issuance and expiry: values from one qualified monotone clock epoch; and
- optional typing target: the room, one retained thread root, or one stable
  topic.

An observer sees `Unknown` when no currently visible, unexpired signal exists.
`Unknown` is not `Offline`: it also covers restart, expiry, retirement,
withholding, unsupported composition, and lack of a published signal. The
public projection may identify a source class such as `ParticipantSession`, but
must not expose transport handles, credential material, or session correlation
values.

Humans and agents use the same states, limits, expiry rules, and authority
checks. No state activates an agent or proves that a human or agent is available
for work.

## Authority and time

The conversation engine remains the authority for participant registration,
current room membership, retained thread roots, and topics. The transient owner
must consult current engine state at publication and observation. It must not
cache a membership decision across a room transition.

Time is an explicit composition input. The engine remains free of clocks and
timers. A qualified Host supplies monotone same-epoch time to publication,
observation, and bounded expiry work. Expiry is checked at the final observation
boundary, not inferred merely because a timer notification ran. Clock
regression, epoch ambiguity, or an unavailable required clock refuses or
returns `Unknown`; it never extends a signal.

Clients request a bounded lifetime rather than choosing an absolute trusted
expiry. The transient owner computes the expiry with checked arithmetic and a
fixed maximum lifetime. Re-publication replaces only the same source and scope;
it cannot extend another source's signal.

## Lifecycle

The Host and service composition must retire a source on disconnect,
authentication revocation, session substitution, or transport close. A leave,
kick, or ban retires every signal for that participant in that room before the
transition can become observable through a later rejoin. Expiry and explicit
clear are separate facts but have the same public `Unknown` projection.

Process restart begins with no transient signals. Checkpoints and durable
request receipts must not replay an old signal as current. A later durable
presence cache would require its own clock-continuity, privacy, migration, and
recovery profile; no such profile is selected here.

Drain stops new publication and may continue bounded read-only projection until
the owning sessions close. Shutdown does not imply that remote observers saw a
clear signal.

## Bounds and ordering

The profile must fix positive limits for total signals, sources per participant,
typing targets, requested lifetime, page size, and bounded expiry work. Capacity
refusal is atomic and cannot evict an unrelated live signal implicitly.

Each accepted source update advances a private generation. Observation cursors
bind the observer, room, engine revision, transient generation, and exact
as-of time. A changed membership revision, signal generation, observer, room,
or as-of time makes the cursor stale or foreign rather than mixing snapshots.
Canonical participant ordering comes from the engine's member page. Multiple
live sources for one participant resolve by a specified deterministic rule; the
projection never multiplies that participant into multiple public entries.
The public Host maps these private cursors to one opaque live token per session
and fixes a positive maximum cursor age no greater than the signal-lifetime
ceiling. Authentication uses current Host time while continuation projects the
original snapshot time; expiry of the cursor refuses rather than extending that
snapshot indefinitely.

Presence and typing updates do not advance the conversation revision, publish a
room event, consume the delivery outbox, or enter the recent-change journal.
They require their own content-free accepted, cleared, expired, capacity,
retired, refused, and fidelity-loss observations.

## Public-profile closure

The first advertised profile must close the feature vertically:

- closed Protocol V2 commands, outcomes, states, refusals, and capability;
- canonical Wire V2 tags, limits, malformed-input closure, and frozen fixtures;
- participant-bound Service and authenticated Host composition using trusted
  current time;
- remote-client and embedded SDK reachability without exposing engine internals;
- human/agent symmetry, privacy, expiry, lifecycle, replay, pagination, Debug
  redaction, model, mutation, and restart evidence; and
- updated compatibility, threat, effect, supply, and release ledgers.

The separately typed presence grammar and canonical codec are closed in
[`transient-wire-v1.md`](transient-wire-v1.md), without changing durable
Protocol V2. The service-owned typing profile is implemented but has no public
protocol, wire, client, or Host route. Protocol V1 remains unchanged. Until
every item above is present, Agora must not advertise transient presence or
typing capability.

## Explicit nonclaims

This boundary supplies no notification policy, unread count, delivery or read
receipt, device roster, last-seen history, analytics feed, organization
directory, cross-device consensus, wall-clock timestamp, remote push, agent
activation, task admission, or proof of attention or execution.
