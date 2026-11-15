# ADR: remote transport profile

Status: accepted; transport and single-connection application foundations qualified

Date: 2026-11-14

## Context

Agora's qualified private Unix profile proves framing, bounded requests,
incarnation, cleanup, and sequential idle drain, but it has no network peer
provenance, hostile-network confidentiality, concurrent admission, in-flight
drain, or rolling-upgrade contract. Reusing the local socket assumptions over
TCP would confuse encrypted transport with participant authentication and would
leave overload, replay, and lifecycle behavior undefined.

## Decision

R105 introduces a separate `agora-remote-transport` package. It owns TCP/TLS
resources, certificate verification, connection concurrency, exact outer-frame
I/O, deadlines, queue budgets, TLS channel binding, and concurrent drain. It
must not import Agora engine, Service, Host, Aegis policy, participant, room,
message, task, delivery, or federation semantics.

The initial profile is TLS 1.3 only with an exact Agora ALPN identifier. TLS
early data is disabled. Server certificates are verified against caller-
configured trust anchors and an exact DNS name; service profiles may additionally
require a client certificate against a separately configured trust store. A
verified certificate identifies the remote transport peer only. It is never an
Agora participant, session, grant, room member, or federation authority.
Application authentication and Aegis authorization remain above this boundary.

After TLS, peers exchange a canonical remote hello before authentication or
Host work. The hello negotiates one exact compatible protocol profile from
closed supported ranges, positive frame/request/connection budgets, capability
discovery, and a nonzero server incarnation. The accepted profile and a TLS
exporter-derived channel binding are fixed for the connection. Unknown versions,
ALPN mismatch, malformed limits, identity failure, and incomplete handshakes
fail before downstream work.

The first profile disables session tickets and reconnection resumption. Every
TCP/TLS connection receives a fresh application session and replay epoch under
the current server incarnation. A disconnected request with an unknown reply
is reconciled only through the ordinary request identity after fresh
authentication; transport reconnection is not semantic replay evidence.

All retained bytes and work are bounded: accepted connections, concurrent
handshakes, per-connection inbound and outbound frames, frame bytes, request
count, and pending writes. Capacity refusal occurs before application admission.
The transport never drops an accepted application reply to relieve pressure;
it backpressures or closes with an explicit transport outcome. Slow or partial
peers are bounded by handshake, read, write, idle, and drain deadlines.

Drain is an irreversible Ready-to-Draining transition. It stops new accepts,
refuses new application requests on admitted connections, allows already
admitted replies to finish within a fixed deadline, then closes remaining
connections and returns exact active/admitted/forced counts. A TLS, ownership,
budget, or lifecycle fence cannot be relabeled draining. Listener handoff,
socket activation, and zero-loss process migration are later deployment
profiles.

Upgrade compatibility is range intersection over closed protocol profiles, not
feature guessing. The server selects one exact profile and returns it in the
canonical hello. Additive profiles never reinterpret prior bytes. Deployments
must prove old/new client and server combinations, drain during rollout,
incarnation rotation, and refusal of unsupported intersections.

## Required evidence

- real-loopback TLS 1.3 server authentication, required-client-certificate
  service profile, wrong-name/root/client/ALPN/version refusal, and exporter
  agreement;
- canonical hello fixtures, mutation/fuzz closure, range negotiation, distinct
  incarnation, and SDK reachability;
- bounded handshake and connection concurrency, frame/request/write budgets,
  slow-reader/writer behavior, and pre-application overload refusal;
- disconnect/reconnect and unknown-reply reconciliation without session or
  authority resurrection;
- concurrent drain schedules covering accepts, handshakes, admitted requests,
  pending replies, deadline expiry, forced close, and sticky fences;
- dependency/effect closure, exact package/supply/SBOM inventory, platform
  matrix, deterministic soak, and commit-pinned produced-artifact qualification.

## Implementation status

Agora Wire now provides the canonical closed-range post-TLS client/server hello,
two frozen fixtures, fail-closed negotiation tests, and client-SDK reachability.
The separate `agora-remote-transport` source revision `4cf7d0a` provides a
TLS-1.3-only rustls profile with exact ALPN, disabled tickets/resumption/early
data, configured server roots, optional required client certificates, exporter
agreement, bounded framing, actual concurrent listener ownership, request
permits, clean and deadline-forced drain, graceful close notification, real-
loopback positive and negative tests, pre-application overload refusal, distinct
reconnect exporter bindings, compatible/disjoint real-TLS upgrade cuts, and a
blocked bounded-writer drain cut with exact pending-reply accounting. Its fixed
256-cycle TLS/mTLS reconnect and clean-drain soak requires distinct exporter
bindings and zero refusal, force, or fidelity-loss counts. Two independently
built 153-file self-contained source candidates are byte-identical; one passes
the complete extracted-source locked/offline Rust surface. The manifest digest
is `c04b3263c8bfd512e5624e132b3db0e759e5b20118d22a885965d2ca25fa9f2a`
and inventory digest is
`c95c6f1ab812e20bb164308ab69aa9993e84bcbf368faa51da5ec765fc6ae167`.
Exact compatibility, supply, package, ordinary, soak, and source-candidate gates
pass; revision `9502788` pins them.

The separately owned `agora-remote-daemon` source revision `5b32b4b` composes
negotiated connections with bounded opaque credential frames, TLS exporters and
certificate provenance, an injected authenticator, Aegis contexts, and a shared
Host V2 owner. Every Host open/request and reply write holds one transport
permit, while network I/O never holds the Host lock. It closes sessions on every
exit and forces overlapping human and agent participants through the identical
path. Revision `b91d23e` pins the passing real TLS/mTLS gate.

A concrete credential authenticator, broader disconnect uncertainty and multi-
writer schedules, platform matrix, binary artifacts, signatures/installers, and
commit-pinned full-profile qualification remain open.

## Nonclaims

R105 does not provide participant identity, OAuth/password/device enrollment,
certificate issuance or revocation service, Aegis grants, end-to-end encryption,
traffic-analysis resistance, federation trust, multi-region routing, proxy or
load-balancer configuration, QUIC, WebSocket/browser transport, session
resumption, zero-loss process handoff, or semantic exactly-once delivery.
