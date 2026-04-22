OS := $(shell uname -s)
TAURI_VERSION := ^2
LINUX_CRATE := rdpls-linux
BUNDLE_DIR_MAC := src-tauri/target/release/bundle
BUNDLE_DIR_DEB := target/debian
BUNDLE_DIR_RPM := target/generate-rpm

PACKAGES_RPM := gtk4-devel webkitgtk6.0-devel libsoup3-devel \
                pango-devel cairo-devel gdk-pixbuf2-devel

PACKAGES_DEB := libgtk-4-dev libwebkitgtk-6.0-dev libsoup-3.0-dev \
                libpango1.0-dev libcairo2-dev libgdk-pixbuf-2.0-dev \
                build-essential pkg-config

CARGO_PACKAGERS := cargo-deb cargo-generate-rpm

.PHONY: install-deps build install uninstall

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
	cargo install $(CARGO_PACKAGERS) $(if $(REINSTALL),--force)
else
	@echo "Unsupported OS: $(OS)" >&2; exit 1
endif

build:
ifeq ($(OS),Darwin)
	cargo tauri build
else ifeq ($(OS),Linux)
	cargo build --release -p $(LINUX_CRATE) --bin rdpls
	cargo deb -p $(LINUX_CRATE) --no-build
	# cargo-generate-rpm 0.20 needs to run inside the crate dir (workspace
	# path bug) and defaults its output to that crate's local `target/`.
	# Redirect the .rpm to the shared workspace target so both bundle types
	# end up under `target/` at the repo root.
	mkdir -p $(BUNDLE_DIR_RPM)
	cd src-linux && cargo generate-rpm --output ../$(BUNDLE_DIR_RPM)
else
	@echo "Unsupported OS: $(OS)" >&2; exit 1
endif

install: build
ifeq ($(OS),Darwin)
	rm -rf /Applications/rdpls.app
	cp -R $(BUNDLE_DIR_MAC)/macos/rdpls.app /Applications/
else ifeq ($(OS),Linux)
	@if command -v dnf >/dev/null 2>&1; then \
	    sudo dnf install -y $(BUNDLE_DIR_RPM)/rdpls-*.rpm; \
	elif command -v apt >/dev/null 2>&1; then \
	    sudo apt install -y ./$(BUNDLE_DIR_DEB)/rdpls_*.deb; \
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
