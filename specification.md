# Core specification

For configured limits, an admitted state and revisioned command define a total
deterministic transition:

```text
(State, RevisionedCommandV2) -> (State, Result<TransitionV2, Refusal>)
```

Expected invalidity is a `Refusal`; it is the identity state transition. Every
accepted command increments the engine revision and event sequence exactly once
and appends exactly one event. Revision conflict, outbox saturation, and counter
exhaustion are checked before the domain command runs.

## State and authority

An engine owns participants, rooms, revision/event cursors, a bounded event
outbox, and bounded publication receipts. A room owns its admission policy,
members and roles, invitations, retained messages, and next message sequence.
Collections with observable order are ordered.

- A room creator becomes owner and first member.
- Owners and moderators may invite registered participants.
- Only owners may promote or demote moderators.
- Open rooms admit registered participants with capacity.
- Invite-only rooms require a pending invitation, consumed on join.
- Owners cannot leave; ownership transfer is absent in v0.3.
- Any member may speak. Participant kind grants no authority.

The host authenticates command actors. Supplying a `ParticipantId` is not proof
of identity, authority outside the room, or possession of an Aegis grant.

## Participant-neutral addressing

`SpeakAddressed` atomically retains a message and its addressed participants.
The list must be strictly increasing by participant ID, contain only room members
at acceptance, and fit `max_addressed_participants_per_message`. Later membership
changes do not rewrite historical messages. Validation precedes message sequence
allocation and retention eviction. `Speak` is the exact empty-addressing case.

Participant kind is descriptive metadata and is not consulted by addressing,
admission, role, or speaking transitions. Agora does not wake a participant,
create work, select a harness, or infer that an addressed participant responded.
An adapter may offer accepted message facts to Moira; orchestration belongs there.
The qualified local profile exposes a pending event as a read-only fact and
requires a separate explicit Moira admission decision; it never projects an
Agora address into execution authority.

## Event outbox

Accepted commands append one `EventEnvelopeV2` containing the event sequence,
accepted revision, and typed domain event. Saturation refuses before state change.
Acknowledgment may advance only through the published edge and cannot regress.
Acknowledged prefixes are removed. The in-memory outbox is not durable evidence.

## Recent change journal

Every accepted command also appends the same ordered envelope to an independent
bounded recent-change journal. This journal is pull-observation state, not a
delivery queue: reaching `max_retained_changes` evicts the oldest journal copy
and advances an exact `discarded_changes_through` edge without refusing the
domain mutation or acknowledging the delivery outbox. Conversely, delivery
acknowledgment removes no journal entry. Outbox backpressure and journal
eviction therefore cannot substitute for each other.

The outbox and journal copies are appended within the same deterministic engine
transition and retain equal event sequence, revision, and event value. Message
withdrawal or moderation redaction scrubs every still-retained copy in both
structures. Canonical profile-14 recovery preserves the independent suffixes
and edges; neither queue is reconstructed by inference from the other.

## Message retention

Each accepted message receives the room's current sequence and caller-supplied
logical time. Checked increment precedes publication. At capacity, the oldest
retained message is evicted before insertion. Zero retention refuses speaking.
Eviction affects retrieval, not sequence continuity.

## Bounds and observation

`Limits` bounds participants, rooms, members, invitations, messages, pending
delivery events, retained changes, publication receipts, addressed participants,
and UTF-8 bytes for IDs, names, and bodies. Empty identifiers, names, and bodies
are refused. Arithmetic overflow is refused. Message and publication debug
output reports sizes and closed metadata without message bodies or idempotency
keys.

### Bounded core observation profile v0.1

`room_page` returns only rooms in which the exact observer is a current member.
`member_page` and `message_page` additionally require current membership in the
named room. Page limits are positive and cannot exceed the corresponding engine
collection ceiling.

Continuations are opaque Rust values issued only by the engine. Each binds the
exact engine revision, observer, room where applicable, and final ordering key.
Using a continuation for another observer or room refuses as foreign. Any
accepted mutation makes the continuation stale, even when the mutation affected
another room; this conservative rule prevents a page sequence from silently
mixing revisions. Page order is canonical room ID, participant ID, or retained
message sequence order. A continuation is present exactly when another visible
item existed at that revision.

Message pages expose both `oldest_retained_sequence` and
`next_message_sequence`. The half-open interval identifies retention loss even
when a page is empty; it is not a complete-history or durable-history claim.
Pages clone bounded observations only, perform no mutation, consume no engine
revision or event sequence, and publish no outbox event. Their custom Debug
representations contain only revisions, sizes, counts, scalar positions, and
continuation presence.

This is an in-process engine profile. Protocol V1 remains frozen and does not
encode these values. Protocol V2 separately projects the bounded pages and room
change feed without reinterpreting V1 bytes.

### Snapshot-aligned room change feed

A current member may capture `RoomChangePosition` only by presenting the exact
engine revision of a completed bounded room snapshot. Mutation between snapshot
and capture returns `Stale`, preventing a silent observation window. A position
binds observer, room, revision, and the exclusive global event sequence. It is a
correlation value, not authority; every pull rechecks current membership.

`room_change_page` filters the retained change journal to that room and returns at most
the requested positive bounded count. Reusing a position returns the same
retained prefix. The returned next position is exclusive, so continuation has no
overlap; when no room changes remain it advances across unrelated events to the
current edge. Domain event sequence and accepted revision are stable deduplication
identities and survive canonical engine recovery.

If journal eviction removed any required prefix, the result is a distinct
change-retention gap with the discarded edge. Delivery acknowledgment has no
effect on this decision. If the observer left and later rejoined
after the supplied position, the result is a membership-epoch gap even when all
events remain retained. Either gap requires a new bounded snapshot and position;
an empty page never implies continuity across lost visibility. This is pull
observation only, not push delivery, presence, read receipt, or acknowledgement.

## Replay-safe response publication

`PublicationCommandV2` is an adapter profile over plain `Speak`. One opaque
16-byte key binds to the room, author, logical time, body, and original accepted
transition. Matching replay returns the retained transition without rerunning
optimistic revision checking. Changed content conflicts. New keys backpressure
before message mutation at capacity. Publication deliberately addresses nobody;
an orchestrator must use an explicit conversation command if addressing is part
of the response semantics.

## Checkpoint profile V9

The closed `AGORACP` version-11 codec captures limits, participants, rooms,
membership, invitations, bans, retained message suffixes and addressing, complete publication
records, revision/event frontiers, acknowledgment cursor, and pending events.
Host byte/cardinality ceilings apply before checkpoint-owned limits or allocation.
Invalid tags, UTF-8, ordering, identities, references, suffixes, cursors, and
receipt attribution, moderation evidence, and ban/member overlap are refused.
Successful recovery has one canonical encoding. Earlier profile bytes are refused
rather than guessed or reinterpreted.

V1 checkpoint bytes are intentionally refused rather than reinterpreted. V1
contained agent-activation state that has no participant-neutral equivalent.
Checkpoint origin authentication, storage atomicity, and durability remain host
or adapter obligations.

## Volatile service profile V1

The service owns one engine behind a serialized mutable boundary. A successful
trusted-host connection binds a service-allocated client handle to one registered
participant. Commands cannot supply their speaking author. Snapshot access
requires current room membership.

The key `(ClientIdV1, RequestIdV1)` binds to one exact request and retained
reply. Matching replay returns the reply without invoking the engine. Changed
content conflicts. New requests backpressure before invocation when the bounded
receipt index is full. Disconnect is idempotent within a bounded recent
retired-client tombstone window and releases that client's receipts. A full or
disabled tombstone window never prevents disconnect; the oldest evidence is
evicted instead.

All service state is volatile. Handles are correlation values rather than
credentials. Authentication, transport binding, persistence, and distributed
serialization remain host obligations.

## Volatile service source profile V2

`ServiceV2` is a distinct serialized owner rather than a reinterpretation of the
V1 service. A V2 connection binds a separately typed client handle to one already
registered participant. Create-room owner, invitation and role actor, joining or
leaving participant, and message author are always derived from that binding.
No V2 command may nominate its own actor.

Kick, ban, and unban use the same participant-bound authority path for humans
and agents. Owners may moderate non-owners; moderators may moderate members but
not owners or moderators. Every accepted transition retains actor, subject,
bounded reason, opaque 16-byte causation, revision, and event sequence. A kick
permits admission again; a ban blocks invitation and joining until an authorized
unban. Moderation evidence is visible only through the current-member room feed.
Only the owner may change a room's admission policy. The accepted event records
actor, prior policy, new policy, revision, and event sequence; it does not erase
or reinterpret earlier invitations or admissions.
The ban set is observable only to a current owner or moderator through bounded
canonical participant-ID pages. Each entry preserves descriptive participant
kind and prior room role; cursors bind observer, room, and exact revision.

The profile closes the current engine lifecycle commands and bounded room,
member, and retained-message pages. The service converts engine continuations
into opaque monotone `PageCursorV2` handles retained in a bounded client-owned
index. Cross-client, unknown, wrong-page-kind, and wrong-room handles share
`InvalidCursor`. A successful continuation consumes its old handle and installs
at most one new handle. Exact request replay returns the retained historical
reply without consuming or recreating cursor state. Disconnect reclaims that
client's requests and cursors. Cursor-index saturation refuses a first page that
would need a continuation before installing one.

V2 request receipts are bounded and keyed by the separately typed
`(ClientIdV2, RequestIdV2)`. Shape validation and receipt capacity precede engine
or cursor effects. Matching replay returns the original reply; changed input
conflicts. Engine revision conflict and observation staleness share a typed
`Stale` reply with the actual revision. Participant kind never selects a command
path or authority result.

The transport-neutral `ClientV2` owns a monotone request namespace over an
abstract V2 service port. Its uncertainty fence is specified below; it adds no
connection lifecycle, authentication, concrete transport, automatic retry, or
cursor traversal policy.

The volatile profile by itself makes no cross-restart cursor, request, client,
or durability claim. The separately versioned service-checkpoint and durable
publication profiles below may wrap it without changing frozen V1 or V2 wire
values.

Protocol V2 exposes `OpenChanges` after a client completes a snapshot and
`Changes` for bounded continuation. Service V2 holds the engine position behind
the existing client-owned opaque cursor namespace, consumes the old cursor when
issuing the next, and preserves exact request replay. It projects only room-
scoped domain events and closed retention/membership gap causes. The private
cursor checkpoint uses `AGORACUR` kind 4, so a complete Service V2 checkpoint
preserves the feed position across durable recovery. Wire V2 assigns distinct
closed command/outcome/event tags; no public value contains the private engine
position.

## Service checkpoint profile V2

The `AGORASV` version-13 record is independent of public wire versioning and is
bound to an exact caller-supplied 16-byte store scope. It contains the canonical
engine checkpoint, configured service limits, active client-to-participant
bindings, ordered retired tombstones, exact request/reply receipts, private
engine cursor checkpoints, service cursor ownership, and next client/cursor
frontiers. Embedded request and reply values use the already canonical bounded
Wire V2 payload grammar, while the outer record owns durable migration and may
advance independently.

Recovery applies host ceilings before allocation, reconstructs the engine under
its own ceiling, checks client and cursor attribution, validates identifier
frontiers and request shapes, refuses duplicate or noncanonical ordering, and
requires byte-identical re-encoding. Opaque engine cursors have their own private
`AGORACUR` version-1 profile and remain observer/room/revision bound. Cursor kind
5 is the authority-scoped ban page. The service profile uses outer page-kind 6
for identified-message continuation while nesting the unchanged engine message
cursor kind 3; this prevents legacy/identified handle substitution. Outer kinds
7, 8, 9, and 10 respectively isolate identified changes, thread roots,
root-bound thread replies, and threaded changes. Process-local
usage counters reset on recovery and are intentionally absent from semantic
checkpoint bytes.

### Capability discovery

`Capabilities` returns one canonical strictly increasing list from a closed
feature vocabulary: room lifecycle V2, retained history V2, room changes V1,
moderation V1, room policy V1, ban observation V1, identified messages V1,
author withdrawal V1, moderator redaction V1, identified room changes V1,
threads V1, and thread lifecycle V1. The request is retained
and replayed under ordinary service request identity but performs no engine
mutation. A listed capability means that this service implements the named
syntax and semantics; it is not authentication, authorization, room membership,
or evidence that any particular request will be admitted.

### Identified messages V1

Every accepted engine message receives a 128-bit `MessageId` built from a
restart-stable room incarnation and a room-local allocation counter. The bytes
are opaque reference identity, not an ordering promise; room message sequence
and global event position remain the specified order coordinates. Identity is
retained with the message and accepted event and survives engine, service, and
host recovery.

Protocol V2 exposes this fact only through additive parallel operations:
`SpeakIdentified`, `IdentifiedMessages`, `IdentifiedMessageAccepted`, and
`IdentifiedMessagePageV1`. Legacy `Speak`, `Messages`, `MessageAccepted`, and
`MessageV2` retain their exact prior grammar. Identified and legacy page cursors
are different service cursor kinds and cannot be substituted. Neither profile
changes membership, authority, addressing, retention, or delivery semantics.

### Author withdrawal V1

`WithdrawMessage` names one retained message by opaque ID. The serialized
service derives the actor from the connected participant; the engine accepts
only when that actor is both a current room member and the original author.
Unknown, evicted, foreign-author, repeated, stale-revision, and malformed
requests refuse atomically. Participant kind has no effect.

Acceptance clears the retained body to the engine-reserved empty value while
preserving message ID, sequence, author, logical time, and addressing. User
message admission remains nonempty, so the empty body is an unambiguous
tombstone even through legacy message pages whose grammar remains unchanged.
Any still-retained `MessageAccepted` outbox copy is scrubbed, and a new ordered
`MessageWithdrawn` event carries actor, message ID, and sequence. Matching
service request replay returns the exact accepted receipt without another event.

Withdrawal is semantic suppression, not guaranteed erasure. Previously
delivered copies, client caches, connectors, backups, indexes, and model context
may retain content. Bounded request/publication replay records may also retain
the submitted body until their owning retention policy removes them.

### Moderator redaction V1

`RedactMessage` is distinct from author withdrawal and carries a bounded
nonempty moderation reason plus fixed opaque causation. The service derives the
actor. The engine applies the existing role hierarchy to the retained message's
author: an owner may redact a non-owner; a moderator may redact a current member
or departed author; an actor cannot use redaction on their own message, another
moderator, or an owner. Participant kind grants no privilege.

Acceptance produces the same reference-preserving empty-body tombstone and
scrubs any retained acceptance-event body, then appends `MessageRedacted` with
actor, author, message ID, sequence, reason, and causation. The accepted reply
contains revision, message ID, and sequence and replays exactly. Unknown,
evicted, already-tombstoned, unauthorized, stale, malformed, and over-bound
requests are inert. The author, addressing, identity, and order remain visible;
the withdrawal non-erasure boundaries apply equally to redaction.

### One-level threads V1

Every message stores either no relationship or one immutable root message ID.
`SpeakReply` accepts only a retained same-room root whose own relationship is
absent. Unknown or evicted roots and attempts to reply to a reply refuse before
mutation. A withdrawn but retained root remains a valid target because
tombstoning suppresses content rather than reference identity. Retained replies
may continue to name a root later evicted by bounded history; recovery validates
the canonical referenced-root set without manufacturing the root content.

`ThreadRoots` and `ThreadReplies` are independent bounded observations over the
same retained interval. Reply continuations bind the exact root in addition to
observer, room, revision, service client, and page profile. `ThreadedMessageV1`
adds a closed optional root ID to the identified message grammar, and the
parallel threaded change profile preserves that relationship on accepted
messages. Legacy and identified pages/feeds intentionally omit it and retain
their exact bytes. Withdrawal and redaction clear bodies but never alter the
relationship. Threads define no recursion, archive state, subscription,
notification, unread state, workflow admission, or agent activation.

### Retained thread lifecycle V1

Each retained root is active unless its ID appears in the canonical archived
set. `ArchiveThread` and `ReopenThread` derive the actor from the service client
and accept only a current root author, owner, or moderator. Both carry a bounded
nonempty reason and opaque causation. Archive prevents `SpeakReply`; reopen
restores admission. Neither changes message content, topology, addressing,
tombstones, or visibility, and neither represents task resolution.

`ThreadStatus` is a current-member observation of one retained root at the
current revision. Archive/reopen events preserve actor, root ID, state, reason,
causation, revision, and event order in legacy, identified, and threaded feeds.
Eviction removes current lifecycle state with the root while retained replies
may keep their historical reference. Engine, service, and host recovery retain
the exact state, events, and request receipts.

### Bounded reactions V1

`AddReaction` and `RemoveReaction` derive the actor from the service binding and
change only that current member's relation to one retained message and one
nonempty reaction key. Keys are limited to 64 UTF-8 bytes and each retained
message to 64 distinct keys. Repeat add, absent remove, unknown/evicted message,
invalid key, nonmember actor, and aggregate saturation refuse before mutation.

`ReactionStatus` returns the current revision, message identity, and the complete
canonical key-ordered bounded aggregate. Each entry contains the count and
whether the observing caller belongs to the set, never the reactor identities.
Relations and attributable events survive withdrawal, moderator redaction,
thread archive, exact replay, and checkpoint recovery. Message eviction removes
current reaction state. A reaction is not permission, acknowledgement, delivery,
attention, execution admission, or completion evidence.

### Bounded message revisions V1

Every admitted message owns revision zero. `EditMessage` derives the actor from
the service binding and permits only the current original author of a retained,
non-tombstoned message to append a different nonempty bounded body and logical
time. Human and agent authors use the same path. A message retains at most 64
revision records; saturation and every invalid edit refuse before mutation.

An edit changes only the current body projection. Message identity, original
author and admission time, room-local sequence, addressing, reply root, thread
lifecycle, and reactions remain unchanged. `MessageRevisions` returns ascending
immutable records containing number, editor, logical time, availability, and
body through a cursor bound to client, observer, room, message, profile, and
exact engine revision. The edit event appears in legacy, identified, and
threaded feeds. Withdrawal or redaction changes every retained record to
`Tombstoned`, clears all revision and retained edit-event bodies, and preserves
metadata and order. Eviction removes the message and its revision state.

## Receipt-gated durable service V2

`agora-durable` wraps exactly one `ServiceV2` and one Keystone checkpoint-store
port. Initialization commits a complete service checkpoint at an empty store;
recovery requires an existing scope-matching checkpoint carrying durable
acknowledgement strength. For connect, disconnect, or execute, the wrapper
captures the pre-offer record, applies the serialized operation, and compares
the candidate bytes. Equal bytes publish against the already committed store
revision and perform no store call. Changed bytes are published only after a
matching next-revision durable receipt.

A known compare-and-exchange refusal establishes that the store did not change,
so the wrapper reconstructs the exact pre-offer service before suppressing the
semantic result. Indeterminate commitment or store error cannot support safe
rollback; weak or malformed success evidence cannot establish durability. Each
case suppresses the result and fences all later access until a fresh owner loads
the store and resolves which checkpoint exists. This makes exact same-request
replay the only safe resolution after transmission uncertainty. It does not make
the abstract store durable: a physical adapter and its crash evidence are a
separate qualification obligation.

## Client uncertainty profile V2

The V2 client port distinguishes `NotSent`, which establishes that transmission
did not begin, from `OutcomeUnknown`, which means service commitment cannot be
inferred. A fresh unknown outcome retains the complete `RequestV2`. While it is
pending, only a structurally equal command may call the port, and it reuses the
exact client/request identity. Different commands return `UnresolvedOutcome`
locally without allocating a request ID or invoking transport. A not-sent exact
retry leaves the original pending request intact; a reply clears it.

There is no hidden retry. Explicit abandonment returns the unresolved request so
the caller can account for the possible service-side outcome. Extracting the
port likewise returns any pending request rather than silently treating it as
cancelled. The client fence is process-local and does not itself persist pending
requests, reconnect a host session, or prove server durability.

## Authenticated host checkpoint and durable publication V2

The `AGORAHST` version-13 record is independently versioned and bound to an exact
16-byte store scope. It contains Host V2 limits, the next non-reused session
frontier, every active mapping from opaque host session to verified subject,
participant, service client and last verification time, the bounded retired
session window, and one complete nested Service V2 checkpoint under the same
scope. Recovery requires a bijection between active host sessions and active
service clients and exact agreement on participant binding. Session IDs must
belong to the canonical V2 namespace below the recovered frontier.

The verifier implementation and passive usage counters are runtime inputs, not
durable bytes. Recovery receives a fresh trusted verifier, and every recovered
session is revalidated with current context before its next frame. Scope is
record separation rather than cryptographic authentication.

`DurableHostV2` places the compare-and-exchange gate above authentication and
service state. It checkpoints the complete host before and after Connect,
Execute, Disconnect, transport close, time-regression cleanup, and revocation.
Any changed result—including an external refusal whose security cleanup changed
state—is suppressed until the combined candidate earns the exact next durable
store revision. Known refusal restores both session and service namespaces;
indeterminate, erroneous, weak, or malformed evidence fences the host. Recovery
therefore cannot expose a durable service receipt whose authenticated session
binding was lost, or a durable session whose service client was absent.

## Exclusive private-directory checkpoint profile V1

`agora-storage-directory` implements the Keystone checkpoint-store port for one
key and one process owner on a local Unix filesystem. Creation makes a private
mode-0700 directory and mode-0600 owner lock. Open refuses symlinks, permissive
resources, an unavailable lock, malformed head bytes, and a surviving
`.pending` file. The live owner pins device/inode identity for its directory and
lock and the full metadata stamp for its current head; disappearance, mutation,
or replacement fences later access. Any error encountered during this active
revalidation—including unsafe mode/type, missing resource, identity/stamp
change, metadata I/O, or pending residue—sets one sticky owner fence before the
first specific error is returned. Later operations return `Fenced` without
touching the filesystem or incrementing that fence again.

Once active validation succeeds, failure to exclusively create the pending
publication file also establishes one sticky owner fence before returning its
specific I/O class. No head or pending file was published at that point, so the
result is definitely uncommitted rather than indeterminate. Failures after the
pending file is created remain outcome-unknown and use the indeterminate fence.
Together with active validation, this means no operational filesystem error can
leave a constructed owner Active.

`agora-storage-observation` defines an effect-free optional extension of the
Keystone checkpoint-store port. Its associated snapshot is adapter-owned but
must be copyable, content-free, passive, and explicitly report loss of counter
fidelity. The directory adapter implements the extension with
`DirectoryObservationV1`. A durable host may combine that value with
`DurableHostObservationV2`; a durable daemon may additionally copy transport and
available Host/Service facts. Each group remains separately named. Aggregate
fidelity loss is the disjunction of every represented layer, and the existing
store-neutral observation remains available for stores without this extension.
The sequential copy is not a persistent metric, concurrent atomic snapshot, or
live filesystem validation.

The `AGDIRCP` version-1 record contains the store revision, UTF-8 key, bounded
checkpoint bytes, declared lengths, and CRC32C binding the header and payload.
Compare-and-exchange writes a new mode-0600 `.pending` file, synchronizes the
file, atomically renames it to `checkpoint.bin`, and synchronizes the directory.
Only completion of that exact sequence returns the mechanism
`agora-private-directory-v1` with durable acknowledgement strength. A failure
after the temporary file is created is indeterminate and fences the owner.
Injected interruption evidence covers pending-file creation, completed write,
pending-file synchronization, head rename, and directory synchronization. The
three pre-rename states leave `.pending` residue and fail closed on reopen; the
two post-rename states recover the complete candidate revision. The interrupted
caller is always fenced and receives an indeterminate outcome.

The live directory owner exposes a passive observation containing only Active
or Fenced state, the last locally acknowledged revision, and saturating counts
for validated logical loads, compare-and-exchange offers, durable commits,
known refusals, indeterminate publications, and resource-integrity fences.
Reading it consults only owner memory: it performs no filesystem or lock access,
does not revalidate resources, and cannot retry or change an operation. It
omits the path, key, checkpoint and record bytes, record length, identity
stamps, and refusal detail. Counts reset when a new owner opens the directory.

Offline backup reuses the exact canonical `AGDIRCP` record rather than defining
a second checkpoint encoding. Export first acquires the ordinary exclusive
store owner, refuses an empty register, re-encodes its validated in-memory
record, and creates a new mode-0600 backup beneath an existing mode-0700 parent.
It synchronizes the file and parent and refuses an existing destination.
Verification applies the ordinary private-file, size, framing, UTF-8, length,
and CRC32C checks. Restore validates before mutation, creates a previously
absent private store, installs and synchronizes the exact record through the
pending/rename path, and returns the locked owner at the preserved revision.
The next compare-and-exchange therefore advances from, rather than resets, the
restored revision. Failure after destination creation may leave a partial
directory or pending/head state for operator inspection; no rollback or retry
is inferred. Export failure after file creation may likewise leave an invalid or
complete output that must be inspected rather than treated as success.
Deterministic interruption injection covers all four export boundaries and all
six restore boundaries. Every injected call reports `Interrupted`. An export
stopped immediately after creation is invalid; later process-visible complete
records verify independently, but only the synchronization boundaries describe
completed operating-system synchronization. A restore stopped after directory
creation reopens empty, each pre-rename pending state refuses as crash residue,
and either post-rename state reopens the exact checkpoint and advances from its
preserved revision. These classifications do not imply automatic cleanup or
simulate device power loss.

Operator restore is semantically distinct from ordinary Host restart. The
physical adapter first restores the exact record. `HostV2` then decodes the
canonical checkpoint, disconnects every recovered Service client (thereby
removing its private cursors and retained request receipts), retires every Host
session, and preserves conversation/outbox state plus the next Host-session and
Service-client frontiers. `DurableHostV2::recover_after_restore` prepares that
sanitized checkpoint and commits it at exactly the restored store revision plus
one with durable strength before returning Active. Any refusal, uncertainty,
weak or malformed receipt, checkpoint error, store error, or authority failure
returns no owner. The transient composition additionally validates its
empty-runtime limits before the authority call and CAS. Ordinary `recover`
retains sessions and exact replay semantics.

The exceptional `recover_after_restore` constructors require an
`OperatorRestoreAuthorityV1` implementation. After the durable record and its
recovered Host (plus transient configuration where applicable) validate, Agora
submits the exact store key, configured Host checkpoint scope, and loaded
revision to that port. Only success permits preparation and exact-revision CAS;
every authority refusal/error is erased to `Unauthorized` without a store
mutation or active owner. The Aegis adapter additionally prebinds the expected
key/scope and asks its public service for unexpired `Agora` + `Administer` on a
configured restore resource. It retains the accepted decision summary together
with the loaded Agora revision as cross-engine correlation evidence. The later
durable Agora CAS is still independent, so the evidence is not an atomic
Aegis/Agora recovery receipt.

For the qualified local operator procedure, physical restore completes first,
the durable transient Host commits that semantic sanitization, bounded entropy
generation selects a nonzero incarnation distinct from the exact prior endpoint
record, and stale-endpoint recovery rotates the locked lifetime record/socket
before binding collaboration profile 2. The next handshake and replay epoch use
that generated incarnation; a fresh authenticated connection continues the
preserved client frontier and can read retained conversation state. These are
ordered independently recoverable boundaries, not one atomic transaction.
If the process stops between steps, restore-only state is the original restored
checkpoint plus old endpoint residue; post-sanitization state is the exact next
checkpoint with no active clients plus old endpoint residue; post-generation is
persistently identical because entropy selection writes nothing; and
post-rotation state has that sanitized checkpoint plus new-incarnation endpoint
record and stale socket. Recovery inspects those resources, repeats sanitation
safely when endpoint rotation has not occurred, and rotates from the incarnation
actually present. This classification does not cover interruption within an
individual store operation. For lifetime rotation, a subprocess also terminates
without unwinding after next-record write, record synchronization, stale-socket
removal, and parent synchronization. Each process-visible residue has the exact
next record, the socket only before removal, and an unlocked record from which
exact retry reaches another distinct incarnation. Neither schedule models
controller caches or device power loss.

“Durable” here means the specified operating-system synchronization mechanism
completed on the qualified local profile. It does not prove storage-controller
power-loss behavior, network filesystem semantics, disk integrity, encryption,
rollback resistance against the trusted same-UID environment, confidential
backup transport or retention, or disaster-recovery objectives. The parent
directory and same-UID processes remain in the
trusted computing base.

## Aegis host-verifier adapter profile V1

`agora-aegis-verifier` implements the existing `PrincipalVerifier` port through
the public Aegis client/protocol contract. One adapter instance is configured
with one opaque 128-bit Agora service resource. Its context binds an exact Aegis
service session, session-local grant handle, claimed Agora participant ID, and
opaque host subject. Authentication first requires the wire claim to equal the
bound participant; revalidation additionally requires the retained host subject
to match. Only then does the adapter request `Authorize { engine: Agora,
action: Submit }` at the explicit host time.

Only an unexpired `Authorized` outcome for the expected namespace and action is
accepted. Aegis refusal, port failure, expired/malformed accepted evidence,
participant mismatch, or subject mismatch collapses to one unavailable verifier
error. The host therefore preserves its non-enumerating external refusal and
revocation cleanup laws. `Submit` authorizes use of the configured Agora service
session; it grants no room membership, role, moderation, observation, delivery,
or execution authority. Context construction remains a trusted credential-to-
principal/subject binding supplied by the composition root.

## Wire profile V2

`agora-wire::v2` maps only Protocol V2 passive values to a distinct version-2
complete-frame grammar. It covers every V2 command, outcome, refusal, page,
cursor, admission, role, and participant kind. Optional cursors are closed tagged
values. Page limits and counts use bounded `u64` wire scalars with checked `u32`
projection where required.

Like V1, encoding measures the complete frame with checked arithmetic before
exact reservation. Decoding bounds the frame, each text, field count, and each
list before owned reconstruction and requires complete consumption. V1 bytes,
unknown root or nested tags, invalid UTF-8, truncation, trailing data, and ceiling
violations refuse. The normative grammar is in `docs/wire-protocol-v2.md`.

This codec alone supplies no stream framing, feature negotiation, authentication,
transport binding, confidentiality, or replay protection beyond service request
identity. The transport-neutral authenticated host is specified separately below.

## Authenticated host profile V2

`agora-host::v2::HostV2` composes exact Wire V2 frames with one `ServiceV2` while
reusing the qualified transport-neutral `PrincipalVerifier`, explicit host time,
host limits, refusal, and passive usage contracts. The authentication evidence
contract is independent of wire version; V2 nevertheless uses a distinct opaque
`HostSessionIdV2` and separately typed V2 service client.

Connect decoding and host/session capacity precede verifier invocation. Exact
participant authentication precedes service connection. Every later frame
requires nonregressing host time, current subject/participant/context
revalidation, and exact equality between the frame client and the service client
retained for that host session. Any revalidation or substitution failure retires
only the caller session and disconnects its client before returning the shared
Unavailable class. Transport close remains an unconditional cleanup path.

V1 frames refuse at V2 decode before authentication. Authentication failures do
not query or reveal participant registration. V2 host sessions and clients are
non-reused within the volatile lifetime, and bounded retirement pressure never
blocks cleanup. The host still performs no network I/O, credential verification,
authorization-policy evaluation, physical persistence, or encryption. Durable
session recovery requires the separate combined profile above.

## Private local daemon profile V1

`agora-local-daemon` is a Unix-only outer transport composition over
`HostV2<AegisVerifierV1<_>>`. Construction requires a real existing mode-0700
parent directory and a previously absent socket path within it. The adapter
first creates an endpoint-derived mode-0600 record containing the declared
incarnation and holds an exclusive kernel lock. A live owner and unlocked
residue refuse distinctly. It then binds the path, changes its permissions to
mode 0600, validates that it is a socket, and pins parent, record, and endpoint
identities. Ownership is checked before and after accept; loss irreversibly
clears readiness and fences before authentication. Teardown removes socket and
record only while every pinned resource still matches, so ambiguous or replaced
objects are preserved.

Ordinary construction never consumes residue. Explicit endpoint recovery
requires the exact previous incarnation, a distinct nonzero next incarnation,
and a new Host composition. While holding the stale record's exclusive lock, it
validates the canonical record and any mode-0600 socket, publishes and
synchronizes the next record, removes only that validated socket when present,
and binds through the normal path. An absent socket is accepted only with the
exact unlocked record as incomplete-rotation residue. A live owner, foreign or
malformed record, unsafe socket object, or incarnation reuse refuses without
adopting state. An I/O error after mutation is not successful recovery; its
exact record/socket class can be retried from the incarnation actually stored.

Before authentication, the peer sends the exact fixed-width handshake-profile-1
ClientHello selecting Wire V2. An incompatible hello refuses before
authenticator, authority, Host, or Service work. The daemon replies with the
fixed-width ServerHello containing the same closed profiles, its configured
positive frame ceiling and post-connect request budget, capability-discovery
profile 1, and a configured nonzero 128-bit server incarnation. Truncation,
deadline expiry, or write failure is a transport failure. The incarnation is
content-safe evidence for one declared logical server lifetime; it grants no
identity or authority and is not a reconnect or replay token. The adapter offers
an explicit injectable generation boundary: its Unix implementation reads a
validated `/dev/urandom` character device, retries all-zero values at most eight
times, and can additionally exclude the immediately prior incarnation for an
explicit recovery. Generation occurs before lifetime acquisition and does not
select a Host, recover residue, or start the daemon automatically.

Every subsequent transport record is a four-byte unsigned big-endian length followed by
exactly that many bytes. Zero, an announced length above the configured ceiling,
truncation, and a response above the same ceiling fail closed. Read and write
deadlines are applied before authentication. A trusted application
`AuthenticatorV1` derives one `AegisContextV1` from the accepted connection, and
a trusted `HostClockV1` supplies explicit nonregressing host time; neither source
is inferred from Agora payloads.

The first record must be a canonical Wire V2 Connect frame. The daemon passes it
to Host V2 and returns the host's canonical connected or shared-refusal frame.
The resulting opaque host session remains process-local and is never accepted
from the wire. Each later bounded record is passed through that exact session,
connection context, and a fresh explicit host time, causing the Aegis adapter to
authorize the retained subject and participant again before Service V2 handles
the request. A fixed positive post-connect frame budget bounds connection work.

The separately constructed volatile collaboration profile uses fixed-width
handshake profile 2 to select conversation Wire V2 and transient Wire V1. Each
outer payload includes one closed route byte for conversation, transient, or
server-only control and remains within the same advertised frame ceiling. The
Connect payload must use the conversation route. After successful authenticated
Connect, the daemon sends the routed connected reply followed by a fixed
`AGORATE1` control hello. Its nonzero opaque 32-byte replay epoch combines the
declared server incarnation with a monotone per-daemon connection sequence,
binding transient client uncertainty to the exact authenticated Host session.
Later conversation and transient frames share that session, context, clock, and
request budget. Client control frames refuse, connection close retires both
transient owners, and replay-epoch exhaustion fences readiness. This profile is
also selectable with `DurableTransientHostV1`. That owner persists only the
underlying Host checkpoint; authenticated transient offers commit any changed
Host verification/session state before reply, while signal state remains
volatile. Known store refusal restores Host state and retires affected signals;
uncertain durability fences access. Recovery retains conversation state but
reconstructs presence and typing empty.

When exact unlocked endpoint residue is present, the combined owner has a
separate caller-selected recovery constructor. The caller first supplies an
already recovered `DurableTransientHostV1`; the daemon then applies the same
prior-incarnation-checked locked record/socket rotation as ordinary explicit
residue recovery and binds collaboration handshake profile 2 under the distinct
next incarnation. This composes semantic recovery with endpoint recovery but
does not make either automatic or atomic, and it never restores the prior
transport session.

EOF is a clean transport close. EOF, write failure (including the initial
connected reply), malformed framing, request-budget exhaustion, and Host refusal
all converge on unconditional idempotent `HostV2::close` after a session has
been allocated. Content-free saturating health signals distinguish accepted,
cleanly completed, refused, and transport-failed connections and expose current
connection count and readiness.

The daemon may also expose one passive point-in-time observation containing
separate transport, Host V2, and Service V2 counter groups. Reading it performs
no effect and changes no result. Its aggregate fidelity-loss bit is sticky once
any represented counter saturates. It contains no identifiers, content,
credentials, frame bytes, timing samples, or detailed refusal values and is not
part of semantic recovery. These counters are operational facts, not proof of
delivery, attention, authorization success beyond the named boundary, or an
SLO.

Durable Host selections add a separate publication group containing only owner
state, numeric store revision, and saturating counts for offers, commits,
already-committed publication, checkpoint failure, known store refusal,
uncertain outcome, weak acknowledgement, invalid receipt, and calls suppressed
after fencing. If durability fences, transport and durability facts remain
observable while the Host/Service projection becomes absent. The aggregate
fidelity bit includes durability saturation. Observation never reads the store,
retries an operation, or affects readiness, authority, or semantic results.

While idle, the sequential daemon owner may irreversibly enter draining state.
That transition clears readiness and every later `serve_next` returns a typed
draining refusal before socket accept or any authentication, authority, host, or
service work. Repeating the transition is inert. The socket remains owned until
normal teardown. Exclusive mutable ownership means this profile cannot overlap
an active connection; concurrent in-flight drain requires a different runtime
composition.

Daemon lifecycle is one closed value: `Starting`, `Ready`, `Draining`,
`Stopped`, or `Fenced`. Starting is construction-local and a constructor returns
an owner only after reaching Ready. Drain is permitted only from Ready, so it
cannot mask a prior fence. `drain_complete` is true only for Draining with zero
active connections; exclusive ownership makes that exact for this sequential
profile. Explicit `stop` consumes the owner and returns Stopped health, the
prior lifecycle, and one cleanup outcome. Clean requires successful removal of
the exact pinned socket and lifetime record plus parent synchronization;
ambiguous ownership preserves residue, while an attempted but incomplete
cleanup is reported separately.

This profile is deliberately sequential: one caller of `serve_next` accepts and
serves one connection at a time. It does not authenticate an OS peer or
credential, protect against a same-UID process controlling the private parent,
encrypt content, prove incarnation uniqueness, automatically repair residue,
preserve request receipts across disconnect, automatically recover a crashed endpoint, implement concurrent
admission, concurrent in-flight draining, signals, socket activation, installation, or
atomically recover Aegis and Agora. Those are separate composition and
operational qualifications.

## Stable topic profile V1

A room owns a bounded canonical map of opaque 128-bit topic identities to
bounded names and attributable creators. Only a current owner or moderator may
create a topic. `SpeakInTopic` requires current membership and an existing topic;
normal message validation, sequence, identity, addressing, retention, outbox,
revision, and replay laws remain unchanged. A reply inherits the root's topic
and accepts no caller-supplied replacement.

Topic and topic-message pages are current-member observations bound to observer,
room, exact engine revision, profile, and topic where applicable. Topic-message
pages retain global room sequence order and disclose the room retained interval.
The topic-aware change profile uses the shared ordered outbox but carries the
optional topic relation on accepted messages; older profiles deliberately omit
it and keep their existing bytes. Topic IDs grant no additional authority.

## Private read-position profile V1

The accepted architecture defines private current-state markers keyed by actor,
room, and one closed room/thread/topic scope. An advance names an exact retained
message and exact observed engine revision, validates current membership and
scope membership, and may only move forward by room-local message sequence.
Repeating the same target is idempotent; regression refuses without mutation.

Read positions are checkpointed private projection state. They do not increment
the authoritative domain revision, enter any room-change feed, or disclose one
participant's marker to another. Service identity is actor-derived and durable
publication must commit marker state plus the exact request receipt before
reply. The engine owns the bounded registry. Participant-bound service commands
expose only the connected actor's marker; protocol and wire use closed scopes,
outcomes, refusals, and capability values. Canonical engine profile 14 and
enclosing service/host profile 16 records retain private markers and exact
receipts through nested recovery without publishing them. Durable execution
commits both before reply, and exact lost-reply replay performs no second store
write.

## Immutable attachment-reference core profile V1

An additive engine command admits one message body, canonical addressing, one
closed room/reply/topic target, and one through eight ordered attachment
descriptors as a single transition. The ordinary membership, one-level reply,
topic-resolution, optimistic revision, message identity, retention, outbox, and
human/agent-neutral laws apply unchanged. A reply inherits its retained root's
topic. Every descriptor is validated before any message or event capacity is
consumed.

A descriptor has a room-local opaque attachment identity, logical opaque store
and object identities, a nonzero bounded declared length, lowercase ASCII
parameter-free `type/subtype`, a bounded control-free UTF-8 display label, and
one closed SHA-256 claim. Attachment IDs and `(store, object)` pairs are unique
inside the message; presentation order is retained. These values are neither
URLs nor credentials, and Agora performs no object-store I/O or byte
verification.

Body edits preserve the immutable relation. Withdrawal and moderator redaction
retain each attachment identity and ordering slot while removing the store,
object, length, media type, label, and digest from current messages and retained
message-acceptance event copies. This is logical non-disclosure, not external
object deletion. Engine checkpoint profile 20 encodes only complete descriptors
or identity-only tombstones; enclosing service and Host profiles advance to 22.
The additive public slice exposes one unified attachment admission command and
one participant-bound retained-history page without changing any legacy message
projection. Protocol target values are room, stable reply root, or stable topic;
the service derives the author exclusively from its bound client. Canonical Wire
V2 uses appended command tags 58/59 and outcome tags 56/57, rejects malformed
descriptor grammar before reconstruction, and redacts locator, label, digest,
and body content from diagnostics. Client uncertainty retains the complete
command and replays only equal descriptor input.

Attachment history filters the ordinary snapshot-bound room message sequence;
its cursor is participant, room, and revision bound and may therefore return an
empty intermediate page when that sequence contains only legacy messages.
Available messages disclose complete descriptors; withdrawn or redacted
messages disclose the same ordered attachment identities with no descriptor.
Service and Host checkpoint profile 23 retain the history cursor and exact
request receipts. A separate attachment-aware change cursor carries the ordinary
identified event sequence plus one full message projection for every acceptance
event, including empty attachment lists for ordinary messages and identity-only
tombstones after content withdrawal. Cursor profiles cannot be substituted.
Checkpoint profile 24 retains this cursor. Capability tag 23 advertises the
closed profile, and the ordinary private-local conversation route carries it
without a new transport handshake.

## Wire profile V1

`agora-wire` canonically encodes passive protocol V1 requests and replies using
fixed tags, big-endian integers, length-prefixed strict UTF-8 text, and counted
lists. Encoding preflights the exact checked length and all semantic ceilings
before output allocation. Decoding bounds the complete input, each text field,
field count, and every list before owned reconstruction, then requires exact
consumption. Unknown versions and tags are closed refusals. The normative byte
grammar and tag tables are in `docs/wire-protocol.md`.

Connect carries a claimed participant ID but grants no identity. Wire client IDs
remain correlation handles. Authentication, stream framing, replay protection
outside service request identity, encryption, and transport binding remain host
obligations.

## Authenticated host profile V1

The host consumes a complete Connect frame plus a trusted adapter-owned context
and explicit host time. Its injected verifier either returns an opaque verified
subject for that exact participant claim or refuses. Authentication error detail
is not projected; refusal uses `ConnectRefused(Unavailable)` before participant
registration is queried.

Successful connection retains `(host session, verified subject, participant,
service client)`. The host session is allocated outside the wire protocol and
must be bound to transport state by the adapter. Every later frame first
revalidates the retained subject/participant against the current context, then
requires its client field to match the retained service client. Failure retires
the host session and disconnects its own client before returning the generic
Unavailable class. Host time is monotone within a session; regression retires
before verifier invocation.

Transport close is an unconditional cleanup operation. Retired-session and
retired-client evidence is a bounded recent window; saturation evicts the oldest
tombstone and cannot block cleanup. Matching Execute replay preserves the
service's existing exact request/reply semantics. The host performs neither
credential verification nor network I/O and imports no authority-engine types.

## SDK distribution profiles V1

SDK is an audience and distribution label, not a semantic layer. The
`agora-sdk-embedded` trusted-host facade directly reexports only the engine,
serialized volatile service, and authenticated host. Those exports are
sufficient to construct the embedded stack; verifier, transport, storage, time,
and runtime choices remain injected by the application.

The `agora-sdk-client` remote-client facade directly reexports only the
transport-neutral client, protocol DTOs, and canonical wire codec. It supplies
no connection lifecycle, transport implementation, credential, retry policy, or
authority. Neither facade defines new values or behavior, and neither is an
umbrella over the other.
