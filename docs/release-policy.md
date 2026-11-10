# Release and distribution policy

Status: coordinated local source distribution only; no registry publication is
authorized.

## Current contract

- Every Agora workspace package has `publish = false`.
- Agora may be cloned, built, and tested from source under the repository's MIT
  or Apache-2.0 licenses.
- The current checkout requires a sibling `keystone` checkout at the relative
  path used by `agora-wire`. This is a source-layout dependency, not evidence
  that a lone source archive, Git URL, or registry package builds.
- No remote, tag, commit availability, signed source bundle, binary artifact,
  installer, or support lifetime is qualified.
- Presentation applications, connectors, agent harnesses, and sibling engines
  are outside this release unit.

## Offline gates

`make check-release-contract` verifies that every current package denies
publication, the exact Rust toolchain is pinned, required legal/security/claim
documents are present, and the sibling Keystone assumption remains explicit.
An isolated copied source-contract fixture must pass, then mutants enabling root
publication, replacing the pinned toolchain with `stable`, and removing the
security policy must all fail.

`make check-baseline` closes over the exact package and canonical-fixture
inventories. `make check-supply-chain` closes over the current eleven locked local
components and refuses source/checksum registry records or dependency edges that
do not use an exact version plus a local path. These gates detect reviewed source
contract drift. They do not perform an advisory lookup, legal conclusion,
license compatibility analysis, binary SBOM, remote reproducibility proof, or
artifact attestation.

Full `make check` additionally runs dependency/effect boundaries, formatting,
warnings-denied lint and documentation, and the complete locked offline test
suite.

## Enabling a distribution channel later

Publication or artifact distribution requires a deliberate change that:

1. selects the packages, audience, channel, platforms, and support policy;
2. replaces or qualifies every cross-repository path dependency for that
   channel;
3. verifies exact package or artifact contents from clean immutable source;
4. records dependency, advisory, license, checksum, provenance, and SBOM
   evidence;
5. qualifies installation, upgrade, rollback, and removal separately from
   artifact construction;
6. updates the changelog, security policy, release-readiness ledger, and this
   policy in the same reviewed change; and
7. changes the fail-closed gates rather than bypassing them.

Until then, passing the source gates authorizes no registry publication or
production deployment claim.
