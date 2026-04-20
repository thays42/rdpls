/// JS injected into every frame that listens for the one intentional
/// local accelerator (Ctrl+Alt+Shift+Escape) and asks the Rust side to exit.
///
/// This runs inside the WebView rather than as a global shortcut because
/// Wayland compositors (niri) do not expose a working global-shortcut path
/// for rdev/X11-style grabs. Intercepting at the WebView is fine: the user
/// always has window focus when they'd want to trigger this.
pub const ESCAPE_HOTKEY_SCRIPT: &str = r#"
(function () {
    if (window.__rdpls_escape_installed) return;
    window.__rdpls_escape_installed = true;
    window.addEventListener('keydown', function (e) {
        if (e.ctrlKey && e.altKey && e.shiftKey && e.key === 'Escape') {
            e.preventDefault();
            e.stopPropagation();
            if (window.__TAURI_INTERNALS__) {
                window.__TAURI_INTERNALS__.invoke('rdpls_exit');
            }
        }
    }, { capture: true });
})();
"#;

#[tauri::command]
pub fn rdpls_exit(app: tauri::AppHandle) {
    app.exit(0);
}
