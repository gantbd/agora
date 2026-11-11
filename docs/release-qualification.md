# Release-candidate qualification

`make qualify-release-candidate` is Agora's commit-pinned, opt-in qualification
profile. It refuses staged or unstaged tracked changes, records the exact HEAD,
and then runs:

1. the complete ordinary source gate;
2. every workspace target in the release profile;
3. the ignored Service V2 lifecycle/replay/recovery soak in release mode; and
4. all three pinned coverage-guided fuzz targets.

It checks tracked-tree cleanliness again after all four stages. A stale lock or
other generated tracked drift therefore fails the candidate instead of silently
changing the source that was supposedly qualified.

The default candidate profile uses 10,000 soak cycles and 1,000,000 libFuzzer
runs per target. `AGORA_RELEASE_SOAK_CYCLES` may select 1 through 10,000;
`AGORA_RELEASE_FUZZ_RUNS` may select 1 through 1,000,000. Invalid, zero, signed,
or over-ceiling values fail before qualification. The underlying fuzz runner
retains its fixed seed, input, timeout, and memory ceilings. Generated fuzz
corpora and artifacts remain temporary.

This command qualifies the selected revision and numeric profile on the current
host. It does not build a distributable archive, prove reproducible binaries,
publish anything, sign or attest an artifact, query advisories, exercise another
architecture, establish a performance SLO, or replace platform/deployment
qualification. An interrupted or partially run command is not a pass.
