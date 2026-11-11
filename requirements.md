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
  semantic-checkpoint decoders must have separately runnable libFuzzer targets
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

## Explicitly open

Additional physical durable storage adapters, durable event sinks,
credential authenticators, network transports, end-to-end encryption,
federation, delivery acknowledgment, and durable audit are not part of the
current v0.3 claim.
Credential authentication, network transport, operational daemon lifecycle,
backup/restore qualification, and durable profiles beyond the qualified
single-owner local Unix directory remain open;
Protocol V1 bytes and semantics are unchanged.

