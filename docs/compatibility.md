# Compatibility policy

Agora is pre-1.0 and currently source-distributed. Public schemas still change
only through an explicit profile revision.

The v0.3 command, event, publication, and checkpoint values use the `V2` suffix.
The `AGORACP` header version is 2. V1 checkpoint bytes are refused with
`CheckpointError::Profile`; they are never interpreted as V2. V1 agent activation
has no migration because activation is orchestration behavior, not conversation
state. A host retaining V1 state must export conversation facts through a
purpose-built migration tool before adopting V2.

Future compatible additions require either a reserved representation or a new
closed version. Unknown tags, fields, or trailing bytes remain refusals.

The immutable `tests/fixtures/checkpoint-v2.hex` artifact pins a representative
V2 state containing both participant kinds, neutral addressing, replay-safe
publication, acknowledgment, and the exact pending outbox suffix.

The operational protocol is separately versioned as `V1` values. `agora-wire`
defines its canonical binary representation and refuses unknown versions, tags,
malformed text, limit violations, and trailing bytes. Golden request/reply
fixtures freeze representative bytes. The codec defines complete message frames,
not stream boundaries; transport framing remains separate.

