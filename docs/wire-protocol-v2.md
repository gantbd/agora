# Agora wire protocol V2

Status: canonical bounded binary profile for `agora-protocol::v2`.

V2 is a separate closed grammar. It does not reinterpret or extend a V1 frame.
It encodes one complete request or reply and supplies no stream framing,
transport, authentication, confidentiality, compression, feature negotiation,
or session authority. Connect's participant ID remains an unauthenticated claim.

## Scalars

V2 uses the same canonical scalar encodings and independent frame/text/field/list
ceilings as V1: `u8`, big-endian `u64`, four-octet-length strict UTF-8 `text`, and
`u64`-counted lists. A `u32` domain value is encoded as `u64` and refuses on
decode if it exceeds `u32::MAX`. Optional cursors use tag `00` for absent or tag
`01` followed by cursor `u64`. Unknown optional tags refuse.

Every V2 frame starts with byte `02`. Root request and reply tags retain V1's
outer meanings (`01` Connect, `02` Execute, `03` Disconnect; `81` Connected,
`82` ConnectRefused, `83` Executed, `84` Disconnected, `85`
DisconnectRefused), but all nested Execute values are V2 types.

## Commands

| Tag | Command | Fields |
| --- | --- | --- |
| `01` | Rooms | optional cursor, limit `u64` constrained to `u32` |
| `02` | Members | room `text`, optional cursor, limit |
| `03` | Messages | room `text`, optional cursor, limit |
| `04` | CreateRoom | room `text`, name `text`, admission, expected revision `u64` |
| `05` | Invite | room `text`, participant `text`, expected revision |
| `06` | Join | room `text`, expected revision |
| `07` | Leave | room `text`, expected revision |
| `08` | SetRole | room `text`, participant `text`, role, expected revision |
| `09` | Speak | room, expected revision, logical time, body, addressed-participant list |
| `0a` | OpenChanges | room `text`, exact completed snapshot revision |
| `0b` | Changes | room `text`, required opaque cursor `u64`, limit |
| `0c` | Kick | room, participant, reason, 16-byte causation, expected revision |
| `0d` | Ban | room, participant, reason, 16-byte causation, expected revision |
| `0e` | Unban | room, participant, reason, 16-byte causation, expected revision |
| `0f` | SetAdmissionPolicy | room, admission, expected revision |
| `10` | Bans | room `text`, optional cursor, limit |
| `11` | Capabilities | no fields |
| `12` | IdentifiedMessages | room `text`, optional cursor, limit |
| `13` | SpeakIdentified | room, expected revision, logical time, body, addressed-participant list |
| `14` | WithdrawMessage | room, expected revision, 16-byte opaque message ID |
| `15` | RedactMessage | room, expected revision, message ID, reason, 16-byte opaque causation |
| `16` | OpenIdentifiedChanges | room `text`, exact completed snapshot revision |
| `17` | IdentifiedChanges | room `text`, required opaque cursor `u64`, limit |
| `18` | ThreadRoots | room `text`, optional cursor, limit |
| `19` | ThreadReplies | room `text`, 16-byte root message ID, optional cursor, limit |
| `1a` | SpeakReply | room, expected revision, root message ID, logical time, body, addressed-participant list |
| `1b` | OpenThreadedChanges | room `text`, exact completed snapshot revision |
| `1c` | ThreadedChanges | room `text`, required opaque cursor `u64`, limit |
| `1d` | ArchiveThread | room, expected revision, root message ID, reason, 16-byte causation |
| `1e` | ReopenThread | room, expected revision, root message ID, reason, 16-byte causation |
| `1f` | ThreadStatus | room, root message ID |
| `20` | AddReaction | room, expected revision, message ID, reaction key |
| `21` | RemoveReaction | room, expected revision, message ID, reaction key |
| `22` | ReactionStatus | room, message ID |
| `23` | EditMessage | room, expected revision, message ID, logical time, replacement body |
| `24` | MessageRevisions | room, message ID, optional cursor, limit |
| `25` | Topics | room, optional cursor, limit |
| `26` | TopicMessages | room, topic ID, optional cursor, limit |
| `27` | CreateTopic | room, expected revision, bounded name |
| `28` | SpeakInTopic | room, expected revision, topic ID, logical time, body, addressed-participant list |
| `29` | OpenTopicChanges | room, exact completed snapshot revision |
| `2a` | TopicChanges | room, required opaque cursor, limit |
| `2b` | ReadPosition | room, closed read scope |
| `2c` | AdvanceReadPosition | room, observed revision, closed read scope, target message ID |
| `2d` | RenameTopic | room, expected revision, topic ID, bounded new name |
| `2e` | ResolveTopic | room, expected revision, topic ID |
| `2f` | ReopenTopic | room, expected revision, topic ID |
| `30` | TopicStatus | room, topic ID |
| `31` | SupersedeTopic | room, expected revision, source topic ID, successor topic ID |
| `32` | ClearTopicSupersession | room, expected revision, source topic ID |
| `33` | TopicSupersessionStatus | room, source topic ID |
| `34` | AddTopicAlias | room, expected revision, topic ID, bounded alias |
| `35` | RemoveTopicAlias | room, expected revision, topic ID, bounded alias |
| `36` | TopicAliasStatus | room, topic ID |
| `37` | AddTopicTag | room, expected revision, topic ID, bounded tag |
| `38` | RemoveTopicTag | room, expected revision, topic ID, bounded tag |
| `39` | TopicTagStatus | room, topic ID |
| `3a` | AttachmentMessages | room, optional cursor, limit |
| `3b` | SpeakWithAttachments | room, expected revision, target, logical time, body, addressed list, descriptor list |
| `3c` | OpenAttachmentChanges | room, exact completed snapshot revision |
| `3d` | AttachmentChanges | room, required opaque cursor, limit |
| `3e` | VellumArtifactMessages | room, optional cursor, limit |
| `3f` | SpeakWithVellumArtifacts | room, expected revision, target, logical time, body, addressed list, reference list |
| `40` | CreatePeerConversation | room, name, profile, canonical founding-participant list, expected revision |
| `41` | ConversationProfile | room |
| `42` | AttentionPreference | room, closed room/thread/topic scope |
| `43` | SetAttentionPreference | room, observed revision, scope, preference |
| `44` | Pins | room |
| `45` | SetPin | room, expected revision, message ID, pinned Boolean |
| `46` | SavedReference | room, message ID |
| `47` | SetSavedReference | room, observed revision, message ID, saved Boolean |
| `48` | SavedReferences | room |
| `49` | Polls | room |
| `4a` | Poll | room, 16-byte opaque poll ID |
| `4b` | PollBallot | room, poll ID; caller is derived from the bound client |
| `4c` | CreatePoll | room, expected revision, prompt, ordered option list, canonical eligible-participant list, maximum choices `u8` |
| `4d` | SetPollBallot | room, expected revision, poll ID, strictly increasing option-index list |
| `4e` | SetPollOpen | room, expected revision, poll ID, open Boolean |
| `4f` | TopicCollection | room, lifecycle/tag filter, optional cursor, limit |
| `50` | AdmissionControl | room; caller-private restriction state is derived from the bound client |
| `51` | SetMinimumMessageInterval | room, minimum interval milliseconds, expected revision |
| `52` | SetPostingRestricted | room, participant, restricted Boolean, bounded reason, 16-byte opaque causation, expected revision |
| `53` | MessageReports | room; returns only reports owned by the bound participant |
| `54` | ModerationReports | room; owner/moderator authority is derived from the bound participant |
| `55` | SubmitMessageReport | room, observed revision, message ID, bounded reason, 16-byte opaque causation |
| `56` | ReviewMessageReport | room, report ID, Dismissed/Escalated disposition, bounded review reason, 16-byte opaque causation |

Admissions are Open `01` and InviteOnly `02`. Roles are Owner `01`, Moderator
`02`, and Member `03`. Participant kinds are Human `01` and Agent `02`.

## Outcomes

An executed protocol reply is Accepted `01` plus an outcome or Refused `02` plus
a refusal. Outcome tags are Rooms `01`, Members `02`, Messages `03`, RoomCreated
`04`, InvitationRecorded `05`, Joined `06`, Left `07`, RoleChanged `08`, and
MessageAccepted `09`, ChangesOpened `0a`, Changes `0b`, ChangeGap `0c`, Kicked
`0d`, Banned `0e`, Unbanned `0f`, RoomPolicyChanged `10`, Bans `11`, and
Capabilities `12`, IdentifiedMessages `13`, IdentifiedMessageAccepted `14`, and
MessageWithdrawn `15`, MessageRedacted `16`, IdentifiedChanges `17`, ThreadRoots
`18`, ThreadReplies `19`, ReplyAccepted `1a`, ThreadedChanges `1b`,
ThreadArchived `1c`, ThreadReopened `1d`, ThreadStatus `1e`, ReactionStatus
`1f`, ReactionAdded `20`, ReactionRemoved `21`, MessageRevisions `22`, and
MessageEdited `23`, Topics `24`, TopicMessages `25`, TopicCreated `26`,
TopicMessageAccepted `27`, TopicChanges `28`, ReadPosition `29`, and
ReadPositionAdvanced `2a`, TopicRenamed `2b`, TopicResolved `2c`,
TopicReopened `2d`, TopicStatus `2e`, TopicSuperseded `2f`,
TopicSupersessionCleared `30`, and TopicSupersessionStatus `31`.
TopicAliasAdded is `32`, TopicAliasRemoved is `33`, and TopicAliasStatus is
`34`. TopicTagAdded is `35`, TopicTagRemoved is `36`, and TopicTagStatus is
`37`. AttachmentMessages, AttachmentMessageAccepted, and AttachmentChanges are
`38`, `39`, and `3a`; VellumArtifactMessages and
VellumArtifactMessageAccepted are `3b` and `3c`. PeerConversationCreated and
ConversationProfile are `3d` and `3e`. AttentionPreference and
AttentionPreferenceSet are `3f` and `40`. Pins, PinChanged, SavedReference,
SavedReferenceSet, and SavedReferences are `41` through `45`.
Polls, Poll, PollBallot, PollCreated, PollBallotChanged, and
PollLifecycleChanged are `46` through `4b`. Poll definitions carry bounded
immutable options and a canonical eligible cohort; public status and mutation
outcomes carry only aggregate totals. The private PollBallot outcome is
caller-bound and never names another participant.
TopicCollection is `4c`. It carries revision, room, the exact closed filter,
stable-ID-ordered complete current metadata entries, and an optional opaque
continuation. Lifecycle filter tags are Any `01`, Active `02`, and Resolved
`03`; the canonical tag list has at most eight strictly increasing nonempty
values. Each entry carries topic ID, current name, creator, resolved Boolean,
canonical aliases and tags, and an optional depth-one successor topic ID.
The capability outcome is a canonical strictly increasing
list of closed feature tags; it describes supported semantics and grants no
participant authority.

AdmissionControl, MessageIntervalChanged, and PostingRestrictionChanged are
`4d` through `4f`. MessageReports, MessageReportSubmitted, and
MessageReportReviewed are `50` through `52`. Admission status carries the
shared minimum interval plus only the bound observer's restriction Boolean.
Report lists are capped at 256 and strictly ordered by opaque report identity.
Each report carries stable target identity, original sequence and author,
current target availability, reporter, bounded reasons, opaque causations,
creation order, and the closed Pending/Dismissed/Escalated disposition. Report
content never appears in ordinary room-change feeds.

A room page contains revision, room-summary list, and optional cursor. Each
summary contains room ID, name, admission, observer role, member count, retained
message count, and next message sequence. A member page contains revision, room
ID, member list, and optional cursor. Each member contains participant ID,
display name, kind, and role. A message page contains revision, room ID, message
list, oldest retained sequence, next message sequence, and optional cursor. Each
message contains sequence, author, logical time, body, and addressed-participant
list.

An identified-message page has the same bounds and retention interval as the
legacy page, but each message begins with a 16-byte opaque message ID before its
sequence. IdentifiedMessageAccepted carries revision, the 16-byte ID, then
sequence. The ID is reference identity, not order or authority. These are new
tags; legacy message pages and acceptance outcomes remain byte-identical.

MessageWithdrawn carries revision, the same 16-byte message ID, then sequence.
Room-change event tag `0b` carries actor text, message ID, and sequence. The
retained message and any still-retained acceptance event use an empty body as a
reference-preserving tombstone; admitted bodies cannot be empty. This profile
does not claim physical erasure or removal from replay records and prior copies.

MessageRedacted carries revision, message ID, then sequence. Room-change event
tag `0c` carries actor text, author text, message ID, sequence, bounded reason,
and 16-byte opaque causation. The same empty-body tombstone and non-erasure
boundaries apply, while authorization follows the room role hierarchy.

A ban page contains revision, room ID, a bounded list, and optional cursor. Each
entry contains participant ID, display name, descriptive kind, and prior room
role. Service authority restricts this page to current owners and moderators;
the wire representation itself grants no authority.

Revision-only lifecycle outcomes carry one `u64`. MessageAccepted carries
revision then room-local sequence.

ChangesOpened carries the snapshot revision and a required opaque cursor.
Changes carries current revision, a counted list of `(event sequence, accepted
revision, room event)`, the next required cursor, and a closed `00`/`01`
has-more byte. Room event tags are RoomCreated `01` (owner, admission),
InvitationRecorded `02` (participant), ParticipantJoined `03` (participant),
ParticipantLeft `04` (participant), RoleChanged `05` (participant, role), and
MessageAccepted `06` (message), ParticipantKicked `07`, ParticipantBanned `08`,
ParticipantUnbanned `09`, and RoomPolicyChanged `0a` (actor, prior admission,
new admission). Each moderation event carries actor and subject
text, bounded reason text, and fixed 16-byte causation encoded as two big-endian
`u64` words. ChangeGap carries current revision and either legacy delivery-
Retention `01` plus acknowledged-through sequence, MembershipChanged `02`, or
independent ChangeRetention `03` plus discarded-through event sequence.
Unknown event, gap, or Boolean tags refuse.

OpenIdentifiedChanges reuses the ChangesOpened scalar result, but the service
binds its handle to a distinct cursor profile. IdentifiedChanges has the same
page envelope, ordering, bounds, and gap laws as Changes. Its closed nested
event vocabulary uses the same tags and fields except event `06`, which carries
an identified message beginning with its opaque 16-byte message ID. This new
outcome does not alter legacy Changes bytes or make the two cursor profiles
substitutable.

A threaded message is the identified-message grammar followed by a closed
optional root-message ID (`00` absent, `01` plus 16 bytes). ThreadRoots pages
require the page selector and every message relationship to be absent.
ThreadReplies pages require one page root and the same root on every message;
encoders and decoders reject inconsistent shapes. Both pages retain the normal
revision, room, retained interval, and optional continuation fields.
ReplyAccepted carries revision, reply ID, root ID, and sequence.

OpenThreadedChanges reuses ChangesOpened but receives a separately typed service
cursor. ThreadedChanges mirrors the identified closed event vocabulary; only
event `06` uses the threaded-message grammar. Cross-use with legacy or
identified feed cursors refuses. The relationship is reference metadata, not
authority: every operation still rechecks membership and room role rules.

ThreadArchived and ThreadReopened carry revision then the 16-byte root ID.
ThreadStatus carries revision, room text, root ID, and state Active `01` or
Archived `02`. Room-event tags `0d` and `0e` carry actor text, root ID, bounded
reason, and causation for archive and reopen respectively in all three change
profiles. Archive is reply-admission state, not deletion or completion.
ReactionStatus carries revision, room text, message ID, and a canonical bounded
list of reaction key, aggregate count, and caller-membership flag. Room-event
tag `0f` carries actor text, message ID, key, and add/remove flag in all three
change profiles. Counts do not enumerate reactors and reactions imply no
authority, delivery, acknowledgement, or completion.

MessageRevisions carries revision, room text, message ID, an ascending bounded
list of `(revision number, editor text, logical time, availability, body)`, and
an optional cursor. Availability is Available `01` with a nonempty body or
Tombstoned `02` with an empty body. MessageEdited carries engine revision,
message ID, and message-revision number. Room-event tag `10` carries actor text,
message ID, message-revision number, logical time, and body in all three change
profiles. Existing message pages continue to expose only the current body.

A topic page contains revision, room text, a canonical topic-ID-ordered list of
`(16-byte topic ID, bounded name, creator text)`, and an optional cursor. The
separate topic-status outcome contains revision, room, topic ID, and the current
resolved bool without changing that established page shape. A
topic-message page adds the exact topic selector and carries identified/threaded
message fields plus a closed optional topic ID; every returned message must
match the page topic. TopicCreated carries revision and topic ID.
TopicMessageAccepted carries revision, message ID, topic ID, and sequence.
TopicChanges uses the normal change envelope and event tags, with event `06`
using the topic-aware message grammar and event `11` carrying actor, topic ID,
and name. Event `12` carries actor, stable topic ID, prior name, and new name in
every change profile. Its service cursor cannot be substituted with any older
feed profile. TopicRenamed carries revision and the unchanged topic ID.

TopicSuperseded carries revision, source topic ID, and successor topic ID;
TopicSupersessionCleared carries revision and source topic ID. The separate
status outcome carries revision, room, source topic ID, and a closed optional
successor. Room-event tag `14` carries actor, source, prior optional successor,
and new optional successor in every change profile. The relation is navigation
metadata only: it neither rewrites retained topic relations nor reroutes posts.

TopicAliasAdded and TopicAliasRemoved carry revision, stable topic ID, and the
bounded alias. The separate status outcome carries revision, room, topic ID,
and at most 16 aliases in strict byte order. Room-event tag `15` carries actor,
stable topic ID, alias, and add/remove state in every change profile. Aliases
are display metadata only and never become wire selectors or redirects.

TopicTagAdded and TopicTagRemoved carry revision, stable topic ID, and the
bounded tag. The separate status outcome carries revision, room, topic ID, and
at most eight tags in strict byte order. Room-event tag `16` carries actor,
stable topic ID, tag, and add/remove state in every change profile. Tags are
display/index metadata only and never become selectors, hierarchy, authority,
routing, workflow state, or execution evidence.

A read scope is Room `01`, Thread `02` plus a 16-byte root message ID, or Topic
`03` plus a 16-byte topic ID. A read-position query carries room and scope. An
advance additionally carries the exact observed engine revision and a 16-byte
target message ID. ReadPosition carries optional marker `00`/`01`;
ReadPositionAdvanced carries one marker. A marker contains room, scope, target
message ID, room-local target sequence, and observed revision. The participant
is derived from the client binding and is never present on the wire.

## Refusals

| Tag | Refusal | Payload |
| --- | --- | --- |
| `01` | UnknownClient | — |
| `02` | UnknownParticipant | — |
| `03` | UnknownRoom | — |
| `04` | NotMember | — |
| `05` | NotAuthorized | — |
| `06` | Stale | actual revision `u64` |
| `07` | RequestConflict | — |
| `08` | ClientLimit | — |
| `09` | RequestLimit | — |
| `0a` | CursorLimit | — |
| `0b` | InvalidCursor | — |
| `0c` | InvalidInput | — |
| `0d` | Capacity | — |
| `0e` | Exhausted | — |
| `0f` | Unavailable | — |
| `10` | Banned | — |
| `11` | ThreadArchived | — |
| `12` | UnknownTopic | — |
| `13` | InvalidReadScope | — |
| `14` | InvalidReadTarget | — |
| `15` | ReadPositionRegression | — |
| `16` | TopicResolved | — |
| `17` | InvalidTopicSupersession | — |
| `18` | TopicMustBeResolved | — |
| `19` | TopicAliasLimit | — |
| `1a` | UnknownTopicAlias | — |
| `1b` | InvalidTopicTag | — |
| `1c` | DuplicateTopicTag | — |
| `1d` | TopicTagLimit | — |
| `1e` | UnknownTopicTag | — |
| `1f` | ImmutableConversationCohort | — |
| `20` | InvalidAttentionScope | — |
| `21` | InvalidPinChange | — |
| `22` | InvalidSaveChange | — |
| `23` | InvalidPoll | — |
| `24` | PollLimit | — |
| `25` | UnknownPoll | — |
| `26` | NotPollEligible | — |
| `27` | PollClosed | — |
| `28` | InvalidPollChange | — |
| `29` | PostingRestricted | — |
| `2a` | MessageThrottled | remaining interval milliseconds `u64` |
| `2b` | InvalidMessageReport | — |
| `2c` | UnknownMessageReport | — |
| `2d` | MessageReportLimit | — |
| `2e` | InvalidMessageReportChange | — |

## Closure and evidence

Encoding preflights the exact checked size and all ceilings before allocation.
Decoding bounds input, fields, text, and lists before reconstruction and requires
complete consumption. Unknown versions, roots, commands, outcomes, refusals,
admissions, roles, kinds, and optional tags refuse.

Canonical fixtures freeze CreateRoom, a room page with continuation, a message
page carrying retention bounds and addressing, a change page carrying its
stable event identity and next cursor, a moderation change preserving evidence,
an admission-policy change, and a ban page preserving its authority-relevant
prior role, a complete canonical capability inventory, an identified-message
page, an author-withdrawal change, a moderator-redaction change, and an
identity-bearing accepted-message change. Round-trip tests cover every command,
outcome, refusal, room-event kind, and gap cause. Thread reply-page and threaded
change fixtures additionally freeze the optional relationship grammar and page
shape. Thread lifecycle-change and status fixtures freeze explicit archive state
and evidence. Reaction change and status fixtures freeze the relation event and
bounded aggregate grammar. Topic commands, pages, receipts, feed messages,
unknown-topic refusal, and mismatched page relations have closed round-trip and
malformed-shape evidence; the expanded capability fixture freezes tag `0f`.
Private read-position commands, optional and present outcomes, all three scope
tags, and three specific refusals have closed round-trip evidence. The expanded
capability fixture freezes tag `10`, and the read-position reply fixture freezes
the complete private marker grammar. Capability tag `11` advertises independent
change retention without granting delivery authority; the frozen
change-retention-gap reply fixes nested gap tag `03` and its discard edge.
Capability tag `12` advertises `TopicRenameV1`; frozen rename request, receipt,
and topic-aware change fixtures pin command `2d`, outcome `2b`, and event `12`.
Capability tag `13` advertises `TopicResolutionV1`; frozen resolve request,
resolved receipt, and topic-aware lifecycle change fixtures pin commands `2e`
and `2f`, outcomes `2c` and `2d`, event `13`, and resolved-topic refusal `16`.
Capability tag `14` advertises `TopicSupersessionV1`; frozen supersede request,
receipt, and topic-aware change fixtures pin commands `31` through `33`,
outcomes `2f` through `31`, event `14`, and refusals `17` and `18`.
Capability tag `15` advertises `TopicAliasesV1`; frozen add request, added
receipt, and topic-aware change fixtures pin commands `34` through `36`,
outcomes `32` through `34`, event `15`, and refusals `19` and `1a`.
Capability tag `16` advertises `TopicTagsV1`; frozen add request, added receipt,
and topic-aware change fixtures pin commands `37` through `39`, outcomes `35`
through `37`, event `16`, and refusals `1b` through `1e`.
Capability tags `17` and `18` advertise immutable attachment and typed Vellum
artifact references; commands `3a` through `3f` and outcomes `38` through `3c`
are their separate history, admission, and attachment-change projections.
Capability tag `19` advertises `ConversationProfilesV1`. Profile scalars are
Ordinary `01`, Direct `02`, and GroupDirect `03`. Command tags `40`/`41`,
outcomes `3d`/`3e`, refusal `1f`, and room-event tag `17` close peer creation
and member-only profile observation. The peer event carries initiator,
nonordinary profile, and a strictly increasing founding cohort containing the
initiator in all four change projections. Frozen creation and status frames pin
the additive grammar without changing the legacy room-summary shape.
Capability tag `1a` advertises `PrivateAttentionPreferencesV1`. Capability tags
`1b` and `1c` independently advertise `SharedPinsV1` and
`PrivateSavedReferencesV1`. Stable-reference availability is Available `00`,
Tombstoned `01`, or Evicted `02`. Pin event tag `18` carries actor, message ID,
original sequence, and resulting Boolean in all four change projections. Pin
lists are capped at 256 and private save lists at 4,096 before allocation. The
frozen set-pin and saved-reference-list frames pin the additive grammar without
changing older frames. Capability tag `1d` advertises `BoundedPollsV1`.
Commands `49` through `4e`, outcomes `46` through `4b`, refusals `23` through
`28`, and room-event tags `19` through `1b` close public definitions and
aggregates, caller-private ballots, and explicit lifecycle in every feed
profile. Poll creation and aggregate-reply fixtures freeze both definition and
aggregate grammar.
Capability tag `1e` advertises `TopicCollectionsV1`. Command `4f` and outcome
`4c` are additive and their frozen request/reply frames pin the filter and full
metadata grammar. They add no mutation, refusal, event, or message-relation
encoding.
Capability tag `1f` advertises `AdmissionModerationControlsV1`. Commands `50`
through `56`, outcomes `4d` through `52`, refusals `29` through `2e`, and room
event tags `1c` and `1d` are additive. Trusted admission time is absent from
the frame: the authenticated Host supplies it out of band only when a nonzero
interval is active. Frozen restriction-request and private-report-list frames
pin the new public grammar without changing any prior fixture.
Truncation, trailing data,
version mismatch, unknown nested tags, and resource ceilings are adversarially
exercised. These tests establish a frame codec, not transport security, push
delivery, or an authenticated host.
