# Threat and privacy model

Status: initial STRIDE/LINDDUN analysis for the v0.3 core boundary.

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

The service checkpoint binds one caller-selected store scope and refuses foreign
scope, malformed structure, noncanonical ordering, invalid attribution, and host
ceiling violations. The durable owner never exposes a changed semantic result
without a matching durable-strength next-revision receipt. Known refusal restores
the pre-offer state; indeterminate, erroneous, weak, or malformed store evidence
fences access. Scope is separation evidence, not a secret or MAC. A physical
adapter must protect record origin, exclusive ownership, rollback, crash residue,
backup/restore, and actual acknowledgement strength.

The qualified private-directory adapter narrows that obligation for a local Unix
deployment. It requires mode-0700/0600 resources, an exclusive process lock,
pinned directory/lock/head identities, CRC32C-bound canonical bytes, and a
file-sync/atomic-rename/directory-sync commit. Symlinks, permissive modes,
replacement, malformed records, and `.pending` crash residue fail closed. The
five publication boundaries are interruption-injected: pre-rename states reopen
only as crash residue, while post-rename states recover the complete candidate;
the interrupted live owner is always fenced. This is process-state evidence,
not a simulated power-loss device.
parent directory and same-UID processes remain trusted; CRC32C is corruption
detection, not authentication. Hardware power-loss behavior, controller caches,
network filesystems, rollback by a trusted process, encryption, backup/restore,
and disaster recovery remain unqualified.

Host persistence binds the verified subject, claimed participant, opaque host
session, service client and monotone verification time in the same record as all
service state. Recovery refuses missing, duplicate, substituted, or orphaned
bindings and still requires current verifier acceptance before a frame. A
durable-strength receipt covers security cleanup before revocation or disconnect
is reported. The stored subject is attribution evidence, not a credential;
credential renewal, revocation source integrity, and verifier availability remain
external obligations.

The Aegis adapter narrows authority admission to an exact public Aegis session,
session-local grant, configured Agora resource, `Submit` action, explicit time,
participant binding, and opaque host subject. Participant or subject mismatch is
rejected before authority I/O; every Aegis refusal, malformed success, expiry,
and port failure remains the same unavailable class. The context constructor is
trusted: a false credential-to-principal/participant/subject binding defeats the
profile. The adapter does not atomically commit Aegis and Agora recovery state.

The private local daemon requires a mode-0700 parent, bounds the unauthenticated
outer length before payload allocation, applies read/write deadlines and a fixed request budget, keeps host
session identity out of frames, and closes the service client after every
terminal connection path. A mode-0600 socket narrows accidental local access;
device/inode-pinned teardown avoids deleting an observed replacement. A claimed
participant still reaches no service operation until the trusted connection
authenticator supplies its Aegis context and Host V2 authorizes it, and every
later frame is reauthorized. Shared connect refusal avoids participant
enumeration, while content-free health avoids adding conversation telemetry.

This does not defend a same-UID attacker controlling the private socket parent,
authenticate Unix peer credentials, hide socket metadata or
traffic from the host, encrypt content, or recover a crashed endpoint. The
authenticator can forge context if its credential binding is wrong. There is no
incarnation handshake or cross-disconnect receipt retention: a lost mutation
reply remains outcome-unknown and must not be blindly retried on a new
connection. Sequential service and finite request budgets are explicit
availability limits, not denial-of-service immunity.

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

Private read positions are sensitive behavioral metadata even though they prove
neither delivery nor attention. The engine exposes only the actor-keyed marker
and rechecks current membership; leave, kick, and ban make it inaccessible
without silently erasing it. Canonical recovery validates participant/room/scope
keys, deterministic target identity, monotone sequence bounds, observed
revision, and retained target relation where evidence remains. Hosts must not
project markers to moderators, other participants, feeds, analytics, or agent
harnesses without a separate consent and product-policy decision.

The SDK crates are reexport-only distribution facades. The trusted-host facade
does not bundle a verifier, transport, storage adapter, or ambient credential;
the remote-client facade cannot reach engine, service, or host internals. A
consumer that treats either facade name as authority bypasses the documented
boundary and must not be considered authenticated.

