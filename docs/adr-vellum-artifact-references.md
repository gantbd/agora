# ADR: typed Vellum artifact references

Status: accepted; core and canonical engine recovery implemented

Date: 2026-11-11

## Context

Agora owns short conversation messages. Vellum owns revisioned documents,
editing, source history, persistence, and the authority needed to read or
change a document. A room nevertheless needs a durable way to say that an
accepted message discussed one exact Vellum document revision.

The attachment profile is the wrong vocabulary for this relation. A Vellum
document has a restart-stable workspace identity, a workspace-scoped document
identity, and a historical source revision. It is not an immutable blob, and
its live `VersionWitness` is deliberately process/client evidence rather than
a durable edit capability. Treating a document as an object-store key would
discard that distinction and invite callers to mistake a retained Agora value
for Vellum authority.

## Decision

Add a separately advertised `VellumArtifactReferencesV1` conversation profile.
The profile is additive: existing message commands, projections, feeds, wire
tags, and checkpoints remain byte-identical.

One artifact-bearing message uses the same closed placement choices as other
messages: room root, one-level reply, or stable topic. It atomically admits at
most eight ordered references. Each reference contains only:

- an opaque 128-bit room-local relation identity;
- a nonzero Vellum `WorkspaceIdV1` value;
- a nonzero workspace-local `WorkspaceDocumentIdV1` value; and
- the exact historical Vellum `source_revision` discussed by the message.

Agora defines its own closed DTO with those scalar fields and has no crate or
runtime dependency on Vellum. The values must round-trip exactly to Vellum's
public protocol vocabulary. A composing adapter is responsible for that typed
conversion and for rejecting unsupported Vellum protocol versions.

The relation identity and complete Vellum target must each be unique within one
accepted message. Caller order is presentation order only. The relation is
immutable: editing a message body cannot retarget it or silently advance it to
the latest Vellum revision. A later document revision requires a new Agora
message or a separately decided attributable relation lifecycle.

Admission performs no Vellum call. Agora does not prove that the workspace,
document, or revision exists, remains retained, is visible to the participant,
or has the content the participant expected. A product that resolves a
reference must authenticate to Vellum, derive current authority there, and
request the exact named revision. Possession of any retained Agora identifier
is neither read nor edit authority.

Author withdrawal and moderator redaction retain only the room-local relation
identity and original ordering slot. They scrub workspace, document, and source
revision from current state and every retained event copy. Retention eviction
removes the relation with the message. Neither operation mutates or deletes a
Vellum document.

Checkpoint recovery retains available references, tombstones, artifact-history
cursors, and exact request receipts. Durable Agora publication commits these as
one Agora unit before success is visible. It does not create a transaction with
Vellum. Message-to-document promotion or any cross-engine mutation remains a
Moira/product-coordinated operation with explicit partial and unknown outcomes.

## Consequences

- Agora can preserve an attributable discussion link to an exact document
  revision without becoming a writing engine.
- Vellum remains the sole source of document content, revision availability,
  editing rules, persistence, and document authority.
- Human and agent participants use the same Agora relation and authorization
  laws. The reference does not activate an agent or admit work.
- A client may separately offer “open exact revision” and “open current
  document” actions, but must not conflate their consistency claims.
- Search V1 continues to index message bodies only. Artifact metadata and
  Vellum content require a separately authorized derived-search profile.

## Rejected alternatives

- Reusing `AttachmentDescriptorV1` misstates a mutable revisioned document as
  an immutable object and imports store/key/digest vocabulary that Vellum does
  not use for document identity.
- Storing a filesystem path, URL, MCP handle, or bearer locator leaks topology
  and may retain authority in conversation history.
- Retaining Vellum `VersionWitness` would persist a live, client-scoped witness
  outside the lifecycle in which Vellum defines it.
- A movable “latest revision” reference makes old Agora history change meaning
  without an Agora event.
- Importing Vellum crates into the engine would couple release graphs and make
  a cross-engine adapter look like shared semantic ownership.
- A generic external-reference map evades closed vocabulary, bounds,
  compatibility fixtures, privacy review, and protocol-version handling.

## Required evidence

- Human/agent-neutral room, reply, and topic admission with exact order,
  uniqueness, bounds, inherited topology, and pre-mutation atomic refusal.
- Message edits preserve references; withdrawal and redaction scrub every
  current and retained copy while keeping identity/count tombstones.
- Canonical engine, service, and Host recovery plus exact request replay,
  receipt recovery, cursor isolation, and retention cleanup.
- Closed protocol/wire round trips, malformed identity refusal, frozen frames,
  client uncertainty, SDK construction, authenticated Host binding, and
  private-local routing.
- A separately owned adapter test proving exact conversion against Vellum's
  public workspace/document/revision types without granting authority.
- Content-safe Debug and passive observations that disclose only counts and
  lifecycle classes.

## Nonclaims

This decision does not establish document existence, revision retention,
content integrity, currentness, Vellum availability, read or edit authority,
cross-engine authentication, atomic promotion, coordinated recovery, search,
previews, connector parity, notifications, agent activation, task admission,
execution, or completion.
