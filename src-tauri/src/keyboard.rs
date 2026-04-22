/// JS injected into every frame that listens for the intentional local
/// accelerators (Ctrl+Alt+Shift+. → keyboard-inhibit toggle,
/// Ctrl+Alt+Shift+F → fullscreen toggle) and invokes the corresponding
/// Tauri commands.
///
/// Chord rationale: GNOME/Mutter eats Ctrl+Alt+Shift+Escape when the
/// keyboard-shortcuts-inhibit is OFF, so the chord can't be used to toggle
/// back from "Keys → Local" to "Keys → Remote". Period key sidesteps this
/// (not bound in any known GNOME default).
///
/// Intercepting at the WebView rather than as a global shortcut: Wayland
/// compositors (niri) do not honor X11/rdev-style global grabs. The app
/// always has focus when the user wants to trigger this, so WebView-level
/// capture is sufficient.
///
/// Cross-frame relay: `window.__TAURI_INTERNALS__` is only exposed in the
/// top frame. When focus lands in a cross-origin subframe (the RDP session
/// iframe), keydown still fires our listener but the direct invoke would
/// no-op. Subframes `postMessage` to `window.top`; the top frame routes
/// allowlisted commands through the IPC bridge.
#[cfg(target_os = "linux")]
pub const HOTKEY_SCRIPT: &str = r#"
(function () {
    if (window.__rdpls_escape_installed) return;
    window.__rdpls_escape_installed = true;

    var IS_TOP = window.top === window;
    var ALLOWED = ['rdpls_toggle_inhibit', 'rdpls_toggle_fullscreen'];

    function showToast(text) {
        if (!IS_TOP) return;
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

        el.textContent = text;

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

    function dispatch(cmd, onLabel, offLabel) {
        if (IS_TOP) {
            if (!window.__TAURI_INTERNALS__) return;
            window.__TAURI_INTERNALS__.invoke(cmd).then(function (on) {
                showToast(on ? onLabel : offLabel);
            });
        } else {
            try {
                window.top.postMessage({
                    __rdpls: { cmd: cmd, onLabel: onLabel, offLabel: offLabel }
                }, '*');
            } catch (_) {}
        }
    }

    if (IS_TOP) {
        window.addEventListener('message', function (e) {
            var msg = e.data && e.data.__rdpls;
            if (!msg) return;
            if (ALLOWED.indexOf(msg.cmd) === -1) return;
            if (!window.__TAURI_INTERNALS__) return;
            window.__TAURI_INTERNALS__.invoke(msg.cmd).then(function (on) {
                showToast(on ? msg.onLabel : msg.offLabel);
            });
        }, false);
    }

    window.addEventListener('keydown', function (e) {
        if (!(e.ctrlKey && e.altKey && e.shiftKey)) return;

        if (e.code === 'Period') {
            e.preventDefault();
            e.stopPropagation();
            dispatch('rdpls_toggle_inhibit', 'Keys → Remote', 'Keys → Local');
        } else if (e.code === 'KeyF') {
            e.preventDefault();
            e.stopPropagation();
            dispatch('rdpls_toggle_fullscreen', 'Fullscreen ON', 'Fullscreen OFF');
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
///   * `Ctrl+Alt+Shift+.` → `rdpls_toggle_passthrough` with a toast,
///     matching the Linux hotkey behavior. Exit on macOS is the red
///     traffic light; see `docs/macos-keyboard-passthrough.md` for why the
///     passthrough semantics are narrower on Mac than on Wayland.
///   * `Ctrl+Alt+Shift+F` → `rdpls_toggle_fullscreen` with a toast.
///   * Cross-frame postMessage relay for the hotkeys (see Linux docs).
#[cfg(target_os = "macos")]
pub const MACOS_INIT_SCRIPT: &str = r#"
(function () {
    if (window.__rdpls_mac_installed) return;
    window.__rdpls_mac_installed = true;

    var IS_TOP = window.top === window;
    var ALLOWED = ['rdpls_toggle_passthrough', 'rdpls_toggle_fullscreen'];

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

    function showToast(text) {
        if (!IS_TOP) return;
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

        el.textContent = text;

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

    function dispatch(cmd, onLabel, offLabel) {
        if (IS_TOP) {
            if (!window.__TAURI_INTERNALS__) return;
            window.__TAURI_INTERNALS__.invoke(cmd).then(function (on) {
                showToast(on ? onLabel : offLabel);
            });
        } else {
            try {
                window.top.postMessage({
                    __rdpls: { cmd: cmd, onLabel: onLabel, offLabel: offLabel }
                }, '*');
            } catch (_) {}
        }
    }

    if (IS_TOP) {
        window.addEventListener('message', function (e) {
            var msg = e.data && e.data.__rdpls;
            if (!msg) return;
            if (ALLOWED.indexOf(msg.cmd) === -1) return;
            if (!window.__TAURI_INTERNALS__) return;
            window.__TAURI_INTERNALS__.invoke(msg.cmd).then(function (on) {
                showToast(on ? msg.onLabel : msg.offLabel);
            });
        }, false);
    }

    window.addEventListener('keydown', function (e) {
        if (!(e.ctrlKey && e.altKey && e.shiftKey)) return;

        if (e.code === 'Period') {
            e.preventDefault();
            e.stopPropagation();
            dispatch('rdpls_toggle_passthrough', 'Keys → Remote', 'Keys → Local');
        } else if (e.code === 'KeyF') {
            e.preventDefault();
            e.stopPropagation();
            dispatch('rdpls_toggle_fullscreen', 'Fullscreen ON', 'Fullscreen OFF');
        }
    }, { capture: true });
})();
"#;

/// Manual exit command, not currently bound to any chord. Retained as a
/// programmatic fallback for future use (e.g., if a UI needs to trigger
/// shutdown).
#[tauri::command]
pub fn rdpls_exit(app: tauri::AppHandle) {
    app.exit(0);
}

/// Toggle the main window between fullscreen and windowed. Returns the new
/// fullscreen state so the caller (injected JS) can show a toast.
#[tauri::command]
pub fn rdpls_toggle_fullscreen(window: tauri::WebviewWindow) -> bool {
    let next = !window.is_fullscreen().unwrap_or(false);
    let _ = window.set_fullscreen(next);
    next
}
