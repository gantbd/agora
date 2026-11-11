# Supply-chain profile

Status: conservative local-source inventory; not a release SBOM or legal review.

The current stable locked graph contains sixteen components: twelve Agora
packages, two Aegis packages, and the sibling `keystone-codec` and
`keystone-storage` path packages. It contains no registry or Git source and no
Cargo checksum records. Every stable-workspace manifest dependency uses an
exact version and a local path.

`make check-supply-chain` compares `Cargo.lock` with the exact component
inventory in `supply-chain-baseline.txt`, rejects source/checksum records and
non-path or non-exact manifest dependency declarations, and exercises missing,
changed-version, and empty-inventory mutants.

The opt-in standalone `fuzz/` workspace is deliberately outside that release
graph. Its separate lock contains the registry-sourced libFuzzer build tooling;
the fuzz contract pins the driver, nightly compiler, and `libfuzzer-sys`
versions. Those tools produce test evidence only and are not linked into any
Agora package or included in the stable local-only supply claim.

The source-candidate profile copies exactly those sixteen stable components into
one local directory. Agora bytes come from an exact clean tracked revision;
selected Aegis and Keystone bytes are individually checksummed and their subtree
cleanliness is stated rather than inferred from the contextual repository
revision. A reduced reviewed Keystone workspace manifest supplies only inherited
metadata for the two selected packages. Isolated locked/offline build evidence
establishes content closure, not origin, advisory, or legal provenance.

The qualified candidate for Agora revision
`e5a02b4ce5bda9c92542b0e5ea59ea3a746b295b` was reproduced byte-for-byte,
closed over 233 payload files, and passed the complete stable locked/offline
workspace build from an isolated target. Both sibling selections were recorded
unclean; the content hashes are evidence, while their contextual Git revisions
are not promoted into provenance.

This evidence does not establish remote availability of Aegis or Keystone, registry
packaging, transitive source integrity outside the local checkouts, advisory
status, license compatibility, binary contents, an SPDX/CycloneDX SBOM, origin
provenance, signatures, or reproducibility from a clean Cargo home. Those remain
release blockers.
