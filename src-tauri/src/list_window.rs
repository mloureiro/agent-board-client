use tauri::{AppHandle, Manager};

pub const LABEL: &str = "main";

pub fn show(app: &AppHandle) -> tauri::Result<()> {
    let window = app
        .get_webview_window(LABEL)
        .ok_or(tauri::Error::WindowNotFound)?;
    window.show()?;
    window.unminimize()?;
    window.set_focus()
}
