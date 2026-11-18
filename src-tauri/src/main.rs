#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod board_window;
mod commands;
mod error;
mod link_route;
mod server_url;
mod store;
mod webview_store;

use tauri::{AppHandle, Manager, RunEvent};

use crate::store::ServerStore;
use crate::webview_store::FailedDeletions;

#[cfg(target_os = "macos")]
fn show_server_list(app: &AppHandle) -> tauri::Result<()> {
    const MAIN_WINDOW: &str = "main";

    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        window.unminimize()?;
        return window.set_focus();
    }

    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == MAIN_WINDOW)
        .ok_or(tauri::Error::WindowNotFound)?;
    tauri::WebviewWindowBuilder::from_config(app, config)?.build()?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn on_run_event(app: &AppHandle, event: RunEvent) {
    if let RunEvent::Reopen { .. } = event {
        if let Err(error) = show_server_list(app) {
            eprintln!("could not show the server list: {error}");
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn on_run_event(_app: &AppHandle, _event: RunEvent) {}

fn main() {
    let app = tauri::Builder::default()
        .setup(|app| {
            let servers_file = app.path().app_config_dir()?.join("servers.json");
            app.manage(FailedDeletions::default());
            app.manage(ServerStore::new(servers_file));

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                webview_store::sweep(&handle).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_servers,
            commands::add_server,
            commands::remove_server,
            commands::rename_server,
            commands::open_server,
            commands::failed_store_deletions,
            commands::retry_store_deletion,
        ])
        .build(tauri::generate_context!())
        .expect("the app failed to start");

    app.run(on_run_event);
}
