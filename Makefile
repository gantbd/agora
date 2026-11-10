.PHONY: check check-boundaries fmt lint test doc

check: check-boundaries fmt lint test doc

check-boundaries:
	./scripts/check-boundaries.sh

fmt:
	cargo fmt --all -- --check

lint:
	cargo clippy --all-targets --all-features --locked --offline -- -D warnings

test:
	cargo test --all-targets --locked --offline

doc:
	cargo test --doc --locked --offline
	RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked --offline

