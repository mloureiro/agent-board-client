use tauri::webview::NewWindowResponse;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, Wry};
use url::Url;
use uuid::Uuid;

use crate::connection;
use crate::error::ClientError;
use crate::link_route::{self, LinkRoute};
use crate::server_url::check_server_url;
use crate::store::Server;
#[cfg(not(any(target_os = "macos", target_os = "ios")))]
use crate::webview_store;

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
    Ok(builder.data_directory(webview_store::webviews_directory(app)?.join(id.to_string())))
}

fn route_new_window(app: &AppHandle, label: &str, id: Uuid, server_url: &Url, requested: &Url) {
    match link_route::route(server_url, requested) {
        LinkRoute::SameWindow => {
            let navigated = app
                .get_webview_window(label)
                .map(|window| window.navigate(requested.clone()));
            if let Some(Err(error)) = navigated {
                eprintln!("could not load a link in the window of server {id}: {error}");
            }
        }
        LinkRoute::SystemBrowser => {
            let target = requested.to_string();
            std::thread::spawn(move || {
                if let Err(error) = open::that_detached(target) {
                    eprintln!(
                        "could not open a link of server {id} in the system browser: {:?}",
                        error.kind()
                    );
                }
            });
        }
        LinkRoute::Drop => {}
    }
}

pub fn open(app: &AppHandle, server: &Server) -> Result<(), ClientError> {
    let label = label(server.id);
    if let Some(window) = app.get_webview_window(&label) {
        window.unminimize()?;
        window.set_focus()?;
        return Ok(());
    }

    let url = check_server_url(server.url.as_str())?;
    let handle = app.clone();
    let window_label = label.clone();
    let server_id = server.id;
    let server_url = url.clone();
    let builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(url))
        .title(&server.name)
        .inner_size(1280.0, 860.0)
        .initialization_script(include_str!("../scripts/external-links.js"))
        .on_new_window(move |requested, _features| {
            route_new_window(&handle, &window_label, server_id, &server_url, &requested);
            NewWindowResponse::Deny
        });
    with_own_storage(app, builder, server.id)?.build()?;
    Ok(())
}

pub async fn connect_and_open(app: &AppHandle, server: &Server) -> Result<(), ClientError> {
    if app.get_webview_window(&label(server.id)).is_none() {
        connection::check(&server.url).await?;
    }
    open(app, server)
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
