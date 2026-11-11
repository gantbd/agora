.PHONY: build-source-archive build-source-candidate check check-baseline check-boundaries check-fuzz-contract check-release-contract check-release-qualification check-source-archive-contract check-source-candidate-contract check-supply-chain fmt lint test doc fuzz-smoke qualify-release-candidate qualify-source-archive qualify-source-candidate soak soak-release

check: check-release-contract check-release-qualification check-source-candidate-contract check-source-archive-contract check-baseline check-supply-chain check-boundaries check-fuzz-contract fmt lint test doc

check-release-contract:
	./scripts/check-release-contract.sh
	./scripts/check-release-contract-mutants.sh

check-release-qualification:
	./scripts/check-release-qualification.sh

check-source-candidate-contract:
	./scripts/check-source-candidate-contract.sh
	./scripts/check-source-candidate-contract-mutants.sh

check-source-archive-contract:
	./scripts/check-source-archive-contract.sh

check-baseline:
	./scripts/check-compatibility-baseline.sh
	./scripts/check-compatibility-baseline-mutants.sh

check-supply-chain:
	./scripts/check-supply-chain.sh
	./scripts/check-supply-chain-mutants.sh
	./scripts/check-source-sbom.sh

check-boundaries:
	./scripts/check-boundaries.sh

check-fuzz-contract:
	./scripts/check-fuzz-contract.sh

fmt:
	cargo fmt --all -- --check

lint:
	cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings

test:
	cargo test --workspace --all-targets --locked --offline

doc:
	cargo test --workspace --doc --locked --offline
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked --offline

fuzz-smoke:
	./scripts/run-fuzz-smoke.sh

qualify-release-candidate:
	./scripts/qualify-release-candidate.sh

build-source-candidate:
	@test -n "$(AGORA_SOURCE_CANDIDATE_DIR)" || (echo 'AGORA_SOURCE_CANDIDATE_DIR is required' >&2; exit 1)
	./scripts/build-source-candidate.sh "$(AGORA_SOURCE_CANDIDATE_DIR)"

qualify-source-candidate:
	@test -n "$(AGORA_SOURCE_CANDIDATE_DIR)" || (echo 'AGORA_SOURCE_CANDIDATE_DIR is required' >&2; exit 1)
	./scripts/qualify-source-candidate.sh "$(AGORA_SOURCE_CANDIDATE_DIR)"

build-source-archive:
	@test -n "$(AGORA_SOURCE_CANDIDATE_DIR)" || (echo 'AGORA_SOURCE_CANDIDATE_DIR is required' >&2; exit 1)
	@test -n "$(AGORA_SOURCE_ARCHIVE)" || (echo 'AGORA_SOURCE_ARCHIVE is required' >&2; exit 1)
	./scripts/build-source-archive.sh "$(AGORA_SOURCE_CANDIDATE_DIR)" "$(AGORA_SOURCE_ARCHIVE)"

qualify-source-archive:
	@test -n "$(AGORA_SOURCE_ARCHIVE)" || (echo 'AGORA_SOURCE_ARCHIVE is required' >&2; exit 1)
	./scripts/qualify-source-archive.sh "$(AGORA_SOURCE_ARCHIVE)"

soak:
	cargo test -p agora-service --test service_v2_soak --locked --offline -- --ignored --nocapture

soak-release:
	cargo test -p agora-service --test service_v2_soak --release --locked --offline -- --ignored --nocapture

