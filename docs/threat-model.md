# Threat and privacy model

Status: initial STRIDE/LINDDUN analysis for the v0.3 core boundary.

| Concern | Core treatment | External obligation |
| --- | --- | --- |
| Spoofing | IDs grant no authentication; room authority is checked against explicit actor IDs | host binds authenticated principals to IDs |
| Tampering | optimistic revisions, canonical checkpoints, and typed validation refuse malformed state | storage authenticates origin and protects bytes |
| Repudiation | ordered volatile events preserve attribution while retained | durable audit and non-repudiation are not claimed |
| Information disclosure | message/publication debug output redacts content and keys | transport, logs, and storage protect content |
| Denial of service | retained state, bytes, fan-out, and outbox are bounded with backpressure | host bounds connections, request rate, and CPU |
| Elevation of privilege | participant kind grants no authority; addressing grants no execution | Aegis/host authority guards cross-engine effects |
| Linkability/identifiability | stable IDs are explicit domain data | pseudonym policy and cross-room correlation belong to host policy |
| Detectability | room membership and events reveal activity to authorized observers | presence and delivery privacy require a higher-layer policy |
| Unawareness/intervenability | no consent or notification UX exists in the core | clients communicate policy and provide controls |
| Non-compliance | retention is bounded but deletion/export policy is absent | deployment supplies policy, jurisdiction, and audit controls |

The volatile service narrows impersonation by deriving message authors from a
server-owned connection binding and narrows room disclosure by requiring
membership for snapshots. It does not authenticate the participant supplied to
`connect`. `ClientIdV1` is sequential and forgeable by design; a transport host
must never expose it as a bearer credential or accept it without a separately
authenticated connection binding. Retained request replies may contain message
content and therefore inherit the deployment's memory-protection obligations.

The wire decoder treats every byte as hostile: total bytes, field bytes, field
count, and list counts are bounded before owned reconstruction; unknown grammar
and trailing data refuse. This limits parser resource amplification but does not
authenticate frames, hide metadata/content, prevent replay, establish stream
boundaries, or protect transport availability. In particular, Connect's
participant ID remains attacker-controlled until the host verifies it.

The authenticated host prevents a wire client ID from becoming ambient authority:
each frame is coupled to an opaque transport-bound host session, a retained
verified subject/participant, current verifier acceptance, and the exact service
client. Authentication failures erase verifier detail and do not query participant
registration. Client substitution and revocation retire only the caller's session.
Per-session time regression also retires before verifier invocation, fencing
expiry rollback.
This treatment still trusts the verifier and transport binding; a verifier that
accepts forged context, or an adapter that accepts host session IDs from payloads,
reintroduces spoofing and confused-deputy risk.

The separately typed V2 host applies the same authentication, monotone-time,
dual-binding, non-enumeration, revocation, and cleanup treatment to V2 frames and
clients. A V1 frame is rejected before verifier invocation. This closes no new
credential, authorization, confidentiality, durable-session, or network threat;
those external obligations remain unchanged.

Addressing a participant is neither authority nor execution evidence. An adapter
that turns addressing directly into model or tool execution violates this model.
The qualified local Moira receiver therefore exposes only a passive pending
message fact. Host policy must separately map an Agora participant to an opaque
Moira participant reference and explicitly admit the message before workflow
state exists. Acknowledgment follows durable accounting for that decision.

The SDK crates are reexport-only distribution facades. The trusted-host facade
does not bundle a verifier, transport, storage adapter, or ambient credential;
the remote-client facade cannot reach engine, service, or host internals. A
consumer that treats either facade name as authority bypasses the documented
boundary and must not be considered authenticated.

