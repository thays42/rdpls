# rdpls

Dedicated RDP browser wrapper built with Tauri. Hosts Microsoft's web-based RDP client (Windows 365 / AVD) in a native WebView with minimal keyboard interception, letting keystrokes pass through to the remote session.

## Status

Phase 1 (scaffold, auth, passthrough basics) and Phase 2 (keyboard matrix, shortcut-inhibit) are complete. Produces installable `.deb` and `.rpm` from `cargo tauri build`. Phase 3 items (session-drop detection, clipboard portal, status strip, macOS port) remain open.

## Architecture

- **Tauri v2** (Rust backend, system WebView frontend)
- **Single WebView** handles the entire flow: Microsoft auth + MFA, tenant picker, app launch, RDP session
- **Entry point:** `https://myapps.microsoft.com` — no session URL persistence
- **WebView engine:** webkit2gtk-4.1 on Linux, WKWebView on macOS

## Key Constraints

- **Keyboard passthrough is the core feature.** No application menu. No Tauri accelerators. Disable WebKitGTK context menu and Ctrl+scroll zoom. The one intentional in-app key is `Ctrl+Alt+Shift+Escape`, captured via JS injection in the WebView (not a global shortcut — those don't work on Wayland).
- **Compositor-level keys pass through via `zwp_keyboard_shortcuts_inhibit_manager_v1`.** rdpls requests the inhibitor at startup against the main surface and default seat. niri honors it automatically, so Alt+Tab, Super, etc. reach the remote client. `Ctrl+Alt+Shift+Escape` toggles the inhibit on/off.
- **UA spoofing required.** WebKitGTK default UA triggers Microsoft "unsupported browser" warnings. Spoof current Firefox or Edge.
- **Persistent cookies.** Stable WebKit data directory so auth survives across launches. Microsoft issues session-only tokens for the per-app RDP step; this requires re-auth on restart even in Firefox. Not fixable on our side.
- **Popup → main-window redirect.** Microsoft launches RDP sessions via `window.open()`; intercept webkit2gtk's `create` signal and load the URL in the main WebView instead.
- **Wayland-native.** No `GDK_BACKEND=x11`. Target is niri on Fedora 43. Shares GTK's existing Wayland connection via `wayland-backend::Backend::from_foreign_display` — never open a second connection, surfaces/seats wouldn't cross over.
- **No window decorations.** `decorations(false)` — no min/max/close chrome. Compositor handles window management.
- **Conditional Access passes without managed browser** — Firefox on Fedora already works, so the same UA string works here.

## Targets

- **Primary:** Fedora 43, Framework 13, niri (Wayland)
- **Secondary (future):** macOS, Mac Studio M4 Max — same codebase, separate build

## Naming

`rdpls` — lowercase everywhere. Binary, window title, desktop file. Pronounced "R-D-please."

## Build Prerequisites

```bash
# Fedora 43
sudo dnf install webkit2gtk4.1-devel gtk3-devel libsoup3-devel \
  javascriptcoregtk4.1-devel pango-devel cairo-devel gdk-pixbuf2-devel
cargo install tauri-cli --version "^2"
```

## Development Commands

Run from the repo root:

```bash
cargo tauri dev          # Run in development mode
cargo tauri build        # Build release binary + bundle .deb / .rpm
cargo test               # Run Rust tests
```

Release artifacts land in `src-tauri/target/release/` (binary) and `src-tauri/target/release/bundle/{deb,rpm}/` (packages).

## Known Follow-ups

- Bundle identifier is `com.rdpls.client`. Renamed from `com.rdpls.app` for the macOS port — `.app` collides with the bundle extension and the Tauri bundler warns on every build. Linux users re-auth once after the rename (WebKit data directory is keyed to the identifier).
- Session-drop detection, clipboard portal wiring, and status strip are Phase 3.
- macOS keyboard passthrough uses an `NSEvent` local monitor (`src-tauri/src/passthrough_macos.rs`) that swallows a small list of browser-like shortcuts (`Cmd+R`, `Cmd+[`, `Cmd+]` and their Shift variants) when inhibit is on, and passes everything else through. Exit on macOS is via the red traffic light — closing the window quits the app. The swallow list is intentionally narrow; see `docs/macos-keyboard-passthrough.md` and extend only with reason.

## Project Layout

```
src-tauri/
  src/main.rs                 # Binary entry → calls rdpls_lib::run
  src/lib.rs                  # Tauri builder, window creation, webkit2gtk setup
  src/keyboard.rs             # Injected JS for Ctrl+Alt+Shift+Escape, rdpls_exit command
  src/shortcuts_inhibit.rs    # Wayland shortcut-inhibit protocol wiring (Linux only)
  Cargo.toml                  # Rust dependencies
  tauri.conf.json             # Tauri config (window, bundle, CSP)
  capabilities/default.json   # Tauri v2 capability file
src/
  index.html                  # Minimal loader; WebView navigates to myapps.microsoft.com
docs/
  keyboard-matrix.md          # Keyboard passthrough test results
  plans/                      # Implementation plans
```
