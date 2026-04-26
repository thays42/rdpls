OS := $(shell uname -s)
TAURI_VERSION := ^2
TAURI_DIR := src-tauri
BUNDLE_DIR := src-tauri/target/release/bundle

PACKAGES_RPM := webkit2gtk4.1-devel gtk3-devel libsoup3-devel \
                javascriptcoregtk4.1-devel pango-devel cairo-devel gdk-pixbuf2-devel

PACKAGES_DEB := libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev \
                libjavascriptcoregtk-4.1-dev libpango1.0-dev libcairo2-dev \
                libgdk-pixbuf-2.0-dev build-essential pkg-config

.PHONY: install-deps setup dev run test test-fast lint fmt typecheck build install uninstall clean ci ci-fast watch

install-deps:
ifeq ($(OS),Darwin)
	@if ! xcode-select -p >/dev/null 2>&1; then xcode-select --install; fi
	cargo install tauri-cli --version "$(TAURI_VERSION)" $(if $(REINSTALL),--force)
else ifeq ($(OS),Linux)
	@if command -v dnf >/dev/null 2>&1; then \
	    sudo dnf $(if $(REINSTALL),reinstall,install) -y $(PACKAGES_RPM); \
	elif command -v apt >/dev/null 2>&1; then \
	    sudo apt install -y $(if $(REINSTALL),--reinstall) $(PACKAGES_DEB); \
	else \
	    echo "Unsupported Linux distro: needs dnf or apt" >&2; exit 1; \
	fi
	cargo install tauri-cli --version "$(TAURI_VERSION)" $(if $(REINSTALL),--force)
else
	@echo "Unsupported OS: $(OS)" >&2; exit 1
endif

setup:
	@command -v lefthook >/dev/null && lefthook install || echo "lefthook not installed; skip"
	@command -v cargo-tauri >/dev/null || cargo install tauri-cli --version '$(TAURI_VERSION)' --locked

dev:
	cargo tauri dev

run: dev

test:
	cargo test --manifest-path $(TAURI_DIR)/Cargo.toml

test-fast:
	cargo test --manifest-path $(TAURI_DIR)/Cargo.toml --lib

lint:
	cargo clippy --manifest-path $(TAURI_DIR)/Cargo.toml --all-targets -- -D warnings
	cargo fmt --manifest-path $(TAURI_DIR)/Cargo.toml -- --check

fmt:
	cargo fmt --manifest-path $(TAURI_DIR)/Cargo.toml

typecheck:
	cargo check --manifest-path $(TAURI_DIR)/Cargo.toml --all-targets

build:
	cargo tauri build

install: build
ifeq ($(OS),Darwin)
	rm -rf /Applications/rdpls.app
	cp -R $(BUNDLE_DIR)/macos/rdpls.app /Applications/
else ifeq ($(OS),Linux)
	@if command -v dnf >/dev/null 2>&1; then \
	    sudo dnf install -y $(BUNDLE_DIR)/rpm/rdpls-*.rpm; \
	elif command -v apt >/dev/null 2>&1; then \
	    sudo apt install -y ./$(BUNDLE_DIR)/deb/rdpls_*.deb; \
	else \
	    echo "Unsupported Linux distro: needs dnf or apt" >&2; exit 1; \
	fi
else
	@echo "Unsupported OS: $(OS)" >&2; exit 1
endif

uninstall:
ifeq ($(OS),Darwin)
	rm -rf /Applications/rdpls.app
else ifeq ($(OS),Linux)
	@if command -v dnf >/dev/null 2>&1; then \
	    sudo dnf remove -y rdpls; \
	elif command -v apt >/dev/null 2>&1; then \
	    sudo apt remove -y rdpls; \
	else \
	    echo "Unsupported Linux distro: needs dnf or apt" >&2; exit 1; \
	fi
else
	@echo "Unsupported OS: $(OS)" >&2; exit 1
endif

clean:
	cargo clean --manifest-path $(TAURI_DIR)/Cargo.toml

ci: lint typecheck test

ci-fast: lint typecheck

watch:
	@command -v watchexec >/dev/null && watchexec -e rs,html,js,css -- $(MAKE) ci-fast || echo "watchexec not installed"
