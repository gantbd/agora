# Transient wire profile V1

Status: canonical bounded presence and typing grammar with authenticated Host,
command-neutral client, and qualified private local transport composition.

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
| `04` | PublishTyping | room text, target, requested lifetime `u64` |
| `05` | ClearTyping | room text, target |
| `06` | TypingPage | room text, target, optional opaque typing cursor, limit encoded as bounded `u32` |

Presence state tags are Available `01`, Away `02`, and Busy `03`. Optional
cursors use absent `00` or present `01` followed by an opaque `u64`; presence
and typing cursor types are not interchangeable. Typing targets are room `01`,
thread `02` plus an opaque 16-byte retained root ID, or topic `03` plus an opaque
16-byte stable topic ID. Request IDs
are scoped to one authenticated transient Host session. The Host retains one
exact prior request/reply per session: an equal request ID and command replays
that semantic reply, changed input conflicts, and skipped or exhausted request
identity refuses without applying the command.

## Replies

Accepted is root tag `81` followed by one outcome; refused is root tag `82`
followed by one refusal. Outcome tags are PresencePublished `01` with a positive
accepted lifetime, PresenceCleared `02` with a canonical Boolean, and
PresencePage `03`. TypingPublished `04` carries a positive accepted lifetime,
TypingCleared `05` carries a canonical Boolean, and TypingPage is `06`.

A presence page is a strictly participant-ID-ordered bounded list and an
optional opaque cursor. Each entry contains participant ID, descriptive kind,
view state, optional source class, and optional remaining lifetime. `Unknown`
must carry neither source nor lifetime. Available, Away, and Busy must carry the
only source class, ParticipantSession `01`, and a positive remaining lifetime.
The wire never exposes the Host clock epoch, session handle, or credential.

A typing page has the same strict participant ordering and privacy-safe source
projection. Its state is Inactive `00` or Active `01`. Inactive carries neither
source nor lifetime; Active carries ParticipantSession and a positive remaining
lifetime. The target is request-scoped and therefore identical for every entry
in the page.

Refusal tags are NotMember `01`, InvalidLifetime `02`, InvalidPage `03`,
InvalidCursor `04`, StaleCursor `05`, Capacity `06`, FidelityLost `07`,
Unavailable `08`, RequestConflict `09`, RequestOutOfSequence `0a`, and
InvalidTarget `0b`.

## Compatibility and nonclaims

The profile adds new entry points and does not alter any V1/V2 frame, durable
request, capability list, checkpoint, or service receipt. The remote-client SDK
can name the raw protocol, codec, and `TransientClientV1` state machine through
its existing facade reexports. The codec, Host, and client composition cover
both presence and typing. The client retains only one exact outcome-unknown
request, permits equal-command replay, and validates exported state. Its opaque
nonzero replay epoch is caller-supplied and must bind both the server
incarnation and exact authenticated session; any reconnect or restart requires
explicit `rebind`, which returns old uncertainty and starts request ID zero.
The authenticated Host derives the signal source from its opaque session,
revalidates authority at current Host time, and maps public cursors to one
private live cursor per session. A cursor binds room, participant, engine
revision, transient generation, and snapshot time; a separately configured
positive cursor age bounds how long that older snapshot can be continued.
Signal or membership change stales it, and another session cannot use it.
The wrapper's monotone transient clock advances for every authenticated offer,
including malformed or refused requests, so another session cannot roll time
back after a high-time refusal. Regression retires that caller and its source.

Local collaboration handshake profile 2 selects conversation Wire V2 and this
transient Wire V1 together. Each outer payload begins with a closed route tag:
conversation `01`, transient `02`, or server-only control `03`; the route byte
is included in the advertised outer-frame ceiling. Connect must use the
conversation route. After successful authenticated connect, the daemon sends a
fixed `AGORATE1` control hello containing a nonzero opaque 32-byte replay epoch.
Its first half is the declared server incarnation and its second half is a
monotone per-daemon connection sequence, binding one client replay namespace to
one authenticated Host session. Client control frames refuse. The
conversation-only handshake profile and durable capability list are unchanged.
