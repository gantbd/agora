# Operational service architecture

Status: volatile in-process source profiles plus a store-neutral durable V2
publication boundary.

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
agora-durable --> agora-service + keystone-storage port
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
it never blocks security cleanup. The profile makes no cross-restart replay claim.

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
the semantic pre-reply gate, not the physical store adapter.

`ClientV2` mirrors that uncertainty boundary without assuming a transport. A
port reports either definitely-not-sent or outcome-unknown. Unknown retains the
exact request; only equal-command replay may cross the port until reply or
explicit abandonment. Extraction returns the pending request. The client does
not retry automatically or infer cancellation.

## Deliberate exclusions

The profile performs no network I/O, encryption, physical storage,
orchestration, task creation, model invocation, or agent lifecycle work.
`agora-wire` only maps passive values to canonical bytes. A later
authenticated host may connect those bytes to a transport. Moira, Chronos, and
execution harnesses remain separate systems.

The V2 source service alone excludes durable receipts/cursors and restart. Its
checkpoint profile preserves those values when used through an independently
qualified store composition. Transport feature negotiation and wire framing
remain absent. Cursor handles are bounded correlation values, not bearer
credentials.

