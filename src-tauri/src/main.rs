#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod board_window;
mod commands;
mod connection;
mod error;
mod link_route;
mod list_window;
mod opened_url;
mod server_url;
mod store;
mod webview_store;

use tauri::{AppHandle, Manager, RunEvent};

use crate::opened_url::OpenFailures;
use crate::store::ServerStore;
use crate::webview_store::FailedDeletions;

#[cfg(target_os = "macos")]
fn on_run_event(app: &AppHandle, event: RunEvent) {
    match event {
        RunEvent::Reopen { .. } => {
            if let Err(error) = list_window::show(app) {
                eprintln!("could not show the server list: {error}");
            }
        }
        RunEvent::Opened { urls } => opened_url::handle(app, &urls),
        _ => {}
    }
}

#[cfg(not(target_os = "macos"))]
fn on_run_event(_app: &AppHandle, _event: RunEvent) {}

fn main() {
    let app = tauri::Builder::default()
        .setup(|app| {
            let servers_file = app.path().app_config_dir()?.join("servers.json");
            app.manage(FailedDeletions::default());
            app.manage(OpenFailures::default());
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
            commands::test_server,
            commands::take_open_failure,
            commands::failed_store_deletions,
            commands::retry_store_deletion,
        ])
        .build(tauri::generate_context!())
        .expect("the app failed to start");

    app.run(on_run_event);
}
