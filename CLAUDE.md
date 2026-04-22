# rdpls

Dedicated RDP browser wrapper built with Tauri. Hosts Microsoft's web-based RDP client (Windows 365 / AVD) in a native WebView with minimal keyboard interception, letting keystrokes pass through to the remote session.

## Status

Phase 1 (scaffold, auth, passthrough basics), Phase 2 (keyboard matrix, shortcut-inhibit), and the macOS port are complete. Produces installable `.deb` and `.rpm` on Linux and `.app` + `.dmg` on macOS from `cargo tauri build`. Remaining Phase 3 items: session-drop detection, clipboard portal wiring, status strip.

## Architecture

Cargo workspace with two per-platform crates. Linux and macOS share the JS-level UX (hotkey chord, toast, popup redirect) but use different native stacks:

- **`src-linux/`** — direct **gtk4-rs + webkit6 + wayland-client** binary. No Tauri. Why: upstream Tauri/wry still targets webkit2gtk-4.1 (GTK3) as of April 2026; the community GTK4/webkit6 port (wry PR #1530) is a stalled draft. GTK4 + webkit6 unlocks `wp_fractional_scale_v1`, the DMA-BUF zero-copy video path, and the newer GStreamer/WebCodecs integration that WebKit's GPU process relies on — the reasons the port exists are in `docs/plans/2026-04-22-gtk4-webkit6-linux.md`.
- **`src-tauri/`** — **Tauri v2 + WKWebView** on macOS. Unchanged in intent from Phase 1; now explicitly macOS-only (its `Cargo.toml` has no `cfg(target_os = "linux")` section and `tauri.conf.json` bundles only `app` / `dmg`).
- **Single WebView** on each platform handles the entire flow: Microsoft auth + MFA, tenant picker, app launch, RDP session.
- **Entry point:** `https://myapps.microsoft.com` — no session URL persistence.
- **WebView engine:** webkitgtk-6.0 on Linux, WKWebView on macOS.

## Key Constraints

- **Keyboard passthrough is the core feature.** No application menu. No Tauri accelerators. Disable WebKitGTK context menu and Ctrl+scroll zoom. Two intentional in-app chords, both captured via JS injection in the WebView (not global shortcuts — those don't work on Wayland): `Ctrl+Alt+Shift+.` toggles the keyboard-inhibit, `Ctrl+Alt+Shift+F` toggles fullscreen. Period was chosen over Escape because GNOME/Mutter eats modifier+Escape when inhibit is OFF, preventing toggle-back.
- **Compositor-level keys pass through via `zwp_keyboard_shortcuts_inhibit_manager_v1`.** rdpls requests the inhibitor at startup against the main surface and default seat. niri honors it automatically, so Alt+Tab, Super, etc. reach the remote client. `Ctrl+Alt+Shift+.` toggles the inhibit on/off.
- **UA spoofing required.** WebKitGTK default UA triggers Microsoft "unsupported browser" warnings. Spoof current Firefox or Edge.
- **Persistent cookies.** Stable WebKit data directory so auth survives across launches. Microsoft issues session-only tokens for the per-app RDP step; this requires re-auth on restart even in Firefox. Not fixable on our side.
- **Popup → main-window redirect.** Microsoft launches RDP sessions via `window.open()`; on Linux intercept webkit6's `create` signal and load the URL in the main WebView; on macOS monkey-patch `window.open` in an injected script (WKWebView exposes no `create` equivalent that can suppress the popup).
- **Wayland-native.** No `GDK_BACKEND=x11`. Target is niri on Fedora 43. Shares GTK's existing Wayland connection via `wayland-backend::Backend::from_foreign_display` — never open a second connection, surfaces/seats wouldn't cross over.
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
sudo dnf install gtk4-devel webkitgtk6.0-devel libsoup3-devel \
  pango-devel cairo-devel gdk-pixbuf2-devel

# Linux (Debian / Ubuntu)
sudo apt install libgtk-4-dev libwebkitgtk-6.0-dev libsoup-3.0-dev \
  libpango1.0-dev libcairo2-dev libgdk-pixbuf-2.0-dev \
  build-essential pkg-config

# Linux bundling tools (on top of either distro)
cargo install cargo-deb cargo-generate-rpm

# macOS
xcode-select --install
```

### Fedora H.264 decoder (runtime; affects RDP session quality)

WebKitGTK decodes Microsoft's RDP session video through GStreamer. Fedora's default `ffmpeg-free` ships without H.264 (patents stripped), so the GStreamer libav plugin doesn't expose `avdec_h264`. The only remaining H.264 decoder is Cisco's `openh264dec` (rank `marginal`), which lacks CABAC and most main/high-profile features — sessions render with blocky macroblocks and pixel-trail ghosting on scroll.

Swap to RPM Fusion's full ffmpeg so `avdec_h264` registers at rank `primary`:

```bash
sudo dnf install https://mirrors.rpmfusion.org/free/fedora/rpmfusion-free-release-$(rpm -E %fedora).noarch.rpm
sudo dnf swap ffmpeg-free ffmpeg --allowerasing
rm -rf ~/.cache/gstreamer-1.0/   # force plugin registry rescan
```

Verify with `gst-inspect-1.0 avdec_h264` (should list `Rank: primary (256)`). Note that AMD Radeon iGPUs on Krackan (gfx1152) and similar currently expose VA-API hardware decoders only for AV1, VP9, and JPEG via Mesa — no hardware H.264 — so the FFmpeg software decoder is the realistic ceiling until either (a) Mesa adds VCN H.264 for gfx1152 or (b) AVD/W365 begins negotiating AV1 through the HTML5 client.

## Development Commands

Run from the repo root. Linux and macOS have different build flows because they are separate crates.

```bash
# Linux (builds rdpls-linux only; cargo build's default-member is src-linux)
cargo run --release -p rdpls-linux
make build                           # release binary + .deb + .rpm
cargo test -p rdpls-linux            # unit tests

# macOS
cargo tauri dev
cargo tauri build                    # release binary + .app/.dmg
cargo test -p rdpls                  # unit tests (the macOS crate is still named `rdpls`)
```

Release artifacts:
- Linux: `target/release/rdpls` (binary), `target/debian/rdpls_*.deb`, `target/generate-rpm/rdpls-*.rpm`.
- macOS: `src-tauri/target/release/bundle/{macos,dmg}/` (`.app` and `.dmg`).

## Known Follow-ups

- **Upstream Tauri/wry does not ship GTK4/webkit6** (as of 2026-04-22). The Linux port in `src-linux/` exists because wry PR #1530 is a conflicting draft and was abandoned by its author the same day we started this work; wry issue #1474 was unassigned the same day. If upstream ships GTK4 later, the Linux binary could in principle move back under Tauri for code unification — re-evaluate at that point whether the rewrite is worth reversing.
- Bundle identifier is `com.rdpls.client`. Renamed from `com.rdpls.app` for the macOS port — `.app` collides with the bundle extension and the Tauri bundler warns on every build. Linux users re-auth once after the rename (WebKit data directory is keyed to the identifier). The new gtk4+webkit6 Linux binary uses the same app id so cookies survive where possible (webkit6 picks its data directory from `gtk::Application::application-id`).
- Session-drop detection, clipboard portal wiring, and status strip are Phase 3.
- macOS keyboard passthrough uses an `NSEvent` local monitor (`src-tauri/src/passthrough_macos.rs`) that swallows a small list of browser-like shortcuts (`Cmd+R`, `Cmd+[`, `Cmd+]` and their Shift variants) when inhibit is on, and passes everything else through. Exit on macOS is via the red traffic light — closing the window quits the app. The swallow list is intentionally narrow; see `docs/macos-keyboard-passthrough.md` and extend only with reason.

## Project Layout

Cargo workspace, `default-members = ["src-linux"]` so bare `cargo build` at the repo root builds only the Linux crate (avoids pulling Tauri/wry's webkit2gtk-4.1 transitive deps on Linux dev boxes).

```
Cargo.toml                     # Workspace root
src-linux/                     # Linux binary (package: rdpls-linux; bin: rdpls)
  Cargo.toml                   # gtk4/webkit6/wayland deps; cargo-deb + cargo-generate-rpm metadata
  src/main.rs                  # gtk::Application, ApplicationWindow, WebView wiring
  src/keyboard.rs              # Injected JS: HOTKEY_SCRIPT + ZOOM_SUPPRESS_SCRIPT
  src/shortcuts_inhibit.rs     # Wayland zwp_keyboard_shortcuts_inhibit protocol wiring
  assets/rdpls.desktop         # XDG desktop entry (installed by bundlers)
src-tauri/                     # macOS binary (package: rdpls; bin: rdpls)
  src/main.rs                  # Binary entry → calls rdpls_lib::run
  src/lib.rs                   # Tauri builder, window creation, WKWebView setup
  src/keyboard.rs              # Injected JS: MACOS_INIT_SCRIPT; rdpls_exit / rdpls_toggle_fullscreen
  src/passthrough_macos.rs     # NSEvent local monitor + rdpls_toggle_passthrough
  Cargo.toml                   # Rust dependencies (macOS-only target section)
  tauri.conf.json              # Tauri config (app + dmg bundle targets, CSP)
  capabilities/default.json    # Tauri v2 capability file
src/
  index.html                   # Minimal loader (macOS only; Linux WebView loads ENTRY_URL directly)
docs/
  keyboard-matrix.md           # Linux keyboard passthrough test results
  macos-keyboard-passthrough.md  # macOS passthrough design + OS-reserved key list
  macos-karabiner.md           # Karabiner-Elements rules for Mac → Windows key remapping
  plans/                       # Implementation plans
```
