use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, Wry};
use uuid::Uuid;

use crate::error::ClientError;
use crate::server_url::check_server_url;
use crate::store::Server;

type BoardWindowBuilder<'a> = WebviewWindowBuilder<'a, Wry, AppHandle>;

fn label(id: Uuid) -> String {
    format!("server-{id}")
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn with_own_storage<'a>(
    _app: &AppHandle,
    builder: BoardWindowBuilder<'a>,
    id: Uuid,
) -> tauri::Result<BoardWindowBuilder<'a>> {
    Ok(builder.data_store_identifier(id.into_bytes()))
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
fn with_own_storage<'a>(
    app: &AppHandle,
    builder: BoardWindowBuilder<'a>,
    id: Uuid,
) -> tauri::Result<BoardWindowBuilder<'a>> {
    let directory = app
        .path()
        .app_data_dir()?
        .join("webviews")
        .join(id.to_string());
    Ok(builder.data_directory(directory))
}

pub fn open(app: &AppHandle, server: &Server) -> Result<(), ClientError> {
    let label = label(server.id);
    if let Some(window) = app.get_webview_window(&label) {
        window.unminimize()?;
        window.set_focus()?;
        return Ok(());
    }

    let url = check_server_url(server.url.as_str())?;
    let builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(url))
        .title(&server.name)
        .inner_size(1280.0, 860.0);
    with_own_storage(app, builder, server.id)?.build()?;
    Ok(())
}

pub fn retitle(app: &AppHandle, server: &Server) -> Result<(), ClientError> {
    if let Some(window) = app.get_webview_window(&label(server.id)) {
        window.set_title(&server.name)?;
    }
    Ok(())
}

pub fn close(app: &AppHandle, id: Uuid) -> Result<(), ClientError> {
    if let Some(window) = app.get_webview_window(&label(id)) {
        window.destroy()?;
    }
    Ok(())
}
