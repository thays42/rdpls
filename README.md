# rdpls

A dedicated browser for Microsoft's web-based RDP client (Windows 365 / AVD).

Microsoft's RDP experience is browser-only — no native client, no `.rdp` file. That's fine, except browsers hijack keys the remote desktop needs: Ctrl+W closes a tab, F5 reloads the page, F11 fullscreens the wrong layer, Alt+Tab switches windows. `rdpls` is a minimal Tauri wrapper around the same web session that gets out of the way: no menus, no browser accelerators, no chrome. Keystrokes pass through to the remote client.

Pronounced "R-D-please."

## Status

Works end-to-end on the primary target (Fedora 43 + niri + Framework 13).

- Completes Microsoft auth including MFA
- Persists the outer auth across launches (the per-app session token is session-only, by Microsoft; Firefox has the same behavior)
- Full keyboard passthrough — every browser-hijacked key reaches the remote
- Compositor-level passthrough too (Alt+Tab, Super) via the `zwp_keyboard_shortcuts_inhibit_manager_v1` Wayland protocol, on by default

macOS (Mac Studio M4 Max) is a planned secondary target.

## Install

### From packaged artifacts

After `cargo tauri build`:

```bash
# Fedora / RHEL
sudo dnf install src-tauri/target/release/bundle/rpm/rdpls-0.1.0-1.x86_64.rpm

# Debian / Ubuntu
sudo dpkg -i src-tauri/target/release/bundle/deb/rdpls_0.1.0_amd64.deb
```

### From source

Build prerequisites on Fedora 43:

```bash
sudo dnf install webkit2gtk4.1-devel gtk3-devel libsoup3-devel \
  javascriptcoregtk4.1-devel pango-devel cairo-devel gdk-pixbuf2-devel
cargo install tauri-cli --version "^2"
```

Run:

```bash
cargo tauri dev
```

## Usage

Launch `rdpls`. The WebView opens `https://myapps.microsoft.com`. Log in, click through to your Windows 365 / AVD app, and the session opens in the same window.

### The one local key

`Ctrl+Alt+Shift+Escape` toggles the Wayland shortcuts-inhibit state.

- **On (default):** the compositor hands all keys to rdpls — Alt+Tab and Super included. This is what you want while working in the remote session.
- **Off:** the compositor handles its own bindings again. Useful when you want to Alt+Tab out of rdpls without closing it.

Everything else is a remote key.

### Closing the app

There's no title-bar close button. Use your compositor's close binding (e.g. `Mod+Q` in niri's default config).

## Known limitations

- **Session-level auth on each restart.** Microsoft issues short-lived per-app tokens for the RDP session itself; these are cleared when the WebView closes and cannot be persisted. This is intrinsic to Microsoft's flow — Firefox behaves the same way if fully closed. The outer `myapps.microsoft.com` auth does persist.
- **Alt+Tab / Super require the inhibit to be on.** The shortcut-inhibit protocol is compositor-dependent. niri supports it natively. Other Wayland compositors may not; in that case those keys stay with the compositor.
- **Icons are placeholders.** Tauri's default logo, pending real branding.

## Targets

- **Primary:** Fedora 43, niri, Framework 13 (x86_64)
- **Secondary (planned):** macOS, Mac Studio M4 Max (aarch64)

## Project layout

```
src-tauri/
  src/lib.rs                  # Tauri app, window + webkit2gtk setup
  src/keyboard.rs             # Injected JS for the escape hotkey
  src/shortcuts_inhibit.rs    # Wayland shortcut-inhibit protocol wiring
  tauri.conf.json             # Bundle + window config
src/
  index.html                  # Empty loader (WebView navigates out immediately)
docs/
  keyboard-matrix.md          # Key passthrough test results
  plans/                      # Implementation plans
```
