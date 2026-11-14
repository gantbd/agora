# Agora wire protocol V1

Status: canonical bounded binary profile for `agora-protocol` V1.

This profile encodes one complete request or reply frame. It supplies no stream
framing, transport, authentication, confidentiality, compression, or session
authority. A transport adapter must establish exact frame boundaries and apply
its own authenticated connection binding.

## Scalar grammar

- `u8`: one octet.
- `u64`: eight octets, unsigned, big-endian. All bit patterns are canonical.
- `text`: a four-octet unsigned big-endian byte length followed by exactly that
  many strict UTF-8 octets. Text is preserved byte-for-byte; no Unicode
  normalization occurs.
- `list<T>`: a `u64` item count followed by exactly that many `T` values.

Every frame begins with version `01`, then one root tag. There are no optional,
reserved, or ignored fields. Unknown versions/tags, truncation, invalid UTF-8,
limit violation, or trailing data refuse the whole frame.

## Request roots

| Tag | Value | Remaining fields |
| --- | --- | --- |
| `01` | Connect | participant ID `text` |
| `02` | Execute | client `u64`, request `u64`, command |
| `03` | Disconnect | client `u64` |

The participant ID in Connect is a claim, not proof. The host must authenticate
it before connecting the service and must bind the returned client ID to the
authenticated transport context. Client IDs are sequential correlation values,
not bearer credentials.

Commands use tag `01` for Snapshot followed by room ID `text`. Speak uses tag
`02`, then room ID `text`, expected revision `u64`, logical time `u64`, body
`text`, and a list of addressed participant-ID texts.

## Reply roots

| Tag | Value | Remaining fields |
| --- | --- | --- |
| `81` | Connected | client `u64` |
| `82` | ConnectRefused | refusal |
| `83` | Executed | protocol reply |
| `84` | Disconnected | none |
| `85` | DisconnectRefused | refusal |

A protocol reply tag `01` is accepted and is followed by an outcome. Tag `02`
is refused and followed by a refusal. Outcome tag `01` is a room snapshot; tag
`02` is MessageAccepted followed by revision and sequence `u64` values.

A snapshot contains revision `u64`, room ID and name texts, a member list, a
message list, and next message sequence `u64`. A member is participant ID text,
kind, and role. Kinds are Human `01` and Agent `02`. Roles are Owner `01`,
Moderator `02`, and Member `03`. A message is sequence `u64`, author text,
logical time `u64`, body text, and an addressed-participant text list.

## Refusal tags

| Tag | Refusal | Payload |
| --- | --- | --- |
| `01` | UnknownClient | — |
| `02` | UnknownParticipant | — |
| `03` | UnknownRoom | — |
| `04` | NotMember | — |
| `05` | Stale | actual revision `u64` |
| `06` | RequestConflict | — |
| `07` | ClientLimit | — |
| `08` | RetiredClientLimit | — |
| `09` | RequestLimit | — |
| `0a` | SnapshotLimit | — |
| `0b` | InvalidInput | — |
| `0c` | Capacity | — |
| `0d` | Exhausted | — |
| `0e` | Unavailable | — |
| `0f` | NotAuthorized | — |

## Bounds and compatibility

`WireLimits` independently bounds total frame bytes, bytes in each text field,
decoded field count, and items in each list. Encoding measures the complete
frame with checked arithmetic and validates all ceilings before reserving exact
output capacity. Decoding applies ceilings before copying text or reserving list
storage and requires complete input consumption.

The fixtures under `protocol/agora-wire/tests/fixtures` freeze Connect, addressed
Speak with multibyte UTF-8, and a mixed human/agent snapshot. Any semantic change
requires a new version; V1 bytes must not be reinterpreted.

