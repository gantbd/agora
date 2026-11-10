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

Protocol, Service, Client, and Wire V2 form an additive profile with a separate
version byte, closed grammar, and three canonical fixtures. Their separately
typed values and V2 bytes do not alter V1 tags or bytes. Authenticated Host V2
uses the same canonical frames without reinterpreting V1.

Service persistence is versioned independently. `AGORASV` version 1 captures
the complete V2 service owner and embeds private `AGORACUR` version-1 cursor
records. A representative complete service record is pinned by
`service-checkpoint-v1.hex`. Public wire compatibility neither implies nor migrates durable state;
future durable schema changes require a bounded explicit migration profile.

Authenticated host persistence has its own `AGORAHST` version-1 record and
`host-checkpoint-v1.hex` fixture. It nests the service record but owns host
session compatibility independently; neither wire nor service checkpoint
decoding implies that a host record can be recovered.

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

