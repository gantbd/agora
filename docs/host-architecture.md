# Authenticated host architecture

Status: qualified transport-neutral host profile V1.

`agora-host` is a composition boundary over exact `agora-wire` frames and one
serialized `agora-service`. It performs no socket I/O and understands no
password, token, certificate, OAuth, or Aegis grant format.

## Trusted verifier port

The injected `PrincipalVerifier` is trusted to establish two facts:

1. at Connect, the exact participant-ID claim is authenticated for the supplied
   transport/credential context at the supplied host time;
2. before every later frame, the current context is still bound to the retained
   opaque subject and participant and has not expired or been revoked.

Host time is an explicit trusted input. Each session retains its last successful
verification time; regression retires the session before calling the verifier,
so an expired credential cannot be revived by replaying an older time.

The verifier returns only `VerifiedSubjectV1`, an opaque attribution value. Its
error vocabulary is erased at the host boundary. Authentication failures produce
the same `ConnectRefused(Unavailable)` frame regardless of participant existence;
later unknown, revoked, mismatched, or substituted sessions all return the same
`HostRefusal::Unavailable` class.

A credential adapter may implement the port directly. An Aegis adapter may
validate a grant and map current `DecisionEvidenceV1` to a verified subject. The
adapter remains responsible for mapping Agora operations to Aegis resource/action
vocabulary; Agora neither imports Aegis types nor defines grants.

## Session and client binding

A successful Connect creates two distinct identities:

- the service allocates a sequential `ClientIdV1` used in protocol frames;
- the host allocates an opaque `HostSessionIdV1` returned only to the trusted
  transport adapter.

The transport must bind the host session to its connection state and must not
accept a session ID from the wire payload. On Execute or Disconnect, the host
revalidates the current context and requires the frame's client ID to equal the
service client retained for that host session. Substitution retires the offending
session without affecting the target client.

Matching Execute retries reach the service's exact request/reply receipt and do
not publish another domain transition. A response-encoding failure after service
acceptance is uncertain delivery, not rollback; retrying the identical request
observes the retained reply.

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

## Deliberate exclusions

The host is not a network server, credential verifier, authorization-policy
engine, secret store, durable session store, rate limiter, TLS terminator, or
audit sink. A production composition root must supply those effects and define
restart, transport replay, credential rotation, and operational abuse policy.

