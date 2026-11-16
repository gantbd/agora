# ADR: Private-local production identity and time inputs

Status: accepted; durable owner and drain receipt implemented, standalone entrypoint open

Date: 2026-11-16

## Context

Agora's local daemon already owns a hardened mode-0600 Unix socket beneath a
mode-0700 parent, pins the endpoint and lifetime record, and explicitly trusts
the private parent plus same-UID processes. Its Host still requires an
`AegisContextV1` for every connection and non-regressing trusted time for every
authorization and admission. Qualification previously supplied test-only
implementations, leaving the smallest production deployment underspecified.

Workbench must not invent participant identity, verified subjects, Aegis
sessions or grants, clock semantics, initial conversation state, or durable
recovery policy. Those are Agora/deployment inputs even when Workbench owns the
process lifecycle.

## Decision

The first production profile is deliberately single-principal and private
local. `PrivateEndpointAuthenticatorV1` binds every connection admitted through
the already qualified private endpoint to one deployment-selected
`AegisContextV1`. The context contains the exact participant, opaque verified
subject, public Aegis service session, and session-local grant. The
authenticator issues none of those values and performs no policy lookup.

The endpoint protection is the credential mechanism for this profile. The
authenticator does not claim kernel account discovery, multiple mutually
untrusted same-UID clients, user switching, external identity, remote peers, or
credential renewal. A multi-user profile requires a separately qualified peer
credential adapter and does not weaken or reinterpret this one.

`SystemHostClockV1` starts from Unix-epoch milliseconds and clamps later reads
to a non-regressing process-local sequence. Startup before the Unix epoch
refuses. Later wall-clock rollback or read failure retains the last value. The
clock supplies authorization/admission time only; it does not schedule work,
establish timestamp accuracy, or provide a durable cross-restart monotonicity
claim.

## Composition law

An Agora-owned or deployment-owned process factory may combine these inputs
with the public Aegis Unix client, an explicitly selected grant/resource,
`DurableHostV2`, the directory checkpoint store, and one local daemon profile.
Workbench may start and stop that factory and retain its public client, but may
not construct the context, grant, initial engine state, or recovery choice.

The same factory and public client must remain usable without Workbench. A
single endpoint selects one exact handshake profile; ordinary participant and
trusted scheduled-composition profiles are not silently multiplexed.

`agora-private-local` is the Agora-owned standalone application root for these
deployment choices. Its first adopted boundary is `BootstrapV1`: one explicitly
selected principal, a bounded participant list, and a bounded initial room
list are applied to a fresh engine before any durable owner or endpoint can be
published. Engine validation remains authoritative; a refusal identifies the
exact participant or room step and returns no partially initialized engine.
The same application now connects through the extracted public
`aegis-local-client`, refuses a participant different from the bootstrap
principal before socket I/O, and invokes a deployment-owned
`SessionGrantSelectorV1` only after Aegis selects the live service session. The
selector provisions or chooses the grant on that exact connection and receives
the same resource later bound to the verifier. `IssueAgoraGrantV1` is the first
concrete deployment profile: it requires an administrator-mapped Aegis
connection, explicit current epoch and bounded lifetime, and issues exactly one
nondelegated scope. The conversation profile grants only `Submit`; scheduled
composition adds only `Execute` and `Observe`. Expired input refuses before
authority I/O, and every returned grant field is checked before publication.
The caller still explicitly supplies the resource and opaque verified subject;
credential mapping, epoch discovery, and clock/lifetime policy remain deployment
inputs. The dependency gate rejects Aegis engine, Host, service, daemon, or
credential-owner imports.

`start_durable_composition_v1` closes the owner-factory boundary. One explicit
plan selects create, clean open, or exact stale-endpoint recovery. Create binds
the connected authority participant to the bootstrap principal, builds the
bounded engine, creates and commits the directory-backed durable Host, and only
then publishes the endpoint. Open recovers the same durable Host before binding
a new endpoint. Residue recovery additionally requires the exact prior and
distinct next server incarnations and delegates the locked lifetime rotation to
the qualified local transport. All three modes use the production
non-regressing clock and the same public Aegis connection; paths and policy
remain explicit deployment inputs. The application boundary may import Agora's
service, durability, directory-store, and local-daemon compositions, while
still rejecting peer engines, Workbench, Aegis internals, Agora wire, and raw
transport mechanics.

`serve_durable_process_v1` is the shared owner loop for standalone and
Workbench-hosted execution. An explicit atomic shutdown request wakes a blocked
accept through the owned endpoint, the owner completes that admitted wakeup,
irreversibly enters `Draining`, and consumes the daemon into its exact stopped
health and cleanup receipt. Per-connection authentication, handshake, Host,
framing, routing, and request refusals do not terminate the owner; ownership,
transport, durability, fencing, capacity, and invalid lifecycle failures return
a closed terminal class plus the cleanup receipt. The shutdown request is
idempotent and supplies no authority or conversation semantics.

## Remaining work

- add the standalone configuration/entrypoint adapter that maps OS shutdown to
  the shared explicit drain request;
- qualify that exact entrypoint standalone and under Workbench lifecycle.

These items are required before claiming a production standalone process.
