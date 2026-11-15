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
agora-service --> agora-search-port
agora-service --> agora-delivery-projection-port
agora-service --> agora-interaction-port
agora-service --> agora-scheduled-publication-port
agora-service --> agora-federated-publication-port
agora-retention-authority-port --> agora-engine closed retention values
agora-export-custody-port --> agora-engine closed export values
agora-export-service --> agora-engine closed export values
agora-export-durable --> export-custody port + agora-export-service + agora-engine + keystone-storage port
agora-durable --> agora-host + agora-service + keystone-storage port
agora-storage-observation --> keystone-storage port
agora-storage-directory --> storage-observation port + keystone-storage port + local Unix filesystem
agora-search-memory --> agora-search-port
agora-sha256 --> agora-engine closed digest value
agora-search-port --> no normal dependencies
agora-delivery-projection-port --> no normal dependencies
agora-interaction-port --> no normal dependencies
agora-scheduled-publication-port --> no normal dependencies
agora-federated-publication-port --> no normal dependencies
agora-aegis-verifier --> agora-host + agora-durable + Aegis public service port
agora-local-transport --> agora-wire + Unix endpoint/stream resources
agora-local-daemon --> agora-local-transport + agora-aegis-verifier + agora-host + agora-durable + agora-wire
```

`agora-engine` and `agora-protocol` have no normal dependencies. The protocol
does not import engine internals, and the client does not import the service.
The wire crate depends only on passive protocol values and the shared bounded
Keystone codec primitive. Transport types never become authoritative domain state.
The host composes wire and service through an injected principal-verification
port; it imports no credential or Aegis vocabulary.

Derived search uses `SearchRuntimeV1` as a volatile sidecar over a borrowed
authoritative `ServiceV2`. The embedded `SearchServiceV1` convenience owner and
authenticated `SearchHostV1` both delegate to that same runtime; neither clones
the engine or creates a second conversation authority. The Host wrapper routes
ordinary mutations through its one Service V2, retires all derived continuations
when revision changes, and retires per-client receipts/cursors on close,
revocation, or client substitution. Search runtime state is deliberately absent
from Service/Host checkpoints.

The leaf local transport owns outer framing and one accepted connection
lifetime without authority or domain knowledge. The daemon application supplies
a trusted connection-derived Aegis context and explicit time to
either the volatile Host V2 or receipt-gated Durable Host V2, but neither imports
transport state into conversation semantics nor exposes the opaque host session
on the wire. The durable selection reuses the generic checkpoint-store port and
has clean physical-store restart evidence; disconnect still retires the service
client receipt namespace, so it is not reconnect replay.

`agora-service::transient` and `agora-service::typing` are separate volatile
owners beside `ServiceV2`, not part of its durable checkpoint. Their
profiles bind expiring presence or typing signals to current engine membership,
opaque session sources, explicit monotone time, fixed capacities, and exact
snapshot cursors. Typing additionally revalidates the room, retained non-reply
thread root, or stable topic at publication and observation. Observation is
current-member-only and projects expired or absent presence as `Unknown` and
typing as inactive; neither advances engine revision or publishes an event.
Expiry reclamation is a separate bounded drive. Generation saturation is sticky
fidelity loss: new publication and observation fence, while source, participant,
membership, and invalid-target cleanup remain available.

`agora-host::transient::TransientHostV1` supplies the first qualified
composition. It derives the source from an opaque authenticated Host session,
revalidates the subject and participant at current Host time for every
transient operation, retires the source on disconnect, revocation, substitution,
or clock regression, and reconciles membership after each ordinary conversation
frame before returning its reply. Reconstructing the wrapper around a recovered
Host deliberately starts empty. The wrapper has no checkpoint implementation,
but its separate public handler authenticates each request and owns one exact
volatile replay receipt plus one opaque live page cursor per Host session. The
presence/typing Protocol/Wire grammar and replay-epoch-bound client state machine
are composed without entering durable Service V2 receipts. Presence and typing
share authenticated replay sequencing and one live cursor slot per session,
while retaining independent owner limits and typed cursor/target validation.
The private local collaboration daemon routes this wrapper through a distinct
handshake profile and binds each transient client replay epoch to the declared
server incarnation plus one exact authenticated connection. The ordinary
durable capability list and conversation-only transport profile remain
unchanged. See
[`transient-collaboration.md`](transient-collaboration.md).

`agora-durable::DurableTransientHostV1` supplies the combined deployment owner.
It receipt-gates the complete underlying Host checkpoint, including session
verification time, while deliberately excluding both transient owners and their
runtime cursor/replay state. Known store refusal restores the Host and retains
privacy-conservative transient retirement; uncertain durability fences the
owner and retires the affected source. Recovery wraps the durable Host in fresh
empty transient owners.

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

The R101 profile keeps admission authority explicit. `ServiceV2::execute_at`
accepts trusted host time outside the protocol request, derives one opaque
engine observation, and routes all six message forms through the same admission
choke point. Roles and participant kinds do not bypass the interval. Posting
restriction remains a revisioned room fact, while reports and their reviews are
private service outcomes and checkpoint state rather than room events. Reporter
views and moderator queues are separate commands; no classifier, Aegis result,
or review disposition performs a moderation transition.

Client, retired-client, request-receipt, identifier, input, fan-out, and snapshot
limits are checked explicitly. Structurally oversized requests are refused before
receipt retention. A full receipt index backpressures a new request before the
engine is called. Disconnect releases that client's volatile receipts and keeps
a bounded recent tombstone window. Tombstone pressure evicts the oldest record;
it never blocks security cleanup. The V1 volatile profile makes no cross-restart
replay claim.

The R107 administrative path is outside participant protocol. It accepts only a
canonical complete selection of retained V2 receipt keys at the current engine
revision, preflights bounded non-reexecution tombstones atomically, and removes
the receipts only after every tombstone is admissible. Service checkpoint
profile 30 preserves the exact retired identities. Host revalidation plus the
injected retention-authority port gates the operation, and Host profile 27 nests
the Service profile without interpreting policy. `DurableHostV2` applies its
ordinary complete-checkpoint commit gate, so known store refusal restores the
receipts while a committed retirement survives restart. This is neither general
logical compaction nor evidence of physical deletion.

Service V2 separately retains the current installed policy metadata. A proposal
must name the current engine revision; equal revision and equal policy is replay,
equal revision with changed bounds conflicts, and policy revision regression is
inert. Service profile 31 and Host profile 28 recover the policy together with
receipt tombstones. This installation step authorizes no discard by itself.

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

Authenticated durable Host owners additionally expose passive publication
observations. They distinguish durable commits, already-committed results,
checkpoint suppression, known refusal, uncertainty, weak or invalid receipts,
and calls after fencing with saturating content-free counters. Daemon
composition preserves the transport and durability groups after a fence but
omits the no-longer-trustworthy Host/Service projection. These process-local
facts are excluded from checkpoints and cannot affect a result.

`agora-storage-observation` is an effect-free optional extension of the
checkpoint-store port. It lets a physical adapter provide an associated passive
snapshot and fidelity bit without making `agora-durable` or the daemon depend on
that adapter. The composed daemon view retains transport, available runtime,
durable publication, and physical store groups separately; it does not infer
semantic success from physical counters or claim a concurrent atomic snapshot.

`agora-storage-directory` is the one qualified physical composition. It owns no
Agora semantics: it implements the Keystone store port for one key using a
private directory, exclusive process lock, checksummed canonical record, and
file-sync/atomic-rename/directory-sync commit. Its filesystem effects are
isolated by the dependency gate. Its passive owner-local observation separates
loads, offers, commits, refusals, indeterminate publications, and integrity
fences without exposing keys, paths, bytes, or record size. The mechanism is
local Unix only. Its offline backup profile copies the exact canonical record
under the source lock and restores only into a new private directory while
preserving revision continuity. Backup content is sensitive, and the profile
classifies interruption at every export and restore persistence boundary as an
invalid/verifiable artifact, empty destination, pending residue, or exact
post-rename recovery. It does not automate cleanup or claim hardware power-loss,
network-filesystem, encryption, remote transport, rollback resistance,
disaster-recovery objectives, or multi-host behavior.

R107 export custody is intentionally not part of `ServiceV2`. Materialized
exports are new copies with independent access, capacity, persistence,
retirement, and reclamation obligations. The adopted `agora-export-service`
owns a bounded exact registry and its own scope-bound, digest-verified canonical
checkpoint. Admission binds the materialization to caller-supplied current
engine and policy revisions before mutation; replay, changed-request conflict,
capacity refusal, and canonical recovery are exact. It does not project source
records, authorize export, or expose participant protocol. The separate pure
`agora-sha256` adapter implements only the engine's named digest profile; standard
empty, single-block, multi-block, million-byte and padding-boundary vectors plus
Agora materialization/recovery and mutation-refusal tests qualify that binding.
`ServiceV2::export_source` adds one deliberately narrow source profile: it
captures the exact canonical complete service checkpoint as a single
`OperationalCopies` record under the administrative export profile. Capture
requires an installed current policy with that class enabled, binds the current
engine and policy revisions, uses a monotone revision-derived record ordinal,
and preserves the checkpoint's own recovery scope. It does not relabel nested
room, change, receipt, private, or federation bytes as class-specific records.
The separate adopted `agora-export-durable` owner publishes that checkpoint
under its own configured store key and revision line. Only an exact strong
receipt publishes a new custody result; equal replay does not re-offer storage,
known refusal restores the prior checkpoint, and store error, indeterminate
outcome, weak acknowledgement, or invalid receipt fences the live owner until
fresh recovery. Concrete physical composition and selection of the qualified
digest provider remain enclosing trusted-host obligations.
`agora-export-custody-port` erases storage vocabulary while preserving exact
materialized/replay success, committed/previously-committed evidence, stale or
conflicting admission, capacity, definite noncommit, unknown outcome, fencing,
and unavailability. `HostV2` revalidates the administrative session, rejects a
noncurrent source or policy before authority I/O, obtains a fresh decision over
the exact manifest identity/profile/scope/revisions, and only then calls this
port. Humans and agents follow the same administrative path. Prepared source
truth and digest-provider composition remain prerequisites outside Host.
The existing directory adapter also cannot support R107 physical
reclamation: its v1 atomic replacement unlinks the prior head before a separate
reopen-validation step. A generational storage profile is required rather than
reinterpreting that mechanism as reclamation evidence.

## Derived search

`agora-search-port` is an effect-free observation boundary, separate from the
conversation engine and its recovery graph. Queries are room-scoped and bounded;
pages contain only stable message identity and room-local sequence, plus exact
source revision and opaque index generation. Available, incomplete, stale, and
unavailable states are distinct. The port receives no participant identity and
cannot decide authority.

`agora-search-memory` is the bounded volatile semantic reference. Explicit room
projection replacement is revision-monotone, advances generation, invalidates
old cursors, and preserves atomic refusal under shape/content/cursor capacity.
It performs byte-exact UTF-8 substring matching and never returns indexed body
content. It is not durable, production-scale, or a service availability claim.
The participant-bound composition must still derive the authenticated observer,
check current membership, and resolve every hit against retained authoritative
history before publishing it.

The additive `agora-service::search::SearchServiceV1` now supplies that first
participant-bound composition. It derives the observer from an active Service
V2 client, derives the current engine revision, checks membership before index
work, and resolves each identity, sequence, and current byte-exact match before
publishing. Its separate bounded exact receipts and client-owned one-shot cursor
registry do not enter Service V2 checkpoints. Conversation mutation and client
disconnect retire exposed adapter cursors. Protocol values exist, but canonical
wire, authenticated Host, client uncertainty, SDK, and local transport routing
are still open and the capability is not advertised.

Physical restore deliberately remains Agora-semantic-free. The higher
`DurableHostV2::recover_after_restore` boundary decodes the restored Host unit,
retires all recovered sessions and disconnects their Service clients, then
durably CAS-publishes the sanitized canonical Host checkpoint before returning
an active owner. This preserves rooms, messages, events, outbox state, and
monotone session/client frontiers while removing private cursors and replay
receipts tied to pre-restore connections. Ordinary clean restart continues to
use `recover` and retains exact session replay.

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

`TopicRenameV1` remains a mutation of that same serialized owner. The service
derives the actor, the engine admits only current owners/moderators, and the
transition replaces only the current bounded display name while retaining the
stable ID and every message relation. Exact replay and profile-isolated change
positions are checkpointed with the prior/new-name event; capability discovery
does not authorize the mutation.

`TopicResolutionV1` is a reversible lifecycle mutation of the same owner. The
service derives the actor; resolve and reopen retain exact replay while the
engine fences new topic roots and inherited replies only during the resolved
state. A separate participant-bound status observation and all change profiles
project the state without rewriting identity, history, or legacy topic-page
bytes. Resolution is not Chronos task completion or agent
orchestration evidence.

`TopicSupersessionV1` is a depth-one navigation relation over that owner. The
service derives the actor and exposes explicit set, replace, clear, and status
operations. The engine requires a resolved source and refuses source/target
role overlap, while allowing multiple sources to share one successor. No
message or post is moved, copied, or silently retargeted; clearing the relation
does not reopen the source. Exact receipts, all-feed attribution, and the graph
survive service and host recovery.

`TopicAliasesV1` is bounded display metadata over that same stable topic owner.
The service derives the actor and exposes explicit add, remove, and status
operations. The engine admits at most 16 aliases per topic under the existing
name ceiling and one room-wide collision domain spanning current names and
aliases. Status is participant-bound and canonically ordered; every mutation is
attributed in all feed profiles and retained by exact receipts and nested
recovery. Aliases never become selectors, redirects, authority, or workflow
state.

`TopicTagsV1` is a distinct bounded display/index set over the stable topic
owner. The service derives the actor and exposes explicit add, remove, and
status operations. The engine admits at most eight byte-exact tags per topic;
duplicates and absent removals refuse atomically, while the same tag may appear
on multiple topics or equal a name/alias. Status is participant-bound and
canonically ordered; every mutation is attributed in all feed profiles and
retained by exact receipts and nested recovery. Tags never become selectors,
hierarchy, permissions, routing, workflow state, or execution evidence.

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

