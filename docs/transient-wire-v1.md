# Transient wire profile V1

Status: canonical bounded presence grammar plus client uncertainty state,
unadvertised and not yet connected to a public Host runtime.

This profile is separate from durable Protocol/Wire V2. A transient request is
not a `CommandV2`, does not receive a durable service receipt, and cannot replay
conversation state. Every frame uses the common wire scalar and limit rules,
begins with profile byte `01`, and contains exactly one value.

## Requests

A request begins with an eight-byte big-endian volatile request ID followed by
one command:

| Tag | Command | Fields |
| --- | --- | --- |
| `01` | PublishPresence | room text, state, requested lifetime `u64` |
| `02` | ClearPresence | room text |
| `03` | PresencePage | room text, optional opaque cursor, limit encoded as bounded `u32` |

Presence state tags are Available `01`, Away `02`, and Busy `03`. Optional
cursors use absent `00` or present `01` followed by an opaque `u64`. Request IDs
are scoped to one authenticated transient Host session; the future Host
composition must enforce exact replay, conflict, and ordering rules.

## Replies

Accepted is root tag `81` followed by one outcome; refused is root tag `82`
followed by one refusal. Outcome tags are PresencePublished `01` with a positive
accepted lifetime, PresenceCleared `02` with a canonical Boolean, and
PresencePage `03`.

A presence page is a strictly participant-ID-ordered bounded list and an
optional opaque cursor. Each entry contains participant ID, descriptive kind,
view state, optional source class, and optional remaining lifetime. `Unknown`
must carry neither source nor lifetime. Available, Away, and Busy must carry the
only source class, ParticipantSession `01`, and a positive remaining lifetime.
The wire never exposes the Host clock epoch, session handle, or credential.

Refusal tags are NotMember `01`, InvalidLifetime `02`, InvalidPage `03`,
InvalidCursor `04`, StaleCursor `05`, Capacity `06`, FidelityLost `07`,
Unavailable `08`, RequestConflict `09`, and RequestOutOfSequence `0a`.

## Compatibility and nonclaims

The profile adds new entry points and does not alter any V1/V2 frame, durable
request, capability list, checkpoint, or service receipt. The remote-client SDK
can name the raw protocol, codec, and `TransientClientV1` state machine through
its existing facade reexports. The client retains only one exact outcome-unknown
request, permits equal-command replay, and validates exported state. Its opaque
nonzero replay epoch is caller-supplied and must bind both the server
incarnation and exact authenticated session; any reconnect or restart requires
explicit `rebind`, which returns old uncertainty and starts request ID zero.
There is no qualified Host handler or transport binding yet. Presence is not
advertised until authenticated Host replay/cursor composition, typing,
local-daemon lifecycle evidence, and the remaining R62 gates close.
