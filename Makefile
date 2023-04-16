.PHONY: check
check:
	cargo check --all

.PHONY: fmt
fmt:
	cargo +nightly fmt --all || cargo fmt --all
	cargo sort -w || printf "cargo-sort binary not found in PATH.\n"
	djhtml -i web/src/templates/* || printf "djhtml binary not found in PATH.\n"

.PHONY: lint
lint:
	cargo clippy --all
