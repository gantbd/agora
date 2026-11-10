.PHONY: check check-baseline check-boundaries check-release-contract fmt lint test doc

check: check-release-contract check-baseline check-boundaries fmt lint test doc

check-release-contract:
	./scripts/check-release-contract.sh

check-baseline:
	./scripts/check-compatibility-baseline.sh
	./scripts/check-compatibility-baseline-mutants.sh

check-boundaries:
	./scripts/check-boundaries.sh

fmt:
	cargo fmt --all -- --check

lint:
	cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings

test:
	cargo test --workspace --all-targets --locked --offline

doc:
	cargo test --workspace --doc --locked --offline
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked --offline

