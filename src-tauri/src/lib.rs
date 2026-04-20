use tauri::{WebviewUrl, WebviewWindowBuilder};

const ENTRY_URL: &str = "https://myapps.microsoft.com";

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

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running rdpls");
}
