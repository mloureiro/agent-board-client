use tauri::{AppHandle, State, Webview};
use uuid::Uuid;

use crate::board_window;
use crate::connection;
use crate::diagnostics;
use crate::error::ClientError;
use crate::opened_url::{OpenFailure, OpenFailures};
use crate::server_url::check_server_url;
use crate::store::{Server, ServerStore};
use crate::tab_session::{self, TabSessions};
use crate::tab_state;
use crate::tabs::{TabSession, TabsView, HOME};
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
    let closed = board_window::close(&app, id);
    if let Err(error) = tab_state::delete(&app, id) {
        eprintln!("server {id}: saved tabs could not be deleted: {error:?}");
    }
    tauri::async_runtime::spawn(async move {
        webview_store::delete_with_retry(&app, id, Some(name)).await;
    });
    closed
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
    let opened = board_window::connect_and_open(&app, &server).await;
    if let Err(error) = &opened {
        diagnostics::record(&app, &format!("could not open server {id}: {error:?}"));
    }
    opened
}

#[tauri::command]
pub async fn test_server(
    app: AppHandle,
    store: State<'_, ServerStore>,
    id: Uuid,
) -> Result<(), ClientError> {
    let server = store.find(id)?;
    let checked = connection::check(&server.url).await;
    if let Err(failure) = &checked {
        diagnostics::record(&app, &format!("test of server {id} failed: {failure:?}"));
    }
    Ok(checked?)
}

async fn check_typed(url: &str) -> Result<(), ClientError> {
    let url = check_server_url(url)?;
    connection::check(&url).await?;
    Ok(())
}

#[tauri::command]
pub async fn test_address(app: AppHandle, url: String) -> Result<(), ClientError> {
    let checked = check_typed(&url).await;
    if let Err(error) = &checked {
        diagnostics::record(&app, &format!("test of a typed address failed: {error:?}"));
    }
    checked
}

#[tauri::command]
pub fn take_open_failure(failures: State<'_, OpenFailures>) -> Option<OpenFailure> {
    failures.take()
}

fn server_of_tabbar(label: &str) -> Option<Uuid> {
    let id = Uuid::parse_str(label.strip_prefix("tabbar-")?).ok()?;
    (tab_session::tabbar_label(id) == label).then_some(id)
}

#[tauri::command]
pub fn tabs_snapshot(
    webview: Webview,
    sessions: State<'_, TabSessions>,
) -> Result<TabsView, ClientError> {
    let server = server_of_tabbar(webview.label()).ok_or(ClientError::NotFound)?;
    sessions.view(server)
}

#[tauri::command]
pub fn select_tab(app: AppHandle, webview: Webview, tab: u64) -> Result<(), ClientError> {
    let server = server_of_tabbar(webview.label()).ok_or(ClientError::NotFound)?;
    tab_session::apply(&app, server, |session: &mut TabSession| session.select(tab))
}

#[tauri::command]
pub fn close_tab(app: AppHandle, webview: Webview, tab: u64) -> Result<(), ClientError> {
    let server = server_of_tabbar(webview.label()).ok_or(ClientError::NotFound)?;
    if tab == HOME {
        return Ok(());
    }
    tab_session::apply(&app, server, |session: &mut TabSession| session.close(tab))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server_url::ServerUrlError;

    #[test]
    fn accepts_only_the_label_of_a_tab_bar() {
        let id = "0b0c3c1e-5a64-4a41-8f1a-6a0a2d2a9b11";
        let cases = [
            (format!("tabbar-{id}"), Some(Uuid::parse_str(id).ok())),
            (format!("board-{id}-home"), None),
            (format!("board-{id}-1"), None),
            (format!("server-{id}"), None),
            ("main".to_owned(), None),
            ("tabbar-nonsense".to_owned(), None),
            ("tabbar-".to_owned(), None),
            (format!("tabbar-{}", id.replace('-', "")), None),
            (format!("tabbar-{}", id.to_uppercase()), None),
        ];

        for (label, expected) in cases {
            assert_eq!(server_of_tabbar(&label), expected.flatten(), "{label}");
        }
    }

    #[test]
    fn a_rejected_address_is_not_contacted() {
        let result = tauri::async_runtime::block_on(check_typed("http://reviews.example.org"));

        assert!(matches!(
            result,
            Err(ClientError::Url {
                reason: ServerUrlError::Insecure
            })
        ));
    }
}
