# Agora project boundaries

Status: exact current-graph classification adopted for Horizon 0.

Agora follows the shared `COMPONENT_ARCHITECTURE.md`. Package names express
semantic contracts; directory categories express ownership and dependency
intent. Neither name nor placement grants authority.

`package-classification.json` is the machine-readable inventory for every Agora
production workspace package. The classification gate checks exact package and
path closure, the closed category/maturity/effect/audience vocabularies, and the
unpublished policy. It refuses missing, stale, duplicate, malformed, or unknown
entries and requires every path to occupy its declared category directory.

## Current classification

| Category | Packages | Ownership |
| --- | --- | --- |
| Engine | `agora-engine` | Pure authoritative conversation state and transitions |
| Protocol | `agora-protocol`, `agora-wire` | Closed values and canonical bounded frames |
| Client | `agora-client` | Transport-neutral request identity and uncertainty |
| Service | `agora-service`, `agora-durable` | Serialization, client state, replay, persistence gating and recovery |
| Host | `agora-host` | Verified principal/context binding and external ports |
| Port | `agora-delivery-projection-port`, `agora-federated-publication-port`, `agora-interaction-port`, `agora-scheduled-publication-port`, `agora-search-port`, `agora-storage-observation` | Effect-free outbound and trusted integration contracts |
| Adapter | `agora-aegis-verifier`, `agora-search-memory`, `agora-storage-directory` | Concrete or reference interpretations of ports |
| Transport | `agora-local-transport` | Authority-free Unix endpoint, stream, framing, lifetime, and entropy mechanics |
| Application | `agora-local-daemon` | Aegis/Host/durability composition, routing, clocks, recovery policy, lifecycle, and observation |
| SDK | `agora-sdk-embedded`, `agora-sdk-client` | Zero-authority audience facades |

The packages occupy explicit engine, protocol, client, service, host, port,
adapter, transport, application, and SDK directories. The local transport owns
only authority-free Unix resource mechanics; the daemon application selects and
coordinates semantic owners and policy-bearing adapters. Their dependency and
effect closures are checked independently.

## Dependency laws

- Engine and protocol retain zero normal runtime dependencies.
- Client does not depend on engine, service, Host, adapters, or applications.
- Protocol and wire do not depend upward on client, service, Host, adapters, or
  applications.
- Service depends on semantic contracts and effect-free ports, never concrete
  search/storage adapters.
- Host does not import a peer engine implementation or a concrete verifier.
- Adapters interpret named ports and gain no conversation authority.
- Transports own connection/resource mechanics and import no authority, domain,
  Host, service, durability, client, or storage implementation.
- Applications select adapters, limits, clocks, recovery policy, and lifecycle.
- SDK facades reexport an exact audience closure and add no behavior.

`scripts/check-boundaries.sh` checks dependency/effect laws. The separate
`scripts/check-package-classification.mjs` closes the package inventory and
classification. Both are required because a legal dependency graph can still
hide a category or ownership mistake.

## Maturity vocabulary

- `qualified`: the package participates in the current full Agora qualification
  for its explicitly bounded profile.
- `adopted`: its contract is selected but not fully qualified across the whole
  advertised surface.
- `reference`: it is a bounded semantic reference or test mechanism and makes no
  production-mechanism claim.

Maturity applies to the package's stated profile, not to every possible
deployment. For example, the qualified local daemon does not establish remote
transport, concurrency, federation, or hardware power-loss behavior.
