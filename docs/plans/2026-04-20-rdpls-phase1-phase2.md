# rdpls Phase 1–2 Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Tauri v2 wrapper that loads Microsoft's web RDP client with full keyboard passthrough and persistent auth.

**Architecture:** Single-window Tauri v2 app with webkit2gtk-4.1 WebView. Rust backend handles window setup, UA spoofing, WebView settings, and one escape hotkey. No frontend framework — the WebView navigates directly to `https://myapps.microsoft.com`. Keyboard passthrough is achieved by omitting all accelerators and menus, not by intercepting and forwarding keys.

**Tech Stack:** Rust, Tauri v2, webkit2gtk-4.1, GTK3

---

## Chunk 1: Project Scaffold and Core WebView

### Task 1: Install Tauri CLI and system dependencies

**Files:** None (system setup)

- [ ] **Step 1: Install system build dependencies**

```bash
sudo dnf install webkit2gtk4.1-devel gtk3-devel libsoup3-devel \
  javascriptcoregtk4.1-devel pango-devel cairo-devel gdk-pixbuf2-devel
```

- [ ] **Step 2: Install Tauri CLI**

```bash
cargo install tauri-cli --version "^2"
```

- [ ] **Step 3: Verify installation**

```bash
cargo tauri --version
```

Expected: `tauri-cli 2.x.x`

---

### Task 2: Initialize Tauri project

**Files:**
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/capabilities/default.json`
- Create: `src/index.html`

- [ ] **Step 1: Scaffold the project**

```bash
cd /home/thays/projects/rdpls
cargo tauri init
```

When prompted:
- App name: `rdpls`
- Window title: `rdpls`
- Frontend dev URL: leave default
- Frontend dist: `../src`

- [ ] **Step 2: Initialize git repo and create .gitignore**

```bash
git init
```

Create `.gitignore`:
```
/target
/src-tauri/target
```

- [ ] **Step 3: Create minimal frontend loader**

Write `src/index.html`:

```html
<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>rdpls</title></head>
<body></body>
</html>
```

This file exists only to satisfy Tauri's dist requirement. The WebView will navigate away from it immediately.

- [ ] **Step 4: Verify the scaffold builds**

```bash
cargo tauri build --debug 2>&1 | tail -5
```

Expected: successful build (ignore warnings about missing icons for now)

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: scaffold Tauri v2 project"
```

---

### Task 3: Configure window and external URL navigation

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Configure tauri.conf.json**

Set these fields in `tauri.conf.json`:

```json
{
  "app": {
    "windows": [
      {
        "title": "rdpls",
        "fullscreen": false,
        "resizable": true,
        "width": 1280,
        "height": 800,
        "url": "https://myapps.microsoft.com"
      }
    ],
    "security": {
      "dangerousRemoteUrlAccess": [
        {
          "url": "https://*",
          "enableWebviewAccess": true
        }
      ]
    }
  },
  "bundle": {
    "identifier": "com.rdpls.app",
    "active": true,
    "targets": "all"
  }
}
```

Note: `dangerousRemoteUrlAccess` is needed in Tauri v2 to allow the WebView to load external URLs and still call Tauri APIs. If this exact key doesn't exist in the Tauri v2 version installed, check the Tauri v2 docs for the equivalent CSP/security config that allows external URL navigation. The window `url` field with an external URL may require specific security permissions.

- [ ] **Step 2: Update main.rs for basic app setup**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error running rdpls");
}
```

- [ ] **Step 3: Test that the app launches and navigates to myapps.microsoft.com**

```bash
cargo tauri dev
```

Expected: A window opens and loads `https://myapps.microsoft.com`. You should see Microsoft's login page or the myapps portal.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/tauri.conf.json src-tauri/src/main.rs
git commit -m "feat: configure window to load myapps.microsoft.com"
```

---

### Task 4: Persistent cookie storage

**Files:**
- Modify: `src-tauri/src/main.rs`

The goal: cookies survive app restarts so users don't re-authenticate every launch.

- [ ] **Step 1: Research Tauri v2's WebView data persistence**

Tauri v2 with webkit2gtk stores WebView data in `~/.local/share/<bundle-identifier>/` by default. Verify this is the case:

```bash
cargo tauri dev &
sleep 5
ls -la ~/.local/share/com.rdpls.app/ 2>/dev/null || ls -la ~/.local/share/rdpls/ 2>/dev/null
kill %1
```

If data persists there already, no code change needed — just document it. If not, configure a stable data directory via Tauri's `WebviewBuilder` or the underlying webkit2gtk data manager.

- [ ] **Step 2: Verify cookie persistence**

1. `cargo tauri dev` — log into Microsoft, complete MFA
2. Close the app
3. `cargo tauri dev` again — should land at the authenticated portal, not the login page

- [ ] **Step 3: Commit (if changes were needed)**

```bash
git add src-tauri/
git commit -m "feat: configure persistent WebView data directory"
```

---

### Task 5: User-Agent spoofing

**Files:**
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Write the UA override**

In `main.rs`, use Tauri's `WebviewBuilder` setup hook or the `on_webview` callback to set a custom User-Agent. The target UA string:

```
Mozilla/5.0 (X11; Linux x86_64; rv:138.0) Gecko/20100101 Firefox/138.0
```

The approach depends on Tauri v2's API. Options:
1. `WebviewBuilder::user_agent()` if available
2. Access the underlying webkit2gtk `WebView` via `with_webview()` and call `webkit_settings_set_user_agent()`

```rust
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let webview = app.get_webview_window("main")
                .expect("no main window");

            // Tauri v2 approach — check docs for exact API
            // Option A: if .with_webview() is available
            webview.with_webview(|wv| {
                #[cfg(target_os = "linux")]
                {
                    use webkit2gtk::prelude::*;
                    let settings = wv.settings().unwrap();
                    settings.set_user_agent(Some(
                        "Mozilla/5.0 (X11; Linux x86_64; rv:138.0) Gecko/20100101 Firefox/138.0"
                    ));
                }
            })?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error running rdpls");
}
```

Note: The exact API may differ. Consult Tauri v2 docs and the `tauri::WebviewWindow` API. The `with_webview()` method gives access to the platform-native WebView handle. On Linux this is a `webkit2gtk::WebView`.

- [ ] **Step 2: Add webkit2gtk dependency if needed**

In `src-tauri/Cargo.toml`, add under `[target.'cfg(target_os = "linux")'.dependencies]`:

```toml
webkit2gtk = "2.0"
```

Check the version that matches your Tauri v2 installation's transitive dependency — use the same major version.

- [ ] **Step 3: Verify UA spoofing**

```bash
cargo tauri dev
```

Navigate to `https://myapps.microsoft.com` — Microsoft's login should not show "unsupported browser" warnings. To confirm the UA, open the WebView inspector (if available) or check network requests.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/
git commit -m "feat: spoof Firefox UA to pass Microsoft browser checks"
```

---

### Task 6: Disable menus, context menu, and zoom

**Files:**
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/tauri.conf.json`

- [ ] **Step 1: Remove application menu**

In `main.rs`, ensure no menu is set. Tauri v2 may create a default menu on some platforms. Explicitly set an empty menu or use `tauri::menu::Menu::new()`:

```rust
use tauri::menu::Menu;

tauri::Builder::default()
    .menu(|app| Menu::new(app))
    // ... rest of setup
```

- [ ] **Step 2: Disable WebKitGTK context menu and zoom**

Inside the `with_webview()` callback:

```rust
webview.with_webview(|wv| {
    #[cfg(target_os = "linux")]
    {
        use webkit2gtk::prelude::*;
        let settings = wv.settings().unwrap();
        settings.set_user_agent(Some(
            "Mozilla/5.0 (X11; Linux x86_64; rv:138.0) Gecko/20100101 Firefox/138.0"
        ));
        // Disable context menu
        settings.set_enable_developer_extras(false);
        // Disable zoom (Ctrl+scroll)
        settings.set_zoom_text_only(false);
    }
})?;

// Also connect to context-menu signal to suppress it
webview.with_webview(|wv| {
    #[cfg(target_os = "linux")]
    {
        use webkit2gtk::prelude::*;
        // Return true from context-menu signal to suppress it
        wv.connect_context_menu(|_wv, _menu, _event, _hit| {
            true // suppress context menu
        });
    }
})?;
```

Note: Check Tauri v2 + webkit2gtk API for exact signal connection. The `connect_context_menu` approach returning `true` suppresses the default context menu.

For zoom, also inject JS to prevent Ctrl+scroll:

```rust
// In setup, after page load:
webview.eval(r#"
    document.addEventListener('wheel', function(e) {
        if (e.ctrlKey) { e.preventDefault(); }
    }, { passive: false });
"#)?;
```

- [ ] **Step 3: Verify**

```bash
cargo tauri dev
```

- No menu bar visible
- Right-click does not show a context menu
- Ctrl+scroll does not zoom the page

- [ ] **Step 4: Commit**

```bash
git add src-tauri/
git commit -m "feat: disable menus, context menu, and Ctrl+scroll zoom"
```

---

### Task 7: Phase 1 acceptance test

- [ ] **Step 1: Full end-to-end test**

```bash
cargo tauri dev
```

Walk through:
1. App launches and shows Microsoft login
2. Complete auth including MFA
3. Navigate to an RDP session through the portal
4. Session renders and is interactive
5. Close app, relaunch — lands at authenticated portal (no re-auth)

- [ ] **Step 2: Document any issues found**

Create `docs/known-issues.md` with any problems encountered.

- [ ] **Step 3: Commit**

```bash
git add docs/
git commit -m "docs: Phase 1 acceptance test results"
```

---

## Chunk 2: Keyboard Passthrough and Escape Hotkey

### Task 8: Keyboard test matrix — baseline

**Files:**
- Create: `docs/keyboard-matrix.md`

- [ ] **Step 1: Create the test matrix template**

```markdown
# Keyboard Test Matrix

Tested on: Fedora 43, niri, webkit2gtk-4.1
Date: 2026-04-20

| Key | Expected (reaches RDP) | Result | Notes |
|-----|----------------------|--------|-------|
| F5 | Refresh in remote | | |
| F11 | Fullscreen in remote | | |
| Ctrl+W | Close tab in remote | | |
| Ctrl+T | New tab in remote | | |
| Ctrl+L | Address bar in remote | | |
| Ctrl+R | Refresh in remote | | |
| Alt+Left | Back in remote | | |
| Alt+Right | Forward in remote | | |
| Ctrl+Shift+T | Reopen tab in remote | | |
| Ctrl+N | New window in remote | | |
| Ctrl+Tab | Switch tab in remote | | |
| Alt+F4 | Close window (WM) | | |
```

- [ ] **Step 2: Test each key during a live RDP session**

Launch `cargo tauri dev`, connect to an RDP session, and test each key. Record results in the matrix.

- [ ] **Step 3: Commit**

```bash
git add docs/keyboard-matrix.md
git commit -m "docs: baseline keyboard test matrix"
```

---

### Task 9: Fix keyboard leaks

**Files:**
- Modify: `src-tauri/src/main.rs` (or create `src-tauri/src/keyboard.rs`)

This task depends entirely on what Task 8 reveals. The likely fixes:

- [ ] **Step 1: Identify leaked keys from the test matrix**

Keys that were intercepted locally instead of reaching the remote session.

- [ ] **Step 2: Fix each leaked key**

Common fixes:
- **Tauri default accelerators:** Remove via `tauri.conf.json` or by clearing the menu
- **WebKitGTK accelerators:** Override in webkit2gtk settings or by connecting to `key-press-event` and consuming the event before GTK processes it
- **Window manager interception:** These can't be fixed in the app — document as known limitations (e.g., Alt+F4 on most WMs)

For GTK-level key suppression, if needed:

```rust
// In with_webview callback
use gdk::prelude::*;
use gtk::prelude::*;

// Get the GTK window and remove default accelerator groups
let gtk_window = wv.parent_window();
for accel_group in gtk_window.accel_groups() {
    gtk_window.remove_accel_group(&accel_group);
}
```

Note: The exact API depends on the GTK/webkit2gtk version. This is the area most likely to need iteration.

- [ ] **Step 3: Re-test the full matrix**

Update `docs/keyboard-matrix.md` with post-fix results.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/ docs/keyboard-matrix.md
git commit -m "fix: suppress local keyboard interception for RDP passthrough"
```

---

### Task 10: Escape hotkey (Ctrl+Alt+Shift+Escape)

**Files:**
- Create: `src-tauri/src/keyboard.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Create keyboard module**

Create `src-tauri/src/keyboard.rs`:

```rust
use tauri::{AppHandle, Manager};

/// Register the single local escape hotkey: Ctrl+Alt+Shift+Escape
/// Behavior: quit the application (simplest safe behavior pending design decision)
pub fn register_escape_hotkey(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};

    let shortcut = Shortcut::new(
        Some(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT),
        Code::Escape,
    );

    app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            std::process::exit(0);
        }
    })?;

    Ok(())
}
```

Note: This uses `tauri-plugin-global-shortcut`. The behavior (quit) is the simplest safe default. The handoff lists this as an unresolved decision — quit is a reasonable starting point that can be changed later.

- [ ] **Step 2: Add the global-shortcut plugin dependency**

In `src-tauri/Cargo.toml`:

```toml
[dependencies]
tauri-plugin-global-shortcut = "2"
```

In `src-tauri/capabilities/default.json`, add the permission:

```json
{
  "permissions": [
    "core:default",
    "global-shortcut:allow-register"
  ]
}
```

- [ ] **Step 3: Wire it into main.rs**

```rust
mod keyboard;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::init())
        .setup(|app| {
            keyboard::register_escape_hotkey(&app.handle())?;
            // ... existing setup
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error running rdpls");
}
```

- [ ] **Step 4: Test the hotkey**

1. `cargo tauri dev`
2. Connect to an RDP session
3. Press `Ctrl+Alt+Shift+Escape`
4. App should quit

- [ ] **Step 5: Commit**

```bash
git add src-tauri/
git commit -m "feat: add Ctrl+Alt+Shift+Escape escape hotkey"
```

---

### Task 11: Phase 2 acceptance — final keyboard matrix

- [ ] **Step 1: Complete the keyboard matrix with all fixes applied**

Run full test pass. Update `docs/keyboard-matrix.md`.

- [ ] **Step 2: Update known-issues.md**

Any keys that can't be fixed (WM-intercepted, etc.) go here with explanation.

- [ ] **Step 3: Commit**

```bash
git add docs/
git commit -m "docs: final Phase 2 keyboard matrix and known issues"
```

---

## Chunk 3: Packaging and Desktop Integration

### Task 12: Desktop file and icon

**Files:**
- Create: `src-tauri/icons/` (Tauri generates these)
- Modify: `src-tauri/tauri.conf.json`

- [ ] **Step 1: Generate placeholder icons**

```bash
cargo tauri icon src-tauri/icons/placeholder.png
```

If no source icon exists, create a simple one or use Tauri's defaults. The handoff marks branding as unresolved — a placeholder is fine.

- [ ] **Step 2: Configure bundle metadata**

In `tauri.conf.json`:

```json
{
  "bundle": {
    "identifier": "com.rdpls.app",
    "icon": ["icons/32x32.png", "icons/128x128.png", "icons/icon.png"],
    "targets": ["deb", "rpm", "appimage"],
    "shortDescription": "Dedicated RDP browser wrapper",
    "category": "Network"
  }
}
```

- [ ] **Step 3: Build release binary**

```bash
cargo tauri build
```

- [ ] **Step 4: Test the packaged app**

Install and run the built package. Verify:
- Desktop file appears in app launcher
- App launches from the desktop file
- Full auth + RDP flow works

- [ ] **Step 5: Commit**

```bash
git add src-tauri/
git commit -m "feat: configure packaging with desktop file and placeholder icons"
```

---

## Verification

After completing all tasks:

1. **Clean build:** `cargo tauri build` succeeds
2. **Auth flow:** Launch -> MS login -> MFA -> portal -> RDP session works
3. **Cookie persistence:** Close and relaunch — no re-auth
4. **Keyboard passthrough:** All keys in matrix reach remote client (except documented WM exceptions)
5. **Escape hotkey:** `Ctrl+Alt+Shift+Escape` quits from inside RDP session
6. **No local interception:** No menus, no context menu, no Ctrl+scroll zoom
7. **UA spoofing:** No Microsoft "unsupported browser" warnings

## Open Items (Not in Scope)

These are from the handoff's "Unresolved Decisions" — to be addressed later:

- **Escape hotkey behavior:** Currently quits. Could become an overlay or mode toggle.
- **Repo hosting:** Decide where this lives permanently.
- **Icon/branding:** Replace placeholder.
- **Phase 3 scope:** Session drop detection, status strip, clipboard portal, multi-session.
