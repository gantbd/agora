# Self-contained stable source candidate

`AGORA_SOURCE_CANDIDATE_DIR=/new/path make build-source-candidate` creates one
new checksum-closed directory with the exact tracked Agora revision plus only
the two Aegis and two Keystone packages in the stable locked graph. Their
existing sibling-relative layout is preserved under `agora/`, `aegis/`, and
`keystone/`; no Cargo dependency declaration is rewritten. The reduced Keystone
workspace manifest is a reviewed repository input because those package
manifests inherit workspace metadata.

The tracked canonical source SBOM is part of the Agora tree and is therefore
included in, and checksum-closed by, every source candidate. It describes the
same sixteen-component stable graph; it does not describe built binary contents.

Construction requires a clean tracked Agora tree and records its exact commit.
The selected sibling bytes are copied from the local Aegis and Keystone trees.
Their current revisions are context only: the manifest separately records
whether each selected subtree is clean, and every staged byte is authoritative
through `SHA256SUMS`. A dirty or untracked sibling input therefore cannot be
misreported as commit provenance. The builder refuses missing, symbolic, or
nonregular inputs and an existing output path.

`make qualify-source-candidate` first proves that checksums close over every
regular file and that no symbolic, special, extra, missing, changed, or
claim-mutated entry is accepted. It then uses a fresh target directory to run
all stable workspace targets, doctests, and warning-denied documentation from
`agora/Cargo.toml` with the lock file and offline mode. Success establishes a
self-contained stable local source directory for those exact bytes.

This directory profile is not itself an archive, binary artifact, registry package, binary SBOM,
signature, attestation, advisory report, legal conclusion, remote-source proof,
reproducible binary, installer, or publication authorization. The standalone
fuzz workspace retains registry-sourced test tooling and is not part of the
stable sixteen-component release graph. Candidate distribution and security
support remain unauthorized until a channel and policy are explicitly selected.

## Qualified local candidate

Agora revision `651374a3e9cf4b51cb21da7c7af03bb746604d25` produced two
byte-identical candidate directories on Darwin 25.6.0 arm64 / macOS 26.6.2.
Each contained 248 payload files plus `SHA256SUMS`, including the canonical
source SBOM and R58 archive tooling. The manifest digest was
`521912b3d03cb40799638cc510f75bf7eddc3d76bf1b2c57e1d532b68fb6b646` and
the checksum-inventory digest was
`54e92c656f80e15d6c1eb4115cb0e6ed3b0b3cd197af71565c54ee95b657d905`.
The manifest truthfully recorded both selected sibling trees as unclean, so
their exact checksummed bytes—not their contextual revisions—are the qualified
inputs. One candidate was archive-extracted and passed all stable workspace
targets, doctests, and warning-denied documentation with a fresh target
directory, the lock file, and offline mode.
