# ADR: native Agora-to-Agora federation

Status: accepted; contract foundation open

Date: 2026-11-14

## Context

R106 federates Agora with Agora before any foreign chat connector. Federation
must preserve Agora's participant-bound authority, ordered room history,
bounded replay, explicit gaps, and human/agent symmetry across independently
operated servers. It must not turn TLS certificate names into participants,
make a replica authoritative, or hide partition uncertainty behind retries.

The R105 remote transport is a bounded authenticated byte path. It does not own
server trust, room placement, replicated history, remote admission, or peer
recovery. Those are federation policy and protocol concerns and therefore need
a separate project and qualification profile.

## Decision

R106 is implemented in a separately owned `agora-federation` project. Agora
keeps the authoritative room state machine; the federation project owns peer
trust composition, native federation protocol, replica journals, subscriptions,
and reconciliation. The remote transport remains authority-free.

### Server identity and trust

Each deployment has one stable opaque 32-byte `FederationServerIdV1`. A
configured trust verifier binds an authenticated channel to exactly one server
ID and one current trust epoch. Certificate identity and transport exporters are
inputs to that verifier, never federation authority by themselves. Trust roots,
enrollment, key rotation, revocation distribution, and organization policy are
separately qualified provider responsibilities.

Every federation session binds both verified server IDs, both nonzero server
incarnations, the transport exporter, and the negotiated protocol profile.
Reconnect creates a new session. Trust revocation fences new admission and
retires the affected session without rewriting already accepted room facts.

### Home-scoped identity and authority

Participant, room, message, and event references are structured pairs of a
home server ID and a bounded home-local identity. They are never concatenated
display names, URLs, DNS names, certificate subjects, or content digests.

Every room has exactly one authoritative home server in the initial profile.
Only that home allocates room event sequence, evaluates current membership and
moderation, accepts mutations, and emits authoritative room events. A remote
server stores a bounded projection and may submit an authenticated participant-
bound admission request to the home; it cannot accept the mutation locally.
Home relocation, multi-writer rooms, event DAGs, conflict merging, and offline
local acceptance are absent.

An imported event is never exported as a new authoritative event. The initial
topology permits direct home-to-subscriber replication only: the authenticated
sender of an authoritative room event must be the room home, and a receiver
must not relay it. This source rule, structured home identity, and duplicate
event identity close loops without hop-count folklore.

### Capability and limit negotiation

After peer trust, the parties exchange canonical closed ranges for the
federation protocol, room-event schema, snapshot schema, and admission schema,
plus positive ceilings for frames, retained bytes, in-flight requests,
subscriptions, replay pages, and snapshot size. Selection uses the highest
common exact profile; operational ceilings are the bounded minimum. A disjoint
range or unknown required capability refuses before room state is exposed.
Older bytes are never reinterpreted by a later profile.

### Replication, replay, and gaps

The home exposes a per-room monotone event sequence. A replica journal binds
the room key, home server, home incarnation, schema profile, next expected
sequence, and exact canonical event bytes. Matching replay is idempotent;
changed bytes under an existing event identity are a conflict and fence that
replica. An event beyond the next sequence is an explicit gap, not implicit
loss or permission to skip.

Bounded replay may either continue from the exact next sequence or return an
explicit retained-history gap. Gap repair uses a canonical bounded snapshot
whose authority, room identity, base sequence, and current continuation are
verified before replacing a replica projection. A snapshot is not a transfer
of room ownership. Replica acknowledgements advance only federation delivery
progress; they are never participant reads, attention, delivery to every
participant, or mutation acceptance.

### Remote admission and uncertainty

A remote admission request binds the verified submitting server, one local
authenticated participant mapping, the authoritative room key, a scoped
nonzero request identity, and one canonical Agora intent. The home revalidates
the current participant mapping and ordinary room authority at execution time.
Human and agent participants traverse the identical path.

The home retains bounded exact receipts. Matching retry returns the original
result; changed input conflicts. Transport outcomes remain `DefinitelyNotSent`
or `OutcomeUnknown`. An unknown result is reconciled with the exact request
identity or authoritative room observation before retry; reconnect, peer
acknowledgement, or replica appearance alone does not manufacture success.

Agora current-policy composition must use a federation-scoped publication
identity. The engine receipt key includes a closed source domain; Chronos,
typed-interaction, federation, and legacy identities with equal 16-byte values
are distinct. New scheduling and interaction receipts use their own domains.
Engine checkpoint profile 149 preserves the domain tag canonically and rejects
unknown tags. A scoped request may replay an equal pre-domain legacy receipt so
upgrade does not duplicate an already accepted message; that compatibility
path does not let one new source claim another new source's receipt.

Agora exposes this capability through the zero-dependency
`agora-federated-publication-port`. The port carries only opaque correlation
plus a local room/body intent. It carries no peer trust, home identity mapping,
credential, clock, transport, retry, or room-home authority. Service binds the
currently connected participant and the reserved federation receipt domain;
Host revalidates the authenticated session and supplies current time; Durable
Host commits the complete resulting Host checkpoint before the semantic reply
escapes. An exact federation receipt replay retains the first accepted Host
time while continuing to require equal room, actor, and body. Legacy migration
and every non-federation source remain time-exact.

### Partitions and revocation

During a partition, replicas may serve only explicitly qualified stale reads
with their last authoritative sequence and gap state. They refuse room
mutations and do not fabricate presence, read state, moderation success, or
membership freshness. Recovery resumes from the exact journal frontier or a
verified snapshot.

Peer trust revocation stops peer sessions. Participant or room revocation is an
authoritative home event and follows the ordinary ordered stream. Replica-side
deletion, retention, and trust removal do not claim that the home erased data.

## Required evidence

- closed canonical server, scoped-identity, capability, request, event,
  acknowledgement, gap, and snapshot values with immutable fixtures;
- trust binding, incarnation/exporter binding, revocation, and reconnect cuts;
- single-home allocation and refusal of non-home mutation or event emission;
- identical human/agent remote admission and current home-policy enforcement;
- exact retry, changed-input conflict, definitely-not-sent, unknown outcome,
  and observation/replay reconciliation;
- source-domain collision tests, exact legacy replay, canonical profile-149
  recovery, and unknown-domain refusal;
- effect-free federated-publication port, current Service policy, authenticated
  Host binding, durable commit gating, later-time exact replay, and revocation;
- ordered replication, duplicate replay, changed-event conflict, explicit gap,
  bounded replay, snapshot repair, and restart recovery;
- direct-home source enforcement, imported-event non-export, and adversarial
  loop/substitution tests;
- acknowledgement/read-state separation and content-safe diagnostics;
- dependency, package, supply, compatibility, corruption, soak, platform, and
  produced-artifact qualification.

## Nonclaims

R106 does not provide global usernames, discovery, DNS trust, automatic trust,
certificate issuance, participant credential issuance, organization tenancy,
multi-home rooms, multi-writer acceptance, event DAG resolution, home transfer,
transparent offline writes, end-to-end encryption, presence federation, read
receipt federation, task or agent execution, workflow, guaranteed delivery,
legal retention, erasure, or foreign-network interoperability. It does not make
a peer acknowledgement into a participant read or a replica into a room home.

