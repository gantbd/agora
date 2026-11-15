# ADR: federation identity namespaces and SPIFFE boundary

Status: accepted cross-engine identity architecture

Date: 2026-11-15

## Decision

Every federation identity is qualified by an application namespace, an
authoritative home, and a resource kind before any home-local bytes are
interpreted.

| Engine | Application namespace | Stable home type | Examples of complete resource identity |
| --- | --- | --- | --- |
| Agora | `gantbd.agora.federation/v1` | `FederationServerIdV1` | `(namespace, room/participant, home, local ID)` |
| Chronos | `gantbd.chronos.federation/v1` | `ChronosHomeIdV1` | `(namespace, task, home, local ID, incarnation[, attempt])` |
| Vellum | `gantbd.vellum.federation/v1` | `VellumHomeIdV1` | `(namespace, workspace/document/actor, home, local ID)` |

The distinct protocol types, canonical frame magic, and closed decoders carry
the application namespace inside each typed protocol. Any generic registry,
policy input, database key, audit envelope, metric label, or cross-engine tool
must carry the explicit application-namespace label and resource kind; it must
not flatten an identity to its raw home or local bytes.

Equal bytes across engines, homes, resource kinds, task incarnations, attempts,
workspaces, or operation profiles never identify the same entity. A source
revision is a state coordinate under a Vellum document, not a global ID. A
Chronos grant or lease is authority, not identity. Display names, DNS names,
certificate subjects, URLs, transport addresses, and content digests are not
application identities.

Request IDs are scoped replay keys rather than global entity identities. Their
effective namespace includes the application namespace, operation profile,
authenticated sender, authoritative target, and complete semantic binding.
Changed input under the same effective request identity conflicts.

## SPIFFE boundary

SPIFFE is a good optional fit for workload authentication and cross-trust-domain
credential validation. A SPIFFE ID is an RFC 3986 URI consisting of a trust
domain and workload path. An SVID proves that workload identity under the
issuing trust domain's bundle. SPIFFE Federation distributes the foreign bundles
needed to authenticate SVIDs across trust domains.

SPIFFE does not define Agora rooms or participants, Chronos tasks or attempts,
Vellum workspaces or documents, request replay identity, or application
authorization. It therefore supplements rather than replaces the application
namespace above.

Each engine may add a SPIFFE verifier only as a federation trust adapter outside
the engine:

```text
(complete SPIFFE ID, exact trust-domain bundle, application namespace)
    -> (stable federation home, current trust epoch, allowed peer profiles)
```

The verifier must preserve the configured trust-domain-to-bundle association
and match the complete SPIFFE ID, not only its path. SPIFFE trust-domain names
are operator-selected and are not guaranteed globally unique. Bundles from
different trust domains must remain distinct. SVID or signing-key rotation must
not silently rotate the stable application home.

SPIFFE identity is transport/workload provenance. The ordinary current Host
authority remains responsible for every room, task, document, disclosure,
edit, scheduling, and persistence decision. Humans and agents retain
application-owned participant/principal/actor identities and are not modeled
as SPIFFE workloads merely because they use a federated service.

## Placement

- `agora-federation`, `chronos-federation`, and `vellum-federation` own these
  application namespaces and trust mappings outside their engines.
- SPIFFE parsing, SVID validation, Workload API integration, and bundle refresh
  belong in separately classified trust adapters.
- Protocol, service, and engine layers receive only verified typed home
  identity plus current trust evidence; they do not fetch SVIDs or bundles.
- Agent harness behavior remains outside Agora and Vellum; task orchestration
  remains Chronos-owned.

## Sources

The executable namespace gates pass at Chronos federation revision
`3e2de542b78066f12512b39a26590ca5315e6428`, Vellum federation revision
`6711be318dc0774db2716d28b12db3b7a13f9a7b`, and Agora federation source
revision `b10c63168c28d51c021804ab22734efcfc4e076a`, recorded by
`e274fcb771f813a01382da1526a1c5021d10ce0a`.

- [SPIFFE Identity and Verifiable Identity Document](https://github.com/spiffe/spiffe/blob/main/standards/SPIFFE-ID.md)
- [SPIFFE Trust Domain and Bundle](https://github.com/spiffe/spiffe/blob/main/standards/SPIFFE_Trust_Domain_and_Bundle.md)
- [SPIFFE Federation](https://github.com/spiffe/spiffe/blob/main/standards/SPIFFE_Federation.md)
