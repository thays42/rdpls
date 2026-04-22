# rdpls

A dedicated browser for Microsoft's web-based RDP client (Windows 365 / AVD).

Microsoft's RDP experience is browser-only — no native client, no `.rdp` file. That's fine, except browsers hijack keys the remote desktop needs: Ctrl+W closes a tab, F5 reloads the page, F11 fullscreens the wrong layer, Alt+Tab switches windows. `rdpls` gets out of the way: no menus, no browser accelerators, no chrome. Keystrokes pass through to the remote client.

Pronounced "R-D-please."

## Status

Works end-to-end on both platforms, with different host stacks:

- **Linux (Wayland):** Firefox Developer Edition in `--kiosk` + sideloaded MV3 WebExtension. Passthrough via the browser Keyboard Lock API — Firefox requests `zwp_keyboard_shortcuts_inhibit_manager_v1` and suppresses its own chrome shortcuts, so Alt+Tab, Super, Ctrl+W, F11, etc. all reach the remote. Single-user build — no packaging.
- **macOS:** Tauri v2 + WKWebView with an `NSEvent` local monitor that swallows a narrow list of WKWebView accelerators (`Cmd+R`, `Cmd+[`, `Cmd+]` and Shift variants). OS-reserved keys (`Cmd+Tab`, `Cmd+Space`, Mission Control, etc.) remain unreachable by any application; remap on the macOS side if you need them as Windows shortcuts.

Both platforms share:

- Microsoft auth including MFA
- Persistent outer auth across launches (the per-app session token is session-only by Microsoft's design; Firefox behaves the same way)
- Full keyboard passthrough of normal and browser-hijacked keys
- Popup / new-window redirect into the main view (Microsoft launches RDP sessions via `window.open`)
- No unnecessary window chrome

## Install

### Linux (from source — the only path)

1. **Install Firefox Developer Edition** once from Mozilla:

   ```bash
   mkdir -p ~/.local/opt && cd ~/.local/opt
   curl -L -o firefox-dev.tar.xz 'https://download.mozilla.org/?product=firefox-devedition-latest-ssl&os=linux64&lang=en-US'
   tar -xJf firefox-dev.tar.xz && mv firefox firefox-dev && rm firefox-dev.tar.xz
   ```

   (Dev Edition is required because it permits sideloaded unsigned extensions. Release Firefox refuses them unconditionally.)

2. **Optional — for the extension dev loop:**

   ```bash
   npm install -g web-ext
   ```

3. **Install rdpls:**

   ```bash
   git clone <this repo> && cd rdpls
   make install
   ```

   This seeds a dedicated profile at `~/.local/share/rdpls/profile/`, symlinks the extension into it, and writes a `.desktop` entry at `~/.local/share/applications/rdpls.desktop`. Launch via your compositor's launcher or:

   ```bash
   ~/.local/opt/firefox-dev/firefox --profile ~/.local/share/rdpls/profile --kiosk --no-remote --new-instance https://myapps.microsoft.com
   ```

Compositor-level keys pass through if your Wayland compositor implements `zwp_keyboard_shortcuts_inhibit_manager_v1`. niri is tested; most modern compositors support it. Without it, Alt+Tab / Super stay with the compositor and the rest of the passthrough still works.

### macOS (from source)

Requires a stable Rust toolchain (1.77.2+; install via [rustup](https://rustup.rs)), the Xcode Command Line Tools, and the Tauri v2 CLI:

```bash
xcode-select --install
cargo install tauri-cli --version "^2"
```

Build and install:

```bash
cargo tauri build            # .app + .dmg in src-tauri/target/release/bundle/
make install                 # copies .app into /Applications/
```

Or `cargo tauri dev` for a live dev session.

## Usage

Launch `rdpls`. The browser opens `https://myapps.microsoft.com`. Log in, click through to your Windows 365 / AVD app, and the session opens in the same window.

### Local keys

**Linux** (chord set handled by the WebExtension):

- `Ctrl+Alt+Shift+.` — toggle keyboard lock. On: compositor and browser chords both reach the remote (Alt+Tab, Super, Ctrl+W, F11, …). Off: local environment owns them again. A transient toast shows the resulting state.
- `Ctrl+Alt+Shift+Q` — quit rdpls.
- `Ctrl+Alt+Shift+/` — safety-net unconditional unlock if you ever get stuck in lock-on mode.

**macOS** (same toggle chord, different mechanism):

- `Ctrl+Alt+Shift+.` — toggle the `NSEvent` monitor. On: WKWebView accelerators like `Cmd+R` are swallowed. Off: back/forward, reload, find work again.
- `Ctrl+Alt+Shift+F` — toggle fullscreen.
- Quit via the red traffic light — closing the window quits the app.

### macOS: Karabiner rules for Windows-style shortcuts

macOS sends `Cmd` where Windows expects `Ctrl`, and the web RDP client doesn't translate between them. The recommended fix is a scoped Karabiner-Elements ruleset — `Cmd+A/Z/S/F/R/...` → `Ctrl+...` only while rdpls is frontmost, plus text-navigation remaps (`Cmd+←` → `Home`, `Option+←` → `Ctrl+←`, etc.). Ready-to-paste JSON lives in `docs/macos-karabiner.md` along with notes on why `Cmd+C/V/X` and `Cmd+Q/H/M` are intentionally left alone.

## Known limitations

- **Session-level auth on each restart.** Microsoft issues short-lived per-app tokens for the RDP session itself; these are cleared when the browser closes and cannot be persisted. This is intrinsic to Microsoft's flow — Firefox behaves the same way if fully closed. The outer `myapps.microsoft.com` auth does persist.
- **Linux: Alt+Tab / Super require lock to be on.** The shortcut-inhibit protocol is compositor-dependent. niri supports it natively; most modern compositors do.
- **macOS: OS-reserved keys cannot reach the remote.** `Cmd+Tab`, `Cmd+Space`, `Ctrl+↑/↓/←/→` (Mission Control / Spaces), F3/F4, media/brightness keys are captured by macOS below the application layer. Remap at System Settings → Keyboard or via `hidutil`. Karabiner covers the in-window cases; OS-level interception is outside what an app can do.
- **macOS: the Win/meta key does not reach the remote.** Microsoft's web RDP client drops the Win/`metaKey` modifier across all platforms. Use `Alt+Home` as the Start-menu substitute.

## Targets

- **Linux:** Wayland compositor that honors `zwp_keyboard_shortcuts_inhibit_manager_v1` (niri tested). Firefox Developer Edition required.
- **macOS:** macOS 14+, Apple Silicon tested. Intel should build from the same source.

## Project layout

```
src-firefox/
  extension/
    manifest.json                # MV3
    background.js                # quit + lock-state tracking
    content.js                   # all frames: quit chord, window.open shim, toast DOM
    content-fullscreen.js        # top frame only: Fullscreen API + Keyboard Lock toggle
  profile/
    user.js                      # seeded prefs
Makefile                          # Linux install/uninstall/dev + macOS install/build
src-tauri/
  src/lib.rs                   # Tauri app, window, WKWebView
  src/keyboard.rs              # Injected JS (chord, toast, popup redirect)
  src/passthrough_macos.rs     # NSEvent local monitor
  tauri.conf.json              # Bundle + window config
src/
  index.html                   # macOS loader
docs/
  firefox-keyboard-flow.md     # Linux Keyboard Lock findings + reference
  macos-keyboard-passthrough.md  # macOS passthrough design + what's reachable
  macos-karabiner.md             # Ready-to-paste Karabiner rules
  superpowers/specs/             # Design specs
  superpowers/plans/             # Implementation plans
```
