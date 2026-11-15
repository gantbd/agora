# Requirements

## Core v0.3

- **R1 — Authoritative state:** one non-cloneable engine owns participant, room,
  membership, invitation, and retained-message state.
- **R2 — Mixed participation:** humans and software agents use the same admission,
  membership, role, and speaking rules; kind is explicit and never inferred.
- **R3 — Bounded state:** all identifiers, names, message bodies, collections, and
  retained histories have profile limits checked before publication.
- **R4 — Determinism:** commands contain all semantic inputs, including timestamps;
  identical state and command produce identical state and outcome.
- **R5 — Admission:** rooms declare open or invite-only admission. Invitation and
  moderation authority are explicit.
- **R6 — Atomic refusal:** a refused command does not change authoritative state or
  consume a message sequence number.
- **R7 — Ordered conversation:** accepted messages receive a room-local increasing
  sequence and retain the caller-supplied logical timestamp.
- **R8 — Privacy-aware observation:** message debug formatting does not disclose
  message content.
- **R9 — Effect isolation:** authentication, network I/O, storage, clocks, model
  calls, and agent processes remain adapter responsibilities.
- **R10 — Optimistic publication:** each accepted command checks an expected
  revision, advances it exactly once, and publishes one ordered domain event.
- **R11 — Bounded delivery:** a full event outbox backpressures commands before
  domain mutation; acknowledgment removes only a monotone published prefix.
- **R12 — Neutral addressing:** a message may explicitly address a bounded,
  canonical set of room members. Human and agent authors and recipients follow
  identical rules. Addressing is conversation state and never proof or a request
  that a participant executed work.
- **R13 — Replay-safe publication:** a cross-engine response key binds to one
  exact message publication. Matching replay returns the original transition;
  changed content conflicts; retained receipts are bounded.

## Volatile service v0.1

- **R14 — Participant-bound requests:** a connected client is bound to one
  registered participant. Speaking derives its author from that binding and
  room snapshots require membership. Human and agent participants follow the
  same service path.
- **R15 — Replay-safe service calls:** request identity is scoped by client;
  matching replay returns the retained reply, changed content conflicts, and a
  full bounded receipt index refuses before engine mutation.
- **R16 — Layered dependencies:** protocol values do not depend on engine or
  service internals, client interpretation depends only on protocol, and the
  serialized service is the only operational layer that owns an engine.
- **R17 — Canonical bounded wire:** protocol V1 has one closed binary encoding
  with explicit version and tag refusal, strict UTF-8, checked preflight sizing,
  independent frame/text/field/list ceilings, and complete-input decoding.

## Authenticated host v0.1

- **R18 — External authentication:** a trusted verifier authenticates the exact
  Connect participant claim and revalidates the current transport/credential
  context before every later frame. Per-session host time cannot regress. Agora
  retains no credential material.
- **R19 — Dual binding:** a server-owned host session binds one opaque verified
  subject, participant, and service client. A frame naming another client retires
  only the offending session and cannot operate on the named target.
- **R20 — Revocation-safe cleanup:** failed revalidation and transport loss release
  service clients even when retirement windows are disabled or saturated.
- **R21 — Disclosure-safe authentication:** unauthenticated participant claims do
  not reveal registration state, and unknown/revoked/mismatched sessions share one
  external refusal class. Host diagnostics redact subjects, sessions, and frames.

## SDK distribution facades v0.1

- **R22 — Audience-specific facades:** the trusted-host SDK must be sufficient to
  construct the engine/service/host stack, while the remote-client SDK exposes
  only client/protocol/wire concerns. Both are reexport-only, unpublished
  distribution labels that add no transport, credentials, authority, storage,
  orchestration, retry, or runtime policy.

## Bounded core observation v0.1

- **R23 — Revision-bound pages:** a registered participant may observe only rooms
  in which it is a current member, and a current member may page that room's
  members and retained messages. Every page is nonempty-limit bounded; a
  continuation is opaque, bound to observer, room where applicable, and exact
  engine revision; foreign and stale continuations refuse distinctly. Message
  pages expose the retained sequence interval rather than implying complete
  history. Observation and its refusals do not mutate engine state, and debug
  output discloses no identifiers, display names, room names, or message bodies.

## Volatile service V2 source profile

- **R24 — Participant-bound functional parity:** a distinct V2 service derives
  every create, invite, join, leave, role, and speak actor from its connected
  participant and projects the bounded core room/member/message pages through
  client-bound opaque cursor handles. V2 requests retain the same exact replay,
  changed-input conflict, capacity-before-execution, disconnect cleanup, and
  participant-kind neutrality laws as V1. Page cursors are bounded, client-bound,
  one-shot on successful continuation, reclaimed on disconnect, and refuse
  unknown/cross-client/type/room use without disclosing cursor ownership. A V2
  client owns a separate monotone request namespace.
- **R25 — Canonical Wire V2:** Protocol V2 has a separate version-2 complete-frame
  grammar covering every command, outcome, refusal, page, cursor, role, kind, and
  admission value. Encoding preflights exact bounded size; decoding bounds before
  reconstruction, consumes all input, and refuses V1, unknown nested tags,
  malformed text, overflow, truncation, and trailing bytes. Canonical V2 fixtures
  must not change without compatibility review.
- **R26 — Authenticated Host V2:** a separately typed V2 host applies the same
  exact Connect authentication, per-frame current-context revalidation, monotone
  host-time, dual host-session/service-client binding, client-substitution fence,
  non-enumerating external refusal, and unconditional transport-loss cleanup laws
  to Wire/Service V2. V1 frames refuse before authentication and V2 failures
  cannot operate on another session.

## Store-neutral durable service V2

- **R27 — Complete service recovery:** a separately versioned, scope-bound,
  bounded service checkpoint captures the canonical engine checkpoint, exact
  active and retired client namespace, request/reply receipts, live cursor
  consequences, and non-reused identifier frontiers. Recovery refuses foreign,
  malformed, future, noncanonical, over-limit, and semantically inconsistent
  records before exposing a service owner. Operational counters are deliberately
  process-local and do not change the semantic checkpoint.
- **R28 — Receipt-gated publication:** a fresh service-state change is externally
  publishable only after a matching durable-strength compare-and-exchange
  receipt. Matching replay and state-inert refusal reuse the current durable
  revision without touching storage. Known store refusal restores the exact
  pre-offer checkpoint; store error, indeterminate commitment, weak
  acknowledgement, or invalid receipt suppresses the semantic result and fences
  the owner until recovery.
- **R29 — Client uncertainty fence:** V2 port failures distinguish transmission
  that did not begin from an outcome that may have committed. After an unknown
  outcome, the client retains the exact request and permits only the equal
  command to reuse that identity; a different command is locally fenced without
  touching the port or allocating an ID. Not-sent retry does not clear earlier
  uncertainty. Resolution by reply clears it; abandonment and port extraction
  return the unresolved request explicitly and make no cancellation claim.
- **R30 — Atomic authenticated recovery:** a separately versioned, scope-bound
  Host V2 checkpoint captures host limits, active authenticated session subject,
  participant, service-client and verification-time bindings, retired session
  tombstones, next-session frontier, and the complete Service V2 checkpoint as
  one canonical unit. Every changed connect, frame, disconnect, revocation, or
  cleanup result is externally publishable only after that combined unit earns
  a matching durable receipt. Recovery restores the exact binding, requires
  fresh verifier revalidation on the next frame, and exact request replay cannot
  duplicate the domain transition.

## Qualified local durable storage V1

- **R31 — Exclusive private-directory checkpoint:** the Unix-only local profile
  owns one checkpoint key under an exclusive process lock in a mode-0700
  directory. A commit writes one bounded checksummed canonical record to a
  mode-0600 temporary file, synchronizes it, atomically renames it over the
  head, and synchronizes the directory before returning durable-strength
  evidence. The adapter pins the directory, lock, and head identities and fails
  closed on symlinks, permissive modes, replacement, malformed bytes, or crash
  residue. A real close/reopen composition must recover the combined Host V2
  record and exact replay must neither duplicate its engine event nor offer a
  second checkpoint.
- **R32 — Snapshot-aligned room change feed:** a current member may bind an
  opaque room change position only to an exact completed snapshot revision, then
  pull bounded room-scoped events strictly after that position. Reusing a
  position replays the same retained prefix; advancing to the returned position
  has no overlap. Event sequence and revision remain stable across engine,
  service-cursor, and durable service recovery. Outbox-prefix loss and any
  leave/rejoin visibility-epoch crossing return distinct explicit gaps requiring
  resnapshot. Protocol V2, Service V2, Wire V2, and the remote-client facade
  carry the closed commands, events, gap causes, and opaque server cursor without
  granting cursor authority or defining push delivery.
- **R33 — Moderated membership lifecycle:** kick, ban, and unban are bounded,
  participant-kind-neutral room transitions authorized only by the current role
  hierarchy. Their ordered events preserve actor, subject, reason, and opaque
  causation; bans prevent reentry until an authorized unban.
- **R34 — Observable room admission policy:** only the current owner may change
  Open/InviteOnly policy. The ordered event preserves actor plus prior and new
  policy without rewriting earlier invitation or admission facts.
- **R35 — Authority-scoped ban observation:** owners and moderators may page a
  canonical bounded ban set with prior-role evidence. Its cursor is observer,
  room, revision, client, and profile bound; other participants learn no set.
- **R36 — Closed capability discovery:** a connected participant may obtain one
  canonical strictly ordered list from a finite feature vocabulary. Discovery
  is replayable, nonmutating, descriptive, and grants no authority.
- **R37 — Stable identified-message profile:** each accepted message has one
  opaque restart-stable 128-bit reference distinct from order. Additive speak,
  history, and change projections preserve that reference while legacy grammars
  and authority laws remain unchanged and cursor profiles remain isolated.
- **R38 — Author withdrawal:** a current member may tombstone only their own
  retained message by stable ID. Identity, order, attribution, addressing, and
  relationship metadata remain; retained bodies are scrubbed from history and
  acceptance events without making a physical-erasure claim.
- **R39 — Moderator message redaction:** authorized owners/moderators may
  tombstone another eligible author's retained message with bounded reason and
  opaque causation. Role hierarchy, exact replay, attribution, and the same
  non-erasure boundary apply independently of author withdrawal.
- **R40 — Identity-bearing room changes:** an additive bounded room-change
  profile preserves stable message IDs across acceptance, history, feed, replay,
  and recovery. Its cursor is not substitutable with the legacy feed cursor.
- **R41 — One-level threads:** a reply names exactly one retained non-reply root
  in the same room. The immutable relationship survives withdrawal, redaction,
  replay, and recovery; retained tombstoned roots remain replyable, while unknown
  or evicted roots and nested replies refuse atomically. Root pages, root-bound
  reply pages, and threaded changes are independently bounded, client/observer/
  room/profile bound, and additive to unchanged legacy and identified grammars.
- **R42 — Retained thread lifecycle:** a retained root has canonical active or
  archived state. Its current author, room owner, or moderator may archive or
  reopen with bounded reason and opaque causation; all must be current members.
  Archive blocks new replies but preserves root, replies, topology, visibility,
  and tombstone laws and implies no completion. Current-member status and every
  room-change profile expose the state transition; replay and recovery preserve
  authority evidence and order. Unknown/evicted/reply targets, repeated state,
  malformed evidence, and unauthorized actors refuse atomically.
- **R43 — Bounded message reactions:** a reaction is a canonical retained
  `(message, key, participant)` set relation. Only a current member may add or
  remove their own relation; repeat add and absent remove refuse atomically.
  Keys and per-message distinct aggregates are profile bounded. Current members
  observe canonical counts and caller membership without enumerating reactors.
  Withdrawal, moderator redaction, thread lifecycle, replay, and recovery
  preserve relations and ordered attribution; retention eviction removes them.
  Reactions grant no authority and prove no delivery, attention, admission, or
  completion.
- **R44 — Bounded immutable message revisions:** a retained message begins with
  revision zero and its current original author may append replacement bodies
  up to a fixed 64-record ceiling. Stable identity, author, admission time,
  sequence, addressing, reply topology, and reactions do not change. Current
  members observe ascending records through client/observer/room/message/profile/
  engine-revision-bound cursors. Edit events appear in every change profile;
  withdrawal and redaction scrub every retained revision and edit-event body
  while preserving attribution and order. Replay and recovery are exact, and
  participant kind never changes authority.
- **R45 — Stable room-local topics:** a topic has opaque deterministic identity,
  a bounded current name, and an attributable creator. Current owners and
  moderators create topics under identical human/agent rules. Topic-scoped
  messages retain an immutable optional topic relation; replies inherit their
  root relation, and edits, reactions, lifecycle, withdrawal, and redaction do
  not change it. Current members receive bounded topic and exact-topic message
  pages plus a profile-isolated topic-aware feed. Legacy, identified, and
  threaded message grammars remain unchanged. Topic knowledge grants no
  authority, subscription, notification, workflow, or execution meaning.
- **R46 — Private monotone read positions:** a current member may advance and
  inspect only its own bounded room-, retained-thread-, or stable-topic-scoped
  marker through an exact retained target and observation revision. Markers are
  monotone, durable private projection state; they publish no room event, do not
  alter domain revision, survive unrelated membership and retention changes,
  and prove no delivery, attention, presence, execution, or completion.
- **R47 — Aegis verifier adapter:** a separate adapter may admit and revalidate
  an Agora host session only after the exact Aegis session and opaque grant
  authorize `Submit` on one configured `Agora` resource at the supplied host
  time. The adapter binds the claimed participant and opaque verified subject
  before calling authority, erases all Aegis and port failures to unavailable,
  and imports neither authority semantics nor grant material into Agora core.
- **R48 — Bounded private local transport:** a separate Unix-only composition
  binds one new mode-0600 domain socket inside an existing mode-0700 directory,
  accepts a trusted connection-derived
  Aegis context and explicit host time, and carries only length-prefixed
  canonical Wire V2 frames. It must bound the outer frame before allocation,
  apply an I/O deadline and per-connection request budget, keep the host session
  server-owned, reauthorize every admitted frame, and unconditionally close the
  host session after EOF, refusal, limit, or transport failure. Existing socket
  paths must fail closed, and teardown may remove only the exact endpoint object
  created by the composition.
- **R49 — Independent change retention:** every accepted domain event enters
  both the lossless bounded delivery outbox and a separately bounded recent
  change journal as one engine transition. A full outbox still backpressures
  before mutation; journal pressure instead evicts only its oldest observation
  copy and records the exact discarded edge. Delivery acknowledgment removes no
  change-journal entry. Pull feeds read only the journal and return a distinct
  change-retention gap when their exclusive position predates its edge. Both
  queues, edges, redaction consequences, and live service cursors must recover
  canonically without turning observation retention into delivery authority.
- **R50 — Reproducible coverage-guided parser evidence:** canonical wire and
  semantic-checkpoint decoders plus the production outer-frame prefix parser
  must have separately runnable libFuzzer targets
  under an exact driver, compiler, library, seed corpus, input ceiling, memory
  ceiling, timeout, run count, and random seed. Every accepted candidate must
  re-encode byte-for-byte and remain equal after decoding. The stable offline
  gate must verify the immutable corpus and toolchain contract without claiming
  that an opt-in campaign ran; every executed campaign must report incomplete
  coverage honestly and preserve failures as focused ordinary regressions.
- **R51 — Finite lifecycle state-machine evidence:** the core room lifecycle
  must be checked against an independent bounded model across owner/member
  speaking, deliberately stale speaking, leave, rejoin, kick, ban, and unban.
  Every schedule must preserve exact refusal precedence, atomic refusal,
  membership and ban state, revision/event continuity, message sequence and
  retention, delivery/change copies, and human/agent symmetry. The evidence
  must state its finite schedule depth and must not claim unbounded proof.
- **R52 — Finite conversation-relation state-machine evidence:** retained root
  and reply behavior must be checked against an independent bounded model across
  reply admission, archive/reopen, reaction add/remove, author edit, author
  withdrawal, unrelated speech, and root eviction. Each schedule must preserve
  exact refusal precedence and atomicity, immutable identity/sequence relations,
  tombstone revision scrubbing, reaction/thread cleanup on eviction, retention,
  event ordering, and human/agent symmetry without claiming unbounded proof.
- **R53 — Commit-pinned integrated release qualification:** an opt-in release
  decision must refuse tracked source drift, record the exact source revision,
  run the complete ordinary gate, run every workspace target in release mode,
  complete a bounded release-mode lifecycle/replay/recovery soak, and complete
  every pinned coverage-guided fuzz target. Numeric budgets must be explicit,
  positive, bounded, and adversarially validated. A partial or interrupted run
  is not a pass, and a pass makes no artifact, provenance, performance,
  architecture-portability, or publication claim.
- **R54 — Passive layered operational observation:** the qualified local daemon
  must expose a content-free point-in-time view that keeps transport, host, and
  service counters in distinct measurement boundaries and reports sticky loss
  of counter fidelity. Reading observations must perform no I/O or authority
  call, mutate no counter or semantic state, and alter no result. Diagnostics
  must contain no participant, subject, grant, room, message, credential, or
  frame data. Process-local counters make no latency, SLO, delivery, attention,
  audit, or recovery-continuity claim.
- **R55 — Irreversible idle drain fence:** the sequential local daemon must
  support an explicit idempotent transition from ready to draining that refuses
  every later `serve_next` before socket accept, authentication, authority, host,
  or service work. The endpoint remains owned until ordinary teardown. Because
  the transition requires exclusive mutable ownership, this profile covers only
  an idle daemon and makes no concurrent or in-flight graceful-drain claim.
- **R56 — Checksum-closed self-contained stable source:** an opt-in builder must
  refuse tracked Agora drift and existing output, stage the exact Agora revision
  plus only the Aegis and Keystone packages in the seventeen-component stable
  graph, preserve the reviewed relative dependency layout, and checksum every
  regular file. Sibling revisions are context, while selected-tree cleanliness
  and exact bytes remain separate evidence. Verification must reject missing,
  extra, changed, symbolic, special, or claim-mutated content before a locked
  offline isolated build, doctest, and warning-denied documentation run. This
  grants no origin provenance, binary reproducibility, support, or publication.
- **R57 — Deterministic stable-source SBOM:** the exact locked stable workspace
  must reproduce one canonical CycloneDX 1.5 JSON inventory offline. It must
  enumerate exactly the seventeen local-path packages and their closed dependency
  edges with exact names, versions, declared license expressions, and denied
  publication state; canonical ordering and vocabulary are part of the profile.
  Validation must reject missing, altered, escaped, host-path-bearing, or
  publication-enabled records. This source inventory makes no binary-content,
  advisory, legal-compatibility, origin-provenance, signature, or attestation
  claim.
- **R58 — Canonical qualified source archive:** a verified R56 directory may be
  encoded only as one bounded canonical USTAR stream rooted at `agora-source/`,
  with strict path and entry vocabularies, lexical order, fixed ownership and
  timestamps, normalized executable modes, zero padding, exactly two terminal
  blocks, and a separate exact SHA-256 record. Construction must refuse existing
  output, symbolic or special input, unsafe paths, and archive ceilings.
  Verification must reject checksum, framing, metadata, order, duplication,
  traversal, and type drift before platform extraction. Qualification must
  verify the extracted R56 closure, reproduce the archive byte-for-byte, and run
  the complete stable locked/offline isolated source qualification. This grants
  no compression, remote-origin, signing, attestation, binary, installation,
  support, or publication claim.
- **R59 — Canonical pre-authentication local handshake:** every accepted local
  connection must exchange an exact fixed-width ClientHello and ServerHello
  before authenticator, authority, host, or service work. The closed profile
  must pin its handshake and Wire versions, positive frame and post-connect
  budgets, capability-discovery support, and a nonzero caller-supplied 128-bit
  server incarnation. Incompatible hellos must refuse before downstream work;
  truncation and deadline failures remain transport failures. The codec must
  have canonical fixtures, exact re-encoding and coverage-guided parser
  evidence, and remote-client SDK reachability. Incarnation bytes and Debug
  output must be content-safe. The value identifies one declared logical
  service lifetime; it grants no participant identity or authority and makes no
  claim for truthful generation, exclusive lifetime locking, restart
  persistence, reconnect resumption, or replay safety.
- **R60 — Pinned local-daemon lifetime lease:** daemon construction must create
  a new endpoint-scoped mode-0600 record binding the declared server
  incarnation, hold an exclusive kernel lock for the complete socket lifetime,
  and pin the parent, record, and endpoint identities. A live owner and unlocked
  residue must be distinct refusals. Ownership must be revalidated before and
  after accept; loss or replacement must irreversibly clear readiness and fence
  authentication, authority, host, and service work. Clean teardown may remove
  only the exact owned socket and record; ambiguity must preserve evidence. The
  record bytes must be frozen and compatibility-inventoried. This grants no
  incarnation entropy, crash-residue repair, state recovery, process handoff,
  concurrency, or protection from the trusted same-UID parent controller.
- **R61 — Explicit local endpoint residue recovery:** recovery must be a
  separate caller-selected operation requiring the exact prior incarnation, a
  distinct nonzero next incarnation, and a newly supplied Host composition. It
  must acquire the stale lifetime lock before mutation, refuse a live owner,
  malformed or foreign record, unsafe socket object, and equal incarnation
  reuse. An absent socket is admissible only beside the exact unlocked record
  as incomplete-rotation residue. Recovery must publish and synchronize the
  next locked record before removing a validated stale socket when present,
  then bind the new endpoint. The resulting handshake must expose
  the next incarnation and no prior volatile Host state may be adopted
  implicitly. I/O failure may leave operator-requiring residue and must never be
  reported as successful recovery. This grants no automatic restart, entropy
  generation, durable Host restoration, migration, or multi-process handoff.
- **R62 — Bounded transient collaboration boundary:** presence and typing must
  be owned outside the durable conversation engine, room revision, event
  streams, and canonical checkpoints. Any advertised profile must use current
  engine membership and retained-target authority, opaque active-session
  sources, a qualified Host-supplied monotone clock, bounded requested
  lifetimes, explicit `Unknown`, atomic capacity refusal, and cursors bound to
  observer, room, engine revision, transient generation, and exact as-of time.
  Disconnect, revocation, substitution, and transport close must retire a
  source; leave, kick, and ban must retire that participant's room signals so a
  later rejoin cannot resurrect them. Restart begins `Unknown`, expiry is
  checked at final observation, humans and agents remain symmetric, and no
  signal implies membership, authority, delivery, attention, consent, agent
  activation, execution, or work completion. Protocol, wire, service, Host,
  SDK, compatibility, privacy, model, mutation, and restart evidence must close
  before capability advertisement. The selected architecture is detailed in
  [`docs/transient-collaboration.md`](docs/transient-collaboration.md).
- **R63 — Selectable durable local daemon owner:** the private local daemon must
  admit an application-selected durable Host owner over the generic checkpoint
  store port without importing a concrete store into Host or conversation
  semantics. Successful publication must remain receipt-gated; suppression or
  fencing must prevent readiness. Clean sequential teardown and reconstruction
  under a distinct server incarnation must retain committed conversation state
  while requiring fresh authentication. This does not restore transport
  sessions, replay disconnected pending requests, automate crash restart, or
  make Aegis and Agora recovery atomic.
- **R64 — Durable conversation with volatile collaboration:** one selected Host
  composition must combine receipt-gated durable conversation/session state
  with transient presence and typing without admitting transient signals,
  cursors, replay receipts, or clock state to the durable checkpoint. Every
  authenticated transient offer that changes Host verification/session state
  must remain commit-before-reply. Known durable refusal must restore the Host
  and conservatively retire affected volatile state; indeterminate, weak, or
  invalid durability must fence semantic access and retire the source. Recovery
  must retain conversation state under a fresh authenticated connection while
  presence begins `Unknown`, typing begins inactive, and the replay epoch changes.
  The private local collaboration transport must compose this owner without
  weakening its framing, authority, lifetime, or readiness fences.
- **R65 — Explicit durable-collaboration residue recovery:** the private local
  collaboration transport must offer a separate stale-endpoint recovery path
  that accepts a caller-selected, already recovered `DurableTransientHostV1`,
  requires the exact prior and distinct next server incarnations, and reuses
  R61's locked record/socket validation and rotation before binding handshake
  profile 2. A fresh authenticated connection must observe retained
  conversation state, empty presence/typing, and a next-incarnation replay
  epoch. The path must not invent durable state, restore a transport session,
  replay a disconnected request, automate restart, or claim atomic recovery
  with Aegis.
- **R66 — Passive durability publication observation:** authenticated durable
  Host owners must expose a content-free, point-in-time observation separating
  offers, durable commits, already-committed publication, checkpoint
  suppression, known store refusal, outcome uncertainty, weak acknowledgement,
  invalid receipt, and already-fenced suppression. Counters must saturate
  without wraparound and report sticky fidelity loss. The durable local daemon
  must retain transport and durability observations after fencing while
  withholding the untrustworthy Host/Service projection. Reading observations
  must perform no store, authority, retry, semantic, or transport effect and
  must disclose no key, checkpoint, participant, room, frame, or credential.
- **R67 — Closed sequential daemon lifecycle and terminal evidence:** the local
  daemon must represent starting, ready, draining, stopped, and fenced as one
  closed state rather than independent flags. Successful construction may
  return only ready; idle drain may transition only ready to draining; an
  ownership, replay-epoch, or durability fence must not later become draining.
  Because the sequential owner is exclusively borrowed while serving, draining
  with zero active connections is exact completion for this profile. Explicit
  stop must consume the owner and return content-free terminal health plus a
  closed clean, residue-preserved, or incomplete cleanup outcome. Clean may be
  reported only after the pinned socket and lifetime record are removed and the
  parent synchronization succeeds. This makes no concurrent drain, signal,
  service-manager, crash-shutdown, or process-exit cleanup claim.
- **R68 — Passive physical-store observation:** the qualified private-directory
  owner must expose a passive, content-free snapshot separating validated
  logical loads, compare-and-exchange offers, durable commits, known refusals,
  indeterminate publications, resource-integrity fences, current owner state,
  and the last locally acknowledged revision. Counters must saturate without
  wraparound. Observation must perform no filesystem, lock, validation, retry,
  or mutation effect and must expose no path, key, checkpoint bytes, record
  length, identity stamp, or detailed refusal. Facts reset on owner reopen and
  are neither durable audit nor proof that filesystem resources remain intact
  after the last operation.
- **R69 — Sticky active store-resource fence:** every failure while revalidating
  the live private-directory owner, lock, current head, or pending-residue state
  must atomically move the owner to Fenced and count one resource fence before
  returning the specific first error. Every later load or compare-and-exchange
  must return Fenced before further filesystem access and must not increment the
  fence again. This covers active resource validation only; constructor/open
  errors and pre-publication I/O failures outside validation retain their
  separately documented behavior.
- **R70 — Sticky operational store-I/O fence:** after a private-directory owner
  is constructed, every filesystem error returned by a load or
  compare-and-exchange must leave that owner Fenced before the error escapes.
  A failure to create the pending publication file is definitely uncommitted,
  reports its exact I/O class, counts one resource fence, and permits no later
  operation or silent retry on that owner. Write, synchronization, rename, and
  post-rename metadata failures retain their stricter indeterminate-publication
  classification. Constructor/open errors remain outside this owner-lifecycle
  claim.
- **R71 — Composable physical-store observation:** an effect-free storage port
  must let a checkpoint-store adapter supply one typed passive observation
  without exposing its implementation to the durable host or daemon. The
  durable and daemon compositions must preserve separate transport, runtime,
  durable-publication, and physical-store groups, propagate fidelity loss from
  every represented layer, and remain passive and content-free. Stores that do
  not implement the optional port retain the existing durable observation
  surface. This composition does not make process-local counters persistent,
  atomic across concurrent owners, or suitable as an integrity probe.
- **R72 — Current-graph source artifact qualification:** after the stable local
  component graph changes, two independently constructed R56 candidates for one
  exact tracked Agora revision must be byte-identical and one must pass the
  complete locked/offline isolated qualification. Canonical R58 archives built
  independently from those candidates must be byte-identical; one archive must
  pass sidecar and canonical validation, platform extraction, exact rearchive,
  and complete extracted-source qualification. The evidence must record exact
  counts and digests and must preserve unclean sibling selections as byte-level
  inputs rather than promote contextual Git revisions into provenance. This
  refresh grants no remote origin, signature, attestation, binary, installer,
  support, distribution, or publication claim.
- **R73 — Bounded server-incarnation generation:** the Unix daemon adapter must
  expose an injectable entropy boundary and one OS-backed implementation for
  producing its 128-bit server-incarnation value. Generation must reject the
  all-zero value, recovery generation must also reject the immediately prior
  incarnation, and both must stop after a fixed positive attempt bound. Entropy
  I/O failures must retain only a content-free error kind. The OS source must
  validate that `/dev/urandom` is a character device, while deterministic
  evidence must cover retry, exhaustion, source failure, and exact projection
  of one generated value into the lifetime record and ServerHello. This grants
  no uniqueness proof, authority, credential quality, automatic restart,
  residue repair, service supervision, or Aegis/Agora atomic recovery claim.
- **R74 — Offline canonical directory backup and restore:** the qualified
  directory adapter must export only while it can acquire the source store's
  exclusive lock, and the backup must be the exact canonical checksummed
  `AGDIRCP` record in a newly created synchronized mode-0600 file beneath an
  existing private directory. Verification must enforce the ordinary record
  bounds and integrity checks. Restore must accept only a validated backup and
  a previously absent destination, preserve the exact key, checkpoint bytes,
  and revision, return the exclusive owner, and permit the next CAS only at the
  preserved next revision. Empty, busy, corrupt, unsafe-mode, existing-output,
  and existing-destination cases must fail closed. Backup bytes are sensitive;
  encryption, remote transport, retention, rotation, rollback resistance,
  automatic scheduling, and disaster-recovery objectives remain external. A
  failed export or restore after output creation may leave explicit fail-closed
  residue and must not be reported as rollback or success.
- **R75 — Backup and restore interruption classification:** deterministic
  evidence must interrupt export after creation, write, file synchronization,
  and parent synchronization, and interrupt restore after destination creation,
  pending creation, write, synchronization, head rename, and directory
  synchronization. Every injected operation must return an interruption rather
  than success. Resulting state must classify as an invalid or independently
  verifiable backup, an empty new store, explicit pending crash residue, or an
  exact post-rename restored checkpoint that continues at the preserved next
  revision. This is process-level boundary injection; it grants no automatic
  cleanup, retry, controller-cache, hardware power-loss, or disaster-recovery
  claim.
- **R76 — Durable operator-restore session invalidation:** Host recovery after
  an operator restore must be distinct from ordinary clean restart. It must
  preserve conversation state, durable outbox state, and monotone Host/Service
  identity frontiers while invalidating every recovered Host session and its
  Service client, private cursor, and request receipt. The sanitized canonical
  Host checkpoint must commit by exact next-revision durable CAS before an
  active owner is returned. Store refusal, uncertainty, weak acknowledgement,
  invalid receipt, checkpoint failure, and store error must never activate the
  owner. Transient configuration must validate before this commit and transient
  signals restart empty. Daemon-incarnation rotation, credential reissue, and
  cross-engine restore coordination remain separate composition obligations.
- **R77 — Local operator-restore startup composition:** the qualified local
  profile must compose exact physical restore, R76 semantic sanitization,
  bounded generation of an incarnation distinct from the endpoint's recorded
  prior value, and R61/R65 locked endpoint-residue rotation in that order. A
  fresh collaboration handshake and replay epoch must carry the generated
  incarnation, the restored client frontier must not rewind, retained
  conversation state must remain readable, and the old session must remain
  unavailable without authority I/O. This is an explicit ordered operator
  procedure, not an atomic store/endpoint transaction, automatic restart,
  credential renewal, or coordinated Aegis recovery.
- **R78 — Operator-restore inter-step interruption classification:** real
  process termination after physical restore, semantic sanitization, bounded
  incarnation generation, and endpoint rotation must leave inspectable state.
  Restore-only state must retain the restored revision and sessions; sanitized
  state must have the exact next revision and zero active clients; generation
  must be persistence-neutral; and completed endpoint rotation must leave the
  sanitized store plus exact new-incarnation record/socket residue. A
  conservative retry from every class must sanitize when necessary, rotate from
  the actually recorded incarnation, admit a fresh higher client identity, and
  expose retained conversation state. This does not classify interruption
  inside each filesystem operation or make the steps atomic.
- **R79 — Endpoint-rotation interruption closure:** deterministic injection
  after next-record write, next-record synchronization, stale-socket removal,
  and parent-directory synchronization must never report success. Every result
  must retain the exact next record under an unlocked private lifetime file;
  the old validated socket may remain only before removal, and must be absent
  afterward. Retrying with that exact recorded incarnation and another distinct
  next incarnation must succeed for every class, including exact record plus
  missing socket, and must not adopt prior volatile runtime state. This is
  process-visible filesystem evidence, not device power-loss durability.
- **R80 — Authorized operator-restore boundary:** every durable Host
  `recover_after_restore` operation must require an injected authority decision
  over the exact store key, Host checkpoint scope, and loaded revision after
  checkpoint/configuration validation and before the sanitizing CAS. Authority
  refusal or failure must collapse to one `Unauthorized` class, perform no CAS,
  and return no owner. The Aegis composition must require an unexpired
  `Administer` decision in the `Agora` namespace for one configured restore
  resource, reject mismatched Agora key/scope before Aegis I/O, and retain
  correlation evidence binding the decision to the loaded Agora revision.
  That evidence must not claim an atomic Aegis/Agora transaction, credential
  authentication, grant issuance, rollback resistance, or coordinated restore.
- **R81 — Endpoint-rotation process-cut evidence:** a subprocess must terminate
  without unwinding immediately after each of R79's four rotation boundaries.
  Every resulting process-visible state must contain the exact next lifetime
  record, retain the old socket only before removal, release the lifetime lock,
  and resume successfully from that recorded incarnation to another distinct
  incarnation. This is real process-death evidence, not controller-cache or
  device power-loss durability.
- **R82 — Stable topic rename:** a separately advertised profile must let a
  current owner or moderator rename one stable topic without changing its
  identity, creator, message/reply relations, reactions, revisions, or read
  scopes. Same-name, collision, stale, unknown, invalid, and unauthorized
  attempts must refuse atomically. Every change projection must retain actor,
  topic ID, prior name, and new name.
- **R83 — Reversible topic resolution:** a separately advertised profile must
  let a current owner or moderator resolve and reopen one stable topic. Resolved
  topics refuse new roots and inherited replies while retaining identity,
  metadata, history, relations, revisions, reactions, and private read scopes.
  Resolution is conversation admission state, never task completion, approval,
  delivery, attention, or execution evidence.
- **R84 — Bounded topic supersession:** a separately advertised profile must
  let a current owner or moderator set, replace, or clear one depth-one
  same-room successor for a resolved topic. The relation is explicit navigation
  metadata; it does not move history, rewrite identity, redirect admission, or
  create task/workflow meaning. Source and target roles, cycles, and reopening
  constraints must refuse atomically.
- **R85 — Bounded topic aliases:** a separately advertised profile must retain
  at most 16 bounded display aliases per topic under one room-wide current-name
  and alias collision domain. Add and remove are explicit attributable events;
  commands and authority continue to select only stable topic identity. Alias
  order is canonical and alias text grants no lookup or routing authority.
- **R86 — Bounded topic tags:** a separately advertised profile must retain at
  most eight bounded byte-exact tags per topic in canonical order. Tags may be
  shared across topics and equal names or aliases because they are display/index
  metadata, not identity, hierarchy, permission, routing, search, subscription,
  workflow, or execution facts.
- **R87 — Derived search port and bounded reference:** an effect-free search
  contract must return only stable message identity and room-local sequence with
  explicit source revision, index generation, and available, incomplete, stale,
  or unavailable state. A bounded volatile reference adapter may implement one
  exact substring profile but must receive no participant authority and must
  never become history, recovery, or durability truth.
- **R88 — Participant-bound derived search:** a separate service/Host/client
  profile must derive the observer from the authenticated client, revalidate
  every identity-only hit against current membership and retained history, bind
  cursors and exact replay to the client/session epoch, and recover with the
  volatile index runtime empty. It makes no durable or production-scale index
  claim.
- **R89 — Bounded immutable attachment references:** a separately advertised
  profile must atomically admit a message with at most eight ordered bounded
  opaque attachment descriptors under room/reply/topic targets. Descriptors are
  references, never URLs or credentials. Edits preserve them; withdrawal and
  redaction retain only relation identities while scrubbing locator metadata in
  every retained projection. Blob I/O, integrity verification, scanning,
  download authority, and external lifecycle remain outside Agora.
- **R90 — Typed Vellum artifact references:** a separately advertised profile
  must admit ordered room-local relations to exact Vellum workspace, document,
  and source-revision scalar identities without depending on Vellum or storing
  content, locators, credentials, live witnesses, or edit authority. Tombstones
  retain only relation identity. Resolution and cross-engine mutation require
  separately authorized adapters and make no atomicity claim.
- **R91 — Normative aggregate model:** one top-level model must classify Agora's
  owned objects and disjoint state classes; state the transition, identity,
  ordering, observation, participant-symmetry, effect, and recovery laws; name
  explicit nonclaims; and link to independent executable refinement evidence.
  It must summarize rather than replace the detailed requirements and
  specification.
- **R92 — Exact package classification:** one machine-readable ledger must
  classify every production workspace package by exact path, closed category,
  maturity, effect class, public audience, and publication state. A mechanical
  gate must reject missing, stale, duplicate, malformed, unknown, reordered, or
  publication-enabled entries. Classification records current truth and grants
  no authority or qualification beyond each package's existing evidence.
- **R93 — Categorical package placement:** every production package must occupy
  the directory category declared by R92: engine, protocol, client, service,
  host, port, adapter, application, transport, or SDK. The workspace root must
  be package-free. Relocation must preserve package names and versions, locked
  dependency closure, canonical wire/checkpoint bytes, capability vocabulary,
  authority, public SDK dependency closures, and all existing functional
  evidence. Empty speculative categories are not required.
- **R94 — Local transport/application separation:** authority-free Unix endpoint
  lifetime, socket ownership, stream deadlines, outer framing, and incarnation
  entropy must live in a dedicated transport package. The daemon application
  must retain authentication, Host and durability selection, routing, clocks,
  recovery policy, lifecycle, and operational aggregation. The transport must
  import no Aegis, engine, service, Host, client, durable, or storage package;
  extraction must preserve public daemon behavior, error classes, canonical
  frame bytes, lifetime records, recovery ordering, and process-cut evidence.
- **R95 — Horizon-zero assurance closure:** every path-sensitive boundary,
  classification, compatibility, supply, SBOM, fuzz, source-candidate,
  source-archive, and release contract must close over the categorically moved
  graph. The complete ordinary gate and bounded commit-pinned release profile
  must pass from one clean revision. Qualification changes no package identity,
  wire or checkpoint bytes, capability advertisement, authority law, transport
  claim, or public SDK closure.
- **R96 — Conversation profiles:** ordinary rooms retain their owner-led
  open/invite-only lifecycle. A direct conversation must atomically establish
  exactly two distinct registered peers; a group-direct conversation must
  atomically establish three through the configured room-member ceiling. The
  complete founding cohort is immutable, ownerless, participant-kind-neutral,
  membership-only discoverable, and departure is permitted but irreversible.
  Current members alone observe the ordinary bounded retained history; leaving
  revokes observation without rewriting or erasing history. Shared room is not
  a local profile: it is a future projection of an ordinary invite-only room
  plus an explicitly qualified Agora federation binding. External identity and
  tenant trust remain directory/Aegis facts.
- **R97 — Private attention state:** one registered current member may observe,
  replace, or clear only its own bounded room/thread/topic attention preference.
  The value independently records follow, mute, and closed notification intent;
  Agora assigns no precedence and makes no delivery or attention claim. New
  thread/topic entries require a valid retained root/current topic, while an
  existing entry remains observable, replaceable, and clearable after message
  retention eviction. All-default removes the entry. Mutation is guarded by the
  observed engine revision, idempotent, private, kind-neutral, domain-revision
  neutral, and room-event neutral. Canonical recovery must preserve exact state
  and every legacy checkpoint byte.
- **R98 — Shared pins and private saves:** a shared pin is a bounded room-owned
  stable message relation changed by current members through revisioned pin and
  unpin events; a private save is a bounded participant-owned stable message
  relation changed without a domain revision or room event. Both preserve only
  message identity and original sequence across tombstone and history eviction,
  classify availability explicitly, and never copy body, revision, attachment,
  or Vellum artifact metadata. Membership loss gates access without silently
  deleting state. Pin creation and save creation require a retained room-local
  message; removal remains possible after eviction. Canonical recovery must
  preserve exact state and every legacy checkpoint byte.
- **R99 — Bounded polls:** a poll is a bounded room-owned structured
  conversation object with one stable identity, immutable ordered options,
  immutable Agora-local eligible cohort, immutable maximum choice count,
  private actor-owned ballots, public derived aggregates, and explicit
  open/closed lifecycle. Creation, ballot replacement or retraction, close, and
  reopen are stale-guarded revisioned room mutations with exact replay. Ballot
  events and observations must reveal aggregate state but neither voter identity
  nor another participant's selections. Closing fences ballots; reopening
  preserves them. Capacity evicts only the oldest closed poll and never an open
  poll. Canonical recovery must prove aggregate/ballot equality, ordering,
  eligibility, bounds, allocation frontiers, and every legacy checkpoint byte.
  Polls carry no approval, workflow, quorum, deadline, winner, or execution
  meaning.
- **R100 — Room-local topic collections and movement boundary:** the room's
  bounded topic registry is the first conversation collection and may be
  observed through a participant-bound, revision-bound stable-ID index filtered
  by closed lifecycle state and a canonical conjunctive tag set. Entries project
  current name, creator, lifecycle, aliases, tags, and depth-one supersession
  without adding state, events, read movement, search authority, or membership
  inheritance. Existing message/topic relations remain immutable admission
  provenance; move and copy commands remain absent, and merge/redirect means
  only explicit qualified topic supersession. Cross-room collections and
  product hierarchy remain absent.
- **R101 — Admission and moderation controls:** a separately advertised profile
  must enforce one owner-controlled room admission interval across every message
  form using opaque non-regressing Host-derived time evidence rather than
  caller `LogicalTime`, with no implicit role or participant-kind bypass. The
  existing hierarchy must support an indefinite posting restriction that blocks
  message admission without removing membership. Current members may create
  bounded stable reports against retained messages; reporters see their own,
  owners/moderators see the authority-scoped room queue, and unrelated members
  see none. Reviews close reports only as dismissed or escalated and make no
  moderation-action or workflow-completion claim. Closed-only report eviction,
  exact replay/recovery, content-free diagnostics, and human/agent symmetry are
  required. Aegis remains a passive decision boundary and classifier evidence
  must have no direct Agora mutation path.

- **R102 — Chronos scheduled-publication composition:** Agora and Chronos MUST
  remain separately owned engines with no new engine-to-engine dependency. A
  separate adapter MUST allocate one opaque non-reused scheduled-publication
  identity before Chronos admission and durably bind it to canonical bounded
  Agora intent, the authenticated participant, one Chronos service scope, and
  the original Chronos task allocation. The identity MUST NOT be derived by
  truncating a task reference, request ID, timestamp, or content digest. Agora
  MUST expose scheduled publication only through a trusted transport-neutral
  port outside ordinary participant Wire V2. The port MUST derive the author
  from the authenticated session, accept no enforcement time from the caller,
  and enforce current membership, posting restriction, admission interval,
  bounds, and capacity at the actual publication attempt. The first profile
  MUST publish only a plain room message with no addressing. Matching identity
  and intent MUST replay the exact accepted transition across recovery;
  conflicting reuse MUST refuse, known pre-mutation refusal MUST NOT fabricate
  success, and outcome-unknown publication MUST reconcile the same identity.
  The adapter MUST receipt-gate canonical monotone preparation, original
  allocation, possible dispatch, accepted publication, pre-dispatch
  cancellation, and reconciliation-required uncertainty. Once dispatch may
  exist, Chronos cancellation MUST NOT prove message absence or retract an
  accepted publication. Chronos remains the owner of eligibility, lateness,
  retry, cancellation, and temporal execution; Agora interprets no schedule
  time, deadline, recurrence, or catch-up policy. Atomic cross-engine commit,
  workflow completion, agent execution, delivery, federation, and erasure are
  nonclaims.
- **R103 — Hermes notification-delivery composition:** Agora and Hermes MUST
  remain separately owned engines and MUST be composed by a separate adapter.
  The first profile MUST use a trusted observer-bound visibility projection and
  a content-minimized wake schema; it MUST NOT copy message bodies, attachment
  locators, Vellum targets, attention settings, credentials, or endpoint
  secrets into Hermes. Subscription identity MUST be independently allocated
  and bound to an authenticated participant. Before Hermes append, the adapter
  MUST durably bind one independently allocated delivery ID to the exact
  subscription, Agora source reference, schema, and payload, and MUST replay the
  exact Hermes envelope after uncertain append. Delivery MUST revalidate current
  subscription, membership visibility, and endpoint authority. Endpoint success
  MUST be durable before Hermes consumer acknowledgment. Hermes acknowledgment
  MUST NOT alter Agora read position, attention state, presence, or domain
  acceptance. Retention gaps, membership loss, revocation,
  definitely-not-sent, and outcome-unknown results MUST remain explicit.
  Provider exactly-once, rich payloads, guaranteed delivery, workflow meaning,
  agent activation, federation, and external erasure are nonclaims.
- **R104 — typed interaction adapter boundary:** Interaction schemas and
  behavior MUST remain in separately owned typed adapters; Agora MUST NOT host a
  plugin runtime, callback registry, executable name, credential, arbitrary
  property bag, prompt, model, or tool definition. Each invocation MUST bind an
  independently allocated identity, one authenticated participant/session, and
  one currently visible retained source without accepting caller-asserted actor
  authority. Humans and agents MUST follow identical binding, visibility,
  replay, and admission rules. Output MUST be classified as either
  session-scoped ephemeral adapter state or an admission proposal. Ephemeral
  output MUST NOT mutate or survive as Agora domain/read/delivery/task state.
  A proposal MUST NOT become a message until it traverses the ordinary trusted
  participant-bound admission path with current membership, Aegis authority,
  admission controls, and Host time. Exact invocation replay, changed-input
  conflict, definitely-not-sent, outcome-unknown, restart, and cancellation
  MUST remain explicit in the adapter. Addressing, invocation, provider success,
  or an external-work reference MUST NOT imply agent activation, task admission,
  execution, completion, or delivery.
- **R105 — bounded authenticated remote transport:** Remote transport MUST be a
  separate authority-free package owning TCP/TLS resources, certificate
  verification, exact framing, connection concurrency, queue budgets,
  deadlines, channel binding, and concurrent drain without importing Agora
  domain, Host, Aegis policy, participant, task, delivery, or federation
  semantics. The initial profile MUST use TLS 1.3 with exact ALPN, disabled
  early data and session tickets, configured trust anchors, exact server-name
  verification, and an optional required-client-certificate service profile.
  Certificate identity MUST NOT become participant identity or authority. A
  canonical post-TLS hello MUST negotiate one exact compatible closed protocol
  profile, positive budgets, capability discovery, and a nonzero server
  incarnation before authentication or Host work. Connections, handshakes,
  frames, retained bytes, requests, and pending writes MUST be bounded;
  overload MUST refuse before application admission, while accepted replies
  MUST backpressure or close explicitly rather than disappear. Drain MUST stop
  accepts and new application admission, bound completion of already admitted
  replies by a deadline, force-close residue, and report exact content-free
  counts without relabeling a fence. Reconnection MUST create a fresh transport
  session and MUST NOT imply replay, authority, or semantic success. Upgrade
  compatibility MUST use closed range intersection and exact profile selection
  without reinterpreting earlier bytes.
- **R106 — native single-home federation:** Agora-to-Agora federation MUST be a
  separately owned composition over the authority-free remote transport. Every
  peer session MUST bind two verified stable server identities, current trust,
  two nonzero incarnations, the transport exporter, and one exactly negotiated
  closed protocol profile. Certificate identity alone MUST NOT grant federation
  authority. Participant, room, message, and event identities MUST be
  structured home-scoped values. Each room MUST have exactly one authoritative
  home; only that home may evaluate current admission, allocate ordered room
  events, or emit authoritative replication. The initial profile MUST accept
  only direct home-to-subscriber streams and MUST NOT re-export imported events.
  Replica journals MUST enforce exact next sequence, idempotent equal replay,
  changed-event conflict, explicit retained-history gaps, and verified bounded
  snapshot repair. Remote mutations MUST bind an authenticated participant,
  preserve human/agent symmetry, revalidate current home authority, retain
  bounded exact receipts, and distinguish definitely-not-sent from outcome-
  unknown. Any Agora publication identity used by federation MUST occupy a
  closed source domain distinct from scheduling and typed-interaction
  identities; equal raw identifier bytes across domains MUST NOT alias.
  Agora's trusted federation publication boundary MUST carry no peer trust,
  identity mapping, credential, clock, transport, retry, or room-home
  authority. Service MUST bind the current participant and federation receipt
  domain; Host MUST revalidate its authenticated session and supply current
  time; Durable Host MUST commit the resulting checkpoint before publishing a
  semantic reply. Exact later retry MUST retain the first accepted Host time
  only when room, participant, and content remain equal. The configured port
  MUST distinguish a stable reply, definitely-not-executed, and outcome-unknown;
  ambiguity MUST NOT be converted into a stable refusal. The receipt identity
  MUST include the full authenticated participant/room home scope in addition
  to the sender-scoped request ID; equal request bytes under distinct scopes
  MUST NOT alias. Agora's concrete durable endpoint MUST capture the configured
  Host owner, authenticated session, verifier context, and trusted clock rather
  than accepting them from the federation caller. It MAY report definitely-not-
  executed only after a known store refusal and successful checkpoint rollback;
  every other suppressed durable outcome MUST remain outcome-unknown.
  Reconnect and peer acknowledgement MUST NOT imply mutation success,
  participant read state, room authority transfer, or gap closure.
- **R107 — retention/export boundary:** Agora logical retention MUST classify
  current projections, authoritative changes, exact receipts, participant-
  private state, federation state, and operational copies independently under
  one complete revisioned policy. Logical unavailability, domain withdrawal or
  redaction, export custody, and physical store reclamation MUST remain distinct
  facts. Receipt retirement MUST atomically replace an exact retained receipt
  with a bounded durable non-reexecution tombstone before removing the receipt;
  identity reuse or retry execution MUST remain impossible while the fence is
  retained. Administrative policy, receipt-retirement, and export operations
  MUST revalidate an authenticated principal through an injected current-
  authority port and MUST treat humans and agents identically. Exact export
  MUST bind request identity, visibility scope, source engine revision,
  retention-policy revision, profile, ordered chunks, digests, frontiers, and
  explicit gaps into canonical replayable manifests; it MUST NOT widen
  visibility, mutate read or delivery state, or reconstruct absent records from
  an unauthorized copy. Independently owned record-class sources MUST compose
  only at one exact engine and policy revision, MUST be canonically ordered and
  nonoverlapping by class, and MUST NOT relabel an enclosing checkpoint as
  narrower records. Each independently owned class source MUST expose a
  persisted monotone source revision that advances exactly once per accepted
  class-state change, not per offer, replay, conflict, or policy-only change;
  revision saturation MUST fence exact export without blocking authorized
  cleanup. Legacy recovery without such a revision MUST remain ineligible for
  that native source until a subsequent class-state mutation establishes the
  new revision domain. The live authoritative room projection MUST NOT be
  treated as a retained historical copy: a positive class bound MUST identify
  only its current generation, a larger bound MUST NOT reconstruct superseded
  generations, and a disabled class MUST NOT silently delete rooms,
  memberships, or current messages. An installed policy MUST remain
  established after its effective engine revision; every later successful
  owner-local domain publication MUST continuously enforce enabled
  authoritative-change bounds
  without acknowledging or pruning independent delivery work. The installed
  request-receipt bound MUST limit new live replay payloads before execution,
  MUST resolve an already retained exact replay before applying that limit, and
  MUST NOT evict a non-reexecution tombstone to admit work. Export retirement
  MUST revalidate the current source
  and policy plus fresh administrative authority, atomically replace one exact
  live custody copy with a separately bounded non-reuse tombstone, replay an
  identical request, conflict on changed identity, and survive custody restart;
  it MUST NOT claim physical reclamation or erasure. Physical reclamation MUST
  name bounded old generations
  only after a complete successor checkpoint is durably published, reopened,
  and validated; unknown publication or validation MUST fence reclamation.
  Storage unlink, replacement, or provider attestation MUST NOT be represented
  as secure erasure. Legal hold, eDiscovery, regulated schedules, subject-rights
  workflow, recipient deletion, backup deletion, and guaranteed erasure remain
  separate compliance concerns and explicit nonclaims.

## Explicitly open

The R96 through R101 contracts are implemented and commit-pinned release-
qualified through the private-local stack. R102 and R103 have qualified generic
composition foundations, and R104 has a qualified Agora context/promotion
boundary plus an initial closed acknowledgement adapter; the remaining live
Chronos atomic-admission, concrete delivery-endpoint, and additional schema-
adapter profiles are tracked explicitly in traceability.
R105's canonical hello, separate TLS transport substrate, bounded concurrent
listener/drain owner, and authenticated request-gated Host-routing composition
are implemented and commit-pinned, including local overload, fresh reconnect,
compatible/disjoint upgrade cuts, plus a committed-mutation/unread-reply cut
that reconnects as a fresh client and reconciles by bounded observation; a
concrete credential-authenticator profile, remaining process/network
disconnect cuts, platform and produced-
binary-artifact evidence, and full-profile qualification remain open; the fixed
reconnect/clean-drain soak and reproducible self-contained source candidate are
commit-pinned, with ordinary/soak/source evidence recorded on
`aarch64-apple-darwin`; a multi-platform matrix remains open.
R106 has an accepted native single-home federation architecture plus a
commit-pinned zero-dependency identity/negotiation protocol and bounded
single-home replica-journal foundation, including bounded authoritative snapshot
gap repair with rollback fencing and honest unavailable-history classification.
Its canonical fixed-width hello and session constructor bind externally verified
server identities/trust epochs, both incarnations, a fresh exporter, exact
profiles, and bounded-minimum limits. Its event, snapshot, and acknowledgement
frames are distinct, frozen, bounded,
and fail closed on wrong kind, zero identity, invalid event sequence,
noncanonical length, truncation, trailing bytes, or negotiated-limit overflow.
The separate federation workspace also has frozen bounded admission
request/reply frames, authenticated-sender participant
and room-home preflight, bounded exact receipts, matching replay, changed-input
conflict, participant-kind-neutral execution, and a client uncertainty fence.
The replica foundation additionally refuses self-replication, redacts opaque
event/snapshot/checkpoint payloads, and canonically recovers only a contiguous
room-bound frontier within current event, snapshot, count, and retained-byte
limits. Canonical admission home/client checkpoints preserve validated exact
receipts and pending uncertainty without re-execution. Agora revision `f8cec20`
source-scopes legacy, scheduled, interaction, and federation publication
receipts, canonically recovers profile 149, and passed the complete gate.
The additive federated-publication port plus Service, Host, and Durable Host
composition implement current policy, authenticated actor/time binding, commit
gating, and later-time replay; revision `2fe8bb0` passed the complete gate.
Revision `eaa22b1` commit-pins the passing full home-scoped receipt identity,
profile-150 recovery, and three-way port certainty result.
The concrete durable endpoint captures local session, verifier, and clock
authority and conservatively maps commit-gate evidence without exposing those
facts to a peer; revision `bbdf9d7` passed the complete gate.
Trust-provider and identity-mapping composition, physical storage, transport
composition, adversarial campaigns, and integrated qualification remain open.
Additional physical durable storage adapters,
durable event sinks, credential authenticators, network transports, end-to-end
encryption, federation, delivery acknowledgment, and durable audit are not part
of the current v0.3 claim. Credential authentication, network transport,
concurrent daemon lifecycle, and durable profiles beyond the qualified
single-owner local Unix directory remain open. Protocol V1 bytes and semantics
are unchanged.

