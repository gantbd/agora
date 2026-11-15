# Compatibility policy

Agora is pre-1.0 and currently source-distributed. Public schemas still change
only through an explicit profile revision.

The v0.3 command, event, publication, and checkpoint values use the `V2` suffix.
The `AGORACP` header version is 17. Earlier checkpoint bytes are refused with
`CheckpointError::Profile`; they are never interpreted as V17. V1 agent activation
has no migration because activation is orchestration behavior, not conversation
state. A host retaining V1 state must export conversation facts through a
purpose-built migration tool before adopting V2.

Additive V2 vocabulary allocations may use previously unassigned tags only when
all existing values retain byte-identical encodings and older implementations
fail closed on the new value. Reinterpreting an assigned tag or changing any
existing field grammar requires a new frame version. Unknown tags, fields, or
trailing bytes remain refusals.

The immutable `engine/agora-engine/tests/fixtures/checkpoint-v17.hex` artifact pins a representative
V18 state containing both participant kinds, stable message and topic identity,
topic-scoped messages, thread lifecycle, reactions, message revisions, neutral addressing, replay-safe
publication, ban and policy-change evidence, one private read position,
acknowledgment, the exact pending outbox suffix, and the independently retained
change-journal suffix and discard edge, plus stable topic rename, resolution,
supersession, and bounded canonical alias state with their attributable events.

The operational protocol is separately versioned as `V1` values. `agora-wire`
defines its canonical binary representation and refuses unknown versions, tags,
malformed text, limit violations, and trailing bytes. Golden request/reply
fixtures freeze representative bytes. The codec defines complete message frames,
not stream boundaries; transport framing remains separate.

Protocol, Service, Client, and Wire V2 form an additive profile with a separate
version byte, closed grammar, and thirty-four canonical fixtures. Their separately
typed values and V2 bytes do not alter V1 tags or bytes. Authenticated Host V2
uses the same canonical frames without reinterpreting V1.

The private local transport handshake is a separate fixed-width Wire contract.
Handshake profile 1 selects Wire V2 exactly and returns positive outer-frame and
post-connect budgets, capability-discovery profile 1, and a nonzero opaque
server-incarnation value. Its ClientHello and ServerHello fixtures freeze the
complete bytes. Unknown profiles, changed magic, trailing bytes, and zero
limits/incarnation refuse; this is exact profile selection, not version-range or
feature negotiation. The incarnation is lifecycle evidence, not identity,
authority, a resume token, or a compatibility promise.

The local daemon's separate 24-byte `AGORALF1` lifetime record binds that opaque
incarnation to an endpoint-scoped exclusive lock. Its frozen fixture is a local
ownership-format contract, not a recoverable checkpoint. Existing, malformed,
or replaced records are never interpreted as conversation state or repaired by
the compatibility layer.

Service persistence is versioned independently. `AGORASV` version 19 captures
the complete V2 service owner and embeds private `AGORACUR` version-1 cursor
records. Cursor kind 4 is the snapshot-aligned room change position and kind 5
is the authority-scoped ban page; neither reinterprets kinds 1–3. Service page
discriminator 6 isolates identified-message continuations while nesting the
unchanged message cursor kind 3. Discriminator 7 isolates identity-bearing
room-change positions from legacy kind 4. Discriminators 8 and 9 isolate root
and root-bound reply continuations, and discriminator 10 isolates threaded
room-change positions. Discriminator 11 isolates one message's revision
continuation and binds the message identity in addition to room and observer.
Discriminators 12 and 13 isolate topic and topic-message continuations; the
latter binds the exact topic. Discriminator 14 isolates topic-aware room-change
positions. A representative complete service record is pinned by
`service-checkpoint-v19.hex`. Public wire
compatibility neither implies nor migrates durable state;
future durable schema changes require a bounded explicit migration profile.

Authenticated host persistence has its own `AGORAHST` version-19 record and
`host-checkpoint-v19.hex` fixture. It nests the service record but owns host
session compatibility independently; neither wire nor service checkpoint
decoding implies that a host record can be recovered.

Author withdrawal is additive: command tag `14`, outcome tag `15`, capability
tag `08`, and room-event tag `0b` were previously unassigned. Existing message
page and acceptance grammars remain unchanged. Because admitted message bodies
are nonempty, an empty body in retained history is the V1 downgrade projection
of a withdrawal tombstone; aware clients correlate the new event by ID and
sequence.

Moderator redaction is likewise additive: command tag `15`, outcome tag `16`,
capability tag `09`, and room-event tag `0c` were previously unassigned. Its
reason and causation evidence are new fields only within those new variants;
legacy frames are not reinterpreted.

Bounded reactions are additive: command tags `20`, `21`, and `22` encode add,
remove, and status; outcome tags `1f`, `20`, and `21` encode status, added, and
removed; capability tag `0d` advertises `ReactionsV1`; room-event tag `0f`
encodes an attributable set transition. Existing message, tombstone, thread,
and feed variants retain their prior bytes. The reaction-change and
reaction-status fixtures freeze the new grammar.

Bounded message revisions are additive: command tags `23` and `24` encode edit
and history-page requests; outcome tags `22` and `23` encode the revision page
and edit receipt; capability tag `0e` advertises `MessageRevisionsV1`; and
room-event tag `10` carries actor, stable message ID, message revision, logical
time, and body in every feed profile. Existing current-message projections and
accepted-message grammars retain their bytes. Revision-page and edit-change
fixtures freeze the new grammar; engine/service/host profiles advance to
11/13/13 because revision records and cursor/receipt state are durable.

Identity-bearing room changes are additive: command tags `16` and `17`, outcome
tag `17`, and capability tag `0a` were previously unassigned. Only the new
outcome's nested event tag `06` uses identified-message grammar. Legacy Changes
bytes, tags, cursor kind 4, and fixtures remain unchanged.

One-level threads are additive: command tags `18` through `1c`, outcome tags
`18` through `1b`, and capability tag `0b` were previously unassigned. The
threaded message grammar appends a closed optional root ID. Root pages require
that field absent; reply pages bind it to one root and require every entry to
match. Legacy and identified message/change grammars remain unchanged.

Retained thread lifecycle is additive: command tags `1d` through `1f`, outcome
tags `1c` through `1e`, capability tag `0c`, refusal tag `11`, and room-event
tags `0d`/`0e` were previously unassigned. Existing thread page and accepted-
message grammars remain unchanged. The new event variants use the same fields in
legacy, identified, and threaded change envelopes.

Stable topics are additive: command tags `25` through `2a` encode topic pages,
topic-message pages, creation, topic-scoped speaking, and topic-aware feed open/
pull; outcome tags `24` through `28` encode their pages, receipts, and feed;
capability tag `0f` advertises `TopicsV1`; refusal tag `12` identifies an unknown
topic; and room-event tag `11` records topic creation. The topic-aware accepted-
message grammar appends a closed optional topic ID to the threaded grammar.
Legacy, identified, and threaded accepted-message bytes remain unchanged.
Engine/service/host profiles advance to 12/14/14 because the topic registry,
allocator, message relation, receipts, and cursor consequences are durable.

Private read positions are additive: command tags `2b` and `2c` query and
advance self-scoped state; outcome tags `29` and `2a` encode optional/current
markers; capability tag `10` advertises `PrivateReadPositionsV1`; refusal tags
`13` through `15` distinguish invalid scope, invalid target, and regression.
Nested scope tags `01`, `02`, and `03` identify room, thread, and topic. Existing
frames remain byte-identical except the intentionally expanded capability list.
Engine/service/host persistence profiles advance to 13/15/15. The engine record
appends one canonical participant/room/scope-keyed registry after room state;
each marker retains exact target identity, sequence, and observed revision.
Earlier checkpoint profiles fail closed rather than dropping private state.

Independent change retention is additive on the wire: capability tag `11`
advertises `IndependentChangeRetentionV1`, and the previously unassigned nested
ChangeGap tag `03` carries the exact discarded-through event sequence. Existing
Retention tag `01` remains decodable for older implementations and is not
reinterpreted; the current service emits tag `03` for its independent journal.
The frozen change-retention-gap fixture pins the new representation, while all
older request, reply, and event fixtures remain byte-identical except the
intentionally expanded capability list.

Engine/service/host persistence profiles advance to 14/16/16. Engine V14 adds
the independent journal ceiling, discard edge, and canonical retained suffix;
the service and host records nest that state with live cursors and exact
receipts. Profiles 13/15/15 fail closed rather than infer missing change history
from the delivery outbox or silently couple acknowledgment back to observation.

Stable topic rename is additive: command tag `2d`, outcome tag `2b`, capability
tag `12`, and room-event tag `12` were previously unassigned. The event has the
same actor, stable topic ID, prior name, and new name in every change profile.
Existing topic/message/relation bytes are not reinterpreted. Frozen request,
receipt, and topic-aware change fixtures pin the new grammar; the intentionally
expanded capability fixture adds `TopicRenameV1`.

Engine/service/host persistence profiles advance to 15/17/17. The engine record
can retain the rename event and renamed registry state; service and host records
also retain the exact request/reply consequence. Profiles 14/16/16 fail closed
rather than discard the fact or reconstruct a historical name from current
state.

Reversible topic resolution is additive: resolve/reopen command tags `2e` and
`2f`, resolved/reopened outcome tags `2c` and `2d`, capability tag `13`, room
event tag `13`, refusal tag `16`, status command tag `30`, and status outcome tag
`2e` were previously unassigned. The separate status observation keeps the
established topic-page bytes unchanged. Frozen resolve, receipt, and topic-aware
change frames pin the grammar; every change profile carries actor, stable topic
ID, prior state, and new state.

Engine/service/host persistence profiles advance to 16/18/18. The engine record
retains current lifecycle state and its event chain; service and host records
retain the exact request/reply consequence. Profiles 15/17/17 fail closed rather
than admit conversation into a resolved topic or invent lifecycle history.

Bounded topic supersession is additive: set, clear, and status command tags
`31` through `33`, set/clear/status outcome tags `2f` through `31`, capability
tag `14`, room-event tag `14`, and refusal tags `17` and `18` were previously
unassigned. The status observation is separate, so established topic-page bytes
remain unchanged. Every change profile carries actor, source topic, prior
successor, and new successor. Frozen set, receipt, and topic-aware change frames
pin the new grammar.

Engine/service/host persistence profiles advance to 17/19/19. The engine record
retains the depth-one successor graph and its event chain; service and host
records retain the exact request/reply consequence. Profiles 16/18/18 fail
closed rather than discard the relation or reconstruct it from current names.

Bounded topic aliases are additive: add, remove, and status command tags `34`
through `36`, added/removed/status outcome tags `32` through `34`, capability
tag `15`, room-event tag `15`, and refusal tags `19` and `1a` were previously
unassigned. The separate status observation preserves established topic-page
bytes; every change profile carries actor, stable topic ID, alias, and add/remove
state. Frozen add, receipt, and topic-aware change frames pin the grammar.

Engine/service/host persistence profiles advance to 18/20/20. The engine record
retains canonical bounded alias sets and their event chain; service and host
records retain exact request/reply consequences. Profiles 17/19/19 fail closed
rather than discard aliases or infer them from rename history.

Bounded topic tags are additive: add, remove, and status command tags `37`
through `39`, added/removed/status outcome tags `35` through `37`, capability
tag `16`, room-event tag `16`, and refusal tags `1b` through `1e` were
previously unassigned. The separate status observation preserves established
topic-page bytes; every change profile carries actor, stable topic ID, tag, and
add/remove state. Frozen add, receipt, and topic-aware change frames pin the
grammar.

Engine/service/host persistence profiles advance to 19/21/21. The engine record
retains canonical bounded tag sets and their event chain; service and host
records retain exact request/reply consequences. Profiles 18/20/20 fail closed
rather than discard tags or infer them from aliases or names.

Bounded attachment-reference core state advances engine/service/host
persistence profiles to 20/22/22. The engine record adds three descriptor
ceilings and an ordered, bounded attachment relation to every retained and
event-copied message. Each
entry is either a complete canonical descriptor or an identity-only tombstone;
partial metadata cannot recover. Service and Host advance because their
canonical records nest the engine checkpoint. The additive admission and
retained-history protocol advances service and Host to 23/23; the isolated
attachment-change cursor advances them to 24/24. Historical fixtures through
20/23/23 remain frozen; new 20/24/24 fixtures and active fuzz seeds pin the
current shape. Public Wire V2 appends command tags 58 through 61, outcome tags
56 through 58, and capability tag 23. All older tags, frames, message
projections, and Wire V1 remain byte-identical. Two frozen attachment frames
raise compatibility closure to 73 fixtures.

Typed Vellum artifact core state selects engine checkpoint profile 21 whenever
an artifact relation remains in current state or a retained event copy. Profile
21 appends an ordered bounded relation section to every retained and
event-copied message. Each entry is either one room-local relation identity plus
the exact nonzero workspace/document identities and source revision, or an
identity-only tombstone. Artifact-free state remains canonical profile 20, so
all existing engine, service, and Host fixtures remain byte-identical; profile
20 recovers with empty artifact relations. The additive retained-history and
admission protocol advances service persistence to profile 25 only when an
artifact-bearing engine checkpoint, artifact cursor, or artifact request receipt
is present; artifact-free service state remains canonical profile 24. Host
profile 24 remains canonical because its length-delimited service record already
permits the service profile to evolve without reinterpretation. Public Wire V2
appends command tags 62 and 63, outcome tags 59 and 60, and capability tag 24.
All older tags, projections, frames, and fixtures remain byte-identical. Two
frozen artifact frames raise compatibility closure to 75 fixtures and seed both
canonical wire fuzz profiles.

Conversation profiles are additive. `CreatePeerConversation` uses command tag
64 (`40` hex), `ConversationProfile` uses 65 (`41`), creation/status outcomes
use 61/62 (`3d`/`3e`), capability tag 25 (`19`), immutable-cohort refusal 31
(`1f`), and peer-creation event 23 (`17`) in every change projection. Profile
scalars are Ordinary `01`, Direct `02`, and GroupDirect `03`; peer cohorts are
strictly increasing and retain the initiator. The legacy room-summary grammar
and older frames remain unchanged except the intentionally expanded canonical
capability response. Engine profiles 22/23 retain peer state
without/with Vellum relations. Service profiles 26/27 and Host profile 25 form
the new canonical envelopes; valid profiles 24/25 service and 24 Host recover
only with their historical payload vocabulary and migrate deterministically.
Two frozen frames raise compatibility closure to 77 fixtures and seed both
canonical wire fuzz profiles.

Private attention is additive. `AttentionPreference` uses command tag 66
(`42` hex), `SetAttentionPreference` uses 67 (`43`), observation/set outcomes
use 63/64 (`3f`/`40`), capability tag 26 (`1a`), and invalid-scope refusal 32
(`20`). Scope tags reuse no read-position bytes despite naming the same closed
room/thread/topic shapes; preference scalars are two canonical Boolean bytes
plus notification intent `00` through `03`. Engine profiles 24–27 are the
attention-bearing counterparts of 20–23, so attention-free checkpoints retain
their exact canonical bytes. Service 26/27 and Host 25 envelopes already carry
the length-delimited engine record and need no reinterpretation. The canonical
capability frame advances intentionally, while every other older wire fixture
is unchanged. Two frozen attention frames raise compatibility closure to 79
fixtures and seed both canonical wire fuzz profiles.

Shared pins and private saves are additive. `Pins`, `SetPin`,
`SavedReference`, `SetSavedReference`, and `SavedReferences` use command tags
68–72 (`44`–`48` hex); their five outcomes use 65–69 (`41`–`45`). Shared pin
events use tag 24 (`18`) in all four change projections, capability tags 27/28
advertise the profiles independently, and refusal tags 33/34 preserve
invalid-change specificity. Availability is a closed `00` available, `01`
tombstoned, or `02` evicted scalar. Engine profile 20 uses independent feature
bits 8 and 16 for saves and pins, making 20–51 the closed exact feature range;
legacy profile bytes remain unchanged. Existing Service 26/27 and Host 25
length-delimited envelopes carry the new engine state and exact receipts without
outer reinterpretation. Two frozen frames raise compatibility closure to 81
fixtures and seed both canonical wire fuzz profiles.

Room-local topic collections are additive derived observation only. Command tag
79 (`4f` hex), outcome tag 76 (`4c`), and capability tag 30 (`1e`) encode one
closed lifecycle/conjunctive-tag filter and a stable-ID-ordered complete current
metadata page. No established topic page, message relation, event, refusal, or
checkpoint byte is reinterpreted. Service profiles 28/29 are emitted only when
a collection request/reply or kind-18 collection cursor is retained; otherwise
profiles 26/27 remain byte-identical. Host profile 26 encloses service 28/29,
while established state remains Host profile 25. Engine cursor kind 10 is a
private checkpoint grammar and adds no engine-state profile. Two frozen wire
frames raise compatibility closure to 83 fixtures and seed both canonical wire
fuzz profiles.

Transient collaboration Wire V1 is a new, separately invoked grammar. Its
profile byte `01`, presence command/outcome tags `01` through `03`, typing
command/outcome tags `04` through `06`, reply tags `81` and `82`, and refusal
tags `01` through `0b` do not reinterpret or extend durable Wire V1/V2 frames.
The four frozen transient frames join the compatibility inventory; every older
fixture, capability list, and checkpoint remains byte-identical. No transient
command is added to durable Wire V2 capability discovery.

Private local collaboration handshake profile 2 is additive and selects
conversation Wire V2 plus transient Wire V1. Its 14-byte ClientHello, 42-byte
ServerHello, and 40-byte `AGORATE1` session-control hello have separate frozen
fixtures. The collaboration ServerHello reports support for this multiplexed
profile; handshake profile 1 and all prior bytes remain unchanged. Routed outer
payloads use a closed one-byte conversation/transient/control discriminator,
and the server-only control hello binds a nonzero replay epoch to one declared
server incarnation and connection.

Derived Search Wire V1 is another separately invoked additive grammar. Its
profile byte `01`, accepted/refused reply tags `81`/`82`, available, incomplete,
stale, and unavailable outcomes, and nine closed refusal tags do not reinterpret
conversation or transient frames. The two frozen frames pin a client-bound
offer and identity-only available page. Pages require unique stable message IDs,
strictly increasing room-local sequences, and no continuation on an empty page.
The service cursor is opaque and fixed-width; these bytes grant no room authority
and remain unreachable until an authenticated host/transport profile selects
them explicitly.

Private local search handshake profile 3 selects conversation Wire V2 plus
Search Wire V1 without changing profiles 1 or 2. Its separate 14-byte
`AGORAHC3` ClientHello and 42-byte `AGORAHS3` ServerHello are frozen. The routed
envelope assigns additive search route tag `04`; profile-specific daemon checks
reject search under collaboration profile 2 and transient frames under search
profile 3. The existing control hello supplies the connection/server-bound
replay epoch used by the search client uncertainty fence.

Admission and moderation controls are additive. Capability tag `1f` advertises
`AdmissionModerationControlsV1`; command tags `50` through `56`, outcome tags
`4d` through `52`, refusal tags `29` through `2e`, and room-event tags `1c` and
`1d` were previously unassigned. Enforcement time is deliberately absent from
the wire grammar. Report records are confined to new private outcomes and never
reinterpret an ordinary change feed. Engine checkpoint profile `148` adds an
explicit feature byte only when private reports exist; profiles `20` through
`147` retain their exact prior grammar and bytes. The frozen restriction request
and report-list reply pin representative new frames.

Engine checkpoint profile `149` is additive and selected only when at least one
source-scoped publication receipt exists. It retains the profile-148 feature
byte, adds a closed private-report-presence byte, and prefixes every publication
key with one closed domain tag before its unchanged 16-byte identity. Legacy
receipts continue to emit and recover under their earlier profile and bytes.
Scheduled and typed-interaction services now allocate distinct domains; exact
namespaced offers may replay an equal legacy receipt during migration. The
frozen `checkpoint-v21.hex` fixture pins the federation-domain representation,
and unknown domain or metadata tags fail closed.

The additive `agora-federated-publication-port` has no wire grammar or durable
bytes of its own. It reuses the profile-149 federation domain through Service,
Host, and Durable Host; Protocol V1/V2 and Wire V1/V2 bytes remain unchanged.

Engine checkpoint profile `150` is selected only when a federation receipt has
a full 64-byte home scope in addition to its 16-byte request ID. It preserves
profile 149's metadata, then gives every receipt a closed short-or-scoped
identity tag. A scoped identity is valid only in the federation domain. The
frozen `checkpoint-v22.hex` fixture pins the grammar; unknown identity tags and
scoped identities under other domains fail closed. Profile 149 bytes remain
unchanged and provide the bounded exact-content migration alias.

Conversation Service profile `32` is additive and selected after the first
receipt-state mutation in the new source-revision domain. It adds an explicit
policy-presence byte, a nonzero receipt revision, and a closed exhaustion byte;
Host profile `29` encloses exactly that Service profile. Service profiles 24
through 31 and Host profiles 24 through 28 retain their established bytes and
recover with no receipt-source revision. They become eligible for the new
domain only after a later receipt-state mutation, preventing migration from
inventing an export generation.

The owner-native receipt export record is a separate administrative grammar:
magic `AGARRC1`, version `1`, followed by its nonzero receipt revision, ordered
live client/request keys with canonical Wire V2 request/reply frames, and
ordered receipt-retirement tombstones. It changes no participant Protocol or
Wire tag and no Service/Host checkpoint grammar. A frozen empty revision-2
record pins the zero-live/zero-tombstone representation; bounded recovery and
canonical re-encoding close all other records.

The independent `agora-export-service` checkpoint remains outside conversation
Engine, Service, Host, Protocol, and Wire profiles. Its additive checkpoint V2
adds an independently bounded retirement-tombstone capacity and a canonical
request-ordered list of exact manifest-derived retirement identities. V1
recovery remains supported. A service with no tombstones and equal live/retired
capacities continues to emit byte-identical V1, so enabling retirement does not
rewrite existing empty or live-only compatible custody checkpoints. Frozen V1
empty-custody and V2 retired-custody fixtures pin both representations.

The directory adapter's generational profile is separately typed from the
unchanged single-head V1 store. Its canonical reclamation journal uses magic
`AGGREC1`, version `1`, bounded sorted plan receipts, exact replacement digest,
and engine generation identities followed by CRC32C. A frozen completed-plan
fixture pins the representation; the compatibility mutation gate rejects byte,
inventory, and empty-fixture drift.

## Conservative baseline gate

`docs/compatibility-baseline.txt` records the exact sorted Cargo package
inventory and every canonical hexadecimal fixture's decoded byte count and
source-file SHA-256 digest. `make check-baseline` refuses package additions,
removals, renames, version changes, fixture additions or removals, and byte or
text-format drift until the baseline is deliberately reviewed and updated.
The gate's adversarial companion removes a package, changes a fixture digest,
and empties the fixture inventory; each mutant must be refused.

This is a conservative change detector, not a semantic-versioning proof. An
unchanged baseline does not establish that every public Rust type or possible
wire value is compatible. A reviewed additive change still requires a new
fixture where it adds a canonical public representation and must update the
requirements, specification, traceability, and release-readiness claims that it
affects.

