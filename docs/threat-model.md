# Threat and privacy model

Status: initial STRIDE/LINDDUN analysis for the v0.3 core boundary.

The adopted direct and group-direct profiles narrow membership mutation by
retaining an immutable founding cohort and allowing only irreversible departure.
Atomic creation must reject duplicates, unknown participants, and invalid
cardinality before publishing any existence fact. Ownerless peer cohorts avoid
creator privilege but deliberately provide no moderator recovery path. A
departed peer immediately loses observation authority, while retained IDs,
events, attribution, and content remain linkable state for current peers and in
qualified checkpoints. Departure is not erasure. Empty cohorts can retain
inaccessible data until a later retention profile acts. A shared-room label is
forbidden as local trust evidence; federation, tenant provenance, remote
membership, and cross-deployment disclosure remain unimplemented threats.

| Concern | Core treatment | External obligation |
| --- | --- | --- |
| Spoofing | IDs grant no authentication; room authority is checked against explicit actor IDs | host binds authenticated principals to IDs |
| Tampering | optimistic revisions, scope-bound canonical engine/service/cursor checkpoints, typed validation, and the local adapter's checksummed record/resource pinning refuse accidental or observed mutation | trusted scope selection and stronger hostile-storage integrity remain deployment obligations |
| Repudiation | ordered events and exact service receipts preserve attribution while retained or checkpointed | durable audit and non-repudiation are not claimed |
| Information disclosure | message/publication debug output redacts content and keys | transport, logs, and storage protect content |
| Denial of service | retained state, bytes, fan-out, and outbox are bounded with backpressure | host bounds connections, request rate, and CPU |
| Elevation of privilege | participant kind grants no authority; addressing grants no execution | Aegis/host authority guards cross-engine effects |
| Linkability/identifiability | stable IDs are explicit domain data | pseudonym policy and cross-room correlation belong to host policy |
| Detectability | room membership and events reveal activity to authorized observers | presence and delivery privacy require a higher-layer policy |
| Unawareness/intervenability | no consent or notification UX exists in the core | clients communicate policy and provide controls |
| Non-compliance | retention is bounded but deletion/export policy is absent | deployment supplies policy, jurisdiction, and audit controls |

The volatile service narrows impersonation by deriving message authors from a
server-owned connection binding and narrows room disclosure by requiring
membership for snapshots. It does not authenticate the participant supplied to
`connect`. `ClientIdV1` is sequential and forgeable by design; a transport host
must never expose it as a bearer credential or accept it without a separately
authenticated connection binding. Retained request replies may contain message
content and therefore inherit the deployment's memory-protection obligations.

The derived-search port never receives participant identity and returns only
message identity plus room-local sequence. Source revision and index generation
make lag/rebuild state explicit, while incomplete and unavailable are not
silently promoted to authoritative history. The reference adapter bounds rooms,
messages, content, terms, pages, and live cursors; projection replacement is
atomic and revision-monotone. Indexed bodies remain sensitive in adapter memory,
and stable hits remain linkable metadata. A participant-bound composition must
check current membership before query and current retained visibility before
publishing each hit; otherwise an index becomes a confused deputy or leaks
withdrawn, redacted, evicted, or cross-room facts.
The participant-bound search service performs those checks and additionally
binds each opaque continuation to the exact client, room, term, match profile,
authoritative revision, index generation, and limit. Exact request replay does
not repeat index work; changed reuse conflicts. Conversation mutation and
disconnect retire adapter continuations. The profile is still volatile and not
remotely reachable: the authenticated Host binds the exact service client and
retires search state on revocation, substitution, close, or conversation
mutation, while the client fences outcome-unknown retry to an opaque exact
session/server epoch. The qualified private local profile selects search
explicitly and rejects cross-profile routes. Recovery deliberately reconstructs
the volatile search runtime empty, rejects old continuations, starts a fresh
request namespace under a distinct connection epoch, and retires adapter
cursors before returning the port. Indexed content remains a separate sensitive
memory copy; no durable or production-scale index is qualified.

Attachment references add linkable external-object metadata without making
Agora an object authority. Store namespaces, object keys, labels, media types,
declared lengths, and digests are attacker-controlled assertions until a
separate adapter verifies them; none may be interpreted as a URL, path,
credential, malware verdict, or proof of availability. The accepted R89 design
bounds descriptor counts and bytes, requires closed canonical shapes, and makes
message acceptance atomic without object-store I/O. Knowledge of a reference
grants no download authority. Future attachment-aware projections must scrub
every locator and metadata field on withdrawal/redaction, keep diagnostics
content-safe, and force a resolver to rederive current membership and message
visibility before disclosing or fetching bytes. Upload orphans, mutable external
retention, byte substitution, cross-message linkability, scanning, deletion,
and cross-engine recovery remain product/adapter threats rather than implied
Agora guarantees.

Typed Vellum artifact references add cross-engine correlation without importing
Vellum authority. The workspace, document, and source-revision scalars are
attacker-controlled assertions at Agora admission and may expose linkable
document history if disclosed outside current room visibility. Agora must never
retain a live Vellum `VersionWitness`, locator, source bytes, credential, or edit
handle. Withdrawal and redaction scrub the complete Vellum target from current
state and retained event copies while preserving only the room-local relation
identity. A resolver must authenticate independently to Vellum, rederive current
Vellum read authority, request the exact historical revision, and report stale,
missing, refused, unavailable, and unknown outcomes without rewriting the Agora
fact. Reference possession is not read/edit authority, a source revision is not
an integrity digest, and neither value activates an agent or admits work.

Room change positions are observer/room-bound correlation state, not bearer
authority. Every pull rechecks current membership, cross-client and cross-room
handles are non-enumerating, and a leave/rejoin crossing becomes a gap rather
than exposing events from the absence interval. Pull history is retained in a
separate bounded journal: delivery acknowledgment cannot hide it, while journal
eviction exposes an exact discard edge requiring resnapshot. This does not
provide per-client retained copies; one busy global journal can still force a
lagging observer to resnapshot, so deployments must size its independent limit
and request rates accordingly.

The wire decoder treats every byte as hostile: total bytes, field bytes, field
count, and list counts are bounded before owned reconstruction; unknown grammar
and trailing data refuse. This limits parser resource amplification but does not
authenticate frames, hide metadata/content, prevent replay, establish stream
boundaries, or protect transport availability. In particular, Connect's
participant ID remains attacker-controlled until the host verifies it.

Capability discovery is a closed descriptive inventory. Its canonical ordering
and tags prevent extension-bag ambiguity, but its presence grants no authority
and reveals the service profile to an already connected participant. Deployments
that consider profile fingerprinting sensitive must constrain connection access;
they must not falsify the advertised list after admitting the request.

Message IDs are stable linkable conversation metadata. They are opaque but not
secret, do not grant read or mutation authority, and must not be interpreted as
chronology. Identified history still rechecks current membership; legacy and
identified cursor kinds are isolated so a handle cannot silently change the
shape or disclosure contract of its continuation.
The identity-bearing change profile exposes the same stable ID to authorized
current members and therefore adds correlation surface, not authority. Every
pull rechecks membership, and an ID never substitutes for a room role, cursor,
or authenticated participant binding.

Thread root IDs add relationship and participation metadata. Knowing a root or
reply ID grants no read, reply, mutation, or feed authority. Root/reply/feed
cursors are isolated and every use rechecks current membership; reply cursors
also bind the root. Nested replies and unknown or evicted targets fail without
state change. Withdrawal and redaction hide retained bodies but intentionally
preserve the relationship, authorship, addressing, and order, so deployments
must treat conversation topology as sensitive even after tombstoning.
Archive state adds behavioral metadata and can be abused to suppress replies.
Agora limits this to current root authors, owners, and moderators, derives the
actor from the service binding, records bounded reason and opaque causation, and
keeps exact replay atomic. Archive never hides existing content or grants read
authority. Deployments still need abuse policy and external audit if privileged
moderators must be accountable beyond the retained room feed.

Author withdrawal rechecks current membership and exact authorship at the
named message ID; participant kind, room role, and knowledge of the ID grant no
substitute authority. A retained tombstone and retained acceptance event expose
no body, but preserve author, time, addressing, ID, and sequence as linkable
metadata. Withdrawal is not erasure: exact replay records, prior recipients,
client caches, notifications, connectors, indexes, backups, logs outside this
boundary, and model context may retain the submitted body. Repeated and unknown
references share no accepted effect, and service mapping avoids exposing engine
identifier detail beyond the caller's existing room visibility.

Moderator redaction is a separate, more privileged action. The engine rechecks
the actor's current room role against the retained author's current role (or the
member-level treatment of a departed author), rejects self/peer/owner
escalation, and retains bounded reason plus opaque causation. A compromised
owner can redact non-owner visible content by design; no Agora role can erase
prior copies or external systems. Reason, actor, author, timing, identity, and
addressing remain sensitive audit metadata even after the body is tombstoned.

The authenticated host prevents a wire client ID from becoming ambient authority:
each frame is coupled to an opaque transport-bound host session, a retained
verified subject/participant, current verifier acceptance, and the exact service
client. Authentication failures erase verifier detail and do not query participant
registration. Client substitution and revocation retire only the caller's session.
Per-session time regression also retires before verifier invocation, fencing
expiry rollback.
This treatment still trusts the verifier and transport binding; a verifier that
accepts forged context, or an adapter that accepts host session IDs from payloads,
reintroduces spoofing and confused-deputy risk.

The separately typed V2 host applies the same authentication, monotone-time,
dual-binding, non-enumeration, revocation, and cleanup treatment to V2 frames and
clients. A V1 frame is rejected before verifier invocation. This closes no new
credential, authorization, confidentiality, durable-session, or network threat;
the base host alone leaves those obligations unchanged. The separately qualified
store-neutral recovery profile below closes only canonical session-state
reconstruction and pre-reply receipt gating.

The dark transient Host wrapper narrows presence spoofing and stale-source
disclosure without turning presence into durable conversation state. Only a
currently authenticated Host session may publish or observe; its opaque session
identity is the private source, every operation revalidates the participant at
qualified monotone Host time, and current room membership is checked again at
publication and observation. Close, revocation, substitution, and clock
regression retire that source before refusal returns. Leave, kick, or ban causes
membership reconciliation before the ordinary conversation reply is returned,
so rejoin cannot resurrect an older signal. Generation saturation fences
publication and observation but never security cleanup. Wrapper reconstruction
starts `Unknown`, preventing stale post-restart presence. Presence still reveals
activity and expiry metadata to current members; there is no consent UX,
device roster, last-seen history, remote wire profile, notification policy, or
proof of availability, attention, execution, or agent activation.

The separate dark typing owner applies the same volatile-source, monotone-time,
capacity, generation-fidelity, and current-membership controls to room,
retained-root, and stable-topic targets. It revalidates the target at both
publication and observation, rejects replies as thread roots, binds cursors to
the exact target and snapshot, and permits bounded lifecycle cleanup even after
the target disappears or generation fidelity is exhausted. Restart projects all
participants inactive. The authenticated Host derives its source, retires it
with presence on session loss, and binds its public cursor to session, room,
target, snapshot, and age. An active typing projection remains only an expiring collaboration hint—not
proof of attention, intent, authorship, execution, or agent activation.

The public transient handler adds one exact semantic replay receipt
and one live cursor token per authenticated Host session. Equal request replay
cannot reapply a presence or typing mutation; changed input and skipped identities refuse.
Cursor tokens are opaque, session/room/target where applicable, single-chain, and age bounded;
current authentication time is distinct from the earlier bounded snapshot time.
Every authenticated offer advances the wrapper's monotone transient clock even
when decoding or semantics refuse, preventing a second session from using a
lower time to extend a signal.
These controls do not make an arbitrary transport reconnect or replay epoch
self-proving. The qualified private local collaboration profile sends its epoch
only after authenticated connect and composes it from the declared server
incarnation plus a per-daemon connection sequence, so it binds the client
namespace to that exact Host session without exposing the Host handle. The
epoch is correlation evidence, not a secret, credential, capability, or source
of authority. Other transports must supply an equivalent binding.

The combined durable-collaboration owner checkpoints only the authenticated
Host and conversation service. Presence, typing, transient replay receipts,
cursors, and clock state never enter the durable record, so reconstruction
cannot disclose or resurrect stale collaboration signals. An authenticated
transient offer may still change durable Host verification/session state; that
change is receipt-gated before reply. Known store refusal restores the Host and
conservatively retires the volatile source, while unknown, weak, or invalid
durability fences semantic access and retires the source. This does not make
the transient signal durable, make store and endpoint recovery atomic, or bind
recovered Agora state to recovered Aegis state.

The service checkpoint binds one caller-selected store scope and refuses foreign
scope, malformed structure, noncanonical ordering, invalid attribution, and host
ceiling violations. The durable owner never exposes a changed semantic result
without a matching durable-strength next-revision receipt. Known refusal restores
the pre-offer state; indeterminate, erroneous, weak, or malformed store evidence
fences access. Scope is separation evidence, not a secret or MAC. A physical
adapter must protect record origin, exclusive ownership, rollback, crash residue,
backup confidentiality/retention, and actual acknowledgement strength.

The qualified private-directory adapter narrows that obligation for a local Unix
deployment. It requires mode-0700/0600 resources, an exclusive process lock,
pinned directory/lock/head identities, CRC32C-bound canonical bytes, and a
file-sync/atomic-rename/directory-sync commit. Symlinks, permissive modes,
replacement, malformed records, and `.pending` crash residue fail closed. The
five publication boundaries are interruption-injected: pre-rename states reopen
only as crash residue, while post-rename states recover the complete candidate;
the interrupted live owner is always fenced. This is process-state evidence,
not a simulated power-loss device.
The parent directory and same-UID processes remain trusted; CRC32C is corruption
detection, not authentication. Hardware power-loss behavior, controller caches,
network filesystems, rollback by a trusted process, encryption, remote backup
transport, retention policy, and disaster-recovery objectives remain unqualified.
The separately typed generational profile narrows only the replacement-before-
reclaim hazard. Immutable generation records remain until a different owner has
reopened the exact successor; a digest-bound bounded plan is synchronized before
unlink, and a completed journal makes retry explicit. Live-generation and plan-
receipt ceilings prevent unbounded amplification. Missing/replaced resources,
unknown files, stale or changed plans, and corrupt journals fail closed; partial
unlink resumes only under the same recorded plan. Directory-synced unlink is not
proof about open handles, filesystem snapshots, copy-on-write extents, flash
translation, backups, replicas, provider media, or secure erasure, and the
parent directory plus same-UID processes remain trusted.
The offline backup mechanism acquires the ordinary source lock, emits the exact
checksummed record to a new synchronized private file, and restores only into a
new private store while retaining revision continuity. CRC32C detects accidental
or observed corruption but authenticates no backup origin; the raw backup
contains the full sensitive Host/Service checkpoint. A failed restore after
destination creation may leave fail-closed residue requiring inspection. Four
export and six restore interruption points prove that no injected operation
reports false success and classify the visible residue. A complete but
unsynchronized process-visible record is not promoted to a durable artifact,
and the campaign is not hardware power-loss simulation.
Restored Host bytes can contain authenticated sessions and private Service
handles that are no longer live transport facts. The explicit operator-restore
path prevents their resurrection by disconnecting every recovered client and
retiring every Host session, then requiring a durable exact-next-revision CAS of
that sanitized checkpoint before activation. A crash immediately afterward
reopens the sanitized state. This does not authenticate the operator's choice
of recovery mode, rotate endpoint incarnation, renew credentials, or coordinate
restores with Aegis or another engine.
The qualified local composition then generates an incarnation distinct from the
exact prior endpoint record and performs locked stale-endpoint rotation only
after Host sanitization commits. Its fresh handshake cannot revive the retired
session, but a failure between the store and endpoint steps can leave one step
complete without the other. Operators must inspect and retry from truthful
state; no cross-resource rollback or atomicity is claimed.
Real process termination at every inter-step boundary makes that partial-state
risk observable. In particular, an entropy value that was generated but not
published grants no durable fact, while post-rotation recovery must trust the
incarnation actually stored in the locked record rather than the operator's
earlier intent. R79 separately injects every mutating lifetime-rotation
boundary. Record-first ordering prevents old-record/missing-socket residue, and
exact-record/missing-socket recovery closes the unavoidable post-removal window.
It still trusts process-visible local filesystem behavior and does not simulate
controller caches, device power loss, or hostile alteration.

Host persistence binds the verified subject, claimed participant, opaque host
session, service client and monotone verification time in the same record as all
service state. Recovery refuses missing, duplicate, substituted, or orphaned
bindings and still requires current verifier acceptance before a frame. A
durable-strength receipt covers security cleanup before revocation or disconnect
is reported. The stored subject is attribution evidence, not a credential;
credential renewal, revocation source integrity, and verifier availability remain
external obligations.

The host Aegis adapter narrows authority admission to an exact public Aegis session,
session-local grant, configured Agora resource, `Submit` action, explicit time,
participant binding, and opaque host subject. Participant or subject mismatch is
rejected before authority I/O; every Aegis refusal, malformed success, expiry,
and port failure remains the same unavailable class. The context constructor is
trusted: a false credential-to-principal/participant/subject binding defeats the
profile.

Operator-restore recovery is a separate privileged path. The durable owner
validates the recovered checkpoint, then requires authority over its exact store
key, Host checkpoint scope, and loaded revision before any sanitizing CAS.
Authority failure is erased and cannot mutate storage or return an owner. The
Aegis restore adapter also prebinds key/scope and requires unexpired
`Administer` on one configured Agora resource. Its retained Aegis decision and
Agora revision support correlation and audit handoff only: the later Agora CAS
can still fail independently, and the adapter neither atomically restores Aegis
nor proves credential provenance, rollback freshness, or grant issuance.

The private local daemon requires a mode-0700 parent, bounds the unauthenticated
handshake and outer length before payload allocation, applies read/write deadlines and a fixed request budget, keeps host
session identity out of frames, and closes the service client after every
terminal connection path. A mode-0600 socket narrows accidental local access;
an endpoint-scoped exclusive lock and exact incarnation record bind the declared
server lifetime, while pinned revalidation and teardown avoid accepting or
deleting an observed replacement. A claimed
participant still reaches no service operation until an exact handshake profile
has been selected and the trusted connection
authenticator supplies its Aegis context and Host V2 authorizes it, and every
later frame is reauthorized. Shared connect refusal avoids participant
enumeration, while content-free health avoids adding conversation telemetry.
Durability observations expose only owner state, numeric revision, bounded
outcome-class counts, and fidelity loss; they contain no store key, checkpoint,
frame, identity, room, credential, or detailed refusal. After a fence the
Host/Service projection is withheld, and reading the remaining facts cannot
trigger a retry or store access.
Daemon lifecycle and stop receipts are likewise content-free. A fenced owner
cannot be relabeled draining, and ambiguous endpoint ownership produces a
residue-preserved outcome without deleting attacker-replaced resources. Clean
is reported only after exact pinned socket and lifetime-record removal plus
parent synchronization; incomplete cleanup remains visible rather than being
reported as stopped-clean.

This does not defend a same-UID attacker controlling the private socket parent,
authenticate Unix peer credentials, hide socket metadata or
traffic from the host, encrypt content, or automatically recover a crashed
endpoint. The
authenticator can forge context if its credential binding is wrong. The
handshake incarnation is unauthenticated lifecycle evidence. It may be supplied
directly or generated from a validated `/dev/urandom` character device through
a bounded nonzero/distinct filter; the operating-system entropy facility is
trusted and uniqueness is not proved. The lease excludes a second live owner
but cannot prove a restart boundary or automatically repair residue. Explicit
endpoint recovery requires exact prior-incarnation evidence and rotates to a distinct declared lifetime without
inventing Host state. The combined explicit path accepts only a caller-recovered
durable-collaboration owner, applies the same residue checks, and binds a fresh
collaboration session and replay epoch; it does not resume the old connection.
A separately selected receipt-gated durable Host plus private directory store
retains conversation state across daemon-owner restart and exposes it only after
a fresh authenticated connection; the transport does not choose or relax that
store boundary. There is no cross-disconnect
receipt retention or reconnect resumption: a lost mutation reply remains
outcome-unknown and must not be blindly retried on a new connection. Sequential
service and finite request budgets are explicit availability limits, not
denial-of-service immunity.

The directory owner's public observation excludes filesystem paths, store keys,
checkpoint and record bytes, record length, inode/device stamps, and detailed
refusals. It exposes only owner state, last acknowledged revision, bounded
outcome-class counts, and saturation. Reading it does not touch the filesystem
or lock, so it cannot validate that trusted same-UID resources remain intact;
operators must not treat it as a live integrity probe or durable audit trail.
Every actual store operation does revalidate the pinned directory, lock, head,
and residue state. Any validation error makes the owner fence sticky before the
specific error escapes; later calls refuse before another filesystem access, so
restoring attacker-replaced paths cannot silently reactivate that owner.
Even after validation succeeds, an exclusive pending-file creation error may
reflect a raced or degraded filesystem. The owner therefore fences before
returning the exact definitely-uncommitted I/O error. Once pending publication
begins, any I/O failure remains indeterminate and fenced; neither class permits
automatic retry by that owner.
The optional storage-observation port is not an authority or integrity oracle.
Only adapter implementations with separate evidence may participate in a
qualified composition; the current claim covers the private-directory adapter.
The daemon preserves physical facts as a distinct group and never infers a
semantic commit from a store counter. Snapshots remain process-local and
sequential, so exporters must not claim persistence or cross-owner atomicity.

Addressing a participant is neither authority nor execution evidence. An adapter
that turns addressing directly into model or tool execution violates this model.
The qualified local Moira receiver therefore exposes only a passive pending
message fact. Host policy must separately map an Agora participant to an opaque
Moira participant reference and explicitly admit the message before workflow
state exists. Acknowledgment follows durable accounting for that decision.

Reaction keys, counts, timing, actor attribution in retained events, and caller
membership are sensitive conversation metadata even when message bodies are
tombstoned. The bounded aggregate API does not enumerate reactor identities, but
feed access may reveal the acting participant to current room members. Hosts
must not reinterpret a reaction as permission, delivery, attention, execution
admission, approval, or completion. Fixed key and per-message aggregate ceilings
bound amplification; message retention eviction removes current relation state.

Revision bodies multiply retained sensitive content. The fixed 64-record ceiling
bounds amplification, author-only mutation prevents foreign rewriting, and
client/observer/room/message/revision-bound cursors prevent continuation
substitution. Withdrawal and moderator redaction scrub every retained revision
and edit-event body but cannot retract prior client, connector, replay-record,
backup, or audit copies; deployments must not present this as physical erasure.

Topic names, creator attribution, stable IDs, and message-topic relations are
sensitive conversation metadata. Knowledge of an ID grants no membership,
posting, subscription, notification, or execution authority; every observation
and mutation rechecks current room membership and role. Fixed room topic/name
ceilings bound namespace amplification, topic-specific cursors bind the exact
room/observer/topic/revision/profile, and cross-profile substitution fails
closed. Retention may remove every topic message while leaving the topic and its
ordered creation fact; deployments must not infer completeness or resolution
from an empty page.
Rename preserves the same stable ID and every message relation, but exposes both
prior and new names in retained change history. Owners and moderators therefore
must treat both values as sensitive metadata. Collision, same-name, stale,
unknown-topic, invalid-name, and unauthorized attempts are validate-before-
mutate refusals. Capability discovery and knowledge of a topic ID remain
non-authorizing.
Resolution exposes current and historical conversation-admission metadata. It
fences new roots and inherited replies but preserves all retained content and
references; clients must not present it as deletion, task completion, approval,
or external workflow truth. Same-state, stale, unknown-topic, and unauthorized
attempts are atomic refusals, and reopening is an equally attributable role-
authorized transition.
Supersession exposes a bounded relationship between two otherwise independent
topic identities. The engine requires a resolved source, distinct same-room
topics, and a depth-one graph; source/target role overlap, same-state, stale,
unknown-topic, and unauthorized attempts refuse atomically. Multiple sources
may share a successor. Clients must not treat the relation as deletion,
canonical-name resolution, implicit posting redirection, content movement, or
workflow truth, and must retain both identities when presenting history.
Aliases expose additional retained naming metadata and therefore share the
topic-name confidentiality boundary. Fixed per-topic count and byte ceilings
bound amplification; one room-wide current-name/alias collision domain prevents
ambiguous presentation. Unauthorized, stale, invalid, duplicate, over-limit,
and absent-remove attempts refuse atomically. Hosts must never accept an alias
as a topic selector, posting redirect, authority grant, search result guarantee,
connector truth, or workflow state.
Tags expose retained display/index metadata under the same content
confidentiality boundary. Fixed count and byte ceilings bound amplification;
canonical per-topic sets prevent duplicate-state ambiguity while deliberately
allowing cross-topic reuse and equality with names or aliases. Unauthorized,
stale, invalid, duplicate, over-limit, and absent-remove attempts refuse
atomically. Hosts must never interpret a tag as hierarchy, selector, authority,
routing, guaranteed search truth, subscription, notification policy, workflow
state, or agent activation.

Private read positions are sensitive behavioral metadata even though they prove
neither delivery nor attention. The engine exposes only the actor-keyed marker
and rechecks current membership; leave, kick, and ban make it inaccessible
without silently erasing it. Canonical recovery validates participant/room/scope
keys, deterministic target identity, monotone sequence bounds, observed
revision, and retained target relation where evidence remains. Hosts must not
project markers to moderators, other participants, feeds, analytics, or agent
harnesses without a separate consent and product-policy decision.

Private attention preferences are likewise sensitive behavioral metadata and
may reveal interests, suppression choices, or anticipated notification policy.
They are actor-keyed and current-membership-gated; no room event, feed, search
projection, moderator view, diagnostic, or other participant response may carry
them. Scope identifiers are validated before a new entry is admitted, stale
writes and global-capacity exhaustion refuse atomically, and all-default removal
releases capacity. Recovery rejects default rows, noncanonical ordering, unknown
participants/rooms, foreign thread identity, and unknown topics. A consumer
must not reinterpret stored intent as a delivery subscription, device policy,
urgency, read receipt, attention proof, workflow admission, or agent activation.

Shared pins are visible room facts while private saves are sensitive behavioral
metadata. Implementations must not merge their storage, observation, events, or
search projections. Both retain only stable message identity and original
sequence: copying body, revision, attachment, or Vellum artifact metadata would
bypass tombstoning and create an unauthorized retention authority. Save access
is actor-bound and current-membership-gated; pin access is room-membership-gated.
Evicted and tombstoned availability is explicit and never implies that payload
or external bytes remain retrievable. Capacity and stale-write refusals occur
before mutation, and diagnostics disclose neither private save membership nor
referenced identities.

Poll prompts, option labels, eligibility cohorts, lifecycle, and aggregates are
sensitive room metadata; individual ballots are a stricter private projection.
The Host must derive the voter from its authenticated binding, and no command,
feed, event, diagnostic, moderator view, or other participant response may
enumerate another participant's ballot or identify the actor behind an
aggregate change. Aggregate deltas and event timing can still permit inference,
so this profile must not be described as cryptographically anonymous or
coercion-resistant. Canonical recovery recomputes totals from eligible private
ballots and refuses disagreement. Fixed option, cohort, ballot, and room-poll
ceilings bound amplification; only the oldest closed poll may be evicted.
Clients and policy adapters must not reinterpret a ballot, plurality, close, or
reopen as authorization, approval, quorum, finality, task state, scheduling, or
agent activation.

Topic-collection filters expose authoritative current metadata and therefore
must remain membership-gated, revision-bound, bounded, and byte-canonical.
Filter tags are selectors only; they do not become authority, routing,
subscription, or search ranking. A collection view must never rewrite the
immutable admitted topic relation of a message or migrate a private read
position. Treating administrative “move” or “copy” as an in-place relation
change would corrupt replay; copying another participant's body under their
identity would counterfeit speech. Only explicit depth-one supersession may be
projected as merge/redirect, and it must not silently retarget reads or writes.
No collection observation may infer cross-room membership, roles, or federation
trust.

Admission enforcement must never trust caller `LogicalTime` or wire-carried
elapsed time. The authenticated Host supplies non-regressing time out of band;
missing, regressed, stale, or foreign evidence refuses before message mutation.
Posting restrictions share the all-message admission choke point and have no
role or participant-kind bypass. Reports are private allegations: ordinary room
feeds expose none of their existence or content, reporters see only their own,
and only current owners/moderators see the bounded queue. Classifier output has
no Agora writer, and Dismissed/Escalated review state must not be represented as
truth, sanction completion, workflow completion, or external notification.

Scheduled publication introduces task-reference aliasing, publication-ID reuse,
stale schedule-time authority, cancellation races, lost replies, and partial
cross-engine persistence as distinct threats. The R102 adapter allocates a
non-derived opaque ID and durably binds canonical intent before Chronos
admission. Agora derives the author from a trusted session and checks current
membership, restriction, interval, and bounds at dispatch; no task or old room
snapshot grants speaking authority.

After the adapter records that an Agora offer may exist, cancellation is not
absence evidence. Recovery must replay or inspect the same publication ID; it
must never issue a fresh ID or task to obtain a convenient result. An accepted
message is not implicitly erased by cancellation. Weak, malformed, failed, or
unknown adapter-store publication fences progress rather than inventing a
cross-engine atomic commit. Chronos task/lease/grant data and message content
remain out of diagnostics and ordinary room events.

Hermes delivery introduces subscription substitution, stale visibility, payload
over-disclosure, endpoint-secret leakage, SSRF/provider abuse, append reply loss,
duplicate external delivery, cursor confusion, revocation races, and retention
gaps. R103 uses actor-bound current visibility, an initially content-minimized
schema, independent delivery IDs, exact Hermes replay, durable endpoint success
before cursor advancement, and pre-offer revocation checks. Hermes identifiers
grant no authority and never become Agora read positions. Providers may still
duplicate or retain notifications; revocation is not remote erasure.

Native federation introduces peer substitution, stale trust, home-identity
confusion, non-home event forgery, replay divergence, hidden gaps, snapshot
rollback, retry duplication, acknowledgement/read conflation, partition writes,
and relay loops. R106 requires a verifier-bound stable server identity and trust
epoch on every fresh exporter/incarnation session; certificates alone grant no
federation authority. Structured home-scoped identities and an exactly one-home
room rule prevent display names, DNS, or replica placement from becoming
authority.

The initial topology accepts authoritative events only directly from the room
home and never re-exports imported events. Replica journals require exact next
sequence, equal-byte replay, explicit gaps, and verified bounded snapshot
replacement; they fence changed bytes instead of merging histories. Remote
admission revalidates current home authority under an authenticated participant
binding and retains exact receipts. Unknown transport outcomes reconcile the
same request or authoritative observation. Peer acknowledgement proves only a
replication frontier, never participant attention, mutation success, global
delivery, erasure, or transfer of room ownership.

Adapter-controlled publication identities are source scoped. Equal 16-byte
values from Chronos, typed interaction, and federation are not the same engine
receipt key, preventing a peer from selecting an existing non-federation key to
manufacture replay or conflict. Profile-149 recovery rejects unknown source
tags. Legacy unscoped receipts remain a bounded upgrade ambiguity: an exact
content match may replay the already accepted transition, but changed content
conflicts and every newly accepted receipt is source scoped.

Federated publication time is a trusted first-execution Host fact, not a peer
claim. A later retry under the exact federation receipt may present a newer
Host instant; the engine returns the retained transition only when room, actor,
and body still match. The newer instant neither rewrites the message nor creates
a second one. Host still revalidates the current session before this path, and
Durable Host receipt-gates any changed verification state before reply.
Federation request IDs are sender-scoped, not globally unique. The Agora
receipt key therefore combines the request ID with a full 64-byte participant
and room-home binding. Equal request bytes under distinct remote scopes cannot
replay or conflict with one another. Profile-150 recovery rejects a scoped key
under any non-federation domain.

The concrete durable federation endpoint captures the local Host owner,
authenticated session, verifier context, and clock, so a peer or adapter cannot
substitute those authorities per request. It publishes definite non-execution
only for a known store refusal whose checkpoint rollback succeeded. Fencing,
checkpoint failure, indeterminate commit, weak durability evidence, and invalid
receipts remain outcome-unknown; this may force conservative reconciliation but
does not permit an unsafe fresh retry.

Multi-owner export composition introduces class substitution, duplicate source
ownership, and mixed-revision manifests. R107 requires each source fragment to
carry exactly one native record class and exact frontier, then rejects
noncanonical class order, duplicate classes, and any engine- or policy-revision
mismatch before digesting bytes. A complete Service checkpoint remains one
`OperationalCopies` record; composition never authorizes callers to relabel its
internals as room, receipt, private, change, or federation records.

The SDK crates are reexport-only distribution facades. The trusted-host facade
does not bundle a verifier, transport, storage adapter, or ambient credential;
the remote-client facade cannot reach engine, service, or host internals. A
consumer that treats either facade name as authority bypasses the documented
boundary and must not be considered authenticated.

