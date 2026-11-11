# Private local daemon profile

Status: qualified bounded Unix transport composition V1.

`agora-local-daemon` owns only the outer connection and framing lifetime around
an Aegis-authorized `HostV2`. It does not enter Agora's engine, protocol,
service, host, wire, Aegis adapter, or SDK semantics.

## Composition

The application supplies:

- a `HostV2<AegisVerifierV1<P>>` with its engine, Service V2 limits, Wire V2
  limits, Aegis service port, and fixed Agora resource;
- an `AuthenticatorV1` that derives one trusted `AegisContextV1` from the
  accepted Unix connection;
- a `HostClockV1` that supplies explicit monotone host time; and
- positive frame, request, and I/O deadline limits.

The daemon requires a real existing mode-0700 parent directory, creates a new
Unix-domain socket within it, hardens the socket to mode 0600, validates its
type, and pins its device/inode identity. A symlink or permissive parent and an
existing regular file, symlink, socket, or other endpoint object refuse
construction. Drop removes only the exact socket object it created; replacement
is never unlinked.

## Connection state machine

1. Accept one connection and apply the configured read/write deadline.
2. Derive the trusted Aegis context from that connection.
3. Read one bounded length-prefixed canonical Wire V2 Connect frame.
4. Return the Host V2 connected frame or its shared unavailable refusal.
5. Keep the opaque host session only in daemon memory.
6. For at most the configured number of later frames, supply fresh host time and
   the same connection context to Host V2. Host V2 and the Aegis adapter bind and
   reauthorize the retained subject and participant before Service V2 executes.
7. On EOF or any later error, close the host session and service client
   unconditionally.

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
connections, accepted connections, cleanly completed connections, refused
connections, transport errors, and counter saturation. It exposes no
participant, subject, grant, room, message, or frame data.

`DaemonObservationV1` is the read-only composition view. It retains distinct
transport, Host V2, and Service V2 counter groups so a socket acceptance cannot
be mistaken for authentication, host execution, or domain acceptance. A sticky
`fidelity_lost` bit is the disjunction of every represented saturation signal.
Reading the view performs no I/O or authority call, changes no counter, and
cannot change a request result. The snapshot contains no identifiers, content,
credentials, frame bytes, timings, or refusal detail; it is process-local and
excluded from semantic recovery.

## Trust boundary and non-claims

The mode-0700 socket parent and same-UID processes are trusted. The
authenticator is trusted to bind peer/credential evidence to the exact Aegis
session, grant, participant, and opaque host subject; this crate does not define
that credential mechanism. The clock is trusted and the Aegis service port must
provide its own availability and context provenance.

The profile makes no claim for TCP or other network transports, confidentiality,
peer-credential extraction, protocol negotiation, server-incarnation evidence,
exclusive lifetime locking, crash-residue recovery, concurrent clients,
overload queues, graceful drain, signal handling, socket activation,
installation, cross-restart request identity, or atomic Aegis/Agora recovery.
The exclusive directory checkpoint adapter remains a separate durability
mechanism; this transport currently owns a volatile `HostV2`.

## Qualification evidence

The five real-socket scenarios verify private-parent/socket modes and type,
Aegis-authorized Connect and per-frame execution, byte-identical request replay,
EOF cleanup, erased authenticator error detail, participant mismatch without
authority I/O, bounded request exhaustion cleanup, oversized length refusal
before host admission, unsafe/existing-path refusal, and preservation of a
replacement endpoint. Dependency and effect gates keep the adapter limited to
the Aegis verifier, Host V2, Wire V2, and Unix filesystem/socket effects. The
successful path additionally verifies exact layer-separated observation,
repeated-read passivity, content-safe diagnostics, and sticky counter
saturation without wraparound.
