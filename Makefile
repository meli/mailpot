.PHONY: check
check:
	cargo check --all-features --all --tests --examples --benches --bins

.PHONY: fmt
fmt:
	cargo +nightly fmt --all || cargo fmt --all
	cargo sort -w || printf "cargo-sort binary not found in PATH.\n"
	djhtml -i web/src/templates/* || printf "djhtml binary not found in PATH.\n"

.PHONY: lint
lint:
	cargo clippy --no-deps --all-features --all --tests --examples --benches --bins


.PHONY: test
test: check lint
	cargo test --all --no-fail-fast --all-features
