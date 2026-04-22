# rdpls

A dedicated browser for Microsoft's web-based RDP client (Windows 365 / AVD).

Microsoft's RDP experience is browser-only — no native client, no `.rdp` file. That's fine, except browsers hijack keys the remote desktop needs: Ctrl+W closes a tab, F5 reloads the page, F11 fullscreens the wrong layer, Alt+Tab switches windows. `rdpls` is a minimal Tauri wrapper around the same web session that gets out of the way: no menus, no browser accelerators, no chrome. Keystrokes pass through to the remote client.

Pronounced "R-D-please."

## Status

Works end-to-end on both platforms:

- **Linux (Wayland):** full compositor-level passthrough via `zwp_keyboard_shortcuts_inhibit_manager_v1` — Alt+Tab, Super, etc. reach the remote on compositors that honor the protocol.
- **macOS:** in-window passthrough via an `NSEvent` local monitor that swallows a narrow list of WKWebView accelerators (`Cmd+R`, `Cmd+[`, `Cmd+]` and Shift variants) so they don't reload or navigate the outer page. OS-reserved keys (`Cmd+Tab`, `Cmd+Space`, Mission Control, etc.) are unreachable by any application; remap on the macOS side if you need them as Windows shortcuts.

Both platforms share:

- Microsoft auth including MFA
- Persistent outer auth across launches (the per-app session token is session-only by Microsoft's design; Firefox has the same behavior)
- Full keyboard passthrough of normal and browser-hijacked keys
- Popup/new-window redirect into the main WebView (Microsoft launches RDP sessions via `window.open`)
- UA spoof to avoid the "unsupported browser" page
- No window chrome beyond what the platform requires (Linux: decorationless; macOS: keeps the title bar so drag/resize still work)

## Install

### From packaged artifacts

After `cargo tauri build`:

```bash
# macOS (Apple Silicon)
open src-tauri/target/release/bundle/dmg/rdpls_0.1.0_aarch64.dmg
# drag rdpls.app into /Applications

# Fedora / RHEL
sudo dnf install src-tauri/target/release/bundle/rpm/rdpls-0.1.0-1.x86_64.rpm

# Debian / Ubuntu
sudo dpkg -i src-tauri/target/release/bundle/deb/rdpls_0.1.0_amd64.deb
```

### From source

Both platforms need a stable Rust toolchain (1.77.2 or newer; install via [rustup](https://rustup.rs)) and the Tauri v2 CLI:

```bash
cargo install tauri-cli --version "^2"
```

#### Linux

Wayland-native build — no `GDK_BACKEND=x11`. You need GTK 4 and WebKitGTK 6.0 development headers, plus the usual graphics/text deps and the `cargo-deb` / `cargo-generate-rpm` bundlers.

```bash
# Fedora / RHEL / openSUSE (dnf/zypper)
sudo dnf install gtk4-devel webkitgtk6.0-devel libsoup3-devel \
  pango-devel cairo-devel gdk-pixbuf2-devel

# Debian / Ubuntu (apt)
sudo apt install libgtk-4-dev libwebkitgtk-6.0-dev libsoup-3.0-dev \
  libpango1.0-dev libcairo2-dev libgdk-pixbuf-2.0-dev \
  build-essential pkg-config

cargo install cargo-deb cargo-generate-rpm
```

The Linux binary is a direct gtk4-rs + webkit6 app under `src-linux/` (not a Tauri/wry WebView — see "Project layout" and `CLAUDE.md` for the why).

Compositor-level keyboard passthrough requires a Wayland compositor that implements `zwp_keyboard_shortcuts_inhibit_manager_v1`. niri is tested; other compositors may or may not honor the protocol. Without it, Alt+Tab / Super stay with the compositor and the rest of the passthrough still works.

#### macOS

Apple Silicon is tested; Intel should build from the same source but isn't regularly verified. Requires the Xcode Command Line Tools for the Apple SDK headers and linker:

```bash
xcode-select --install
```

#### Build + run

Platform-specific, from the repo root:

```bash
# Linux
cargo run --release -p rdpls-linux   # run in place
make build                           # release binary + .deb + .rpm

# macOS
cargo tauri dev          # run against the dev WebView
cargo tauri build        # release binary + installable bundle
```

Bundles land in:

- Linux: `target/debian/rdpls_*.deb` and `target/generate-rpm/rdpls-*.rpm`
- macOS: `src-tauri/target/release/bundle/{macos,dmg}/` (`.app` + `.dmg`)

## Usage

Launch `rdpls`. The WebView opens `https://myapps.microsoft.com`. Log in, click through to your Windows 365 / AVD app, and the session opens in the same window.

### Local keys

`Ctrl+Alt+Shift+.` toggles keyboard passthrough, and `Ctrl+Alt+Shift+F` toggles fullscreen. The toast tells you which state you're in.

- **Keys → Remote** (default): the remote session gets the keys.
  - On Linux, the Wayland compositor inhibit is on — Alt+Tab and Super pass through too.
  - On macOS, the `NSEvent` monitor is installed — WKWebView accelerators like `Cmd+R` are swallowed so they don't reload the page.
- **Keys → Local**: the local environment handles its bindings again.
  - On Linux, useful when you want to Alt+Tab out of rdpls without closing it.
  - On macOS, useful if you need the WebView's own `Cmd+R` / `Cmd+F` / back/forward.

Everything else is always a remote key.

### Closing the app

- **Linux:** no title-bar close button. Use your compositor's close binding (e.g. `Mod+Q` in niri's default config).
- **macOS:** click the red traffic light. Closing the window quits the app.

### macOS: Karabiner rules for Windows-style shortcuts

macOS sends `Cmd` where Windows expects `Ctrl`, and the web RDP client doesn't translate between them. The recommended fix is a scoped Karabiner-Elements ruleset — `Cmd+A/Z/S/F/R/...` → `Ctrl+...` only while rdpls is frontmost, plus text-navigation remaps (`Cmd+←` → `Home`, `Option+←` → `Ctrl+←`, etc.). Ready-to-paste JSON lives in `docs/macos-karabiner.md` along with notes on why `Cmd+C/V/X` and `Cmd+Q/H/M` are intentionally left alone.

## Known limitations

- **Session-level auth on each restart.** Microsoft issues short-lived per-app tokens for the RDP session itself; these are cleared when the WebView closes and cannot be persisted. This is intrinsic to Microsoft's flow — Firefox behaves the same way if fully closed. The outer `myapps.microsoft.com` auth does persist.
- **Linux: Alt+Tab / Super require the inhibit to be on.** The shortcut-inhibit protocol is compositor-dependent. niri supports it natively. Other Wayland compositors may not; in that case those keys stay with the compositor.
- **macOS: OS-reserved keys cannot reach the remote.** `Cmd+Tab`, `Cmd+Space`, `Ctrl+↑/↓/←/→` (Mission Control / Spaces), the F3/F4 keys, and media/brightness keys are captured by macOS below the application layer. Remap at System Settings → Keyboard or via `hidutil` if you need them routed to the remote. The Karabiner rules above cover the in-window cases; OS-level interception is outside what an app can do.
- **macOS: the Win/meta key does not reach the remote.** Microsoft's web RDP client drops the Win/`metaKey` modifier across all platforms. Use `Alt+Home` as the Start-menu substitute.

## Targets

- **Linux:** Wayland compositor that honors `zwp_keyboard_shortcuts_inhibit_manager_v1` for full passthrough (niri tested; others may partially work). Builds on Fedora-family and Debian-family distros; see the build section above.
- **macOS:** macOS 14+, Apple Silicon tested. Intel should build from the same source.

## Project layout

Cargo workspace with per-platform crates — Linux runs a direct gtk4-rs + webkit6 app (no Tauri), macOS stays on Tauri.

```
Cargo.toml                     # Workspace (default-members = ["src-linux"])
src-linux/                     # Linux binary (gtk4-rs + webkit6 + wayland-client)
  Cargo.toml                   # incl. cargo-deb / cargo-generate-rpm metadata
  src/main.rs                  # gtk::Application + ApplicationWindow + WebView
  src/keyboard.rs              # Injected JS (hotkey chord + toast, webkit6 IPC)
  src/shortcuts_inhibit.rs     # Wayland shortcut-inhibit protocol wiring
  assets/rdpls.desktop         # XDG desktop entry (installed by cargo-deb/-rpm)
src-tauri/                     # macOS binary (Tauri v2 + WKWebView)
  src/lib.rs                   # Tauri app, window + WebView setup
  src/keyboard.rs              # Injected JS (macOS init script + Tauri commands)
  src/passthrough_macos.rs     # NSEvent local monitor
  tauri.conf.json              # Bundle + window config
src/
  index.html                   # Empty loader (macOS WebView navigates out immediately)
docs/
  keyboard-matrix.md           # Linux key passthrough test results
  macos-keyboard-passthrough.md  # macOS passthrough design + what's reachable
  macos-karabiner.md           # Ready-to-paste Karabiner rules
  plans/                       # Implementation plans
```
