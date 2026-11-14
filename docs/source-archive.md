# Canonical stable source archive

Status: one exact local source archive qualified; distribution remains denied.

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

## Qualified local archive

Agora revision `cd5ab9386bde597e83d58a101feef01871cb78b9` produced two
byte-identical canonical archives from independently built candidates on Darwin
25.6.0 arm64 / macOS 26.6.2. Each archive is 3,537,408 bytes with 499 entries
and SHA-256
`079b986e2c84a33923d4f80be1781987ea60326ea6f34cd96616df9984777e5b`.
One archive passed checksum and canonical validation, platform extraction,
extracted checksum closure, exact rearchive, every stable workspace target,
doctests, and warning-denied documentation with locked/offline dependencies and
a fresh target directory. Both selected sibling trees remain truthfully
recorded as unclean; the archive closes exact bytes and does not promote
contextual Git revisions into origin provenance.

The earlier qualification at Agora revision
`46098fb3cd1a6f9164365d8c49cf7edbc0957ef7` produced two byte-identical
2,450,432-byte, 362-entry archives with SHA-256
`4437ccfa263ff913b36679706a5585be5170cc1abd5f32869db8f4d20544c0c5`.
