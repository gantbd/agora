# Supply-chain profile

Status: conservative local-source inventory; not a release SBOM or legal review.

The current locked graph contains eleven components: nine Agora packages and the
sibling `keystone-codec` and `keystone-storage` path packages. It contains no
registry or Git source and no Cargo checksum records. Every manifest dependency
uses an exact version and a local path.

`make check-supply-chain` compares `Cargo.lock` with the exact component
inventory in `supply-chain-baseline.txt`, rejects source/checksum records and
non-path or non-exact manifest dependency declarations, and exercises missing,
changed-version, and empty-inventory mutants.

This evidence does not establish remote availability of Keystone, registry
packaging, transitive source integrity outside the local checkouts, advisory
status, license compatibility, binary contents, an SPDX/CycloneDX SBOM,
provenance, signatures, or reproducibility from a clean Cargo home. Those remain
release blockers.
