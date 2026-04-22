mod keyboard;
mod shortcuts_inhibit;

use gtk::prelude::*;
use gtk::{glib, Application, ApplicationWindow};
use webkit6::prelude::*;
use webkit6::{HardwareAccelerationPolicy, UserContentInjectedFrames, UserContentManager, UserScript, UserScriptInjectionTime, WebView};

const APP_ID: &str = "com.rdpls.client";
const ENTRY_URL: &str = "https://myapps.microsoft.com";

/// Spoofed user agent. WebKitGTK's default UA hits Microsoft's "unsupported
/// browser" wall; Firefox's UA does not and already works under the same
/// Conditional Access policy.
const SPOOFED_USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64; rv:138.0) Gecko/20100101 Firefox/138.0";

fn main() -> glib::ExitCode {
    env_logger::init();

    // Force WebKitGTK accelerated compositing for all content and route video
    // frames through the GPU. Must be set before WebKit initializes. See
    // CLAUDE.md "Key Constraints" for motivation.
    std::env::set_var("WEBKIT_FORCE_COMPOSITING_MODE", "1");

    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("rdpls")
        .default_width(1280)
        .default_height(800)
        .build();

    // Platform-split decorations: niri owns window management for
    // undecorated surfaces; GNOME/Mutter expects CSD and offers no drag
    // fallback, so an undecorated window there is stuck in place — fall
    // back to a standard titlebar everywhere else.
    if !compositor_owns_decorations() {
        window.set_decorated(true);
    } else {
        window.set_decorated(false);
    }

    // Start fullscreen so any compositor-drawn titlebar is out of the way
    // from the moment the window appears. Ctrl+Alt+Shift+F toggles.
    window.set_fullscreened(true);

    let manager = UserContentManager::new();

    // Suppress Ctrl+scroll zoom — injected at document start so it runs before
    // any page scripts latch wheel handlers.
    manager.add_script(&UserScript::new(
        keyboard::ZOOM_SUPPRESS_SCRIPT,
        UserContentInjectedFrames::AllFrames,
        UserScriptInjectionTime::Start,
        &[],
        &[],
    ));

    // Instrument VideoDecoder / mediaCapabilities / RTCRtpReceiver / Worker so
    // we can see what codec Microsoft actually picks at runtime. Must be
    // document-start so hooks land before page scripts grab references.
    manager.add_script(&UserScript::new(
        keyboard::HOOK_SCRIPT,
        UserContentInjectedFrames::AllFrames,
        UserScriptInjectionTime::Start,
        &[],
        &[],
    ));

    // Hotkey + toast + IPC relay — injected at document end in every frame.
    manager.add_script(&UserScript::new(
        keyboard::HOTKEY_SCRIPT,
        UserContentInjectedFrames::AllFrames,
        UserScriptInjectionTime::End,
        &[],
        &[],
    ));

    // WebCodecs capability probe — runs once, top frame only, at document end
    // so document.body exists and the overlay can paint immediately. Prints
    // results to stderr (via rdpls_log) and shows an on-screen panel.
    manager.add_script(&UserScript::new(
        keyboard::CODEC_PROBE_SCRIPT,
        UserContentInjectedFrames::TopFrame,
        UserScriptInjectionTime::End,
        &[],
        &[],
    ));

    // Register script message handlers that replace Tauri's invoke. Each
    // handler is triggered by `window.webkit.messageHandlers.<name>.postMessage(_)`
    // in the injected JS.
    for name in [
        "rdpls_toggle_inhibit",
        "rdpls_toggle_fullscreen",
        "rdpls_exit",
        "rdpls_log",
    ] {
        manager.register_script_message_handler(name, None);
    }
    manager.connect_script_message_received(Some("rdpls_toggle_inhibit"), move |_mgr, _val| {
        shortcuts_inhibit::toggle();
    });
    let window_for_fs = window.clone();
    manager.connect_script_message_received(Some("rdpls_toggle_fullscreen"), move |_mgr, _val| {
        window_for_fs.set_fullscreened(!window_for_fs.is_fullscreen());
    });
    let app_for_exit = app.clone();
    manager.connect_script_message_received(Some("rdpls_exit"), move |_mgr, _val| {
        app_for_exit.quit();
    });
    manager.connect_script_message_received(Some("rdpls_log"), |_mgr, val| {
        eprintln!("{}", val.to_str());
    });

    let webview = WebView::builder()
        .user_content_manager(&manager)
        .build();

    let settings = webkit6::prelude::WebViewExt::settings(&webview)
        .expect("webview always has a settings object");
    settings.set_user_agent(Some(SPOOFED_USER_AGENT));
    settings.set_enable_developer_extras(false);
    // Default is OnDemand, which WebKit can demote back to software when it
    // decides content doesn't need it — causing worse pacing on the RDP video
    // canvas. `Always` pins the GPU path on.
    settings.set_hardware_acceleration_policy(HardwareAccelerationPolicy::Always);

    // Suppress the default context menu. Microsoft's RDP client captures mouse
    // inside the session, but outside (auth pages, etc.) the WebKit context
    // menu would show navigation items that don't belong in a kiosk wrapper.
    webview.connect_context_menu(|_wv, _menu, _hit| true);

    // Microsoft launches RDP sessions via window.open. Intercept and load the
    // URL in the main WebView instead of opening a popup.
    let webview_for_popup = webview.clone();
    webview.connect_create(move |_wv, nav_action| {
        if let Some(req) = nav_action.request() {
            if let Some(uri) = req.uri() {
                webview_for_popup.load_uri(uri.as_str());
            }
        }
        // Returning None tells webkit not to create a new view — we handled
        // the navigation in-place on the main webview.
        None
    });

    window.set_child(Some(&webview));

    // Inhibit needs a realized surface to bind to. Hook realize so we install
    // exactly once, after GTK has given us a Wayland surface.
    window.connect_realize(|w| {
        shortcuts_inhibit::install(w);
    });

    // Closing the window quits the app. Single-window app; no tray, no
    // persistent background process.
    let app_for_close = app.clone();
    window.connect_close_request(move |_| {
        app_for_close.quit();
        glib::Propagation::Proceed
    });

    webview.load_uri(ENTRY_URL);
    window.present();
}

/// True when `XDG_CURRENT_DESKTOP` names a compositor that manages
/// undecorated surfaces (currently just niri). GNOME/Mutter expects CSD
/// and offers no drag fallback, so a decoration-less window there is
/// stuck in place.
fn compositor_owns_decorations() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|v| desktop_owns_decorations(&v))
        .unwrap_or(false)
}

fn desktop_owns_decorations(xdg_current_desktop: &str) -> bool {
    xdg_current_desktop
        .split(':')
        .any(|part| part.eq_ignore_ascii_case("niri"))
}

#[cfg(test)]
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
        assert!(!desktop_owns_decorations("myniri"));
        assert!(!desktop_owns_decorations("niri-ish"));
    }

    #[test]
    fn empty_is_false() {
        assert!(!desktop_owns_decorations(""));
    }
}
