.POSIX:
.SUFFIXES:

HTML_FILES := $(shell find web/src/templates -type f -print0 | tr '\0' ' ')

.PHONY: check
check:
	@cargo check --all-features --all --tests --examples --benches --bins

.PHONY: fmt
fmt:
	@cargo +nightly fmt --all || cargo fmt --all
	@cargo sort -w || printf "cargo-sort binary not found in PATH.\n"
	@djhtml -i $(HTML_FILES) || printf "djhtml binary not found in PATH.\n"

.PHONY: lint
lint:
	@cargo clippy --no-deps --all-features --all --tests --examples --benches --bins


.PHONY: test
test: check lint
	@cargo test --all --no-fail-fast --all-features
