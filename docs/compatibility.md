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

The immutable `tests/fixtures/checkpoint-v17.hex` artifact pins a representative
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

