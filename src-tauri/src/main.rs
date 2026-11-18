#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod board_window;
mod commands;
mod error;
mod list_window;
mod server_url;
mod store;
mod tray;

use tauri::{AppHandle, Manager, RunEvent, Window, WindowEvent};

use crate::store::ServerStore;

fn on_window_event(window: &Window, event: &WindowEvent) {
    let WindowEvent::CloseRequested { api, .. } = event else {
        return;
    };

    if window.label() == list_window::LABEL {
        api.prevent_close();
        if let Err(error) = window.hide() {
            eprintln!("could not hide the server list: {error}");
        }
    }
}

#[cfg(target_os = "macos")]
fn on_run_event(app: &AppHandle, event: RunEvent) {
    if let RunEvent::Reopen { .. } = event {
        if let Err(error) = list_window::show(app) {
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
            app.manage(ServerStore::new(servers_file));
            tray::install(app.handle())?;
            Ok(())
        })
        .on_window_event(on_window_event)
        .invoke_handler(tauri::generate_handler![
            commands::list_servers,
            commands::add_server,
            commands::remove_server,
            commands::set_server_notifications,
            commands::open_server,
        ])
        .build(tauri::generate_context!())
        .expect("the app failed to start");

    app.run(on_run_event);
}
