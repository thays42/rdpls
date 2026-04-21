OS := $(shell uname -s)
TAURI_VERSION := ^2
BUNDLE_DIR := src-tauri/target/release/bundle

PACKAGES_RPM := webkit2gtk4.1-devel gtk3-devel libsoup3-devel \
                javascriptcoregtk4.1-devel pango-devel cairo-devel gdk-pixbuf2-devel

PACKAGES_DEB := libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev \
                libjavascriptcoregtk-4.1-dev libpango1.0-dev libcairo2-dev \
                libgdk-pixbuf-2.0-dev build-essential pkg-config

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
	cargo install tauri-cli --version "$(TAURI_VERSION)" $(if $(REINSTALL),--force)
else
	@echo "Unsupported OS: $(OS)" >&2; exit 1
endif

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
