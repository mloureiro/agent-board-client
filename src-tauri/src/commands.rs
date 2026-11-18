use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::board_window;
use crate::error::ClientError;
use crate::store::{Server, ServerStore, StoredData};

#[tauri::command]
pub fn list_servers(store: State<'_, ServerStore>) -> Result<Vec<Server>, ClientError> {
    store.list()
}

// Tauri deadlocks on Windows when a window is created from a synchronous command.
#[tauri::command]
pub async fn add_server(
    app: AppHandle,
    store: State<'_, ServerStore>,
    name: String,
    url: String,
) -> Result<Server, ClientError> {
    let server = store.add(&name, &url)?;
    board_window::start_reporting(&app, &server);
    Ok(server)
}

#[tauri::command]
pub async fn remove_server(
    app: AppHandle,
    store: State<'_, ServerStore>,
    id: Uuid,
    delete_data: bool,
) -> Result<(), ClientError> {
    store.find(id)?;
    board_window::close(&app, id)?;
    if !delete_data {
        return store.remove(id, StoredData::Keep);
    }

    board_window::delete_stored_data(&app, id).await?;
    store.remove(id, StoredData::Delete)
}

#[tauri::command]
pub async fn set_server_notifications(
    app: AppHandle,
    store: State<'_, ServerStore>,
    id: Uuid,
    enabled: bool,
) -> Result<Server, ClientError> {
    let server = store.set_notifications(id, enabled)?;
    if enabled {
        board_window::load_in_background(&app, &server)?;
    } else {
        board_window::unload_if_hidden(&app, id)?;
    }
    Ok(server)
}

#[tauri::command]
pub async fn open_server(
    app: AppHandle,
    store: State<'_, ServerStore>,
    id: Uuid,
) -> Result<(), ClientError> {
    board_window::open(&app, &store.find(id)?)
}
