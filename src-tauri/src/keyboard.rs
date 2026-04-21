/// JS injected into every frame that listens for the one intentional
/// local accelerator (Ctrl+Alt+Shift+Escape) and toggles the Wayland
/// keyboard-shortcuts-inhibit state on the Rust side.
///
/// Intercepting at the WebView rather than as a global shortcut: Wayland
/// compositors (niri) do not honor X11/rdev-style global grabs. The app
/// always has focus when the user wants to trigger this, so WebView-level
/// capture is sufficient.
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

/// Retained as a fallback / manual exit — not currently wired to any key.
#[tauri::command]
pub fn rdpls_exit(app: tauri::AppHandle) {
    app.exit(0);
}
