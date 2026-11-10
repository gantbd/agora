# SDK taxonomy

Status: qualified for the unpublished V0.1 distribution facades.

The shared component architecture defines SDK as an audience or distribution
label, not an architectural layer. Agora therefore publishes no generic
`agora-sdk` umbrella. Each facade names one audience and reexports existing
contracts without adding behavior or authority.

## Trusted-host distribution

`agora-sdk-embedded` is for an application that owns the in-process composition
root.

```text
agora-sdk-embedded
    +-- agora-engine
    +-- agora-service
    `-- agora-host
```

The three exports are sufficient to construct the bounded engine, its serialized
volatile service, and the authenticated host. The application must still provide
the trusted `PrincipalVerifier`, explicit host time, transport-bound context,
transport framing/lifecycle, persistence, and runtime policy. The facade does not
select adapters and does not authenticate credentials itself.
The same three-package closure exposes the separately typed V2 service and host
modules without adding another dependency or authority source.

## Remote-client distribution

`agora-sdk-client` is for a peer that accesses Agora through client and protocol
contracts.

```text
agora-sdk-client
    +-- agora-client
    +-- agora-protocol
    `-- agora-wire
```

The facade contains a transport-neutral client, closed DTOs, and the canonical
bounded wire codec. It supplies no socket or stream transport, connection
lifecycle, credentials, retry loop, service discovery, or engine access. A
Connect participant ID remains an unauthenticated claim until a host verifies it.
The same dependency closure exposes the separately typed V2 client, protocol,
and wire modules, including the process-local exact-request uncertainty fence;
it does not imply that a V2 authenticated host, durable client journal, or
transport is available.

## Laws and evidence

- Neither facade defines domain types, commands, adapters, or runtime behavior.
- The embedded facade cannot depend on the remote-client facade.
- The remote-client facade cannot depend on engine, service, or host.
- Both facades remain `publish = false` until release provenance is approved.
- Facade-only tests prove that each advertised surface is usable without direct
  dependencies on the crates it groups.
- The boundary gate fixes each direct dependency closure and the workspace gate
  applies format, warning-denied lint, tests, doctests, and warning-denied docs.

These claims cover package composition only. They do not establish a durable
service, network security, credential verification, transport interoperability,
or crates.io readiness.
