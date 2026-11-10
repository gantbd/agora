# Operational service architecture

Status: initial volatile in-process profile.

The repository now carries two source profiles. `Service` preserves frozen
Protocol V1 Snapshot/Speak behavior. `v2::ServiceV2` is separately typed and adds
participant-bound room lifecycle parity plus bounded room/member/message pages.
It has a separately versioned canonical wire codec but no authenticated-host
projection yet; V1 is not reinterpreted.

Agora follows an atomic-evolution layout: domain engine, protocol values,
serialized service owner, and client interpretation live in one repository but
remain separate crates. Their normal dependency direction is:

```text
agora-client  -> agora-protocol <- agora-service -> agora-engine
                                <- agora-wire -> keystone-codec
agora-host -----> agora-wire + agora-service + trusted verifier port
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

## Deliberate exclusions

The profile performs no network I/O, authentication, encryption, durable
storage, orchestration, task creation, model invocation, or agent lifecycle
work. `agora-wire` only maps passive values to canonical bytes. A later
authenticated host may connect those bytes to a transport. Moira, Chronos, and
execution harnesses remain separate systems.

The V2 source service additionally excludes durable receipts/cursors, cursor
resumption across restart, transport feature negotiation, wire framing, and
authentication. Its cursor handles are bounded correlation values scoped to one
volatile client, not bearer credentials.

