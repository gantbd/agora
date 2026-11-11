# Coverage-guided fuzzing

Agora has two opt-in libFuzzer targets:

- `wire_canonical` drives every public V1/V2 request and reply decoder. Any
  accepted input must re-encode byte-for-byte and decode to the same value.
- `checkpoint_canonical` drives the engine, Service V2, and Host V2 recovery
  profiles under production bounds. Any recovered input must prepare the exact
  same canonical bytes.

The standalone `fuzz/` workspace pins `libfuzzer-sys` 0.4.13 and
`nightly-2026-11-01`. The driver is cargo-fuzz 0.13.2. Install the exact driver
with `cargo install cargo-fuzz --version 0.13.2 --locked` and install the pinned
toolchain with `rustup toolchain install nightly-2026-11-01 --profile minimal`.
The ordinary stable gate does not install or run either external tool.

`make check-fuzz-contract` is part of `make check`. It verifies the exact
toolchain/dependency pins and proves every committed binary seed is the exact
decoded form of a canonical wire or checkpoint fixture. Refresh those seeds
after an intentional compatibility-profile change with
`./scripts/refresh-fuzz-seeds.sh`.

`make fuzz-smoke` copies the immutable seeds into a temporary corpus and runs
20,000 coverage-guided cases per target with a fixed seed and explicit input,
timeout, and memory ceilings. A longer campaign may replace `-runs` with a
reviewed `-max_total_time`, but its duration, platform, tool versions, corpus,
and result must remain visible release evidence. Generated corpus entries and
crash artifacts are not compatibility evidence; minimize a failure, add a
focused ordinary regression test, then decide whether the input adds durable
seed coverage.

Passing campaigns prove neither parser correctness nor exhaustive state-space
coverage. The checkpoint target uses the frozen fixture scopes and reference
limits; it does not fuzz physical storage, filesystem interruption, migration,
thread-parallel linearizability, or authentication behavior.
