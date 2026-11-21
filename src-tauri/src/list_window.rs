use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

const LABEL: &str = "main";

pub fn show(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(LABEL) {
        window.unminimize()?;
        return window.set_focus();
    }

    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == LABEL)
        .ok_or(tauri::Error::WindowNotFound)?;
    tauri::WebviewWindowBuilder::from_config(app, config)?.build()?;
    Ok(())
}

pub fn announce<S: Serialize + Clone>(app: &AppHandle, event: &str, payload: S) {
    if let Err(error) = app.emit_to(LABEL, event, payload) {
        eprintln!("could not send {event} to the server list: {error}");
    }
}
