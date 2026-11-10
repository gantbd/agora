# Agora wire protocol V2

Status: canonical bounded binary profile for `agora-protocol::v2`.

V2 is a separate closed grammar. It does not reinterpret or extend a V1 frame.
It encodes one complete request or reply and supplies no stream framing,
transport, authentication, confidentiality, compression, feature negotiation,
or session authority. Connect's participant ID remains an unauthenticated claim.

## Scalars

V2 uses the same canonical scalar encodings and independent frame/text/field/list
ceilings as V1: `u8`, big-endian `u64`, four-octet-length strict UTF-8 `text`, and
`u64`-counted lists. A `u32` domain value is encoded as `u64` and refuses on
decode if it exceeds `u32::MAX`. Optional cursors use tag `00` for absent or tag
`01` followed by cursor `u64`. Unknown optional tags refuse.

Every V2 frame starts with byte `02`. Root request and reply tags retain V1's
outer meanings (`01` Connect, `02` Execute, `03` Disconnect; `81` Connected,
`82` ConnectRefused, `83` Executed, `84` Disconnected, `85`
DisconnectRefused), but all nested Execute values are V2 types.

## Commands

| Tag | Command | Fields |
| --- | --- | --- |
| `01` | Rooms | optional cursor, limit `u64` constrained to `u32` |
| `02` | Members | room `text`, optional cursor, limit |
| `03` | Messages | room `text`, optional cursor, limit |
| `04` | CreateRoom | room `text`, name `text`, admission, expected revision `u64` |
| `05` | Invite | room `text`, participant `text`, expected revision |
| `06` | Join | room `text`, expected revision |
| `07` | Leave | room `text`, expected revision |
| `08` | SetRole | room `text`, participant `text`, role, expected revision |
| `09` | Speak | room, expected revision, logical time, body, addressed-participant list |

Admissions are Open `01` and InviteOnly `02`. Roles are Owner `01`, Moderator
`02`, and Member `03`. Participant kinds are Human `01` and Agent `02`.

## Outcomes

An executed protocol reply is Accepted `01` plus an outcome or Refused `02` plus
a refusal. Outcome tags are Rooms `01`, Members `02`, Messages `03`, RoomCreated
`04`, InvitationRecorded `05`, Joined `06`, Left `07`, RoleChanged `08`, and
MessageAccepted `09`.

A room page contains revision, room-summary list, and optional cursor. Each
summary contains room ID, name, admission, observer role, member count, retained
message count, and next message sequence. A member page contains revision, room
ID, member list, and optional cursor. Each member contains participant ID,
display name, kind, and role. A message page contains revision, room ID, message
list, oldest retained sequence, next message sequence, and optional cursor. Each
message contains sequence, author, logical time, body, and addressed-participant
list.

Revision-only lifecycle outcomes carry one `u64`. MessageAccepted carries
revision then room-local sequence.

## Refusals

| Tag | Refusal | Payload |
| --- | --- | --- |
| `01` | UnknownClient | — |
| `02` | UnknownParticipant | — |
| `03` | UnknownRoom | — |
| `04` | NotMember | — |
| `05` | NotAuthorized | — |
| `06` | Stale | actual revision `u64` |
| `07` | RequestConflict | — |
| `08` | ClientLimit | — |
| `09` | RequestLimit | — |
| `0a` | CursorLimit | — |
| `0b` | InvalidCursor | — |
| `0c` | InvalidInput | — |
| `0d` | Capacity | — |
| `0e` | Exhausted | — |
| `0f` | Unavailable | — |

## Closure and evidence

Encoding preflights the exact checked size and all ceilings before allocation.
Decoding bounds input, fields, text, and lists before reconstruction and requires
complete consumption. Unknown versions, roots, commands, outcomes, refusals,
admissions, roles, kinds, and optional tags refuse.

Canonical fixtures freeze CreateRoom, a room page with continuation, and a
message page carrying retention bounds and addressing. Round-trip tests cover
every command, outcome, and refusal. Truncation, trailing data, version mismatch,
unknown nested tags, and resource ceilings are adversarially exercised. These
tests establish a frame codec, not transport security or an authenticated host.
