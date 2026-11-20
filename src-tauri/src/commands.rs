use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::board_window;
use crate::connection;
use crate::error::ClientError;
use crate::opened_url::{OpenFailure, OpenFailures};
use crate::store::{Server, ServerStore};
use crate::webview_store::{self, FailedDeletion, FailedDeletions};

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
    let name = store.find(id)?.name;
    store.remove(id)?;
    board_window::close(&app, id)?;
    tauri::async_runtime::spawn(async move {
        webview_store::delete_with_retry(&app, id, Some(name)).await;
    });
    Ok(())
}

#[tauri::command]
pub fn failed_store_deletions(failed: State<'_, FailedDeletions>) -> Vec<FailedDeletion> {
    failed.list()
}

#[tauri::command]
pub fn retry_store_deletion(
    app: AppHandle,
    failed: State<'_, FailedDeletions>,
    id: Uuid,
) -> Result<(), ClientError> {
    let record = failed.find(id).ok_or(ClientError::NotFound)?;
    tauri::async_runtime::spawn(async move {
        webview_store::delete_with_retry(&app, id, record.name).await;
    });
    Ok(())
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
    let server = store.find(id)?;
    board_window::connect_and_open(&app, &server).await
}

#[tauri::command]
pub async fn test_server(store: State<'_, ServerStore>, id: Uuid) -> Result<(), ClientError> {
    let server = store.find(id)?;
    Ok(connection::check(&server.url).await?)
}

#[tauri::command]
pub fn take_open_failure(failures: State<'_, OpenFailures>) -> Option<OpenFailure> {
    failures.take()
}
