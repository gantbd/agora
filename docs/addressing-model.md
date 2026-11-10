# Bounded addressing model

The executable model in `tests/addressing_model.rs` enumerates a room with three
participants. For every one of the eight human/agent kind assignments, each of
the three authors, and each of the eight canonical recipient subsets, it checks
that acceptance, retained addressing, revision movement, and event movement equal
the all-human reference execution.

The state count is finite and deliberately small. The model can expose kind-based
branching or recipient-set inconsistencies in the implemented profile; it does
not prove behavior for unbounded populations or external adapters.

