# Moira integration profile

Status: qualified by the local durable receiver in the Moira repository.

## Boundary

Agora owns participants, rooms, membership, roles, messages, participant-neutral
addressing, ordered events, and replay-safe response publication. Moira owns the
decision to create workflow state, the workflow recipient reference, task and
execution correlation, uncertainty, and response-publication intent.

An Agora address is conversation data. It is neither authority nor a workflow
command, regardless of whether the author or recipient is recorded as human or
agent. Harness selection, prompting, retries, model behavior, and tool authority
remain outside Agora and outside the ingress projection.

## Qualified flow

```text
Agora MessageAccepted event
        |
        v
read-only pending message fact
        |
        | host policy and participant-reference mapping
        v
explicit Moira AdmitMessage
        |
        v
stable Chronos submission intent
```

The durable Moira adapter resolves an event retained by its Agora checkpoint and
source/room catalogue. The fact contains the event sequence, an opaque Agora
message reference, the author ID, and canonical addressed IDs. It contains no
message body or participant kind, does not acknowledge the event, does not mutate
Moira, and does not construct a Moira command.

The host decides whether to admit the fact and supplies an opaque Moira
participant reference for one intended recipient. A matching retry returns the
original workflow and submission identity. Reusing the message reference with a
different participant mapping conflicts without changing state.

## Recovery obligations

The pending fact survives adapter restart and remains derivable from the outbox
after room retention evicts the corresponding message body. After explicit
admission, a lost SQLite reply can be replayed without creating another workflow.
Response publication uses a separate keyed boundary: a matching retry returns the
original Agora publication without another message.

These boundaries are independent durable operations, not a distributed
transaction. A host must durably account for its admission decision before it
acknowledges the Agora event. The profile supplies no automatic event pump,
authorization adapter, harness, retry scheduler, or network transport.
