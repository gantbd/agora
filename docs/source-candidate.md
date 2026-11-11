# Self-contained stable source candidate

`AGORA_SOURCE_CANDIDATE_DIR=/new/path make build-source-candidate` creates one
new checksum-closed directory with the exact tracked Agora revision plus only
the two Aegis and two Keystone packages in the stable locked graph. Their
existing sibling-relative layout is preserved under `agora/`, `aegis/`, and
`keystone/`; no Cargo dependency declaration is rewritten. The reduced Keystone
workspace manifest is a reviewed repository input because those package
manifests inherit workspace metadata.

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

This profile is not a tarball, binary artifact, registry package, SBOM,
signature, attestation, advisory report, legal conclusion, remote-source proof,
reproducible binary, installer, or publication authorization. The standalone
fuzz workspace retains registry-sourced test tooling and is not part of the
stable sixteen-component release graph. Candidate distribution and security
support remain unauthorized until a channel and policy are explicitly selected.

## Qualified local candidate

Agora revision `e5a02b4ce5bda9c92542b0e5ea59ea3a746b295b` produced two
byte-identical candidate directories on Darwin 25.6.0 arm64 / macOS 26.6.2.
Each contained 233 payload files plus `SHA256SUMS`. The manifest digest was
`d67a6dcc09198a8e237b4fb17e3ecd8dbf37d172dbe95d8e641c7561c132e146` and
the checksum-inventory digest was
`3068c303ce5e4f7d930bbef949c781e9530bf198f0aaa278b39c6a7576b1175a`.
The manifest truthfully recorded both selected sibling trees as unclean, so
their exact checksummed bytes—not their contextual revisions—are the qualified
inputs. One candidate passed all stable workspace targets, doctests, and
warning-denied documentation with a fresh target directory, the lock file, and
offline mode.
