# Plan: Migrate Linux rdpls from Tauri (webkit2gtk-4.1 / GTK3) to direct gtk4-rs + webkit6

## Context

The Microsoft RDP session renders with blocky H.264 macroblock noise and pixel-trail
ghosting on scroll under `rdpls` compared to Firefox on the same machine. Earlier
fixes (RPM Fusion `ffmpeg` swap to get `avdec_h264` at rank `primary`, enabling
`WEBKIT_FORCE_COMPOSITING_MODE`, pinning `HardwareAccelerationPolicy::Always`)
closed part of the gap but the user reports the quality is still visibly worse
than Firefox. The remaining plausible wins are in WebKit's media/compositor
stack, which has moved to the GTK4 port (webkitgtk-6.0):

- Native `wp_fractional_scale_v1` support (GTK3 can't do fractional scaling on Wayland)
- Mature DMA-BUF zero-copy video renderer
- Newer GStreamer/WebCodecs integration with the GPU process
- Proper Wayland surface handling

Upstream Tauri/wry does not ship GTK4 support: the community port (wry PR #1530,
tao #1104) is a conflicting draft and was explicitly abandoned by its author on
2026-04-22. Waiting isn't a realistic path. The Linux binary is small (~500
lines across four files) and most of what Tauri provides (window, WebView embed,
JS↔Rust bridge) maps 1:1 onto a small set of gtk4-rs + webkit6 calls.

**Decision (already confirmed with user):** drop Tauri on Linux and build a
direct gtk4-rs + webkit6 binary. macOS stays on Tauri unchanged.

## Final shape

Cargo workspace at the repo root with two member crates:

- `src-tauri/` — **macOS-only** Tauri app. All Linux code paths, deps, and
  Tauri bundle targets removed.
- `src-linux/` — **new Linux-only** standalone binary using `gtk4`, `webkit6`,
  `wayland-client`. Bundles via `cargo-deb` / `cargo-generate-rpm`.

Shared assets (icons) continue to live under `src-tauri/icons/`; `src-linux`
references them by relative path for its bundle metadata.

## Step-by-step

### 1. Workspace scaffold

- **New file: `/Cargo.toml`** (workspace root). Members: `["src-tauri", "src-linux"]`. Resolver 2.
- Root `.gitignore` already covers `target/`; confirm still correct under workspace.

### 2. Trim Tauri crate to macOS-only

- `src-tauri/Cargo.toml`: delete the `[target.'cfg(target_os = "linux")'.dependencies]` block (webkit2gtk, gtk, gdk, glib, gdkwayland-sys, wayland-*).
- `src-tauri/tauri.conf.json`: set `bundle.targets` to `["app", "dmg"]` (drop `deb`, `rpm`).
- `src-tauri/src/lib.rs`:
  - Delete the `#[cfg(target_os = "linux")] mod shortcuts_inhibit;` and Linux-only imports.
  - Delete the Linux arm of `configure_webview`, `compositor_owns_decorations`, `desktop_owns_decorations`, and the Linux cfg test module. Keep the stub `#[cfg(not(target_os = "linux"))] fn configure_webview` (now always selected on macOS).
  - Remove Linux-only env var at top of `run()` and Linux branches of `WebviewWindowBuilder` (decorations logic) — macOS is the only path.
  - Remove `shortcuts_inhibit::rdpls_toggle_inhibit` from `invoke_handler`.
- `src-tauri/src/shortcuts_inhibit.rs`: **delete** (moves to `src-linux`).
- `src-tauri/src/keyboard.rs`: remove the Linux-only `HOTKEY_SCRIPT` constant and the `#[cfg(target_os = "linux")]` gates; keep the macOS path. The `rdpls_exit` / `rdpls_toggle_fullscreen` commands stay Tauri commands for macOS.

### 3. New `src-linux` crate

- **`src-linux/Cargo.toml`**:
  ```toml
  [package] name = "rdpls"  edition = "2021"  rust-version = "1.77.2"
  [[bin]] name = "rdpls"  path = "src/main.rs"
  [dependencies]
  gtk4 = "0.9"
  webkit6 = "0.5"             # GTK4 WebKitGTK bindings; verify current version at execute time
  glib = "0.20"
  gdk4-wayland = "0.9"
  wayland-client = "0.31"
  wayland-backend = { version = "0.3", features = ["client_system"] }
  wayland-protocols = { version = "0.32", features = ["unstable", "client"] }
  log = "0.4"
  env_logger = "0.11"
  once_cell = "1"

  [package.metadata.deb]  # cargo-deb bundle config
  maintainer = "Tyler Hays <tyhays@gmail.com>"
  depends = "libwebkitgtk-6.0-4, libgtk-4-1"
  assets = [
    ["target/release/rdpls", "usr/bin/", "755"],
    ["assets/rdpls.desktop", "usr/share/applications/", "644"],
    ["../src-tauri/icons/128x128.png", "usr/share/icons/hicolor/128x128/apps/rdpls.png", "644"],
  ]

  [package.metadata.generate-rpm]  # cargo-generate-rpm bundle config
  # equivalent assets + requires webkitgtk6.0, gtk4
  ```
  Pin concrete versions by running `cargo add` during execute; the numbers
  above are approximate and should be confirmed against crates.io at
  execution time.

- **`src-linux/src/main.rs`** — GTK4 `Application` entry point:
  - Set `WEBKIT_FORCE_COMPOSITING_MODE=1` before `gtk::init`.
  - Build a `gtk::Application`, connect `activate` to create a single `ApplicationWindow`:
    - Title `"rdpls"`, default size 1280×800, fullscreened on realize, decorated per `XDG_CURRENT_DESKTOP` check (reuse the existing `desktop_owns_decorations` logic verbatim).
  - Create a `webkit6::WebView` bound to a persistent `WebsiteDataManager` keyed to `~/.local/share/com.rdpls.client/` (matches current Tauri data dir so existing cookies survive).
  - Wire `Settings`:
    - `user_agent` → `SPOOFED_USER_AGENT` (move the Linux constant from `src-tauri/src/lib.rs` into `src-linux/src/main.rs`).
    - `hardware_acceleration_policy` → `HardwareAccelerationPolicy::Always`.
    - `enable_developer_extras` → false.
  - Connect `context-menu` signal → return true (suppress default menu).
  - Connect `create` signal → `load_uri(req.uri())` on the main WebView and return None (popup redirect; 1:1 with existing webkit2gtk code path in `src-tauri/src/lib.rs` at line 121).
  - Attach the injected user scripts (see step 4).
  - Register script message handlers that replace Tauri commands: `rdpls_toggle_inhibit`, `rdpls_toggle_fullscreen`, `rdpls_exit`. Each handler calls the corresponding Rust function and then evaluates a small JS snippet to send the result back to the page (or the JS calls `.postMessage(null)` synchronously and observes window state itself — see step 4).
  - On `connect_realize` of the window, call `shortcuts_inhibit::install(&window, &webview)`.
  - On `close-request` → `app.quit()`.

- **`src-linux/src/shortcuts_inhibit.rs`** — port of the existing
  `src-tauri/src/shortcuts_inhibit.rs`:
  - Keep the `InhibitState`, `Sink`, `toggle()`, and overall structure intact
    — the `wayland-client` / `wayland-protocols` usage is unchanged.
  - Replace the GTK3 surface extraction:
    - `gtk_window.window()` (GdkWindow) → `gtk_window.surface()` (GdkSurface).
    - `gdk_wayland_window_get_wl_surface(...)` → `gdk4_wayland::WaylandSurface::wl_surface(&surface)` (or raw FFI `gdk_wayland_surface_get_wl_surface`, whichever the `gdk4-wayland` crate exposes cleanly).
    - `gdk_wayland_seat_get_wl_seat(...)` → `gdk4_wayland::WaylandSeat::wl_seat(&seat)`.
  - Replace the Tauri `install(webview_window)` signature with a plain
    function taking `&gtk::ApplicationWindow`.
  - Replace `#[tauri::command]` on `rdpls_toggle_inhibit` with a plain
    function invoked from the webkit6 script message handler.

- **`src-linux/src/keyboard.rs`** — port of the existing Linux
  `HOTKEY_SCRIPT`:
  - Keep the chord logic (`Ctrl+Alt+Shift+.` → toggle inhibit, `Ctrl+Alt+Shift+F` → toggle fullscreen) and the toast DOM snippet verbatim.
  - Replace the Tauri IPC call:
    ```js
    window.__TAURI_INTERNALS__.invoke(cmd).then(function (on) {...})
    ```
    with webkit6's script-message path:
    ```js
    window.webkit.messageHandlers[cmd].postMessage(null);
    ```
  - Because webkit6 script-message handlers don't return a promise, the
    current "toast shows new state" logic needs a small change: the Rust
    handler, after flipping state, calls `webview.evaluate_javascript` to
    push the new state back into a global callback (e.g.,
    `window.__rdpls_state_update('inhibit', true)`). The toast function
    then reads from this callback. Alternatively, compute the new state
    synchronously in JS (track `window.__rdpls_inhibit_on` locally) and
    display the toast before the message round-trip; the Rust side is just
    the effector. The latter is simpler — use it.
  - Cross-frame relay (`postMessage` to `window.top`) stays exactly the same.

- **`src-linux/assets/rdpls.desktop`** — standard XDG desktop entry:
  ```
  [Desktop Entry] Name=rdpls  Exec=rdpls  Icon=rdpls  Type=Application  Categories=Utility;
  ```

### 4. Packaging

- Install bundling tools as part of `install-deps`:
  ```
  cargo install cargo-deb cargo-generate-rpm
  ```
- Build flow on Linux (replaces `cargo tauri build`):
  ```
  cargo build --release -p rdpls --bin rdpls
  cargo deb -p rdpls --no-build
  cargo generate-rpm -p rdpls
  ```
  Artifacts land in `target/release/` and `target/debian/` / `target/generate-rpm/`.

### 5. Makefile

`/Makefile` currently treats Linux and macOS symmetrically through `cargo
tauri build`. Rework:

- `PACKAGES_RPM` → `webkitgtk6.0-devel gtk4-devel` + the same non-toolkit deps
  (`pango-devel`, `cairo-devel`, `gdk-pixbuf2-devel`).
- `PACKAGES_DEB` → `libwebkitgtk-6.0-dev libgtk-4-dev` + the same base deps.
- `install-deps`: on Linux additionally `cargo install cargo-deb cargo-generate-rpm`; on macOS still installs `tauri-cli`.
- `build`: on Linux runs the three-command flow from step 4; on macOS stays `cargo tauri build`.
- `install`: path changes under Linux (`target/debian/*.deb`, `target/generate-rpm/*.rpm`) instead of `src-tauri/target/release/bundle/`.
- `uninstall` unchanged.

### 6. Documentation

- `README.md`: update the Linux build-from-source block to list the new packages and build command. The compositor/Wayland/passthrough notes stay accurate (that code path didn't change functionally).
- `CLAUDE.md`:
  - "Architecture" section: note the Linux/macOS code split and the move to gtk4-rs + webkit6 on Linux.
  - "Key Constraints" section: the `decorations(false)` constraint is now expressed via `ApplicationWindow::set_decorated(false)` gated on the same `XDG_CURRENT_DESKTOP` check.
  - "Build Prerequisites" section: replace the webkit2gtk-4.1 / gtk3 package list with webkitgtk6.0 / gtk4. Keep the existing "Fedora H.264 decoder" subsection verbatim — it applies identically to webkit6.
  - "Project Layout": reflect the `src-tauri/` (macOS) + `src-linux/` (Linux) split.
  - "Known Follow-ups": add a note that upstream Tauri/wry has not shipped GTK4, wry PR #1530 is stalled as of 2026-04-22, and rejoining mainstream Tauri on Linux requires waiting for an upstream release or revisiting this decision.
- `docs/keyboard-matrix.md`: the tested-on line should be updated after verification on the new build.

## Critical files

Will be created:
- `/Cargo.toml` (workspace)
- `/src-linux/Cargo.toml`
- `/src-linux/src/main.rs`
- `/src-linux/src/shortcuts_inhibit.rs`
- `/src-linux/src/keyboard.rs`
- `/src-linux/assets/rdpls.desktop`

Will be deleted:
- `/src-tauri/src/shortcuts_inhibit.rs`

Will be modified:
- `/src-tauri/Cargo.toml` (drop Linux deps)
- `/src-tauri/tauri.conf.json` (drop Linux bundle targets)
- `/src-tauri/src/lib.rs` (remove Linux branches; the file becomes a macOS-only Tauri app)
- `/src-tauri/src/keyboard.rs` (drop Linux-only `HOTKEY_SCRIPT` and cfg gates)
- `/Makefile` (swap package lists and Linux build flow)
- `/README.md` (Linux deps + build commands)
- `/CLAUDE.md` (architecture/layout/prereqs + known-follow-up note)

## Functions / logic to reuse verbatim

- Decoration-policy logic: `desktop_owns_decorations(&str) -> bool` and the
  six existing unit tests in `src-tauri/src/lib.rs:158-210`. Move to
  `src-linux/src/main.rs` (or a small `util.rs`) unchanged.
- Wayland inhibit protocol plumbing: `shortcuts_inhibit::{InhibitState, Sink, toggle, setup}` core shape. Only the GDK surface-extraction calls change.
- All three injected-JS scripts in `src-tauri/src/keyboard.rs` HOTKEY_SCRIPT: IS_TOP branching, cross-frame postMessage relay, showToast animation, keydown listener with the two chords. The only edit is swapping `window.__TAURI_INTERNALS__.invoke(cmd)` for `window.webkit.messageHandlers[cmd].postMessage(null)` plus computing the post-toggle state locally in JS rather than awaiting a round-trip.
- User-agent constants and zoom-suppression user script (`src-tauri/src/lib.rs:131-138`).
- Popup redirect via `create` signal handler (`src-tauri/src/lib.rs:121-128`).

## Verification

After implementation, confirm in order:

1. **Build (Linux):** `make install-deps REINSTALL=1` installs the new GTK4 packages. Then `cd src-linux && cargo build --release` must succeed. `cargo test -p rdpls` must pass (the decoration unit tests carry over).
2. **Build (macOS):** on a Mac, `cargo tauri build` must still produce `.app` and `.dmg`. Verify bundle identifier `com.rdpls.client` unchanged.
3. **Runtime (Linux):** launch the new binary. Step through:
   - Opens fullscreen at `https://myapps.microsoft.com`.
   - Auth flow completes (cookies persist across restarts at the same data directory as before — verifies no re-auth regression).
   - Clicking a Windows 365 / AVD app launches an RDP session in the same window (popup redirect works).
   - `Ctrl+Alt+Shift+.` toggles keyboard-inhibit with a toast and actually affects Alt+Tab passthrough on niri/GNOME.
   - `Ctrl+Alt+Shift+F` toggles fullscreen with a toast.
   - Closing the window quits the process.
4. **Bundles (Linux):** `cargo deb -p rdpls` and `cargo generate-rpm -p rdpls` produce `.deb`/`.rpm`. Install one, confirm the `.desktop` shows in the launcher and the binary runs from `/usr/bin/rdpls`.
5. **Quality check (the whole reason we did this):** re-run the Firefox-vs-rdpls comparison on an active RDP session. Document the result in `docs/keyboard-matrix.md` (or a sibling `docs/video-quality.md`). If quality now matches Firefox, the migration paid off; if it doesn't, we have eliminated GTK3/webkit-4.1 as the bottleneck and the remaining gap is in AMD driver / Mesa / Microsoft's tenant — no further client-side lever.
6. **Smoke (macOS):** launch the rebuilt macOS bundle, confirm passthrough hotkey, `Cmd+R` swallow, and exit-on-close behavior all still work (we removed Linux-only cfg gates from shared files; need to make sure nothing load-bearing was deleted).

## Risk notes

- The `webkit6` crate version to pin must be confirmed at execution time — it's been iterating. Same for `gdk4-wayland`.
- webkit6 `Settings::set_hardware_acceleration_policy` exists (same API as webkit2gtk 2.0.2); verify when touching the code.
- If `gdk4-wayland` doesn't surface `WaylandSeat::wl_seat()` through the high-level crate, fall back to raw FFI via `gdk4-wayland-sys` (same pattern the current code uses).
- If `cargo-deb` / `cargo-generate-rpm` prove awkward for the asset layout Tauri was producing, `cargo-bundle` is a backup.
