# Release and distribution policy

Status: coordinated local source distribution only; no registry publication is
authorized.

## Current contract

- Every Agora workspace package has `publish = false`.
- Agora may be cloned, built, and tested from source under the repository's MIT
  or Apache-2.0 licenses.
- The development checkout requires sibling `aegis` and `keystone` checkouts at
  the reviewed relative paths. This is a source-layout dependency, not remote or
  commit provenance.
- The opt-in source-candidate builder stages the exact tracked Agora revision and
  only the four sibling packages in the stable locked graph under the same
  relative layout. It records sibling revisions as context, reports selected
  subtree cleanliness separately, and closes every staged byte with SHA-256.
  Qualification is locked and offline from an isolated target directory.
- A verified candidate may be encoded as the canonical bounded USTAR profile
  with an adjacent SHA-256 record. Archive qualification verifies before
  extraction, checks extracted candidate closure, requires an exact rearchive,
  and repeats the locked/offline isolated source qualification.
- No remote, tag, commit availability, signed source bundle, binary artifact,
  installer, or support lifetime is qualified.
- Presentation applications, connectors, agent harnesses, and sibling engines
  are outside this release unit.

## Offline gates

`make check-release-contract` verifies that every current package denies
publication, the exact Rust toolchain is pinned, required legal/security/claim
documents are present, and the sibling Aegis/Keystone assumptions remain explicit.
An isolated copied source-contract fixture must pass, then mutants enabling root
publication, replacing the pinned toolchain with `stable`, and removing the
security policy must all fail.

`make check-baseline` closes over the exact package and canonical-fixture
inventories. `make check-supply-chain` closes over the current twenty locked local
components and refuses source/checksum registry records or dependency edges that
do not use an exact version plus a local path. These gates detect reviewed source
contract drift. They do not perform an advisory lookup, legal conclusion,
license compatibility analysis, binary SBOM, remote reproducibility proof, or
artifact attestation.

Full `make check` additionally runs dependency/effect boundaries, formatting,
warnings-denied lint and documentation, and the complete locked offline test
suite.

`make check-source-candidate-contract` pins the construction schema, reduced
Keystone workspace input, checksum closure, and fail-closed verifier behavior.
Actual candidate construction and isolated qualification are opt-in because
they copy sibling inputs and rebuild the complete stable workspace. They do not
authorize distributing the resulting directory.

Revision `cd5ab9386bde597e83d58a101feef01871cb78b9` has the current qualified
local candidate/archive profile: two 412-payload candidate constructions and
their 3,537,408-byte, 499-entry canonical archives were byte-identical; both
sibling selections were explicitly unclean; one direct candidate and one
archive-extracted candidate passed the stable locked/offline isolated build.
The archive at SHA-256
`079b986e2c84a33923d4f80be1781987ea60326ea6f34cd96616df9984777e5b`
also passed validation, platform extraction, exact rearchive, and isolated
qualification. This does not establish remote origin or authorize a
distribution channel.

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
