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
    window.addEventListener('keydown', function (e) {
        if (e.ctrlKey && e.altKey && e.shiftKey && e.key === 'Escape') {
            e.preventDefault();
            e.stopPropagation();
            if (window.__TAURI_INTERNALS__) {
                window.__TAURI_INTERNALS__.invoke('rdpls_toggle_inhibit');
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
