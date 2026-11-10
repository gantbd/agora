# Compatibility policy

Agora is pre-1.0 and currently source-distributed. Public schemas still change
only through an explicit profile revision.

The v0.3 command, event, publication, and checkpoint values use the `V2` suffix.
The `AGORACP` header version is 13. Earlier checkpoint bytes are refused with
`CheckpointError::Profile`; they are never interpreted as V13. V1 agent activation
has no migration because activation is orchestration behavior, not conversation
state. A host retaining V1 state must export conversation facts through a
purpose-built migration tool before adopting V2.

Additive V2 vocabulary allocations may use previously unassigned tags only when
all existing values retain byte-identical encodings and older implementations
fail closed on the new value. Reinterpreting an assigned tag or changing any
existing field grammar requires a new frame version. Unknown tags, fields, or
trailing bytes remain refusals.

The immutable `tests/fixtures/checkpoint-v13.hex` artifact pins a representative
V13 state containing both participant kinds, stable message and topic identity,
topic-scoped messages, thread lifecycle, reactions, message revisions, neutral addressing, replay-safe
publication, ban and policy-change evidence, one private read position,
acknowledgment, and the exact pending outbox suffix.

The operational protocol is separately versioned as `V1` values. `agora-wire`
defines its canonical binary representation and refuses unknown versions, tags,
malformed text, limit violations, and trailing bytes. Golden request/reply
fixtures freeze representative bytes. The codec defines complete message frames,
not stream boundaries; transport framing remains separate.

Protocol, Service, Client, and Wire V2 form an additive profile with a separate
version byte, closed grammar, and twenty canonical fixtures. Their separately
typed values and V2 bytes do not alter V1 tags or bytes. Authenticated Host V2
uses the same canonical frames without reinterpreting V1.

Service persistence is versioned independently. `AGORASV` version 15 captures
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
`service-checkpoint-v15.hex`. Public wire
compatibility neither implies nor migrates durable state;
future durable schema changes require a bounded explicit migration profile.

Authenticated host persistence has its own `AGORAHST` version-15 record and
`host-checkpoint-v15.hex` fixture. It nests the service record but owns host
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

Private read-position persistence advances engine/service/host profiles to
13/15/15. The engine record appends one canonical participant/room/scope-keyed
registry after room state; each marker retains exact target identity, sequence,
and observed revision. Scope tags `01`, `02`, and `03` identify room, thread,
and topic selectors. No public wire tag is allocated by this persistence-only
advance, and earlier checkpoint profiles fail closed rather than dropping the
private state.

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

