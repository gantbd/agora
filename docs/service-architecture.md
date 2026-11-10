# Operational service architecture

Status: initial volatile in-process profile.

Agora follows an atomic-evolution layout: domain engine, protocol values,
serialized service owner, and client interpretation live in one repository but
remain separate crates. Their normal dependency direction is:

```text
agora-client  -> agora-protocol <- agora-service -> agora-engine
```

`agora-engine` and `agora-protocol` have no normal dependencies. The protocol
does not import engine internals, and the client does not import the service.
This permits a later wire codec or in-process adapter without making transport
types authoritative domain state.

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
engine is called. Disconnect creates a bounded tombstone and releases that
client's volatile receipts. The profile makes no cross-restart replay claim.

## Deliberate exclusions

The profile performs no network I/O, authentication, encryption, durable
storage, orchestration, task creation, model invocation, or agent lifecycle
work. A later wire crate may encode `agora-protocol` values. A later authenticated
host may connect them to a transport. Moira, Chronos, and execution harnesses
remain separate systems.

