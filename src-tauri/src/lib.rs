use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

const ENTRY_URL: &str = "https://myapps.microsoft.com";
const SPOOFED_USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64; rv:138.0) Gecko/20100101 Firefox/138.0";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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

            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title("rdpls")
                .inner_size(1280.0, 800.0)
                .resizable(true)
                .build()?;

            let main = app.get_webview_window("main").expect("main window missing");
            configure_webview(&main)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running rdpls");
}

#[cfg(target_os = "linux")]
fn configure_webview(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    window.with_webview(|wv| {
        use webkit2gtk::{SettingsExt, WebViewExt};
        let webview = wv.inner();
        if let Some(settings) = webview.settings() {
            settings.set_user_agent(Some(SPOOFED_USER_AGENT));
        }
    })?;
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn configure_webview(_window: &tauri::WebviewWindow) -> tauri::Result<()> {
    Ok(())
}
