.POSIX:
.SUFFIXES:
CARGOBIN	   = cargo
CARGOSORTBIN = cargo-sort
DJHTMLBIN    = djhtml
BLACKBIN     = black
PRINTF       = /usr/bin/printf

HTML_FILES   := $(shell find web/src/templates -type f -print0 | tr '\0' ' ')
PY_FILES     := $(shell find . -type f -name '*.py' -print0 | tr '\0' ' ')

.PHONY: check
check:
	@$(CARGOBIN) check --all-features --all --tests --examples --benches --bins

.PHONY: fmt
fmt:
	@$(CARGOBIN) +nightly fmt --all || $(CARGOBIN) fmt --all
	@OUT=$$($(CARGOSORTBIN) -w 2>&1) || $(PRINTF) "ERROR: %s cargo-sort failed or binary not found in PATH.\n" "$$OUT"
	@OUT=$$($(DJHTMLBIN) $(HTML_FILES) 2>&1) || $(PRINTF) "ERROR: %s djhtml failed or binary not found in PATH.\n" "$$OUT"
	@OUT=$$($(BLACKBIN) -q $(PY_FILES) 2>&1) || $(PRINTF) "ERROR: %s black failed or binary not found in PATH.\n" "$$OUT"

.PHONY: lint
lint:
	@$(CARGOBIN) clippy --no-deps --all-features --all --tests --examples --benches --bins


.PHONY: test
test: check lint
	@$(CARGOBIN) test --all --no-fail-fast --all-features

.PHONY: rustdoc
rustdoc:
	@RUSTDOCFLAGS="--html-before-content ./.github/doc_extra.html" $(CARGOBIN) doc --workspace --all-features --no-deps --document-private-items

.PHONY: rustdoc-open
rustdoc-open:
	@RUSTDOCFLAGS="--html-before-content ./.github/doc_extra.html" $(CARGOBIN) doc --workspace --all-features --no-deps --document-private-items --open
