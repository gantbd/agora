# Operational service architecture

Status: volatile in-process source profiles, a store-neutral durable V2
publication boundary, and one qualified local Unix checkpoint-store adapter.

The repository now carries two source profiles. `Service` preserves frozen
Protocol V1 Snapshot/Speak behavior. `v2::ServiceV2` is separately typed and adds
participant-bound room lifecycle parity plus bounded room/member/message pages.
It has a separately versioned canonical wire codec. `agora-host::v2` supplies the
separately qualified authenticated composition; V1 is not reinterpreted.

Agora follows an atomic-evolution layout: domain engine, protocol values,
serialized service owner, and client interpretation live in one repository but
remain separate crates. Their normal dependency direction is:

```text
agora-client  -> agora-protocol <- agora-service -> agora-engine
                                <- agora-wire -> keystone-codec
agora-host -----> agora-wire + agora-service + trusted verifier port
agora-durable --> agora-host + agora-service + keystone-storage port
agora-storage-directory --> keystone-storage port + local Unix filesystem
```

`agora-engine` and `agora-protocol` have no normal dependencies. The protocol
does not import engine internals, and the client does not import the service.
The wire crate depends only on passive protocol values and the shared bounded
Keystone codec primitive. Transport types never become authoritative domain state.
The host composes wire and service through an injected principal-verification
port; it imports no credential or Aegis vocabulary.

## Connection and identity

`Service::connect` binds a newly allocated client handle to one already
registered `ParticipantId`. Every speaking command derives its author from that
binding; the request cannot nominate another author. Snapshot observation is
limited to rooms containing that participant. Human and agent participants use
the same path.

This is identity binding, not authentication. The initial service accepts a
participant ID from a trusted host. A network host must authenticate that claim,
keep the client handle server-side or bind it to an unforgeable transport
context, and revoke the binding on disconnect. Sequential `ClientIdV1` values
must never be treated as bearer credentials.

## Serialization and replay

One `Service` owns one `Engine` and accepts calls through `&mut self`; the host
must preserve that serialized ownership. Request identity is scoped by client.
The service retains the exact request and reply. Matching retry returns the
original reply without applying a second transition; changed content under the
same request ID conflicts.

Client, retired-client, request-receipt, identifier, input, fan-out, and snapshot
limits are checked explicitly. Structurally oversized requests are refused before
receipt retention. A full receipt index backpressures a new request before the
engine is called. Disconnect releases that client's volatile receipts and keeps
a bounded recent tombstone window. Tombstone pressure evicts the oldest record;
it never blocks security cleanup. The V1 volatile profile makes no cross-restart
replay claim.

After completing bounded room state pages at one revision, a client can open a
room change position at that exact revision and pull retained room events through
an opaque client-owned cursor. Pages are exclusive-after and replayable by exact
request identity. The service consumes each successful cursor into a next cursor;
foreign, cross-room, or consumed handles share `InvalidCursor`. Outbox retention
loss and leave/rejoin visibility discontinuity are explicit gap outcomes that
require resnapshot. The cursor is included in complete service recovery, but the
feed remains pull observation rather than delivery acknowledgment or presence.

## Durable state and publication

`ServiceV2::prepare_checkpoint` captures the complete semantic service owner,
including private cursor state and exact request replies, under an independent
record version and store-scope binding. Recovery is bounded, canonical, and
fail-closed. Process-local usage counters are excluded so exact replay does not
manufacture a state change.

`agora-durable` owns one service and one revisioned checkpoint-store port. It
suppresses changed-state results until the store returns the exact next revision
with durable acknowledgement strength. Known refusal reconstructs the pre-offer
service. Unknown outcome, store error, weak acknowledgement, or malformed receipt
fences the owner. No mutable service access is exposed while fenced. This closes
the semantic pre-reply gate independently of any store implementation.

`agora-storage-directory` is the one qualified physical composition. It owns no
Agora semantics: it implements the Keystone store port for one key using a
private directory, exclusive process lock, checksummed canonical record, and
file-sync/atomic-rename/directory-sync commit. Its filesystem effects are
isolated by the dependency gate. The mechanism is local Unix only and does not
claim hardware power-loss, network-filesystem, backup/restore, encryption, or
multi-host behavior.

`ClientV2` mirrors that uncertainty boundary without assuming a transport. A
port reports either definitely-not-sent or outcome-unknown. Unknown retains the
exact request; only equal-command replay may cross the port until reply or
explicit abandonment. Extraction returns the pending request. The client does
not retry automatically or infer cancellation.

The authenticated durable composition moves the checkpoint boundary above
`HostV2`. Its canonical unit contains every active subject/participant/session/
service-client binding and the entire service record. This prevents the partial
composition in which a committed service receipt survives without the host
session needed to replay it. A recovered verifier is runtime-owned and must
revalidate current context before the retained session can act.

## Deliberate exclusions

The semantic profiles perform no network I/O, encryption, direct physical
storage, orchestration, task creation, model invocation, or agent lifecycle
work. The separate directory adapter performs only its documented local
filesystem I/O.
`agora-wire` only maps passive values to canonical bytes. A later
authenticated host may connect those bytes to a transport. Moira, Chronos, and
execution harnesses remain separate systems.

The V2 source service alone excludes durable receipts/cursors and restart. Its
checkpoint profile preserves those values when used through an independently
qualified store composition. Transport feature negotiation and wire framing
remain absent. Cursor handles are bounded correlation values, not bearer
credentials.

