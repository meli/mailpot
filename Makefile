.PHONY: check
check:
	cargo check --all

.PHONY: fmt
fmt:
	cargo fmt --all
	cargo sort -w || true

.PHONY: lint
lint:
	cargo clippy --all
