# Operational service architecture

Status: volatile in-process source profiles, a store-neutral durable V2
publication boundary, one qualified local Unix checkpoint-store adapter, and one
bounded private local Unix transport composition.

The repository now carries two source profiles. `Service` preserves frozen
Protocol V1 Snapshot/Speak behavior. `v2::ServiceV2` is separately typed and adds
participant-bound room lifecycle parity plus bounded room/member/message pages.
It has a separately versioned canonical wire codec. `agora-host::v2` supplies the
separately qualified authenticated composition; V1 is not reinterpreted.

Agora follows an atomic-evolution layout: domain engine, protocol values,
serialized service owner, and client interpretation live in one repository but
remain separate crates. Their normal dependency direction is:

```text
agora-client  -> agora-protocol <- agora-service -> agora-engine
                                <- agora-wire -> keystone-codec
agora-host -----> agora-wire + agora-service + trusted verifier port
agora-durable --> agora-host + agora-service + keystone-storage port
agora-storage-directory --> keystone-storage port + local Unix filesystem
agora-aegis-verifier --> agora-host + Aegis public service port
agora-local-daemon --> agora-aegis-verifier + agora-host + agora-wire + Unix socket
```

`agora-engine` and `agora-protocol` have no normal dependencies. The protocol
does not import engine internals, and the client does not import the service.
The wire crate depends only on passive protocol values and the shared bounded
Keystone codec primitive. Transport types never become authoritative domain state.
The host composes wire and service through an injected principal-verification
port; it imports no credential or Aegis vocabulary.

The leaf local daemon owns outer framing and one accepted connection lifetime.
It supplies a trusted connection-derived Aegis context and explicit time to
Host V2 but neither imports transport state into conversation semantics nor
exposes the opaque host session on the wire.

`agora-service::transient` is a separate volatile owner beside `ServiceV2`, not
part of its durable checkpoint. Its dark profile binds expiring presence
signals to current engine membership, opaque session sources, explicit monotone
time, fixed capacities, and exact snapshot cursors. Observation is
current-member-only and projects expired or absent state as `Unknown`; it does
not advance engine revision or publish an event. Expiry reclamation is a
separate bounded drive. Generation saturation is sticky fidelity loss: new
publication and observation fence, while source and membership cleanup remain
available.

`agora-host::transient::TransientHostV1` supplies the first qualified
composition. It derives the source from an opaque authenticated Host session,
revalidates the subject and participant at current Host time for every
transient operation, retires the source on disconnect, revocation, substitution,
or clock regression, and reconciles membership after each ordinary conversation
frame before returning its reply. Reconstructing the wrapper around a recovered
Host deliberately starts empty. The wrapper has no checkpoint implementation,
Protocol/Wire grammar, or SDK surface, so no presence capability is advertised
yet. See
[`transient-collaboration.md`](transient-collaboration.md).

## Connection and identity

`Service::connect` binds a newly allocated client handle to one already
registered `ParticipantId`. Every speaking command derives its author from that
binding; the request cannot nominate another author. Snapshot observation is
limited to rooms containing that participant. Human and agent participants use
the same path.

This is identity binding, not authentication. The initial service accepts a
participant ID from a trusted host. A network host must authenticate that claim,
keep the client handle server-side or bind it to an unforgeable transport
context, and revoke the binding on disconnect. Sequential `ClientIdV1` values
must never be treated as bearer credentials.

## Serialization and replay

One `Service` owns one `Engine` and accepts calls through `&mut self`; the host
must preserve that serialized ownership. Request identity is scoped by client.
The service retains the exact request and reply. Matching retry returns the
original reply without applying a second transition; changed content under the
same request ID conflicts.

Client, retired-client, request-receipt, identifier, input, fan-out, and snapshot
limits are checked explicitly. Structurally oversized requests are refused before
receipt retention. A full receipt index backpressures a new request before the
engine is called. Disconnect releases that client's volatile receipts and keeps
a bounded recent tombstone window. Tombstone pressure evicts the oldest record;
it never blocks security cleanup. The V1 volatile profile makes no cross-restart
replay claim.

After completing bounded room state pages at one revision, a client can open a
room change position at that exact revision and pull retained room events through
an opaque client-owned cursor. Pages are exclusive-after and replayable by exact
request identity. The service consumes each successful cursor into a next cursor;
foreign, cross-room, or consumed handles share `InvalidCursor`. Independent
change-journal eviction and leave/rejoin visibility discontinuity are explicit
gap outcomes that require resnapshot. Delivery acknowledgment does not consume
the journal. Both the cursor and journal suffix are included in complete service
recovery, but the feed remains pull observation rather than delivery
acknowledgment or presence.
The additive identified-change feed uses the same engine log and continuity
laws but a distinct service cursor kind and outcome vocabulary. Cross-profile
cursor substitution refuses without consuming either cursor. Stable message
identity is reference data only; it does not alter sequence order or authority.
The one-level thread profile adds independent root and root-bound reply cursors
plus a threaded change cursor. All are client/observer/room/profile bound; reply
continuations additionally bind the exact root. Substitution refuses without
consumption. A reply names one retained non-reply root, and the immutable
relationship survives withdrawal and redaction tombstones and exact recovery.
This remains serialized bounded pull, not a subscription or execution trigger.
The separately advertised lifecycle profile stores only archived retained-root
IDs. Root authors, owners, and moderators can archive or reopen with bounded
evidence; archive blocks reply mutation but leaves pages and topology visible.
`ThreadStatus` is a participant-bound current-state observation, and all three
change profiles carry the ordered lifecycle fact. No lifecycle result is a task
or completion signal.

## Durable state and publication

`ServiceV2::prepare_checkpoint` captures the complete semantic service owner,
including private cursor state and exact request replies, under an independent
record version and store-scope binding. Recovery is bounded, canonical, and
fail-closed. Process-local usage counters are excluded so exact replay does not
manufacture a state change.

`agora-durable` owns one service and one revisioned checkpoint-store port. It
suppresses changed-state results until the store returns the exact next revision
with durable acknowledgement strength. Known refusal reconstructs the pre-offer
service. Unknown outcome, store error, weak acknowledgement, or malformed receipt
fences the owner. No mutable service access is exposed while fenced. This closes
the semantic pre-reply gate independently of any store implementation.

`agora-storage-directory` is the one qualified physical composition. It owns no
Agora semantics: it implements the Keystone store port for one key using a
private directory, exclusive process lock, checksummed canonical record, and
file-sync/atomic-rename/directory-sync commit. Its filesystem effects are
isolated by the dependency gate. The mechanism is local Unix only and does not
claim hardware power-loss, network-filesystem, backup/restore, encryption, or
multi-host behavior.

`ClientV2` mirrors that uncertainty boundary without assuming a transport. A
port reports either definitely-not-sent or outcome-unknown. Unknown retains the
exact request; only equal-command replay may cross the port until reply or
explicit abandonment. Extraction returns the pending request. The client does
not retry automatically or infer cancellation. `ClientStateV2` additionally
exports the client ID, next request namespace and optional exact pending request;
restore accepts a pending request only when it is the immediately allocated tail.
The state carries no endpoint/session proof and does not choose durable storage
or reconnect policy.

The authenticated durable composition moves the checkpoint boundary above
`HostV2`. Its canonical unit contains every active subject/participant/session/
service-client binding and the entire service record. This prevents the partial
composition in which a committed service receipt survives without the host
session needed to replay it. A recovered verifier is runtime-owned and must
revalidate current context before the retained session can act.

## Bounded reactions

`ReactionsV1` remains conversation state inside the serialized owner: commands
derive the actor from the bound client, engine set semantics enforce one
participant/message/key relation, status returns only bounded canonical counts
and caller membership, and every change feed preserves attribution. Tombstones
and thread archive retain relations; message eviction removes them. No reaction
is a notification, acknowledgement, policy grant, or workflow signal.

## Bounded message revisions

`MessageRevisionsV1` remains inside the same serialized conversation owner.
The service derives the editor from the bound client, projects only original-
author edits, and stores revision-page handles as client-owned opaque cursors
bound to room, message, observer, profile, and exact engine revision. Current
message pages remain unchanged; edit events are projected into each feed.
Checkpoint recovery preserves exact receipts, history, cursor continuation, and
tombstone scrubbing. Rich collaborative writing remains a Vellum concern.

## Stable topics

`TopicsV1` remains conversation state inside the same serialized owner. The
service derives topic creators and message authors from bound clients, keeps
topic and topic-message cursors isolated by profile and exact topic identity,
and exposes a parallel topic-aware room-change projection. Replies inherit the
root relation; edits, reactions, lifecycle changes, and tombstones cannot move
messages between topics. Topic identity grants no membership, subscription,
notification, workflow, or execution authority.

## Private read positions

`PrivateReadPositionsV1` is private current-state projection inside the same
serialized owner. The service derives the actor from the bound client and never
accepts a participant selector. Room, retained-thread, and stable-topic markers
advance monotonically to exact retained message identities at the current engine
revision. They change neither domain revision nor room events, while service and
host checkpoints retain the marker and exact request receipt. The durable owner
commits that combined state before publishing success. No marker implies
delivery, attention, unread policy, presence, activation, or task completion.

## Deliberate exclusions

The semantic profiles perform no network I/O, encryption, direct physical
storage, orchestration, task creation, model invocation, or agent lifecycle
work. The separate directory adapter performs only its documented local
filesystem I/O.
`agora-wire` only maps passive values to canonical bytes. A later
authenticated host may connect those bytes to a transport. Moira, Chronos, and
execution harnesses remain separate systems.

The V2 source service alone excludes durable receipts/cursors and restart. Its
checkpoint profile preserves those values when used through an independently
qualified store composition. Transport feature negotiation and wire framing
remain absent. Cursor handles are bounded correlation values, not bearer
credentials.

