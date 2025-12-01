.POSIX:
.SUFFIXES:
CARGOBIN	   = cargo
CARGOSORTBIN = cargo-sort
DJHTMLBIN    = djhtml
BLACKBIN     = black
PRINTF       = /usr/bin/printf

HTML_FILES   := $(shell find mailpot-web/src/templates -type f -print0 | tr '\0' ' ')
PY_FILES     := $(shell find . -type f -name '*.py' -print0 | tr '\0' ' ')

.PHONY: check
check:
	@echo $(CARGOBIN) check --all-features --all --tests --examples --benches --bins
	@$(CARGOBIN) check --all-features --all --tests --examples --benches --bins

.PHONY: fmt
fmt:
	@$(CARGOBIN) +nightly fmt --all || $(CARGOBIN) fmt --all
	@OUT=$$($(CARGOSORTBIN) -w 2>&1) || $(PRINTF) "ERROR: %s cargo-sort failed or binary not found in PATH.\n" "$$OUT"
	@OUT=$$($(DJHTMLBIN) $(HTML_FILES) 2>&1) || $(PRINTF) "ERROR: %s djhtml failed or binary not found in PATH.\n" "$$OUT"
	@OUT=$$($(BLACKBIN) -q $(PY_FILES) 2>&1) || $(PRINTF) "ERROR: %s black failed or binary not found in PATH.\n" "$$OUT"

.PHONY: lint
lint:
	@echo $(CARGOBIN) clippy --no-deps --all-features --all --tests --examples --benches --bins
	@$(CARGOBIN) clippy --no-deps --all-features --all --tests --examples --benches --bins


.PHONY: test
test: check lint
	@echo $(CARGOBIN) nextest run --all --no-fail-fast --all-features
	@$(CARGOBIN) nextest run --all --no-fail-fast --all-features

.PHONY: rustdoc
rustdoc:
	@echo RUSTDOCFLAGS=\"--html-before-content ./.github/doc_extra.html\" $(CARGOBIN) doc --workspace --all-features --no-deps --document-private-items
	@RUSTDOCFLAGS="--html-before-content ./.github/doc_extra.html" $(CARGOBIN) doc --workspace --all-features --no-deps --document-private-items

.PHONY: rustdoc-open
rustdoc-open:
	@echo RUSTDOCFLAGS=\"--html-before-content ./.github/doc_extra.html\" $(CARGOBIN) doc --workspace --all-features --no-deps --document-private-items --open
	@RUSTDOCFLAGS="--html-before-content ./.github/doc_extra.html" $(CARGOBIN) doc --workspace --all-features --no-deps --document-private-items --open

.PHONY: rustdoc-nightly
rustdoc-nightly:
	@echo RUSTDOCFLAGS=\"--html-before-content ./.github/doc_extra.html\" $(CARGOBIN) +nightly doc -Zrustdoc-map -Z rustdoc-scrape-examples --workspace --all-features --no-deps --document-private-items
	@RUSTDOCFLAGS="--html-before-content ./.github/doc_extra.html" $(CARGOBIN) +nightly doc -Zrustdoc-map -Z rustdoc-scrape-examples --workspace --all-features --no-deps --document-private-items

.PHONY: rustdoc-nightly-open
rustdoc-nightly-open:
	@echo RUSTDOCFLAGS=\"--html-before-content ./.github/doc_extra.html\" $(CARGOBIN) +nightly doc -Zrustdoc-map -Z rustdoc-scrape-examples --workspace --all-features --no-deps --document-private-items --open
	@RUSTDOCFLAGS="--html-before-content ./.github/doc_extra.html" $(CARGOBIN) +nightly doc -Zrustdoc-map -Z rustdoc-scrape-examples --workspace --all-features --no-deps --document-private-items --open
