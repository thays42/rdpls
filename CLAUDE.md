# rdpls

Dedicated RDP browser wrapper built with Tauri. Hosts Microsoft's web-based RDP client (Windows 365 / AVD) in a native WebView with minimal keyboard interception, letting keystrokes pass through to the remote session.

## Architecture

- **Tauri v2** (Rust backend, system WebView frontend)
- **Single WebView** handles the entire flow: Microsoft auth + MFA, tenant picker, app launch, RDP session
- **Entry point:** `https://myapps.microsoft.com` — no session URL persistence
- **WebView engine:** webkit2gtk-4.1 on Linux, WKWebView on macOS

## Key Constraints

- **Keyboard passthrough is the core feature.** No application menu. No global shortcut registrations except `Ctrl+Alt+Shift+Escape`. Disable WebKitGTK context menu and Ctrl+scroll zoom.
- **UA spoofing required.** WebKitGTK default UA triggers Microsoft "unsupported browser" warnings. Spoof current Firefox or Edge.
- **Persistent cookies.** Stable WebKit data directory so auth survives across launches.
- **Wayland-native.** No `GDK_BACKEND=x11`. Target is niri on Fedora 43.
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

```bash
cargo tauri dev          # Run in development mode
cargo tauri build        # Build release binary
cargo test               # Run Rust tests
```

## Project Layout

```
src-tauri/
  src/main.rs            # Tauri app setup, window config, UA override
  src/keyboard.rs        # Escape hotkey registration, key passthrough logic
  Cargo.toml             # Rust dependencies
  tauri.conf.json        # Tauri config (window, permissions, CSP)
  capabilities/          # Tauri v2 capability files
src/
  index.html             # Minimal loader (redirects to myapps.microsoft.com)
```
