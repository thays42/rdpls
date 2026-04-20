use std::error::Error;

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{
    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
};

/// The single intentional local accelerator.
///
/// Everything else is left to the remote RDP client. This one exists as a
/// safety valve: if the remote session hangs or misbehaves, the user always
/// has a way to quit rdpls cleanly without reaching for the mouse.
pub fn register_escape_hotkey(app: &AppHandle) -> Result<(), Box<dyn Error>> {
    let shortcut = Shortcut::new(
        Some(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT),
        Code::Escape,
    );

    let app_handle = app.clone();
    app.global_shortcut()
        .on_shortcut(shortcut, move |_app, _sc, event| {
            if event.state() == ShortcutState::Pressed {
                app_handle.exit(0);
            }
        })?;

    Ok(())
}
