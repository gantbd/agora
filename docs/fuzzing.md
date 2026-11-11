# Coverage-guided fuzzing

Agora has three opt-in libFuzzer targets:

- `wire_canonical` drives every public conversation/transient request and reply decoder. Any
  accepted input must re-encode byte-for-byte and decode to the same value. It
  also drives all five fixed-width local handshake/session-control decoders and
  the closed local route envelope with the same exact-byte oracle.
- `checkpoint_canonical` drives the engine, Service V2, and Host V2 recovery
  profiles under production bounds. Any recovered input must prepare the exact
  same canonical bytes.
- `outer_frame_canonical` drives the exact big-endian length-prefix parser used
  by the local Unix daemon. Any accepted first frame must re-encode to the exact
  consumed prefix, preserve the unconsumed stream suffix, and decode again to
  the same payload.

The standalone `fuzz/` workspace pins `libfuzzer-sys` 0.4.13 and
`nightly-2026-11-01`. The driver is cargo-fuzz 0.13.2. Install the exact driver
with `cargo install cargo-fuzz --version 0.13.2 --locked` and install the pinned
toolchain with `rustup toolchain install nightly-2026-11-01 --profile minimal`.
The ordinary stable gate does not install or run either external tool.

`make check-fuzz-contract` is part of `make check`. It compiles every fuzz target
under the exact offline locked dependency graph, verifies the toolchain/dependency
pins, and proves every committed binary seed is the exact
decoded form of a canonical wire or checkpoint fixture. Outer-frame seeds are
the twenty-nine canonical complete-frame protocol fixtures with their exact
four-byte length prefix. The five fixed-width local handshake/session-control
fixtures are deliberately excluded from that corpus.
Refresh those seeds
after an intentional compatibility-profile change with
`./scripts/refresh-fuzz-seeds.sh`.

Release qualification checks tracked-tree cleanliness both before and after the
campaign, so a stale fuzz lock or any other generated tracked drift makes the
candidate fail rather than silently changing the qualified source.

`make fuzz-smoke` copies the immutable seeds into a temporary corpus and runs
20,000 coverage-guided cases per target with a fixed seed and explicit input,
timeout, and memory ceilings. `AGORA_FUZZ_RUNS` may select a positive budget up
to 1,000,000; the release-candidate profile uses that ceiling. A longer
time-based campaign requires separate review, and its duration, platform, tool
versions, corpus, and result must remain visible release evidence. Generated
corpus entries and crash artifacts are not compatibility evidence; minimize a
failure, add a focused ordinary regression test, then decide whether the input
adds durable seed coverage.

Passing campaigns prove neither parser correctness nor exhaustive state-space
coverage. The checkpoint target uses the frozen fixture scopes and reference
limits. The outer-frame target covers the pure prefix boundary, not partial
kernel reads, deadlines, socket behavior, or authentication. The profile does
not fuzz physical storage, filesystem interruption, migration, thread-parallel
linearizability, or credential behavior.
