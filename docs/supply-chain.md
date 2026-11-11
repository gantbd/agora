# Supply-chain profile

Status: deterministic local-source inventory and source SBOM; not a binary SBOM
or legal review.

The current stable locked graph contains seventeen components: thirteen Agora
packages, two Aegis packages, and the sibling `keystone-codec` and
`keystone-storage` path packages. It contains no registry or Git source and no
Cargo checksum records. Every stable-workspace manifest dependency uses an
exact version and a local path.

`make check-supply-chain` compares `Cargo.lock` with the exact component
inventory in `supply-chain-baseline.txt`, rejects source/checksum records and
non-path or non-exact manifest dependency declarations, and exercises missing,
changed-version, and empty-inventory mutants.

The same gate regenerates `docs/agora-source.cdx.json` from locked offline Cargo
metadata and requires byte equality. This canonical CycloneDX 1.5 document
contains exactly the seventeen stable local-path components, their closed Cargo
dependency edges, exact versions, declared `MIT OR Apache-2.0` expressions, and
denied publication state. Its SHA-256 digest is
`8b4ec0469c8a392b35e7f4c23acd114fcd43cd2fd05ab0111a78d06a13669b21`.
The validator rejects vocabulary drift, noncanonical order, missing or duplicate
components and edges, unknown references, host paths, altered licenses, and
publication-enabled records; focused mutants exercise those boundaries.

The opt-in standalone `fuzz/` workspace is deliberately outside that release
graph. Its separate lock contains the registry-sourced libFuzzer build tooling;
the fuzz contract pins the driver, nightly compiler, and `libfuzzer-sys`
versions. Those tools produce test evidence only and are not linked into any
Agora package or included in the stable local-only supply claim.

The source-candidate profile copies exactly those seventeen stable components into
one local directory. Agora bytes come from an exact clean tracked revision;
selected Aegis and Keystone bytes are individually checksummed and their subtree
cleanliness is stated rather than inferred from the contextual repository
revision. A reduced reviewed Keystone workspace manifest supplies only inherited
metadata for the two selected packages. Isolated locked/offline build evidence
establishes content closure, not origin, advisory, or legal provenance.

The qualified candidate for Agora revision
`46098fb3cd1a6f9164365d8c49cf7edbc0957ef7` was reproduced byte-for-byte,
closed over 291 payload files including the canonical source SBOM, archive tooling,
and storage-observation port, and passed
the complete stable locked/offline workspace build from an isolated target.
Its manifest digest is
`21aae72df2634c215fc1075161d51e4046a66c5f06e341ab73e623cc15ad521f` and
its checksum-inventory digest is
`3c5c1781b9592a92e700e8023b46f6096a547205328adbdfb9de1e087679c8f3`.
Both sibling selections were recorded unclean; the content hashes are evidence,
while their contextual Git revisions are not promoted into provenance.
Two canonical archives over those independent candidates were byte-identical at
2,450,432 bytes and 362 entries at SHA-256
`4437ccfa263ff913b36679706a5585be5170cc1abd5f32869db8f4d20544c0c5`;
one passed complete extracted-source qualification.

This evidence does not establish remote availability of Aegis or Keystone, registry
packaging, transitive source integrity outside the local checkouts, advisory
status, license compatibility, binary contents or a binary SBOM, origin
provenance, signatures, attestations, or reproducibility from a clean Cargo home.
Those remain release blockers.
