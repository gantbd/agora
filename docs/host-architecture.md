# Authenticated host architecture

Status: qualified transport-neutral host profiles V1 and V2 plus canonical,
store-neutral authenticated V2 recovery, one local Unix store composition, and
one separate Aegis public-service verifier adapter composed with a bounded
private local Unix transport.

`agora-host` is a composition boundary over exact `agora-wire` frames and one
serialized `agora-service`. It performs no socket I/O and understands no
password, token, certificate, OAuth, or Aegis grant format.

`Host` binds frozen Wire/Service V1. `v2::HostV2` binds Wire/Service V2 through a
distinct `HostSessionIdV2`. Both use the same verifier, explicit time, limits,
generic refusal, usage, revocation, and cleanup contracts. The verifier evidence
is transport and credential attribution, not a wire-version-specific value;
V2's reuse of `VerifiedSubjectV1` and `HostTimeV1` does not make V1 frames valid
for a V2 host.

## Trusted verifier port

The injected `PrincipalVerifier` is trusted to establish two facts:

1. at Connect, the exact participant-ID claim is authenticated for the supplied
   transport/credential context at the supplied host time;
2. before every later frame, the current context is still bound to the retained
   opaque subject and participant and has not expired or been revoked.

Host time is an explicit trusted input. Each session retains its last successful
verification time; regression retires the session before calling the verifier,
so an expired credential cannot be revived by replaying an older time.

For the additive admission-control profile, Host V2 also derives engine
admission-time evidence from that same accepted `HostTimeV1` and supplies it
directly to Service V2. No enforcement timestamp is accepted from Wire V2.
Only message commands need the evidence, and only an enabled room interval
consumes it; exact service replay resolves before a later host time can alter a
retained reply. This is immediate admission enforcement, not scheduling, a
rate-limiter service, or a Chronos task.

The verifier returns only `VerifiedSubjectV1`, an opaque attribution value. Its
error vocabulary is erased at the host boundary. Authentication failures produce
the same `ConnectRefused(Unavailable)` frame regardless of participant existence;
later unknown, revoked, mismatched, or substituted sessions all return the same
`HostRefusal::Unavailable` class.

The qualified `agora-aegis-verifier` adapter maps host admission/revalidation to
`Submit` on one configured Aegis `Agora` resource through an exact public service
session and opaque grant handle. It binds the claimed participant and retained
opaque subject, validates every frame at explicit host time, and erases all
authority failures. This is service-session admission only; Agora roles still
authorize room operations. Credential authentication, context provenance, grant
issuance, and transport remain composition responsibilities.

The separate `agora-local-daemon` composition supplies one such transport
binding for V2. Its trusted connection authenticator creates the Aegis context;
the daemon retains the returned host session outside payloads, passes that same
context plus fresh explicit time on every bounded frame, and closes the session
on every terminal path. This qualifies the binding mechanism, not the
authenticator's credential-to-context provenance.

## Session and client binding

A successful Connect creates two distinct identities:

- the service allocates a sequential `ClientIdV1` used in protocol frames;
- the host allocates an opaque `HostSessionIdV1` returned only to the trusted
  transport adapter.

The V2 profile creates the analogous separately typed `ClientIdV2` and
`HostSessionIdV2`; identities cannot be substituted across versions.

The transport must bind the host session to its connection state and must not
accept a session ID from the wire payload. On Execute or Disconnect, the host
revalidates the current context and requires the frame's client ID to equal the
service client retained for that host session. Substitution retires the offending
session without affecting the target client.

Matching Execute retries reach the service's exact request/reply receipt and do
not publish another domain transition. A response-encoding failure after service
acceptance is uncertain delivery, not rollback; retrying the identical request
observes the retained reply.

The additive `SearchHostV1` wrapper binds separately framed Search Wire V1 to
the same authenticated V2 session and exact service client. Its volatile search
runtime borrows the Host's one authoritative Service V2 rather than owning a
copy. Revalidation precedes decoding and index access; client substitution
retires the session without disclosing the target. Session close, revocation,
and every accepted conversation revision change retire affected search receipts
and adapter continuations. Search replies remain identity-only derived
observations and the wrapper has no checkpoint profile.
Private local handshake profile 3 selects this wrapper explicitly, admits only
conversation/search/control routes, and emits the same server-only
connection/incarnation-bound replay-epoch control hello used by the client
uncertainty fence. It does not reinterpret collaboration profile 2.
When a Host checkpoint is recovered, callers construct a fresh search runtime.
No search receipt or service cursor survives; old continuations refuse, while a
fresh request may run after current authentication and authority checks. Taking
the wrapper apart retires every live adapter continuation before returning the
port.

For the durable V2 composition, `AGORAHST` checkpoints the opaque verified
subject, participant, host session, service client, last verification time,
retired-session window and full service state as one scope-bound canonical unit.
Recovery refuses orphaned or substituted bindings and receives a fresh verifier;
the retained session cannot act until current context is revalidated again.
`DurableHostV2` publishes any changed host result only after a durable-strength
receipt for that combined record.

The separate `agora-storage-directory` package can supply that receipt for the
qualified single-owner local Unix profile. End-to-end restart evidence closes
and reopens its real filesystem record, recovers this combined host checkpoint,
and proves exact replay adds neither another engine event nor store offer.

## Revocation and cleanup

Failed revalidation retires the host session and disconnects its service client
before returning. Transport loss uses `Host::close` without authentication so
cleanup remains possible after credentials disappear. Host and service retirement
windows evict their oldest tombstones under pressure; tombstone capacity never
blocks revocation or disconnect. Session and client identities are never reused
within a host/service lifetime.

All active sessions, retired tombstones, frames, fields, text, lists, service
clients, and request receipts have explicit ceilings. Malformed frames and host
session capacity refuse before the verifier is invoked. Diagnostics expose only
opaque identities and aggregate counts.

## Qualified private local transport

The Unix-only daemon requires a real mode-0700 parent, binds a previously absent
path, applies and verifies socket mode 0600, pins the socket's device/inode
identity, and removes it only while that identity remains unchanged. An
endpoint-scoped mode-0600 record binds the declared incarnation under an
exclusive lock; parent, record, and socket replacement fence readiness. Four-byte
big-endian outer lengths are checked before allocation; zero, overflow,
truncation, deadline expiry, and response
overflow terminate the connection. Fixed-width handshake profile 1 selects
conversation Wire V2; additive profile 2 selects Wire V2 plus transient Wire V1
with closed conversation/transient/control routing. Both report limits and
incarnation before authentication. The first framed payload must carry a Wire
V2 Connect, and a fixed positive budget bounds later payloads. The collaboration
profile emits a server-only post-connect replay epoch bound to the declared
incarnation and exact authenticated connection.

Serving is deliberately sequential. Content-free counters distinguish clean
completion, authentication/host refusal, and transport failure. The profile
trusts same-UID control of the private parent, the authenticator, clock, and Aegis
port. It supplies neither peer-credential extraction nor endpoint lifecycle
automation, fresh incarnation generation, compatibility-range negotiation,
concurrent admission, in-flight graceful drain, signal handling, or durable Host
V2 reconnect recovery. The daemon can instead be constructed over the existing
receipt-gated Durable Host and checkpoint-store port. The combined durable
collaboration owner stores only the Host checkpoint and reconstructs presence
and typing empty; suppression retires volatile sources before fencing or
returning control. Qualification proves clean
owner teardown/reopen and retained state through a fresh authenticated
connection, not old-session resumption or lost-reply replay. A specialized
constructor composes the caller-recovered combined owner with exact
prior-incarnation-checked endpoint-residue rotation; it remains explicit, not
automatic restart. See
[`local-daemon.md`](local-daemon.md).

## Deliberate exclusions

The host itself is not a network server, credential verifier, authorization-policy
engine, secret store, general rate-limiter service, TLS terminator, or audit sink. A production
composition root may pair it with the qualified directory and local transport
adapters, but must still define transport uncertainty, credential rotation,
backup/restore, power-loss expectations, operational lifecycle, and abuse policy.

