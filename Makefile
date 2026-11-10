.PHONY: check check-baseline check-boundaries check-release-contract check-supply-chain fmt lint test doc soak

check: check-release-contract check-baseline check-supply-chain check-boundaries fmt lint test doc

check-release-contract:
	./scripts/check-release-contract.sh
	./scripts/check-release-contract-mutants.sh

check-baseline:
	./scripts/check-compatibility-baseline.sh
	./scripts/check-compatibility-baseline-mutants.sh

check-supply-chain:
	./scripts/check-supply-chain.sh
	./scripts/check-supply-chain-mutants.sh

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

soak:
	cargo test -p agora-service --test service_v2_soak --locked --offline -- --ignored --nocapture

