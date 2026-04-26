mod keyboard;
mod tap_tracker;

#[cfg(target_os = "linux")]
mod shortcuts_inhibit;

#[cfg(target_os = "linux")]
mod passthrough_linux;

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
            keyboard::rdpls_toggle_fullscreen,
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

            let url = ENTRY_URL.parse().expect("ENTRY_URL must be a valid URL");

            let builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title("rdpls")
                .inner_size(1280.0, 800.0)
                .resizable(true)
                .fullscreen(true);

            // Linux: drop chrome only on compositors that own window
            // management for undecorated surfaces (niri). GNOME/Mutter
            // expects CSD and offers no drag fallback, so an undecorated
            // window there is stuck in place — fall back to a normal
            // titlebar everywhere else.
            #[cfg(target_os = "linux")]
            let builder = if compositor_owns_decorations() {
                builder.decorations(false)
            } else {
                builder
            };

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
                keyboard::HOTKEY_SCRIPT,
                UserContentInjectedFrames::AllFrames,
                UserScriptInjectionTime::End,
                &[],
                &[],
            );
            manager.add_script(&escape_script);
        }

        passthrough_linux::install(&webview);
    })?;
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn configure_webview(_window: &tauri::WebviewWindow) -> tauri::Result<()> {
    Ok(())
}

#[cfg(target_os = "linux")]
fn compositor_owns_decorations() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|v| desktop_owns_decorations(&v))
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn desktop_owns_decorations(xdg_current_desktop: &str) -> bool {
    xdg_current_desktop
        .split(':')
        .any(|part| part.eq_ignore_ascii_case("niri"))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::desktop_owns_decorations;

    #[test]
    fn niri_owns_decorations() {
        assert!(desktop_owns_decorations("niri"));
    }

    #[test]
    fn niri_in_colon_list_owns_decorations() {
        assert!(desktop_owns_decorations("niri:wlroots"));
        assert!(desktop_owns_decorations("wlroots:niri"));
    }

    #[test]
    fn case_insensitive() {
        assert!(desktop_owns_decorations("Niri"));
        assert!(desktop_owns_decorations("NIRI"));
    }

    #[test]
    fn gnome_does_not_own_decorations() {
        assert!(!desktop_owns_decorations("GNOME"));
        assert!(!desktop_owns_decorations("ubuntu:GNOME"));
    }

    #[test]
    fn no_substring_match() {
        // "niri" appearing inside another token should not count.
        assert!(!desktop_owns_decorations("myniri"));
        assert!(!desktop_owns_decorations("niri-ish"));
    }

    #[test]
    fn empty_is_false() {
        assert!(!desktop_owns_decorations(""));
    }
}
