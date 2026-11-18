use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::board_window;
use crate::error::ClientError;
use crate::store::{Server, ServerStore};

#[tauri::command]
pub fn list_servers(store: State<'_, ServerStore>) -> Result<Vec<Server>, ClientError> {
    store.list()
}

#[tauri::command]
pub fn add_server(
    store: State<'_, ServerStore>,
    name: String,
    url: String,
) -> Result<Server, ClientError> {
    store.add(&name, &url)
}

#[tauri::command]
pub fn remove_server(
    app: AppHandle,
    store: State<'_, ServerStore>,
    id: Uuid,
) -> Result<(), ClientError> {
    store.remove(id)?;
    board_window::close(&app, id)
}

#[tauri::command]
pub fn rename_server(
    app: AppHandle,
    store: State<'_, ServerStore>,
    id: Uuid,
    name: String,
) -> Result<Server, ClientError> {
    let server = store.rename(id, &name)?;
    board_window::retitle(&app, &server)?;
    Ok(server)
}

// Tauri deadlocks on Windows when a window is created from a synchronous command.
#[tauri::command]
pub async fn open_server(
    app: AppHandle,
    store: State<'_, ServerStore>,
    id: Uuid,
) -> Result<(), ClientError> {
    board_window::open(&app, &store.find(id)?)
}
