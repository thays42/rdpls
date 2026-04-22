# rdpls

Dedicated RDP browser wrapper with platform-split hosts: Firefox Developer Edition on Linux, Tauri + WKWebView on macOS. Hosts Microsoft's web-based RDP client (Windows 365 / AVD) and keeps keystrokes reaching the remote session.

## Status

Linux pivoted from WebKitGTK (Tauri v2 webkit2gtk-4.1, later a direct gtk4/webkit6 attempt on `gtk4-webkit6-linux`) to Firefox Developer Edition in `--kiosk` + a sideloaded MV3 WebExtension. Single-user scope — no packaging/signing/distribution. macOS port (Tauri v2 + WKWebView) is unchanged from Phase 1/2. Phase 3 items (session-drop detection, clipboard portal, status strip) remain outstanding on both platforms.

## Architecture

- **Linux:** Firefox Developer Edition in `--kiosk` + sideloaded MV3 WebExtension. No Rust/Tauri on Linux. Extension owns chord capture and `navigator.keyboard.lock()` toggling from a top-frame content script (activation-preserving). Installed via `make install`.
- **macOS:** Tauri v2 + WKWebView. Unchanged.
- **Single WebView per platform** handles the entire flow: Microsoft auth + MFA, tenant picker, app launch, RDP session.
- **Entry point:** `https://myapps.microsoft.com` — no session URL persistence.
- **Engine:** Gecko (Firefox Dev Edition) on Linux, WKWebView on macOS.

## Key Constraints

- **Keyboard passthrough via Keyboard Lock API on Linux.** The extension's top-frame content script (`content-fullscreen.js`) calls `document.documentElement.requestFullscreen()` then `navigator.keyboard.lock()`. Firefox then requests `zwp_keyboard_shortcuts_inhibit_manager_v1` on its Wayland surface and suppresses its own chrome shortcuts, so compositor chords (Super, Alt+Tab) and browser chords (Ctrl+W, Ctrl+L, F11, …) reach the page. Chord set:
  - `Ctrl+Alt+Shift+.` — toggle keyboard lock (Fullscreen API + Keyboard Lock)
  - `Ctrl+Alt+Shift+Q` — quit (`browser.windows.remove`)
  - `Ctrl+Alt+Shift+/` — safety-net unconditional unlock + exitFullscreen
- **Keyboard passthrough on macOS** uses the existing `NSEvent` local monitor (`src-tauri/src/passthrough_macos.rs`) with a narrow swallow list. Unchanged from Phase 1/2.
- **No UA spoofing needed** on Linux — Firefox's default UA already passes M365 browser checks.
- **Persistent cookies** via the dedicated profile at `~/.local/share/rdpls/profile/`. Microsoft's per-app RDP tokens are session-only even in stock Firefox, so re-auth on launch is expected.
- **Popup redirect**: page-world `window.open` shim injected by the extension is the primary mechanism — Microsoft passes a features string to `window.open`, which bypasses the `browser.link.open_newwindow` prefs; the prefs are kept as a backstop. macOS still uses its injected-script monkey-patch.
- **Always fullscreen**: Firefox `--kiosk` on Linux; Tauri `.fullscreen(true)` on macOS. Fullscreen API state (distinct from kiosk window fullscreen) is coupled to lock state on Linux — not an independently togglable chord.
- **Wayland-native on Linux.** Target is niri on Fedora 43; any compositor that implements `zwp_keyboard_shortcuts_inhibit_manager_v1` works, since Firefox itself requests the inhibit via Keyboard Lock.

## Targets

- **Linux:** Wayland-native; niri on Fedora 43 is the tested compositor. Requires Firefox Developer Edition (sideloaded extensions require its `xpinstall.signatures.required=false` path, which Release Firefox refuses).
- **macOS:** macOS 14+; Apple Silicon is the tested arch. Builds via `cargo tauri build`.

## Naming

`rdpls` — lowercase everywhere. Binary, window title, desktop file. Pronounced "R-D-please."

## Build Prerequisites

### Linux (Firefox Dev Edition, installed by the user)

```bash
mkdir -p ~/.local/opt && cd ~/.local/opt
curl -L -o firefox-dev.tar.xz 'https://download.mozilla.org/?product=firefox-devedition-latest-ssl&os=linux64&lang=en-US'
tar -xJf firefox-dev.tar.xz && mv firefox firefox-dev && rm firefox-dev.tar.xz
# Optional: for 'make dev' live reload during extension work
npm install -g web-ext
```

### macOS (Tauri v2 CLI)

```bash
xcode-select --install
cargo install tauri-cli --version "^2"
```

## Development Commands

```bash
# Linux
make install          # seeds profile, symlinks extension, writes desktop entry
make dev              # web-ext run with live reload
make uninstall        # remove install (preserves profile data)
make uninstall-hard   # remove install + profile data

# macOS (unchanged)
cargo tauri dev
cargo tauri build     # .app + .dmg in src-tauri/target/release/bundle/
```

## Known Follow-ups

- Extension addon-id is `rdpls@local`; profile sideload path is `~/.local/share/rdpls/profile/extensions/rdpls@local/` (symlink to `src-firefox/extension/`). The directory name must exactly match `browser_specific_settings.gecko.id` in `manifest.json`.
- macOS bundle identifier is `com.rdpls.client` (renamed from `com.rdpls.app` to avoid the `.app` bundle-extension collision).
- Session-drop detection, clipboard portal wiring, and status strip are Phase 3.
- macOS keyboard passthrough uses an `NSEvent` local monitor that swallows a small list of browser-like shortcuts (`Cmd+R`, `Cmd+[`, `Cmd+]` and Shift variants) when inhibit is on, and passes everything else through. Exit on macOS is via the red traffic light. The swallow list is intentionally narrow; see `docs/macos-keyboard-passthrough.md` and extend only with reason.

## Project Layout

```
src-firefox/                     # Linux (Firefox-kiosk + WebExtension)
  extension/
    manifest.json                # MV3; gecko.id = "rdpls@local"
    background.js                # lock-state tracking + quit handler
    content.js                   # all frames: quit chord, window.open shim, toast DOM
    content-fullscreen.js        # top frame only: Fullscreen API + Keyboard Lock toggle
  profile/
    user.js                      # seeded prefs (dom.keyboard-lock.enabled, etc.)
Makefile                          # Linux install/uninstall/dev + macOS install/build
src-tauri/                        # macOS (Tauri v2 + WKWebView) — unchanged from Phase 1/2
  src/main.rs                    # Binary entry -> rdpls_lib::run
  src/lib.rs                     # Tauri builder, window, WebView
  src/keyboard.rs                # Injected JS: MACOS_INIT_SCRIPT, rdpls_exit, rdpls_toggle_fullscreen
  src/passthrough_macos.rs       # NSEvent local monitor + rdpls_toggle_passthrough
  Cargo.toml                     # macOS-only target section
  tauri.conf.json                # app + dmg bundle targets, CSP
  capabilities/default.json
src/
  index.html                     # macOS loader (Linux WebExtension runs against ENTRY_URL directly)
docs/
  firefox-keyboard-flow.md       # Linux Keyboard Lock findings + ongoing reference
  macos-keyboard-passthrough.md  # macOS passthrough design + OS-reserved key list
  macos-karabiner.md             # Karabiner-Elements rules for Mac -> Windows key remapping
  plans/                         # legacy implementation plans (pre-superpowers)
  superpowers/
    specs/                       # design specs
    plans/                       # implementation plans
```
