use tauri::webview::{NewWindowResponse, PageLoadEvent, WebviewBuilder};
use tauri::window::WindowBuilder;
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, PhysicalSize, WebviewUrl, Window,
    WindowEvent, Wry,
};
use url::Url;
use uuid::Uuid;

use crate::connection;
use crate::error::ClientError;
use crate::layout::{self, Slot};
use crate::link_route::{self, LinkRoute};
use crate::sentinel::{self, Signal};
use crate::server_url::check_server_url;
use crate::store::Server;
use crate::tab_session::{self, TabSessions};
use crate::tabs::HOME;
#[cfg(not(any(target_os = "macos", target_os = "ios")))]
use crate::webview_store;

type BoardWebviewBuilder = WebviewBuilder<Wry>;

pub fn window_label(id: Uuid) -> String {
    format!("server-{id}")
}

pub fn tab_label(id: Uuid, tab: u64) -> String {
    if tab == HOME {
        format!("board-{id}-home")
    } else {
        format!("board-{id}-{tab}")
    }
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

fn path_and_query(url: &Url) -> String {
    match url.query() {
        Some(query) => format!("{}?{query}", url.path()),
        None => url.path().to_owned(),
    }
}

fn open_tab(app: &AppHandle, id: Uuid, path: &str) {
    let _ = tab_session::apply(app, id, |session| session.open(path));
}

fn route_new_window(app: &AppHandle, id: Uuid, server_url: &Url, requested: &Url) {
    match link_route::route(server_url, requested) {
        LinkRoute::SameWindow => open_tab(app, id, &path_and_query(requested)),
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

fn handle_navigation(app: &AppHandle, id: Uuid, tab: u64, requested: &Url) -> bool {
    if !sentinel::is_sentinel(requested) {
        return true;
    }

    match sentinel::parse(requested) {
        Some(Signal::Open(path)) if tab == HOME => open_tab(app, id, &path),
        Some(Signal::Route(path)) if tab != HOME => {
            let _ = tab_session::apply(app, id, |session| session.route(tab, &path));
        }
        _ => {}
    }

    false
}

fn report_load(app: &AppHandle, id: Uuid, tab: u64, server_url: &Url, loaded: &Url) {
    if loaded.origin() != server_url.origin() {
        return;
    }
    let path = path_and_query(loaded);
    let _ = tab_session::apply(app, id, |session| session.route(tab, &path));
}

fn board_builder(
    app: &AppHandle,
    id: Uuid,
    server_url: &Url,
    tab: u64,
    target: Url,
) -> BoardWebviewBuilder {
    let role = if tab == HOME { "home" } else { "tab" };
    let navigation_app = app.clone();
    let window_app = app.clone();
    let window_server = server_url.clone();
    let builder = WebviewBuilder::new(tab_label(id, tab), WebviewUrl::External(target))
        .initialization_script(routing_script(role))
        .initialization_script(include_str!("../scripts/external-links.js"))
        .on_navigation(move |requested| handle_navigation(&navigation_app, id, tab, requested))
        .on_new_window(move |requested, _features| {
            route_new_window(&window_app, id, &window_server, &requested);
            NewWindowResponse::Deny
        });

    if tab == HOME {
        return builder;
    }

    let load_app = app.clone();
    let load_server = server_url.clone();
    builder.on_page_load(move |_webview, payload| {
        if payload.event() == PageLoadEvent::Finished {
            report_load(&load_app, id, tab, &load_server, payload.url());
        }
    })
}

fn window_size(window: &Window) -> tauri::Result<(f64, f64)> {
    let size = window
        .inner_size()?
        .to_logical::<f64>(window.scale_factor()?);
    Ok((size.width, size.height))
}

pub fn add_board_webview(
    app: &AppHandle,
    window: &Window,
    id: Uuid,
    server_url: &Url,
    tab: u64,
    target: Url,
) -> Result<(), ClientError> {
    let builder = with_own_storage(app, board_builder(app, id, server_url, tab, target), id)?;
    let (width, height) = window_size(window)?;
    let content = layout::bounds(Slot::Content, width, height);
    window.add_child(
        builder,
        LogicalPosition::new(content.x, content.y),
        LogicalSize::new(content.width, content.height),
    )?;
    Ok(())
}

fn add_tab_bar(window: &Window, id: Uuid) -> Result<(), ClientError> {
    let (width, height) = window_size(window)?;
    let bar = layout::bounds(Slot::TabBar, width, height);
    window.add_child(
        WebviewBuilder::new(
            tab_session::tabbar_label(id),
            WebviewUrl::App("tabbar.html".into()),
        ),
        LogicalPosition::new(bar.x, bar.y),
        LogicalSize::new(bar.width, bar.height),
    )?;
    Ok(())
}

fn lay_out(app: &AppHandle, id: Uuid, size: PhysicalSize<u32>, scale_factor: f64) {
    let Some(window) = app.get_window(&window_label(id)) else {
        return;
    };
    let logical = size.to_logical::<f64>(scale_factor);
    for webview in window.webviews() {
        let slot = layout::slot_of(webview.label());
        let bounds = layout::bounds(slot, logical.width, logical.height);
        if let Err(error) = webview.set_bounds(bounds.into()) {
            eprintln!("could not resize the window of server {id}: {error}");
        }
    }
}

fn build_window(app: &AppHandle, server: &Server, url: Url) -> Result<(), ClientError> {
    let id = server.id;
    let window = WindowBuilder::new(app, window_label(id))
        .title(&server.name)
        .inner_size(1280.0, 860.0)
        .build()?;

    app.state::<TabSessions>().insert(id);

    let layout_app = app.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::Resized(size) => {
            let scale = layout_app
                .get_window(&window_label(id))
                .and_then(|window| window.scale_factor().ok());
            if let Some(scale) = scale {
                lay_out(&layout_app, id, *size, scale);
            }
        }
        WindowEvent::ScaleFactorChanged {
            scale_factor,
            new_inner_size,
            ..
        } => lay_out(&layout_app, id, *new_inner_size, *scale_factor),
        WindowEvent::Destroyed => layout_app.state::<TabSessions>().remove(id),
        _ => {}
    });

    let built = add_tab_bar(&window, id)
        .and_then(|()| add_board_webview(app, &window, id, &url, HOME, url.clone()));
    if built.is_err() {
        let _ = window.destroy();
    }
    built
}

pub fn open(app: &AppHandle, server: &Server) -> Result<(), ClientError> {
    if let Some(window) = app.get_window(&window_label(server.id)) {
        window.unminimize()?;
        window.set_focus()?;
        return Ok(());
    }

    let url = check_server_url(server.url.as_str())?;
    build_window(app, server, url)
}

pub async fn connect_and_open(app: &AppHandle, server: &Server) -> Result<(), ClientError> {
    if app.get_window(&window_label(server.id)).is_none() {
        connection::check(&server.url).await?;
    }
    open(app, server)
}

pub fn retitle(app: &AppHandle, server: &Server) -> Result<(), ClientError> {
    if let Some(window) = app.get_window(&window_label(server.id)) {
        window.set_title(&server.name)?;
    }
    Ok(())
}

pub fn close(app: &AppHandle, id: Uuid) -> Result<(), ClientError> {
    app.state::<TabSessions>().remove(id);
    if let Some(window) = app.get_window(&window_label(id)) {
        window.destroy()?;
    }
    Ok(())
}
