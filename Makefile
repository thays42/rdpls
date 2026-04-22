OS := $(shell uname -s)

# macOS (Tauri) variables
TAURI_VERSION := ^2
BUNDLE_DIR    := src-tauri/target/release/bundle

# Linux (Firefox-kiosk) variables
PREFIX        ?= $(HOME)/.local
FIREFOX       ?= $(HOME)/.local/opt/firefox-dev/firefox
PROFILE_DIR   ?= $(HOME)/.local/share/rdpls/profile
DESKTOP_DIR   ?= $(PREFIX)/share/applications
ADDON_ID      ?= rdpls@local
REPO_ROOT     := $(shell pwd)
EXT_SRC       := $(REPO_ROOT)/src-firefox/extension
PROFILE_SRC   := $(REPO_ROOT)/src-firefox/profile
ENTRY_URL     ?= https://myapps.microsoft.com

.PHONY: install-deps build install install-macos install-linux \
        uninstall uninstall-macos uninstall-linux uninstall-hard \
        dev check-firefox

install-deps:
ifeq ($(OS),Darwin)
	@if ! xcode-select -p >/dev/null 2>&1; then xcode-select --install; fi
	cargo install tauri-cli --version "$(TAURI_VERSION)" $(if $(REINSTALL),--force)
else ifeq ($(OS),Linux)
	@echo "Linux uses Firefox Developer Edition — no system packages to install."
	@echo "Install Firefox Dev Edition manually (see README) and optionally:"
	@echo "  npm install -g web-ext   # for 'make dev' live-reload"
else
	@echo "Unsupported OS: $(OS)" >&2; exit 1
endif

# --- Build (macOS only; Linux has nothing to build) ---
build:
ifeq ($(OS),Darwin)
	cargo tauri build
else
	@echo "Nothing to build on Linux — Firefox host needs no compile step."
endif

# --- Install ---
install:
ifeq ($(OS),Darwin)
	$(MAKE) install-macos
else ifeq ($(OS),Linux)
	$(MAKE) install-linux
else
	@echo "Unsupported OS: $(OS)" >&2; exit 1
endif

install-macos: build
	rm -rf /Applications/rdpls.app
	cp -R $(BUNDLE_DIR)/macos/rdpls.app /Applications/

check-firefox:
	@test -x "$(FIREFOX)" || { \
	  echo "error: Firefox not found at $(FIREFOX)"; \
	  echo "       install Dev Edition and/or set FIREFOX=..."; \
	  exit 1; }

install-linux: check-firefox
	@mkdir -p "$(PROFILE_DIR)/extensions"
	@mkdir -p "$(DESKTOP_DIR)"
	@if [ ! -f "$(PROFILE_DIR)/user.js" ]; then \
	  cp "$(PROFILE_SRC)/user.js" "$(PROFILE_DIR)/user.js"; \
	  echo "seeded $(PROFILE_DIR)/user.js"; \
	else \
	  echo "preserved existing $(PROFILE_DIR)/user.js (use uninstall-hard to reset)"; \
	fi
	@if [ -L "$(PROFILE_DIR)/extensions/$(ADDON_ID)" ] || [ ! -e "$(PROFILE_DIR)/extensions/$(ADDON_ID)" ]; then \
	  ln -sfn "$(EXT_SRC)" "$(PROFILE_DIR)/extensions/$(ADDON_ID)"; \
	  echo "linked extension -> $(PROFILE_DIR)/extensions/$(ADDON_ID)"; \
	else \
	  echo "error: $(PROFILE_DIR)/extensions/$(ADDON_ID) exists and is not a symlink; aborting" >&2; \
	  exit 1; \
	fi
	@printf '%s\n' \
	  '[Desktop Entry]' \
	  'Type=Application' \
	  'Name=rdpls' \
	  'Comment=Windows 365 / AVD session' \
	  'Exec=$(FIREFOX) --profile $(PROFILE_DIR) --kiosk --no-remote --new-instance $(ENTRY_URL)' \
	  'Icon=rdpls' \
	  'Terminal=false' \
	  'Categories=Network;RemoteAccess;' \
	  'StartupWMClass=firefox-developer-edition' \
	  > "$(DESKTOP_DIR)/rdpls.desktop"
	@echo "wrote $(DESKTOP_DIR)/rdpls.desktop"
	@echo "install complete."

# --- Uninstall ---
uninstall:
ifeq ($(OS),Darwin)
	$(MAKE) uninstall-macos
else ifeq ($(OS),Linux)
	$(MAKE) uninstall-linux
else
	@echo "Unsupported OS: $(OS)" >&2; exit 1
endif

uninstall-macos:
	rm -rf /Applications/rdpls.app

uninstall-linux:
	rm -f "$(DESKTOP_DIR)/rdpls.desktop"
	rm -f "$(PROFILE_DIR)/extensions/$(ADDON_ID)"
	@echo "uninstalled extension link and desktop entry."
	@echo "profile data preserved at $(PROFILE_DIR). Use 'make uninstall-hard' to delete it."

uninstall-hard: uninstall-linux
	rm -rf "$(PROFILE_DIR)"
	@echo "profile $(PROFILE_DIR) removed."

# --- Dev loop (Linux) ---
dev: check-firefox
	cd src-firefox/extension && \
	  web-ext run \
	    --firefox="$(FIREFOX)" \
	    --firefox-profile="$(PROFILE_DIR)" \
	    --keep-profile-changes \
	    --start-url="$(ENTRY_URL)"
