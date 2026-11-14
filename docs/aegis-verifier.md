# Aegis authorization adapters

Status: qualified transport-neutral adapter profile V1.

`agora-aegis-verifier` composes Agora's `PrincipalVerifier` and exceptional
operator-restore authority ports with Aegis's public client/protocol service
contract. It does not add Aegis types to Agora engine, protocol, service, wire,
host, or SDK semantics.

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

Agora's passive room-action vocabulary includes `ConfigureAdmission`,
`RestrictPosting`, `RestorePosting`, `Report`, and `ReviewReport` so a
composition may correlate policy evidence without creating another mutation
path. This adapter continues to authorize only session-level `Submit`; an Aegis
decision does not set an interval, restrict a participant, create or review a
report, or establish classifier truth. The authenticated Agora transition
derives its actor and independently enforces room authority and invariants.

The separate restore adapter prebinds one Agora durable store key, Host
checkpoint scope, and opaque 128-bit Aegis resource. For each validated loaded
checkpoint it requests `Administer` in the `Agora` namespace at explicit trusted
time. A key/scope mismatch refuses before Aegis I/O; refusal, port failure,
expiry, wrong namespace, and wrong action are one unavailable class. Success
retains the Aegis decision summary and loaded Agora revision as copyable
correlation evidence. The caller can inspect or consume that evidence after
Agora has committed the sanitized exact-next-revision checkpoint.

The composition root remains responsible for authenticating credentials,
creating the Aegis service session, obtaining the grant, truthfully binding the
Aegis principal to the opaque Agora subject and participant, selecting the
resource, supplying time, and choosing a transport. The decision/revision pair
does not prove the later Agora CAS occurred, and Agora and Aegis checkpoint
commits are not made atomic by this adapter.
