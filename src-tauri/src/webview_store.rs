#[cfg(not(any(target_os = "macos", target_os = "ios")))]
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::error::ClientError;
use crate::list_window::announce;
use crate::store::{Server, ServerStore};

const DELETION_FAILED: &str = "store-deletion-failed";
const DELETION_SUCCEEDED: &str = "store-deletion-succeeded";
const RETRY_DELAYS: [Duration; 5] = [
    Duration::from_millis(200),
    Duration::from_millis(500),
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FailedDeletion {
    pub id: Uuid,
    pub name: Option<String>,
}

#[derive(Default)]
pub struct FailedDeletions {
    records: Mutex<Vec<FailedDeletion>>,
}

impl FailedDeletions {
    fn records(&self) -> MutexGuard<'_, Vec<FailedDeletion>> {
        self.records.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn list(&self) -> Vec<FailedDeletion> {
        self.records().clone()
    }

    pub fn find(&self, id: Uuid) -> Option<FailedDeletion> {
        self.records()
            .iter()
            .find(|record| record.id == id)
            .cloned()
    }

    fn record(&self, failure: FailedDeletion) {
        let mut records = self.records();
        records.retain(|record| record.id != failure.id);
        records.push(failure);
    }

    fn clear(&self, id: Uuid) {
        self.records().retain(|record| record.id != id);
    }
}

pub fn orphans(existing: &[Uuid], servers: &[Server]) -> Vec<Uuid> {
    existing
        .iter()
        .copied()
        .filter(|id| servers.iter().all(|server| server.id != *id))
        .collect()
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
pub fn webviews_directory(app: &AppHandle) -> tauri::Result<PathBuf> {
    Ok(app.path().app_data_dir()?.join("webviews"))
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub async fn existing(app: &AppHandle) -> Result<Vec<Uuid>, ClientError> {
    let identifiers = app.fetch_data_store_identifiers().await?;
    Ok(identifiers.into_iter().map(Uuid::from_bytes).collect())
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
pub async fn existing(app: &AppHandle) -> Result<Vec<Uuid>, ClientError> {
    let entries = match std::fs::read_dir(webviews_directory(app)?) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };

    let mut ids = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        if let Some(id) = entry
            .file_name()
            .to_str()
            .and_then(|name| Uuid::parse_str(name).ok())
        {
            ids.push(id);
        }
    }
    Ok(ids)
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub async fn delete(app: &AppHandle, id: Uuid) -> Result<(), ClientError> {
    if existing(app).await?.contains(&id) {
        app.remove_data_store(id.into_bytes()).await?;
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
pub async fn delete(app: &AppHandle, id: Uuid) -> Result<(), ClientError> {
    match std::fs::remove_dir_all(webviews_directory(app)?.join(id.to_string())) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
}

pub async fn delete_with_retry(app: &AppHandle, id: Uuid, name: Option<String>) {
    let failed = app.state::<FailedDeletions>();

    let mut outcome = delete(app, id).await;
    for delay in RETRY_DELAYS {
        if outcome.is_ok() {
            break;
        }
        tokio::time::sleep(delay).await;
        outcome = delete(app, id).await;
    }

    match outcome {
        Ok(()) => {
            failed.clear(id);
            announce(app, DELETION_SUCCEEDED, id);
        }
        Err(error) => {
            eprintln!("could not delete the data store of server {id}: {error:?}");
            let failure = FailedDeletion { id, name };
            failed.record(failure.clone());
            announce(app, DELETION_FAILED, failure);
        }
    }
}

pub async fn sweep(app: &AppHandle) {
    let servers = match app.state::<ServerStore>().list() {
        Ok(servers) => servers,
        Err(error) => {
            eprintln!("skipped the store sweep, the server list could not be read: {error:?}");
            return;
        }
    };
    let existing = match existing(app).await {
        Ok(existing) => existing,
        Err(error) => {
            eprintln!("skipped the store sweep, the stores could not be listed: {error:?}");
            return;
        }
    };

    for id in orphans(&existing, &servers) {
        delete_with_retry(app, id, None).await;
    }
}

#[cfg(test)]
mod tests {
    use url::Url;

    use super::*;

    fn server(id: Uuid) -> Server {
        Server {
            id,
            name: "Board".to_owned(),
            url: Url::parse("https://board.example.com").unwrap(),
        }
    }

    #[test]
    fn reports_only_stores_without_a_server() {
        let kept = Uuid::from_u128(1);
        let other_kept = Uuid::from_u128(2);
        let orphan = Uuid::from_u128(3);
        let not_opened_yet = Uuid::from_u128(4);

        let cases = [
            ("no stores", vec![], vec![server(kept)], vec![]),
            (
                "every store has a server",
                vec![kept, other_kept],
                vec![server(kept), server(other_kept)],
                vec![],
            ),
            (
                "one orphan among kept stores",
                vec![kept, orphan, other_kept],
                vec![server(kept), server(other_kept)],
                vec![orphan],
            ),
            (
                "a server with no store yet",
                vec![orphan],
                vec![server(not_opened_yet)],
                vec![orphan],
            ),
        ];

        for (name, existing, servers, expected) in cases {
            assert_eq!(orphans(&existing, &servers), expected, "{name}");
        }
    }

    #[test]
    fn records_one_failure_per_server() {
        let failed = FailedDeletions::default();
        let id = Uuid::from_u128(1);
        failed.record(FailedDeletion { id, name: None });
        failed.record(FailedDeletion {
            id,
            name: Some("Board".to_owned()),
        });
        assert_eq!(failed.list().len(), 1);
        assert_eq!(failed.find(id).unwrap().name.as_deref(), Some("Board"));
        failed.clear(id);
        assert!(failed.find(id).is_none());
    }
}
