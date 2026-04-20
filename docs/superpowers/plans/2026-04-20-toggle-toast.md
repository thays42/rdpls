# Toggle Toast Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show a transient on-screen pill (`Keys → Remote` / `Keys → Local`) each time `Ctrl+Alt+Shift+Escape` toggles the Wayland keyboard-shortcuts-inhibit state.

**Architecture:** Extend the existing WebKitGTK-injected user script in `src-tauri/src/keyboard.rs`. The Tauri command `rdpls_toggle_inhibit` already returns `bool` (`true` ⇒ inhibit ON / keys reach remote, `false` ⇒ inhibit OFF / keys stay local); the resolved value flows through `invoke`'s promise and drives the toast text. No Rust changes. No new native surfaces.

**Tech Stack:** Rust raw string literal holding vanilla JS; WebKitGTK's `UserContentManager` injects the script into every frame.

**Spec:** `docs/superpowers/specs/2026-04-20-toggle-toast-design.md`

---

## Chunk 1: Extend the injected script

### Task 1: Replace `ESCAPE_HOTKEY_SCRIPT` with the toast-capable version

**Files:**
- Modify: `src-tauri/src/keyboard.rs:9-23` (the `ESCAPE_HOTKEY_SCRIPT` constant)

**Design notes for the implementer (read before editing):**
- The injected script runs in every frame (`UserContentInjectedFrames::AllFrames`). The keydown handler must continue firing wherever focus lives, but the toast DOM must only be created in the top frame — a `position: fixed` element inside a nested (possibly cross-origin) iframe would anchor to the iframe's viewport, not the window. Guard the toast code with `window.top !== window`. Iframe handlers still invoke the toggle; they just don't paint.
- Reuse a single DOM node across toggles via `window.__rdpls_toast_el`. Re-check `el.isConnected` in case the page (a SPA) replaced `document.body`.
- Reset the hide timer on every toggle so rapid re-presses don't stack or hide early. Force a reflow between `transition:none` and `transition:opacity` so the opacity snap-back cleanly restarts the fade.
- Use inline styles only — no classes, no stylesheet injection. No color swap between states; the text is the signal.
- Silent bail if `document.body` is not yet attached; the next toggle will succeed.
- Silent no-op if `invoke` rejects (no `.catch` handler means the rejection is swallowed, which is fine here — we don't want to spam the page with error UI).

- [ ] **Step 1: Replace the `ESCAPE_HOTKEY_SCRIPT` constant**

Open `src-tauri/src/keyboard.rs` and replace the existing constant (lines 9-23) with:

```rust
pub const ESCAPE_HOTKEY_SCRIPT: &str = r#"
(function () {
    if (window.__rdpls_escape_installed) return;
    window.__rdpls_escape_installed = true;

    function showToast(inhibiting) {
        if (window.top !== window) return;
        if (!document.body) return;

        var el = window.__rdpls_toast_el;
        if (!el || !el.isConnected) {
            el = document.createElement('div');
            el.style.cssText =
                'position:fixed;top:16px;left:50%;' +
                'transform:translateX(-50%);' +
                'padding:10px 20px;border-radius:999px;' +
                'background:rgba(0,0,0,0.75);color:#fff;' +
                'font:500 14px system-ui,sans-serif;' +
                'pointer-events:none;z-index:2147483647;' +
                'opacity:0;transition:opacity 200ms ease-out;';
            document.body.appendChild(el);
            window.__rdpls_toast_el = el;
        }

        el.textContent = inhibiting ? 'Keys → Remote' : 'Keys → Local';

        if (window.__rdpls_toast_timer) {
            clearTimeout(window.__rdpls_toast_timer);
        }

        el.style.transition = 'none';
        el.style.opacity = '1';
        void el.offsetHeight;
        el.style.transition = 'opacity 200ms ease-out';

        window.__rdpls_toast_timer = setTimeout(function () {
            el.style.opacity = '0';
        }, 1500);
    }

    window.addEventListener('keydown', function (e) {
        if (e.ctrlKey && e.altKey && e.shiftKey && e.key === 'Escape') {
            e.preventDefault();
            e.stopPropagation();
            if (window.__TAURI_INTERNALS__) {
                window.__TAURI_INTERNALS__
                    .invoke('rdpls_toggle_inhibit')
                    .then(showToast);
            }
        }
    }, { capture: true });
})();
"#;
```

Notes:
- The `→` arrow is a literal UTF-8 character inside the raw string (`r#"..."#`). That's fine — Rust source files are UTF-8 and the WebKit injection is UTF-8 too.
- The `void el.offsetHeight` line is a deliberate forced reflow — do not delete it thinking it's dead code.
- Leave the `rdpls_exit` function and its doc comment at the bottom of the file untouched.

- [ ] **Step 2: Verify the crate still compiles**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: `Finished` with no errors. Raw-string literal must be balanced (`r#"..."#`).

- [ ] **Step 3: Manual smoke test — base behavior**

Run: `cargo tauri dev`
Wait for the window to load the Microsoft sign-in page.

Perform, in order:
1. Press `Ctrl+Alt+Shift+Escape` once. Expected: `Keys → Local` pill appears top-center, visible ~1.5 s, fades out over ~200 ms.
2. Press it again. Expected: `Keys → Remote` pill appears top-center, same fade.
3. Mash the hotkey 4–5 times within 2 seconds. Expected: text alternates each press, exactly one pill on screen throughout, timer resets each time (it never disappears while you're still pressing).
4. Wait for the pill to fully fade out. Press the hotkey once more. Expected: pill reappears from invisible — no flash of stale opacity.

If any of the above fails, do not commit. Debug first.

- [ ] **Step 4: Manual smoke test — RDP-canvas overlay**

Still in `cargo tauri dev`:
1. Complete auth and launch a Windows 365 / AVD session so the RDP canvas is rendering.
2. With the RDP canvas focused, press `Ctrl+Alt+Shift+Escape`. Expected: the pill renders on top of the canvas at the top of the window, and clicking through the pill's area reaches the RDP canvas (mouse passes through — no click capture).
3. Toggle once more. Confirm the text flips.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/keyboard.rs
git commit -m "feat: toast on Ctrl+Alt+Shift+Escape toggle

Shows a transient 'Keys → Remote' / 'Keys → Local' pill for ~1.5s
(fading out over 200ms) whenever the inhibit hotkey fires. The pill
lives in the top frame only (position:fixed wouldn't anchor correctly
inside a nested iframe). No Rust-side changes — the existing toggle
command already returns the bool that drives the label."
```
