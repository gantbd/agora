# ADR: typed interaction adapter boundary

Status: accepted; Agora boundary implemented; separate adapters open

Date: 2026-11-14

## Context

Chat products expose commands, buttons, cards, forms, and application actions.
Those shapes can improve participant interaction, but an unrestricted extension
bag or server plugin runtime would import callback execution, network access,
credentials, agent behavior, and product-specific schemas into Agora. Humans
and agents must remain ordinary participants under the same room authority and
message-admission laws. Addressing or mentioning a participant is not work
admission or execution proof.

## Decision

R104 uses separately owned, schema-specific interaction adapters. Agora defines
only an effect-free participant-bound interaction context and an explicit output
classification. It does not register plugins, load code, interpret arbitrary
schemas, execute callbacks, select models or tools, activate agents, or own
external work.

An adapter invocation is bound to one authenticated participant, one current
Agora session/client, one retained visible source message, one adapter-owned
nonzero invocation identity, and one adapter-owned closed schema version. The
adapter cannot assert another participant or use an addressed participant as
authority. Current membership, source visibility, and Aegis invocation
authority are rechecked before external behavior and again before any proposed
Agora admission.

Schema-specific request and response values remain in the separate adapter.
Agora carries no unbounded property bag, callback URL, executable name, prompt,
tool description, credential, provider response, or opaque payload whose
meaning the engine cannot validate. New interaction schemas require new typed
adapter contracts and qualification; a schema identifier alone never makes
arbitrary bytes safe.

The output boundary is closed:

- an ephemeral result is session-scoped adapter state, is absent after restart,
  and never changes room history, revision, events, read state, delivery state,
  attention, tasks, or workflow facts;
- an admission proposal is not yet a message. Promotion must traverse the
  ordinary trusted participant-bound Agora admission path, derive the author
  from the current authenticated session, recheck current admission controls at
  trusted Host time, and produce an ordinary message or ordinary refusal;
- an external-work reference may be displayed ephemerally or admitted as an
  explicitly typed reference under a later profile, but Agora never treats it
  as task admission, progress, completion, or execution evidence.

Adapters own exact invocation replay, external transport certainty, schema
validation, and durable reconciliation. Definitely-not-sent may retry the same
identity. Outcome-unknown requires schema/endpoint-specific receipt evidence;
changed reuse conflicts. Aegis invocation, inspection, cancellation, and
promotion authority are distinct. Chronos owns scheduling, orchestration engines
own agent/work behavior, Hermes owns generic delivery, and Vellum remains the
writing engine for separately admitted document artifacts.

## Required evidence

- human/agent symmetry and authenticated participant/source binding;
- closed schema-specific request values with bounded canonical validation;
- ephemeral results proven domain-neutral and restart-empty;
- explicit promotion through ordinary admission with current authority/time;
- exact replay, conflict, definitely-not-sent, outcome-unknown, and recovery;
- Aegis action separation and content-safe diagnostics;
- dependency/effect boundaries, SDK taxonomy, corruption/model evidence, and
  commit-pinned qualification in the separate adapter.

## Implementation status

Agora now provides the zero-dependency `agora-interaction-port` plus Service,
Host, durable Host, and embedded-SDK surfaces. Context lookup derives the
participant from the authenticated binding, exposes only a retained visible
message event sequence, distinguishes retention gaps from invisibility and
unavailability, and is domain-neutral. Promotion derives the author from that
same binding and traverses ordinary publication admission at current Host time.
An exact retained publication replays even after the source event is evicted;
changed reuse conflicts. Commit uncertainty is reconciled through the durable
Host before a result is returned.

The separate `agora-interaction-adapter` implements and qualifies an initial
closed acknowledgement schema. Its strong-receipt journal, restart-empty
ephemeral cache, exact promotion replay, distinct Aegis actions, frozen
checkpoint, supply inventory, and mutation closure are pinned at revisions
`6c7429f` and `da40c14`. Additional schemas and any external-provider transport
remain separately qualified future profiles; the schema identifier is not an
extension escape hatch.

## Nonclaims

R104 does not provide a plugin runtime, arbitrary commands or cards, callback
hosting, agent harnesses, activation, prompts, model/tool selection, task or
workflow state, execution attestation, provider delivery, remote transport,
federation, or guaranteed side effects.
