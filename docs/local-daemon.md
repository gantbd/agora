# Private local daemon profile

Status: qualified bounded Unix transport composition V1 with separately selected
volatile conversation, volatile collaboration, volatile derived search,
receipt-gated durable conversation, receipt-gated durable conversation plus
volatile collaboration, or trusted scheduled-publication composition over a
volatile or receipt-gated durable conversation owner.

`agora-local-transport` owns the authority-free outer connection, framing,
endpoint lifetime, deadlines, and incarnation entropy. `agora-local-daemon`
composes that mechanism around an Aegis-authorized volatile, transient-wrapped, durable, or combined
durable-conversation/volatile-collaboration Host owner. Its normal dependency
closure reaches only the transport, verifier adapter, Host/Durable owners, Wire, and the
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
  incarnation, either selected directly or obtained through the adapter's
  injectable bounded generation boundary and OS-backed `/dev/urandom` source;
- positive frame, request, and I/O deadline limits.

The injected transport requires a real existing mode-0700 parent directory, creates a new
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
old record lock, validates the exact record and stale socket, synchronizes the
next locked record, removes only that socket, and then uses the ordinary bind
path. An absent socket is accepted only beside the exact unlocked record as
incomplete-rotation residue. Live, foreign, malformed, or unsafe-object residue
refuses without adoption.
For the combined durable-collaboration owner,
`recover_stale_durable_collaboration` accepts a caller-selected owner already
recovered from its checkpoint store, applies that same exact residue rotation,
and binds collaboration handshake profile 2. The daemon neither selects the
store nor restores an old transport session.

The qualified operator-restore procedure composes those existing boundaries in
one explicit order: exact directory restore, Aegis `Administer` authorization
bound to the loaded Agora revision, durable Host session sanitization, bounded
distinct-incarnation generation, then exact stale-endpoint rotation.
The resulting handshake and replay epoch carry the generated incarnation, while
a fresh connection advances the restored client frontier and reads retained
conversation state. Store recovery and endpoint rotation remain separate
failure domains; the Aegis decision/revision pair is correlation evidence, and
this procedure is not atomic or automatic.
Real subprocess cuts classify each persistent boundary. Before sanitization the
restored Host still contains its old session. After sanitization it has the next
store revision and zero clients while endpoint residue remains old. Generated
incarnation selection has no persistent effect. After endpoint rotation, OS
process exit leaves the sanitized store and exact new-incarnation record/socket
residue. Conservative restart from each class reaches another distinct
incarnation and a fresh authenticated client without losing the retained room.
Four deterministic endpoint-rotation interruptions cover next-record write,
record synchronization, socket removal, and parent synchronization. Each
returns `Interrupted`, retains the exact next record, and leaves either the old
validated socket or no socket. Retrying from that recorded incarnation reaches
a second distinct record in every case. A public daemon scenario separately
proves exact-record/missing-socket residue rebinds without runtime adoption.
The same four boundaries are exercised by child processes that call
`process::exit` before unwinding. Every child releases the operating-system lock,
leaves the same exact record/socket class as deterministic injection, and
resumes to another distinct incarnation. This strengthens process-death
evidence only; it does not simulate power loss.

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

`DaemonHealthV1` is deliberately content-free. It reports one closed lifecycle
state, connections, accepted and refused handshakes, cleanly completed
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

`begin_drain` is an irreversible, idempotent Ready-to-Draining transition; every
later `serve_next` refuses before
accepting a socket or invoking authentication, authority, Host V2, or Service
V2. A prior fence remains fenced rather than becoming contradictory drain
state. The endpoint remains pinned and owned until teardown. Since the method
requires exclusive mutable access to this sequential owner, it cannot overlap an
in-flight connection and is not a concurrent graceful-drain mechanism.

`stop` consumes that exclusive owner and returns a content-free terminal
receipt: prior lifecycle, Stopped health, and `Clean`, `ResiduePreserved`, or
`Incomplete` cleanup. Clean requires removal of the exact pinned socket and
lifetime record plus parent synchronization. Ambiguous ownership preserves
evidence. Draining with zero active connections is exact completion only because
this profile is sequential and exclusively borrowed while serving.

## Trust boundary and non-claims

The mode-0700 socket parent and same-UID processes are trusted. The
authenticator is trusted to bind peer/credential evidence to the exact Aegis
session, grant, participant, and opaque host subject; this crate does not define
that credential mechanism. The clock is trusted and the Aegis service port must
provide its own availability and context provenance.

The implemented private-local production inputs select the narrowest honest
interpretation of that trust boundary. `PrivateEndpointAuthenticatorV1` binds
every connection admitted by the hardened endpoint to one deployment-supplied
`AegisContextV1`; it issues no identity, session, or grant and does not claim
multi-user authentication. `SystemHostClockV1` supplies Unix-epoch milliseconds
and clamps rollback to a non-regressing process-local sequence. Its initial
wall-clock read remains trusted, and it makes no durable cross-restart clock
claim. The complete create/open/residue-recovery durable owner factory now
lives in `agora-private-local`. Its shared owner loop also turns an explicit
shutdown wakeup into irreversible drain and a terminal cleanup receipt; the
standalone bounded configuration and POSIX signal wrapper reuse that exact
owner. Real Aegis/Agora standalone and Workbench process qualification remain
open.

Create-time participant and room policy is not part of this daemon. The
separate `agora-private-local` application root now builds a bounded fresh
engine before endpoint publication and can issue one exact session-local Aegis
grant for either conversation or scheduled composition. Deployment still owns
the administrator credential mapping, authority epoch, grant lifetime, opaque
subject, and resource. The application factory composes those results with this
daemon for new stores, clean reopen, and exact stale endpoint rotation without
moving bootstrap or grant policy into the transport boundary.

The handshake selects one exact protocol profile; it is not version-range or
feature negotiation. Its configured incarnation is not authenticated; the
lifetime record binds that value to one exclusive live endpoint but does not
prove uniqueness or a restart boundary. The qualified OS source validates and
reads `/dev/urandom`, rejects zero, and can reject the immediately prior value
within eight attempts. The OS entropy facility remains trusted, and generation
does not acquire the endpoint, select recovered state, or automate restart. The
profile makes no claim for
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

The separate search selection uses fixed-width handshake profile 3 and the same
routed envelope/control hello, but admits conversation/search rather than
conversation/transient frames. Search route tag `04` reaches `SearchHostV1`
under the exact authenticated session. Each EOF closes the Host client and
retires volatile search receipts and adapter continuations. Reconnecting starts
a fresh request namespace and replay epoch. Separately, Host checkpoint recovery
wraps the recovered authority in an empty search runtime and rejects old search
continuations; an integrated durable-daemon/index recovery owner is not claimed.

The scheduled-publication composition uses fixed-width handshake profile 4 and
the same actor-bound conversation connect plus replay-epoch control hello. It
admits only conversation, control, and scheduled route tag `05`; transient and
search routes refuse. The scheduled codec is closed over publish and retained
observe. The daemon supplies fresh Host time and the authenticated session to
the owner, which first revalidates ordinary `Submit` authority and then obtains
an operation-specific Aegis decision: `Execute` for publish or `Observe` for
observation. This owner-side enforcement is required even when a calling
cross-engine adapter has already authorized its own `Dispatch` or
`InspectAgora` action.

`agora-local-client::ScheduledLocalClientV1` is the public concrete client for
profile 4. It verifies the endpoint is a mode-0600 single-link Unix socket,
negotiates the profile, validates the control hello, and classifies failures
before the scheduled frame as definitely not sent. Failures after the offer are
outcome-unknown; invalid or wrong-operation replies are never accepted. A real
directory-store scenario publishes and then observes the retained result across
two fresh sessions and six committed durable boundaries. This establishes a
private-local deployed composition, not a remote transport or a Workbench UI.

## Qualification evidence

The real-socket scenarios verify private-parent/socket modes and type,
canonical incarnation-bound lifetime bytes, live-owner exclusion, distinct
stale-residue refusal, replacement fencing before authentication, and
conservative teardown,
the exact ServerHello fields, refusal of an incompatible ClientHello before any
authentication, authority, Host, or Service work, Aegis-authorized Connect and
per-frame execution, byte-identical request replay, EOF cleanup, erased
authenticator error detail, participant mismatch without authority I/O, bounded
request exhaustion cleanup, oversized length refusal before host admission,
unsafe/existing-path refusal, and preservation of a replacement endpoint.
One scenario additionally proves an OS-generated nonzero incarnation is copied
exactly into the canonical lifetime record and pre-authentication ServerHello,
then cleans up through the ordinary pinned path. Pure deterministic evidence
covers zero and prior-value retry, the fixed eight-read exhaustion bound, and
content-free entropy failure.
Dependency and effect gates keep the adapter limited to the Aegis verifier,
Host V2, Wire V2, and Unix filesystem/socket effects. The successful path
additionally verifies exact layer-separated observation, repeated-read
passivity, content-safe diagnostics, and sticky counter saturation without
wraparound. Durable selections add a separate passive publication group for
commit, already-committed, checkpoint, store-refusal, uncertainty, weak/invalid
receipt, and fenced outcomes. After a durability fence, transport and durability
facts remain available but the Host/Service projection is absent. A separate real-endpoint scenario verifies the idle drain
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
physical directory store and exact unlocked endpoint residue: the record and
incarnation rotate, the room survives reconstruction, and presence and typing
do not.

The recovery scenario separately proves live-owner exclusion; equal, wrong, and
malformed prior-incarnation refusal without mutation; exact locked rotation to a
new record; a fresh Host with zero prior offers; next-incarnation handshake; and
clean teardown. It does not exercise every filesystem interruption point or
claim that a failed endpoint recovery is safe to retry. Durable Host restoration
remains independently caller-selected; the combined scenario qualifies its
explicit composition with endpoint residue recovery, which never invents or
adopts semantic state.

The outer length prefix is also a pure bounded codec used by the socket path.
Ordinary tests close empty, incomplete, over-limit, exact-prefix, remainder, and
content-safe diagnostic behavior. Pinned coverage-guided targets mutate
canonical handshake/session-control and framed conversation/transient seeds. Every
accepted handshake or frame prefix must re-encode exactly; outer framing cannot
consume the next stream frame. The remote-client SDK facade proves the handshake
codec is available without depending on daemon or authentication policy.
