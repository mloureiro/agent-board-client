use tauri::webview::{NewWindowResponse, WebviewBuilder};
use tauri::window::WindowBuilder;
use tauri::{
    AppHandle, LogicalPosition, Manager, PhysicalSize, Rect, WebviewUrl, WindowEvent, Wry,
};
use url::Url;
use uuid::Uuid;

use crate::connection;
use crate::error::ClientError;
use crate::link_route::{self, LinkRoute};
use crate::sentinel::{self, Signal};
use crate::server_url::check_server_url;
use crate::store::Server;
use crate::tabs::{self, Route};
#[cfg(not(any(target_os = "macos", target_os = "ios")))]
use crate::webview_store;

type BoardWebviewBuilder = WebviewBuilder<Wry>;

fn label(id: Uuid) -> String {
    format!("server-{id}")
}

fn home_label(id: Uuid) -> String {
    format!("board-{id}-home")
}

fn routing_script(role: &str) -> String {
    let script = include_str!("../scripts/tab-routing.js")
        .trim_end()
        .trim_end_matches(';');
    format!("({script})('{role}', window);")
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn with_own_storage(
    _app: &AppHandle,
    builder: BoardWebviewBuilder,
    id: Uuid,
) -> tauri::Result<BoardWebviewBuilder> {
    Ok(builder.data_store_identifier(id.into_bytes()))
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
fn with_own_storage(
    app: &AppHandle,
    builder: BoardWebviewBuilder,
    id: Uuid,
) -> tauri::Result<BoardWebviewBuilder> {
    Ok(builder.data_directory(webview_store::webviews_directory(app)?.join(id.to_string())))
}

fn navigate_home(app: &AppHandle, id: Uuid, target: Url) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let navigated = app
            .get_webview(&home_label(id))
            .map(|webview| webview.navigate(target));
        if let Some(Err(error)) = navigated {
            eprintln!("could not load a link in the window of server {id}: {error}");
        }
    });
}

fn route_new_window(app: &AppHandle, id: Uuid, server_url: &Url, requested: &Url) {
    match link_route::route(server_url, requested) {
        LinkRoute::SameWindow => navigate_home(app, id, requested.clone()),
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

fn handle_navigation(app: &AppHandle, id: Uuid, server_url: &Url, requested: &Url) -> bool {
    if !sentinel::is_sentinel(requested) {
        return true;
    }

    if let Some(Signal::Open(path)) = sentinel::parse(requested) {
        if let Some(target) = sentinel::same_origin_url(server_url, &path)
            .filter(|target| matches!(tabs::page_of(target.path()), Route::Page(_)))
        {
            eprintln!("server {id}: tab open requested");
            navigate_home(app, id, target);
        }
    }

    false
}

fn layout(app: &AppHandle, id: Uuid, size: PhysicalSize<u32>, scale_factor: f64) {
    let Some(webview) = app.get_webview(&home_label(id)) else {
        return;
    };
    let bounds = Rect {
        position: LogicalPosition::new(0.0, 0.0).into(),
        size: size.to_logical::<f64>(scale_factor).into(),
    };
    if let Err(error) = webview.set_bounds(bounds) {
        eprintln!("could not resize the window of server {id}: {error}");
    }
}

fn build_window(app: &AppHandle, server: &Server, url: Url) -> Result<(), ClientError> {
    let id = server.id;
    let window = WindowBuilder::new(app, label(id))
        .title(&server.name)
        .inner_size(1280.0, 860.0)
        .build()?;

    let navigation_app = app.clone();
    let navigation_server = url.clone();
    let window_app = app.clone();
    let window_server = url.clone();
    let builder = WebviewBuilder::new(home_label(id), WebviewUrl::External(url))
        .initialization_script(routing_script("home"))
        .initialization_script(include_str!("../scripts/external-links.js"))
        .on_navigation(move |requested| {
            handle_navigation(&navigation_app, id, &navigation_server, requested)
        })
        .on_new_window(move |requested, _features| {
            route_new_window(&window_app, id, &window_server, &requested);
            NewWindowResponse::Deny
        });

    let size = window
        .inner_size()?
        .to_logical::<f64>(window.scale_factor()?);
    window.add_child(
        with_own_storage(app, builder, id)?,
        LogicalPosition::new(0.0, 0.0),
        size,
    )?;

    let layout_app = app.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::Resized(size) => {
            let scale = layout_app
                .get_window(&label(id))
                .and_then(|window| window.scale_factor().ok());
            if let Some(scale) = scale {
                layout(&layout_app, id, *size, scale);
            }
        }
        WindowEvent::ScaleFactorChanged {
            scale_factor,
            new_inner_size,
            ..
        } => layout(&layout_app, id, *new_inner_size, *scale_factor),
        _ => {}
    });
    Ok(())
}

pub fn open(app: &AppHandle, server: &Server) -> Result<(), ClientError> {
    if let Some(window) = app.get_window(&label(server.id)) {
        window.unminimize()?;
        window.set_focus()?;
        return Ok(());
    }

    let url = check_server_url(server.url.as_str())?;
    build_window(app, server, url)
}

pub async fn connect_and_open(app: &AppHandle, server: &Server) -> Result<(), ClientError> {
    if app.get_window(&label(server.id)).is_none() {
        connection::check(&server.url).await?;
    }
    open(app, server)
}

pub fn retitle(app: &AppHandle, server: &Server) -> Result<(), ClientError> {
    if let Some(window) = app.get_window(&label(server.id)) {
        window.set_title(&server.name)?;
    }
    Ok(())
}

pub fn close(app: &AppHandle, id: Uuid) -> Result<(), ClientError> {
    if let Some(window) = app.get_window(&label(id)) {
        window.destroy()?;
    }
    Ok(())
}
