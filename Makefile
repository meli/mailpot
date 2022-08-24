.PHONY: check
check:
	cargo check --all

.PHONY: fmt
fmt:
	cargo fmt --all

.PHONY: lint
lint:
	cargo clippy --all
