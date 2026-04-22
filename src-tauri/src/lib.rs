mod keyboard;
mod passthrough_macos;

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

const ENTRY_URL: &str = "https://myapps.microsoft.com";

/// Spoofed user agent. WKWebView's default UA hits Microsoft's "unsupported
/// browser" wall; Firefox's UA does not.
const SPOOFED_USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14.7; rv:138.0) Gecko/20100101 Firefox/138.0";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            keyboard::rdpls_exit,
            keyboard::rdpls_toggle_fullscreen,
            passthrough_macos::rdpls_toggle_passthrough,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            let url = ENTRY_URL.parse().expect("ENTRY_URL must be a valid URL");

            // Keep standard decorations — the window needs a real title bar for
            // drag/resize. `decorations(false)` leaves the window completely
            // unmovable and unresizable on Mac because there's no compositor-level
            // fallback like niri provides on Wayland, and overlay-style title bars
            // don't make the WKWebView region draggable without custom drag regions
            // we can't inject into Microsoft's pages.
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title("rdpls")
                .inner_size(1280.0, 800.0)
                .resizable(true)
                .fullscreen(true)
                .user_agent(SPOOFED_USER_AGENT)
                .initialization_script_for_all_frames(keyboard::MACOS_INIT_SCRIPT)
                .build()?;

            let main = app
                .get_webview_window("main")
                .expect("main window missing");

            passthrough_macos::install();

            // Default Cocoa behavior keeps the process alive after the last window
            // closes. rdpls is a single-window app; closing the traffic light
            // should actually quit.
            let handle = app.handle().clone();
            main.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { .. } = event {
                    handle.exit(0);
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running rdpls");
}
