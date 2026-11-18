#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod board_window;
mod commands;
mod error;
mod list_window;
mod notifications;
mod server_url;
mod store;
mod tray;

use tauri::{AppHandle, Manager, RunEvent, Window, WindowEvent};

use crate::store::ServerStore;

fn on_window_event(window: &Window, event: &WindowEvent) {
    let WindowEvent::CloseRequested { api, .. } = event else {
        return;
    };

    let hides =
        window.label() == list_window::LABEL || board_window::stays_loaded_when_closed(window);
    if hides {
        api.prevent_close();
        if let Err(error) = window.hide() {
            eprintln!("could not hide the window {}: {error}", window.label());
        }
    }
}

fn start_reporting(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    notifications::identify_as(&app.config().identifier);

    match app.state::<ServerStore>().list() {
        Ok(servers) => servers
            .iter()
            .for_each(|server| board_window::start_reporting(app, server)),
        Err(error) => eprintln!("could not read the server list: {error:?}"),
    }
    board_window::keep_hidden_boards_awake(app.clone());
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
            start_reporting(app.handle());
            Ok(())
        })
        .on_window_event(on_window_event)
        .invoke_handler(tauri::generate_handler![
            commands::list_servers,
            commands::add_server,
            commands::remove_server,
            commands::set_server_notifications,
            commands::open_server,
            notifications::notify,
        ])
        .build(tauri::generate_context!())
        .expect("the app failed to start");

    app.run(on_run_event);
}
