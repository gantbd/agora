# Aegis verifier adapter

Status: qualified transport-neutral adapter profile V1.

`agora-aegis-verifier` is a leaf composition adapter between Agora's existing
`PrincipalVerifier` port and Aegis's public client/protocol service contract. It
does not add Aegis types to Agora engine, protocol, service, wire, host, or SDK
semantics.

One adapter instance selects one opaque 128-bit Agora service resource. Each
trusted context binds:

- the exact claimed Agora participant ID;
- one opaque `VerifiedSubjectV1` established by the composition root;
- one Aegis public service session handle; and
- one session-local opaque Aegis grant handle.

Connect and every later host frame request Aegis authorization for `Submit` on
that resource in the `Agora` namespace at the explicit host time. Connect first
compares the wire participant claim with the bound participant. Revalidation
also compares the host's retained opaque subject. Mismatch is rejected before
the Aegis port is called.

Only an unexpired `Authorized` outcome carrying the expected namespace and
action succeeds. Refusals, transport/port errors, expired or structurally wrong
accepted outcomes, participant mismatch, and subject mismatch are all erased to
the same unavailable verifier error. Agora Host consequently emits its existing
non-enumerating unavailable projection and retires the host/service session on
failed revalidation.

The Aegis grant admits use of an Agora host session. It does not grant room
membership, conversation visibility, speaking, moderation, delivery, task, or
agent-execution authority; Agora still evaluates every room operation from the
participant bound to the service client.

The composition root remains responsible for authenticating credentials,
creating the Aegis service session, obtaining the grant, truthfully binding the
Aegis principal to the opaque Agora subject and participant, selecting the
resource, supplying time, and choosing a transport. Agora and Aegis checkpoint
commits are not made atomic by this adapter.
