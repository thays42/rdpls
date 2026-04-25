# rdpls

Dedicated RDP browser wrapper built with Tauri. Hosts Microsoft's web-based RDP client (Windows 365 / AVD) in a native WebView with minimal keyboard interception, letting keystrokes pass through to the remote session.

## Status

Phase 1 (scaffold, auth, passthrough basics), Phase 2 (keyboard matrix, shortcut-inhibit), and the macOS port are complete. Produces installable `.deb` and `.rpm` on Linux and `.app` + `.dmg` on macOS from `cargo tauri build`. Remaining Phase 3 items: session-drop detection, clipboard portal wiring, status strip.

## Architecture

- **Tauri v2** (Rust backend, system WebView frontend)
- **Single WebView** handles the entire flow: Microsoft auth + MFA, tenant picker, app launch, RDP session
- **Entry point:** `https://myapps.microsoft.com` — no session URL persistence
- **WebView engine:** webkit2gtk-4.1 on Linux, WKWebView on macOS

## Key Constraints

- **Keyboard passthrough is the core feature.** No application menu. No Tauri accelerators. Disable WebKitGTK context menu and Ctrl+scroll zoom. Two intentional in-app chords, both captured via JS injection in the WebView (not global shortcuts — those don't work on Wayland): `Ctrl+Alt+Shift+.` toggles the keyboard-inhibit, `Ctrl+Alt+Shift+F` toggles fullscreen. Period was chosen over Escape because GNOME/Mutter eats modifier+Escape when inhibit is OFF, preventing toggle-back.
- **Compositor-level keys pass through via `zwp_keyboard_shortcuts_inhibit_manager_v1`.** rdpls requests the inhibitor at startup against the main surface and default seat. niri honors it automatically, so Alt+Tab, Super, etc. reach the remote client. `Ctrl+Alt+Shift+.` toggles the inhibit on/off.
- **UA spoofing required.** WebKitGTK default UA triggers Microsoft "unsupported browser" warnings. Spoof current Firefox or Edge.
- **Persistent cookies.** Stable WebKit data directory so auth survives across launches. Microsoft issues session-only tokens for the per-app RDP step; this requires re-auth on restart even in Firefox. Not fixable on our side.
- **Popup → main-window redirect.** Microsoft launches RDP sessions via `window.open()`; intercept webkit2gtk's `create` signal and load the URL in the main WebView instead.
- **Wayland-native.** No `GDK_BACKEND=x11`. Target is niri on Fedora 43. Shares GTK's existing Wayland connection via `wayland-backend::Backend::from_foreign_display` — never open a second connection, surfaces/seats wouldn't cross over.
- **Web client cannot receive Win-modifier keys.** Microsoft's HTML5 RDP
  client drops Meta/Super/Cmd keydowns — `Win+L`, `Win+D`, `Win+Arrow`
  etc. are not deliverable through the web client, period. The only
  Win-proxy it accepts is `Alt+F3` (Start menu). rdpls remaps a bare
  Super/Cmd *tap* to Alt+F3; Win-combos must be handled in-guest (e.g.
  AutoHotkey). See `docs/keyboard-matrix.md` for the full matrix.
- **Window decorations are platform-split.** Linux uses `decorations(false)` only when `XDG_CURRENT_DESKTOP` names a compositor that manages undecorated surfaces (currently just niri); on GNOME/Mutter and unknown compositors it falls back to a standard titlebar because Mutter expects CSD and offers no drag fallback. macOS keeps the standard title bar; WKWebView has no compositor-level drag/resize fallback and injecting custom drag regions into Microsoft's pages isn't practical.
- **Starts fullscreen.** Window builder sets `.fullscreen(true)` so Mutter's titlebar is out of the way from the start. `Ctrl+Alt+Shift+F` toggles in and out.
- **Conditional Access passes without managed browser** — Firefox on Fedora already works, so the same UA string works here.

## Targets

- **Linux:** Wayland-native; niri is the tested compositor. Builds on Fedora-family (dnf) and Debian-family (apt) distros. Compositor must honor `zwp_keyboard_shortcuts_inhibit_manager_v1` for full passthrough.
- **macOS:** macOS 14+; Apple Silicon is the tested arch. Same codebase, separate build.

## Naming

`rdpls` — lowercase everywhere. Binary, window title, desktop file. Pronounced "R-D-please."

## Build Prerequisites

Stable Rust (1.77.2+) + Tauri v2 CLI on both platforms:

```bash
cargo install tauri-cli --version "^2"
```

Platform-specific system deps:

```bash
# Linux (Fedora / RHEL / openSUSE)
sudo dnf install webkit2gtk4.1-devel gtk3-devel libsoup3-devel \
  javascriptcoregtk4.1-devel pango-devel cairo-devel gdk-pixbuf2-devel

# Linux (Debian / Ubuntu)
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev libpango1.0-dev libcairo2-dev libgdk-pixbuf-2.0-dev \
  build-essential pkg-config

# macOS
xcode-select --install
```

## Development Commands

Run from the repo root:

```bash
cargo tauri dev          # Run in development mode
cargo tauri build        # Build release binary + bundle (Linux: .deb/.rpm; macOS: .app/.dmg)
cargo test               # Run Rust tests
```

Release artifacts land in `src-tauri/target/release/` (binary) and `src-tauri/target/release/bundle/{deb,rpm,macos,dmg}/` (packages).

## Known Follow-ups

- Bundle identifier is `com.rdpls.client`. Renamed from `com.rdpls.app` for the macOS port — `.app` collides with the bundle extension and the Tauri bundler warns on every build. Linux users re-auth once after the rename (WebKit data directory is keyed to the identifier).
- Session-drop detection, clipboard portal wiring, and status strip are Phase 3.
- macOS keyboard passthrough uses an `NSEvent` local monitor (`src-tauri/src/passthrough_macos.rs`) that swallows a small list of browser-like shortcuts (`Cmd+R`, `Cmd+[`, `Cmd+]` and their Shift variants) when inhibit is on, and passes everything else through. Exit on macOS is via the red traffic light — closing the window quits the app. The swallow list is intentionally narrow; see `docs/macos-keyboard-passthrough.md` and extend only with reason.

## Project Layout

```
src-tauri/
  src/main.rs                  # Binary entry → calls rdpls_lib::run
  src/lib.rs                   # Tauri builder, window creation, per-platform WebView setup
  src/keyboard.rs              # Injected JS: escape hotkey + toast (Linux + macOS), rdpls_exit
  src/shortcuts_inhibit.rs     # Linux only: Wayland shortcut-inhibit protocol wiring
  src/passthrough_macos.rs     # macOS only: NSEvent local monitor + rdpls_toggle_passthrough
  src/passthrough_linux.rs     # Linux only: WebView key-event hook + virtual-keyboard injection for Super-tap → Alt+F3
  src/tap_tracker.rs           # Pure state machine for Super/Cmd tap detection (platform-agnostic)
  Cargo.toml                   # Rust dependencies (target-gated per-OS blocks)
  tauri.conf.json              # Tauri config (window, bundle targets, CSP)
  capabilities/default.json    # Tauri v2 capability file
src/
  index.html                   # Minimal loader; WebView navigates to myapps.microsoft.com
docs/
  keyboard-matrix.md           # Linux keyboard passthrough test results
  macos-keyboard-passthrough.md  # macOS passthrough design + OS-reserved key list
  macos-karabiner.md           # Karabiner-Elements rules for Mac → Windows key remapping
  plans/                       # Implementation plans
```
