# Supply-chain profile

Status: deterministic local-source inventory and source SBOM; not a binary SBOM
or legal review.

The current stable locked graph contains sixteen components: twelve Agora
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
contains exactly the sixteen stable local-path components, their closed Cargo
dependency edges, exact versions, declared `MIT OR Apache-2.0` expressions, and
denied publication state. Its SHA-256 digest is
`7cd0c0a92f8d1000a4d5c6895e9f29e7d5bc4fa2e44b9944f0f9390d1c9c8431`.
The validator rejects vocabulary drift, noncanonical order, missing or duplicate
components and edges, unknown references, host paths, altered licenses, and
publication-enabled records; focused mutants exercise those boundaries.

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
`651374a3e9cf4b51cb21da7c7af03bb746604d25` was reproduced byte-for-byte,
closed over 248 payload files including the canonical source SBOM and archive tooling, and passed
the complete stable locked/offline workspace build from an isolated target.
Its manifest digest is
`521912b3d03cb40799638cc510f75bf7eddc3d76bf1b2c57e1d532b68fb6b646` and
its checksum-inventory digest is
`54e92c656f80e15d6c1eb4115cb0e6ed3b0b3cd197af71565c54ee95b657d905`.
Both sibling selections were recorded unclean; the content hashes are evidence,
while their contextual Git revisions are not promoted into provenance.
Two canonical archives over those independent candidates were byte-identical at
SHA-256 `41e22b596d32c6bd44578afde4421c4834d2b87c45b4d4a7dd3a5637976f4ad2`;
one passed complete extracted-source qualification.

This evidence does not establish remote availability of Aegis or Keystone, registry
packaging, transitive source integrity outside the local checkouts, advisory
status, license compatibility, binary contents or a binary SBOM, origin
provenance, signatures, attestations, or reproducibility from a clean Cargo home.
Those remain release blockers.
