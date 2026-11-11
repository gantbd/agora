# Canonical stable source archive

Status: construction and verification mechanism established; exact source
archive qualification remains opt-in and separately recorded.

`AGORA_SOURCE_CANDIDATE_DIR=/candidate AGORA_SOURCE_ARCHIVE=/new/agora.tar make
build-source-archive` first verifies the complete R56 checksum closure, then
writes a dependency-free canonical USTAR stream and adjacent
`agora.tar.sha256`. Existing outputs fail closed.

The archive has exactly one `agora-source/` root. Entries are strictly ordered;
paths use a restricted traversal-free ASCII vocabulary; symbolic and special
inputs are refused. Ownership and time are zero, owner/group names are `root`,
directories and executable files use mode 0755, other files use 0644, padding is
zero, and exactly two zero blocks terminate the stream. Per-file, entry-count,
and total-byte ceilings bound construction and validation.

`AGORA_SOURCE_ARCHIVE=/path/agora.tar make qualify-source-archive` verifies the
sidecar before parsing, rejects any noncanonical header, metadata, entry, path,
padding, or framing, extracts with the platform `tar`, verifies the extracted
R56 checksums, and reproduces the original archive byte-for-byte. It then runs
all stable workspace targets, doctests, and warning-denied documentation from
the extracted source with the lock file, offline mode, and a fresh target.

The ordinary contract builds the same fixture twice, proves system-archive
listing and extraction, and rejects changed bytes, truncation with a recomputed
sidecar, extra terminal framing, traversal with a valid USTAR header checksum,
and symbolic candidate input.

This profile does not qualify compression, a remote origin, clean sibling
commits, a binary, installer, signature, attestation, timestamp authority,
advisory status, legal compatibility, support, or a publication channel.
