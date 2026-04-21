/// JS injected into every frame that listens for the one intentional
/// local accelerator (Ctrl+Alt+Shift+Escape) and toggles the Wayland
/// keyboard-shortcuts-inhibit state on the Rust side.
///
/// Intercepting at the WebView rather than as a global shortcut: Wayland
/// compositors (niri) do not honor X11/rdev-style global grabs. The app
/// always has focus when the user wants to trigger this, so WebView-level
/// capture is sufficient.
#[cfg(target_os = "linux")]
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

/// JS injected into every frame on macOS. Combines:
///   * Ctrl/Cmd+scroll zoom suppression
///   * context menu suppression (no webkit2gtk context-menu signal on Mac)
///   * `window.open` + `target="_blank"` click → top-frame navigation
///     (replaces Linux's `create` signal interception; Microsoft launches
///     RDP sessions via `window.open`, so without this the session-launch
///     flow breaks)
///   * `Ctrl+Alt+Shift+Escape` → `rdpls_toggle_passthrough` with a toast,
///     matching the Linux hotkey behavior. Exit on macOS is the red
///     traffic light; see `docs/macos-keyboard-passthrough.md` for why the
///     passthrough semantics are narrower on Mac than on Wayland.
#[cfg(target_os = "macos")]
pub const MACOS_INIT_SCRIPT: &str = r#"
(function () {
    if (window.__rdpls_mac_installed) return;
    window.__rdpls_mac_installed = true;

    function navTop(url) {
        if (!url) return;
        try { window.top.location.assign(url); }
        catch (_) { window.location.assign(url); }
    }

    window.addEventListener('wheel', function (e) {
        if (e.ctrlKey || e.metaKey) e.preventDefault();
    }, { passive: false, capture: true });

    window.addEventListener('contextmenu', function (e) {
        e.preventDefault();
    }, { capture: true });

    // target="_blank" / rel="noopener" anchors don't go through window.open;
    // WKWebView routes them to WKUIDelegate createWebViewWithConfiguration,
    // which we don't implement. Intercept the click and navigate the top
    // frame instead so the Microsoft RDP launch button lands in-window.
    document.addEventListener('click', function (e) {
        var el = e.target;
        while (el && el !== document) {
            if (el.tagName === 'A' && (el.target === '_blank' || el.target === 'new') && el.href) {
                e.preventDefault();
                e.stopPropagation();
                navTop(el.href);
                return;
            }
            el = el.parentElement;
        }
    }, { capture: true });

    // Some Microsoft flows call window.open with an empty URL then set
    // w.location = computedUrl on the returned handle. Return a stub that
    // forwards those writes to the top frame instead of returning null,
    // which would break `if (w)` guards in their code.
    window.open = function (url) {
        if (url) navTop(url);
        var loc = {
            assign: function (v) { navTop(String(v)); },
            replace: function (v) { navTop(String(v)); },
            get href() { return ''; },
            set href(v) { navTop(String(v)); }
        };
        return {
            closed: false,
            focus: function () {}, blur: function () {}, close: function () {},
            postMessage: function () {},
            document: {
                write: function () {}, writeln: function () {},
                open: function () { return this; }, close: function () {}
            },
            get location() { return loc; },
            set location(v) { navTop(String(v)); }
        };
    };

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

        el.textContent = inhibiting ? 'Keys \u2192 Remote' : 'Keys \u2192 Local';

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
                    .invoke('rdpls_toggle_passthrough')
                    .then(showToast);
            }
        }
    }, { capture: true });
})();
"#;

/// Invoked from the injected JS as the app's exit path. On macOS this is
/// wired to `Ctrl+Alt+Shift+Escape` (no window chrome, no menu bar). On
/// Linux the same key toggles the Wayland shortcut-inhibit instead; this
/// command is retained as a manual fallback.
#[tauri::command]
pub fn rdpls_exit(app: tauri::AppHandle) {
    app.exit(0);
}
