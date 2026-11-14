# Supply-chain profile

Status: deterministic local-source inventory and source SBOM; not a binary SBOM
or legal review.

The current stable locked graph contains twenty components: sixteen Agora
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
contains exactly the twenty stable local-path components, their closed Cargo
dependency edges, exact versions, declared `MIT OR Apache-2.0` expressions, and
denied publication state. Its SHA-256 digest is
`a34ab2774391d301e737e5983e753d97ac2b6d10fd1e962c417c7b7c58e789e5`.
The validator rejects vocabulary drift, noncanonical order, missing or duplicate
components and edges, unknown references, host paths, altered licenses, and
publication-enabled records; focused mutants exercise those boundaries.

The opt-in standalone `fuzz/` workspace is deliberately outside that release
graph. Its separate lock contains the registry-sourced libFuzzer build tooling;
the fuzz contract pins the driver, nightly compiler, and `libfuzzer-sys`
versions. Those tools produce test evidence only and are not linked into any
Agora package or included in the stable local-only supply claim.

The source-candidate profile copies exactly those twenty stable components into
one local directory. Agora bytes come from an exact clean tracked revision;
selected Aegis and Keystone bytes are individually checksummed and their subtree
cleanliness is stated rather than inferred from the contextual repository
revision. A reduced reviewed Keystone workspace manifest supplies only inherited
metadata for the two selected packages. Isolated locked/offline build evidence
establishes content closure, not origin, advisory, or legal provenance.

The qualified candidate for Agora revision
`cd5ab9386bde597e83d58a101feef01871cb78b9` was reproduced byte-for-byte
from two independent constructions and closes the current twenty-component,
412-payload-file graph. Its manifest digest is
`4a7a65a671bc80ff15637c024615eb2c6202f448c5a5daf27b67ee4d30ceb61e` and
its checksum-inventory digest is
`cd3026890cfbf78bf5b6c28a4e24e69a636a10d342eb75945dee64005369c0ee`.
Both sibling selections were recorded unclean; the content hashes are evidence,
while their contextual Git revisions are not promoted into provenance. Two
canonical archives over those independent candidates were byte-identical at
3,537,408 bytes and 499 entries at SHA-256
`079b986e2c84a33923d4f80be1781987ea60326ea6f34cd96616df9984777e5b`;
one passed canonical validation, exact rearchive, and complete extracted-source
qualification. The earlier seventeen-component artifact remains historical
evidence only.

This evidence does not establish remote availability of Aegis or Keystone, registry
packaging, transitive source integrity outside the local checkouts, advisory
status, license compatibility, binary contents or a binary SBOM, origin
provenance, signatures, attestations, or reproducibility from a clean Cargo home.
Those remain release blockers.
