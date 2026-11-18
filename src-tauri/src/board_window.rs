use std::time::Duration;

use tauri::ipc::CapabilityBuilder;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Window, Wry};
use uuid::Uuid;

use crate::error::ClientError;
use crate::server_url::check_server_url;
use crate::store::{Server, ServerStore};

type BoardWindowBuilder<'a> = WebviewWindowBuilder<'a, Wry, AppHandle>;

const LABEL_PREFIX: &str = "server-";
const NOTIFICATION_SCRIPT: &str = include_str!("../../src/board-notifications.js");
const NOTIFY_PERMISSION: &str = "allow-notify";
const KEEP_AWAKE_EVERY: Duration = Duration::from_secs(5);

fn label(id: Uuid) -> String {
    format!("{LABEL_PREFIX}{id}")
}

pub fn server_id(label: &str) -> Option<Uuid> {
    label.strip_prefix(LABEL_PREFIX)?.parse().ok()
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
fn data_directory(app: &AppHandle, id: Uuid) -> tauri::Result<std::path::PathBuf> {
    Ok(app
        .path()
        .app_data_dir()?
        .join("webviews")
        .join(id.to_string()))
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
fn with_own_storage<'a>(
    app: &AppHandle,
    builder: BoardWindowBuilder<'a>,
    id: Uuid,
) -> tauri::Result<BoardWindowBuilder<'a>> {
    Ok(builder.data_directory(data_directory(app, id)?))
}

fn stored_data_error(error: impl std::fmt::Display) -> ClientError {
    ClientError::StoredData {
        message: error.to_string(),
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub async fn delete_stored_data(app: &AppHandle, id: Uuid) -> Result<(), ClientError> {
    let store = id.into_bytes();
    let exists = app
        .fetch_data_store_identifiers()
        .await
        .map_err(stored_data_error)?
        .contains(&store);
    if exists {
        app.remove_data_store(store)
            .await
            .map_err(stored_data_error)?;
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
pub async fn delete_stored_data(app: &AppHandle, id: Uuid) -> Result<(), ClientError> {
    let directory = data_directory(app, id).map_err(stored_data_error)?;
    match std::fs::remove_dir_all(directory) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(stored_data_error(error)),
    }
}

fn build(app: &AppHandle, server: &Server, visible: bool) -> Result<WebviewWindow, ClientError> {
    let url = check_server_url(server.url.as_str())?;
    let builder = WebviewWindowBuilder::new(app, label(server.id), WebviewUrl::External(url))
        .title(&server.name)
        .inner_size(1280.0, 860.0)
        .initialization_script(NOTIFICATION_SCRIPT)
        .visible(visible);
    Ok(with_own_storage(app, builder, server.id)?.build()?)
}

// The ACL reads a remote URL as a URLPattern, where a bare colon inside an IPv6 host starts a group name.
fn origin_pattern(server: &Server) -> String {
    let origin = server.url.origin().ascii_serialization();
    let Some((before_host, bracketed)) = origin.split_once('[') else {
        return origin;
    };
    let Some((host, after_host)) = bracketed.split_once(']') else {
        return origin;
    };
    format!("{before_host}[{}]{after_host}", host.replace(':', "\\:"))
}

fn allow_notifications(app: &AppHandle, server: &Server) -> Result<(), ClientError> {
    let label = label(server.id);
    let capability = CapabilityBuilder::new(&label)
        .local(false)
        .window(&label)
        .remote(origin_pattern(server))
        .permission(NOTIFY_PERMISSION);
    Ok(app.add_capability(capability)?)
}

pub fn start_reporting(app: &AppHandle, server: &Server) {
    let started = allow_notifications(app, server).and_then(|()| {
        if server.notifications.enabled {
            load_in_background(app, server)?;
        }
        Ok(())
    });
    if let Err(error) = started {
        eprintln!("{} cannot report in the background: {error:?}", server.name);
    }
}

pub fn open(app: &AppHandle, server: &Server) -> Result<(), ClientError> {
    let window = match app.get_webview_window(&label(server.id)) {
        Some(window) => window,
        None => build(app, server, true)?,
    };
    window.show()?;
    window.unminimize()?;
    window.set_focus()?;
    Ok(())
}

pub fn load_in_background(app: &AppHandle, server: &Server) -> Result<(), ClientError> {
    if app.get_webview_window(&label(server.id)).is_none() {
        build(app, server, false)?;
    }
    Ok(())
}

pub fn unload_if_hidden(app: &AppHandle, id: Uuid) -> Result<(), ClientError> {
    if let Some(window) = app.get_webview_window(&label(id)) {
        if !window.is_visible()? {
            window.destroy()?;
        }
    }
    Ok(())
}

pub fn close(app: &AppHandle, id: Uuid) -> Result<(), ClientError> {
    if let Some(window) = app.get_webview_window(&label(id)) {
        window.destroy()?;
    }
    Ok(())
}

pub fn stays_loaded_when_closed(window: &Window) -> bool {
    server_id(window.label())
        .and_then(|id| window.state::<ServerStore>().find(id).ok())
        .is_some_and(|server| server.notifications.enabled)
}

pub fn keep_hidden_boards_awake(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(KEEP_AWAKE_EVERY);
        for (label, window) in app.webview_windows() {
            let is_hidden_board =
                server_id(&label).is_some() && matches!(window.is_visible(), Ok(false));
            if !is_hidden_board {
                continue;
            }
            // WKWebView stops a hidden page's timers within seconds unless script is evaluated in it.
            if let Err(error) = window.eval("void 0") {
                eprintln!("could not keep {label} awake: {error}");
            }
        }
    });
}

pub fn activate_notification(app: &AppHandle, id: Uuid, key: &str) {
    let script = format!(
        "window.__agentBoardClient?.activateNotification({})",
        serde_json::Value::from(key)
    );
    let handle = app.clone();
    let scheduled = app.run_on_main_thread(move || {
        let opened = handle
            .state::<ServerStore>()
            .find(id)
            .and_then(|server| open(&handle, &server));
        if let Err(error) = opened {
            eprintln!("could not open the notifying server: {error:?}");
            return;
        }

        let evaluated = handle
            .get_webview_window(&label(id))
            .map(|window| window.eval(&script));
        if let Some(Err(error)) = evaluated {
            eprintln!("could not hand the click to the board: {error}");
        }
    });
    if let Err(error) = scheduled {
        eprintln!("could not open the notifying server: {error}");
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use serde::Deserialize;
    use tauri::utils::acl::RemoteUrlPattern;

    use super::*;
    use crate::store::NotificationSettings;

    #[derive(Deserialize)]
    struct Case {
        url: Option<String>,
    }

    fn server_at(url: &str) -> Server {
        Server {
            id: Uuid::new_v4(),
            name: "Home".to_owned(),
            url: url.parse().unwrap(),
            notifications: NotificationSettings::default(),
        }
    }

    #[test]
    fn a_window_label_gives_back_its_server_id() {
        let id = Uuid::new_v4();

        assert_eq!(server_id(&label(id)), Some(id));
        assert_eq!(server_id("main"), None);
        assert_eq!(server_id("server-not-a-uuid"), None);
    }

    #[test]
    fn every_accepted_address_gives_an_origin_the_acl_matches() {
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("../../fixtures/server-url-cases.json")).unwrap();

        for accepted in cases.into_iter().filter_map(|case| case.url) {
            let server = server_at(&accepted);

            let pattern = RemoteUrlPattern::from_str(&origin_pattern(&server))
                .unwrap_or_else(|error| panic!("pattern for {accepted}: {error:?}"));

            assert!(pattern.test(&server.url), "{accepted}");
            let deeper = server.url.join("t/task-1?x=1#y").unwrap();
            assert!(pattern.test(&deeper), "{deeper}");
        }
    }

    #[test]
    fn the_acl_pattern_of_one_server_rejects_another_origin() {
        let server = server_at("https://board.example.com/");
        let pattern = RemoteUrlPattern::from_str(&origin_pattern(&server)).unwrap();

        for other in [
            "https://board.example.com.evil.test/",
            "https://evil.test/board.example.com",
            "http://board.example.com/",
            "https://board.example.com:8443/",
        ] {
            assert!(!pattern.test(&other.parse().unwrap()), "{other}");
        }
    }
}
