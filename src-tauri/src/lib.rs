mod keyboard;

#[cfg(target_os = "linux")]
mod shortcuts_inhibit;

#[cfg(target_os = "macos")]
mod passthrough_macos;

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

const ENTRY_URL: &str = "https://myapps.microsoft.com";

#[cfg(target_os = "linux")]
const SPOOFED_USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64; rv:138.0) Gecko/20100101 Firefox/138.0";

#[cfg(target_os = "macos")]
const SPOOFED_USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14.7; rv:138.0) Gecko/20100101 Firefox/138.0";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            keyboard::rdpls_exit,
            #[cfg(target_os = "linux")]
            shortcuts_inhibit::rdpls_toggle_inhibit,
            #[cfg(target_os = "macos")]
            passthrough_macos::rdpls_toggle_passthrough
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            let url = ENTRY_URL
                .parse()
                .expect("ENTRY_URL must be a valid URL");

            let builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title("rdpls")
                .inner_size(1280.0, 800.0)
                .resizable(true);

            // Linux: no chrome — the compositor (niri) owns window management
            // and decorations(false) is fine because drag/resize are handled
            // at the compositor layer.
            #[cfg(target_os = "linux")]
            let builder = builder.decorations(false);

            // macOS: keep standard decorations — the window needs a real title
            // bar for drag/resize. `decorations(false)` leaves the window
            // completely unmovable and unresizable on Mac because there's no
            // compositor-level fallback like niri provides on Wayland, and
            // overlay-style title bars don't make the WKWebView region
            // draggable without custom drag regions we can't inject into
            // Microsoft's pages.
            #[cfg(target_os = "macos")]
            let builder = builder
                .user_agent(SPOOFED_USER_AGENT)
                .initialization_script_for_all_frames(keyboard::MACOS_INIT_SCRIPT);

            builder.build()?;

            let main = app.get_webview_window("main").expect("main window missing");
            configure_webview(&main)?;

            #[cfg(target_os = "linux")]
            shortcuts_inhibit::install(&main)?;

            #[cfg(target_os = "macos")]
            {
                passthrough_macos::install();

                // Default Cocoa behavior keeps the process alive after the
                // last window closes. rdpls is a single-window app; closing
                // the traffic light should actually quit.
                let handle = app.handle().clone();
                main.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { .. } = event {
                        handle.exit(0);
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running rdpls");
}

#[cfg(target_os = "linux")]
fn configure_webview(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    window.with_webview(|wv| {
        use webkit2gtk::{
            SettingsExt, URIRequestExt, UserContentInjectedFrames, UserContentManagerExt,
            UserScript, UserScriptInjectionTime, WebViewExt,
        };

        let webview = wv.inner();

        if let Some(settings) = webview.settings() {
            settings.set_user_agent(Some(SPOOFED_USER_AGENT));
            settings.set_enable_developer_extras(false);
        }

        webview.connect_context_menu(|_wv, _menu, _event, _hit| true);

        webview.connect_create(|wv, nav_action| {
            if let Some(req) = nav_action.request() {
                if let Some(uri) = req.uri() {
                    wv.load_uri(uri.as_str());
                }
            }
            None
        });

        if let Some(manager) = webview.user_content_manager() {
            let zoom_script = UserScript::new(
                "window.addEventListener('wheel', function(e) { if (e.ctrlKey) { e.preventDefault(); } }, { passive: false, capture: true });",
                UserContentInjectedFrames::AllFrames,
                UserScriptInjectionTime::Start,
                &[],
                &[],
            );
            manager.add_script(&zoom_script);

            let escape_script = UserScript::new(
                keyboard::ESCAPE_HOTKEY_SCRIPT,
                UserContentInjectedFrames::AllFrames,
                UserScriptInjectionTime::End,
                &[],
                &[],
            );
            manager.add_script(&escape_script);
        }
    })?;
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn configure_webview(_window: &tauri::WebviewWindow) -> tauri::Result<()> {
    Ok(())
}
