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
| `0a` | OpenChanges | room `text`, exact completed snapshot revision |
| `0b` | Changes | room `text`, required opaque cursor `u64`, limit |
| `0c` | Kick | room, participant, reason, 16-byte causation, expected revision |
| `0d` | Ban | room, participant, reason, 16-byte causation, expected revision |
| `0e` | Unban | room, participant, reason, 16-byte causation, expected revision |
| `0f` | SetAdmissionPolicy | room, admission, expected revision |
| `10` | Bans | room `text`, optional cursor, limit |

Admissions are Open `01` and InviteOnly `02`. Roles are Owner `01`, Moderator
`02`, and Member `03`. Participant kinds are Human `01` and Agent `02`.

## Outcomes

An executed protocol reply is Accepted `01` plus an outcome or Refused `02` plus
a refusal. Outcome tags are Rooms `01`, Members `02`, Messages `03`, RoomCreated
`04`, InvitationRecorded `05`, Joined `06`, Left `07`, RoleChanged `08`, and
MessageAccepted `09`, ChangesOpened `0a`, Changes `0b`, ChangeGap `0c`, Kicked
`0d`, Banned `0e`, Unbanned `0f`, RoomPolicyChanged `10`, and Bans `11`.

A room page contains revision, room-summary list, and optional cursor. Each
summary contains room ID, name, admission, observer role, member count, retained
message count, and next message sequence. A member page contains revision, room
ID, member list, and optional cursor. Each member contains participant ID,
display name, kind, and role. A message page contains revision, room ID, message
list, oldest retained sequence, next message sequence, and optional cursor. Each
message contains sequence, author, logical time, body, and addressed-participant
list.

A ban page contains revision, room ID, a bounded list, and optional cursor. Each
entry contains participant ID, display name, descriptive kind, and prior room
role. Service authority restricts this page to current owners and moderators;
the wire representation itself grants no authority.

Revision-only lifecycle outcomes carry one `u64`. MessageAccepted carries
revision then room-local sequence.

ChangesOpened carries the snapshot revision and a required opaque cursor.
Changes carries current revision, a counted list of `(event sequence, accepted
revision, room event)`, the next required cursor, and a closed `00`/`01`
has-more byte. Room event tags are RoomCreated `01` (owner, admission),
InvitationRecorded `02` (participant), ParticipantJoined `03` (participant),
ParticipantLeft `04` (participant), RoleChanged `05` (participant, role), and
MessageAccepted `06` (message), ParticipantKicked `07`, ParticipantBanned `08`,
ParticipantUnbanned `09`, and RoomPolicyChanged `0a` (actor, prior admission,
new admission). Each moderation event carries actor and subject
text, bounded reason text, and fixed 16-byte causation encoded as two big-endian
`u64` words. ChangeGap carries current revision and either
Retention `01` plus acknowledged-through sequence or MembershipChanged `02`.
Unknown event, gap, or Boolean tags refuse.

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
| `10` | Banned | — |

## Closure and evidence

Encoding preflights the exact checked size and all ceilings before allocation.
Decoding bounds input, fields, text, and lists before reconstruction and requires
complete consumption. Unknown versions, roots, commands, outcomes, refusals,
admissions, roles, kinds, and optional tags refuse.

Canonical fixtures freeze CreateRoom, a room page with continuation, a message
page carrying retention bounds and addressing, a change page carrying its
stable event identity and next cursor, a moderation change preserving evidence,
an admission-policy change, and a ban page preserving its authority-relevant
prior role. Round-trip tests cover every command,
outcome, refusal, room-event kind, and gap cause. Truncation, trailing data,
version mismatch, unknown nested tags, and resource ceilings are adversarially
exercised. These tests establish a frame codec, not transport security, push
delivery, or an authenticated host.
