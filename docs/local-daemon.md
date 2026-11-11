# Private local daemon profile

Status: qualified bounded Unix transport composition V1 with separately selected
volatile conversation, volatile collaboration, receipt-gated durable
conversation, or receipt-gated durable conversation plus volatile collaboration.

`agora-local-daemon` owns only the outer connection and framing lifetime around
an Aegis-authorized volatile, transient-wrapped, durable, or combined
durable-conversation/volatile-collaboration Host owner. Its normal dependency
closure reaches only the verifier adapter, Host/Durable owners, Wire, and the
generic Keystone checkpoint-store port; the physical directory store used by
qualification remains test-only. It does not enter Agora's engine, protocol,
service, storage implementation, Aegis policy, or SDK semantics.

## Composition

The application supplies:

- either a `HostV2<AegisVerifierV1<P>>`, a
  `TransientHostV1<AegisVerifierV1<P>>`, Agora's
  `DurableHostV2<AegisVerifierV1<P>, S>`, or
  `DurableTransientHostV1<AegisVerifierV1<P>, S>` with its engine, Service V2 limits,
  Wire V2 limits, Aegis service port, fixed Agora resource, and—only for the
  durable selection—an independently qualified checkpoint store;
- an `AuthenticatorV1` that derives one trusted `AegisContextV1` from the
  accepted Unix connection;
- a `HostClockV1` that supplies explicit monotone host time; and
- a nonzero opaque 128-bit value naming the declared logical server
  incarnation;
- positive frame, request, and I/O deadline limits.

The daemon requires a real existing mode-0700 parent directory, creates a new
Unix-domain socket within it, hardens the socket to mode 0600, validates its
type, and pins its device/inode identity. A symlink or permissive parent and an
existing regular file, symlink, socket, or other endpoint object refuse
construction. Before binding, it creates an endpoint-derived mode-0600 lifetime
record containing the declared incarnation, holds an exclusive kernel lock, and
pins the parent and record identities. A live lock and an unlocked prior record
refuse distinctly. Drop removes the socket and record only when every pinned
resource still matches; replacement or ambiguity is preserved.

Crash residue is never repaired by ordinary construction. The separate
`recover_stale` path requires the exact recorded prior incarnation, a distinct
nonzero next incarnation, and a newly supplied Host composition. It acquires the
old record lock, validates the exact record and stale socket, removes only that
socket, rotates the locked record, and then uses the ordinary bind path. Live,
foreign, malformed, missing, or unsafe residue refuses without adoption.

## Connection state machine

1. Revalidate the parent, lifetime record, and socket, then accept one connection.
2. Revalidate ownership again and apply the configured read/write deadline.
3. Read the exact fixed-width handshake-profile-1 ClientHello selecting Wire V2.
4. Return the fixed-width ServerHello with the selected profiles, configured
   frame and request limits, capability-discovery support, and server incarnation.
5. Derive the trusted Aegis context from that connection.
6. Read one bounded length-prefixed canonical Wire V2 Connect frame.
7. Return the Host V2 connected frame or its shared unavailable refusal.
8. Keep the opaque host session only in daemon memory.
9. For at most the configured number of later frames, supply fresh host time and
   the same connection context to Host V2. Host V2 and the Aegis adapter bind and
   reauthorize the retained subject and participant before Service V2 executes.
10. On EOF or any later error, close the host session and service client
   unconditionally.

The separately selected collaboration state machine uses fixed-width handshake
profile 2, selecting conversation Wire V2 and transient Wire V1. Every outer
payload is routed by one included byte: conversation, transient, or control.
Connect must be conversation-routed. After successful authenticated connect,
the server sends the connected reply and then a server-only `AGORATE1` control
hello. Its opaque nonzero replay epoch combines the declared 16-byte server
incarnation and a monotone 16-byte per-daemon connection sequence. Later
conversation and transient requests share the exact Host session and request
budget; client control frames refuse. Epoch exhaustion fences readiness.

Each outer record is `u32_be length || frame`. Length zero and values above the
configured ceiling refuse before payload allocation. Truncated prefixes and
payloads, deadline expiry, response overflow, Host refusal, and request-budget
exhaustion terminate the connection. The final in-budget request is handled and
replied to before budget exhaustion closes the session.

The daemon serves a single connection per `serve_next` call and performs no
automatic retry. A lost reply is not converted into success or transparently
replayed. In particular, disconnect reclaims the volatile service receipt, so a
caller must treat an unresolved mutation as outcome-unknown rather than assume a
new connection can safely repeat it.

## Observable operations

`DaemonHealthV1` is deliberately content-free. It reports readiness, current
drain and ownership-fenced state, connections, accepted and refused handshakes, cleanly completed
connections, refused connections, transport errors, and counter saturation. It
exposes no incarnation bytes, participant, subject, grant, room, message, or
frame data.

`DaemonObservationV1` is the read-only composition view. It retains distinct
transport, Host V2, and Service V2 counter groups so a socket acceptance cannot
be mistaken for authentication, host execution, or domain acceptance. A sticky
`fidelity_lost` bit is the disjunction of every represented saturation signal.
Reading the view performs no I/O or authority call, changes no counter, and
cannot change a request result. The snapshot contains no identifiers, content,
credentials, frame bytes, timings, or refusal detail; it is process-local and
excluded from semantic recovery.

`begin_drain` is an irreversible, idempotent idle transition. It clears
readiness and sets the drain fact; every later `serve_next` refuses before
accepting a socket or invoking authentication, authority, Host V2, or Service
V2. The endpoint remains pinned and owned until normal drop. Since the method
requires exclusive mutable access to this sequential owner, it cannot overlap an
in-flight connection and is not a concurrent graceful-drain mechanism.

## Trust boundary and non-claims

The mode-0700 socket parent and same-UID processes are trusted. The
authenticator is trusted to bind peer/credential evidence to the exact Aegis
session, grant, participant, and opaque host subject; this crate does not define
that credential mechanism. The clock is trusted and the Aegis service port must
provide its own availability and context provenance.

The handshake selects one exact protocol profile; it is not version-range or
feature negotiation. Its caller-supplied incarnation is not authenticated; the
lifetime record binds that value to one exclusive live endpoint but does not
prove fresh generation or a restart boundary. The profile makes no claim for
TCP or other network transports, confidentiality, peer-credential extraction,
automatic crash restart, concurrent clients, overload queues, concurrent
in-flight drain, signal handling, socket activation, installation,
cross-restart request identity, reconnect resumption, or atomic Aegis/Agora
recovery.
The collaboration selection binds each replay epoch to one declared server
incarnation and authenticated connection, but the epoch is correlation evidence,
not a credential or authority token. The durable selection consumes Agora's existing receipt-gated Host boundary and
generic checkpoint-store port; the transport neither defines storage nor weakens
its publication fence. A real-socket directory-store scenario proves clean owner
teardown/reopen and a fresh authenticated connection observe retained conversation
state under a distinct server incarnation. It does not preserve the old transport
session or make a disconnected pending request replayable: terminal connection
cleanup still closes the service client and reclaims its receipt namespace. The
qualified idle drain fence does not add concurrent in-flight drain or signal
integration. The combined owner checkpoints only durable Host state and always
reconstructs transient state empty.

## Qualification evidence

The twelve real-socket scenarios verify private-parent/socket modes and type,
canonical incarnation-bound lifetime bytes, live-owner exclusion, distinct
stale-residue refusal, replacement fencing before authentication, and
conservative teardown,
the exact ServerHello fields, refusal of an incompatible ClientHello before any
authentication, authority, Host, or Service work, Aegis-authorized Connect and
per-frame execution, byte-identical request replay, EOF cleanup, erased
authenticator error detail, participant mismatch without authority I/O, bounded
request exhaustion cleanup, oversized length refusal before host admission,
unsafe/existing-path refusal, and preservation of a replacement endpoint.
Dependency and effect gates keep the adapter limited to the Aegis verifier,
Host V2, Wire V2, and Unix filesystem/socket effects. The successful path
additionally verifies exact layer-separated observation, repeated-read
passivity, content-safe diagnostics, and sticky counter saturation without
wraparound. A separate real-endpoint scenario verifies the idle drain
transition, zero downstream work after its fence, endpoint retention, and
pinned teardown. The durable scenario composes `DurableHostV2` with the physical private
directory store, commits a room through the socket, tears down both owners,
reopens them, publishes a distinct server incarnation and reads the retained
room through a fresh authenticated connection.

The collaboration scenario multiplexes a conversation mutation with presence
and typing publication, proves EOF retirement through `Unknown`/inactive
observation on a second connection, observes distinct per-connection epochs,
and reconstructs empty transient owners under a distinct restart incarnation.
The combined durable-collaboration scenario repeats that boundary through the
physical directory store: the room survives reconstruction while presence and
typing do not.

The recovery scenario separately proves live-owner exclusion; equal, wrong, and
malformed prior-incarnation refusal without mutation; exact locked rotation to a
new record; a fresh Host with zero prior offers; next-incarnation handshake; and
clean teardown. It does not exercise every filesystem interruption point or
claim that a failed endpoint recovery is safe to retry. Durable Host restoration
is independently selected and qualified by the clean restart scenario; endpoint
residue recovery never invents or adopts semantic state.

The outer length prefix is also a pure bounded codec used by the socket path.
Ordinary tests close empty, incomplete, over-limit, exact-prefix, remainder, and
content-safe diagnostic behavior. Pinned coverage-guided targets mutate
canonical handshake/session-control and framed conversation/transient seeds. Every
accepted handshake or frame prefix must re-encode exactly; outer framing cannot
consume the next stream frame. The remote-client SDK facade proves the handshake
codec is available without depending on daemon or authentication policy.
