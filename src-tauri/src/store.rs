use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use crate::error::ClientError;
use crate::server_url::check_server_url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationSettings {
    pub enabled: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Server {
    pub id: Uuid,
    pub name: String,
    pub url: Url,
    #[serde(default)]
    pub notifications: NotificationSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredData {
    Keep,
    Delete,
}

#[derive(Serialize, Deserialize)]
struct RetainedData {
    id: Uuid,
    url: Url,
}

#[derive(Default, Serialize, Deserialize)]
struct ServerFile {
    servers: Vec<Server>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    retained_data: Vec<RetainedData>,
}

pub struct ServerStore {
    path: Mutex<PathBuf>,
}

impl ServerStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path: Mutex::new(path),
        }
    }

    fn path(&self) -> MutexGuard<'_, PathBuf> {
        self.path.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn list(&self) -> Result<Vec<Server>, ClientError> {
        Ok(read(&self.path())?.servers)
    }

    pub fn find(&self, id: Uuid) -> Result<Server, ClientError> {
        self.list()?
            .into_iter()
            .find(|server| server.id == id)
            .ok_or(ClientError::NotFound)
    }

    pub fn add(&self, name: &str, url: &str) -> Result<Server, ClientError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(ClientError::Name);
        }
        let url = check_server_url(url)?;

        let path = self.path();
        let mut file = read(&path)?;
        let retained = file
            .retained_data
            .iter()
            .position(|retained| retained.url == url)
            .map(|position| file.retained_data.remove(position));
        let server = Server {
            id: retained.map_or_else(Uuid::new_v4, |retained| retained.id),
            name: name.to_owned(),
            url,
            notifications: NotificationSettings::default(),
        };
        file.servers.push(server.clone());
        write(&path, &file)?;
        Ok(server)
    }

    pub fn set_notifications(&self, id: Uuid, enabled: bool) -> Result<Server, ClientError> {
        let path = self.path();
        let mut file = read(&path)?;
        let server = file
            .servers
            .iter_mut()
            .find(|server| server.id == id)
            .ok_or(ClientError::NotFound)?;
        server.notifications.enabled = enabled;
        let updated = server.clone();

        write(&path, &file)?;
        Ok(updated)
    }

    pub fn remove(&self, id: Uuid, stored_data: StoredData) -> Result<(), ClientError> {
        let path = self.path();
        let mut file = read(&path)?;
        let position = file
            .servers
            .iter()
            .position(|server| server.id == id)
            .ok_or(ClientError::NotFound)?;
        let removed = file.servers.remove(position);
        if stored_data == StoredData::Keep {
            file.retained_data.push(RetainedData {
                id: removed.id,
                url: removed.url,
            });
        }

        write(&path, &file)?;
        Ok(())
    }
}

fn read(path: &PathBuf) -> io::Result<ServerFile> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(ServerFile::default()),
        Err(error) => return Err(error),
    };
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

fn write(path: &PathBuf, file: &ServerFile) -> io::Result<()> {
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory)?;
    }
    let bytes = serde_json::to_vec_pretty(file).map_err(io::Error::other)?;
    let staged = path.with_extension("json.tmp");
    fs::write(&staged, bytes)?;
    fs::rename(staged, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_path() -> PathBuf {
        std::env::temp_dir()
            .join(format!("agent-board-client-test-{}", Uuid::new_v4()))
            .join("servers.json")
    }

    #[test]
    fn a_missing_file_is_an_empty_list() {
        let store = ServerStore::new(scratch_path());

        assert_eq!(store.list().unwrap(), Vec::new());
    }

    #[test]
    fn an_added_server_is_read_back_by_a_new_store_on_the_same_file() {
        let path = scratch_path();

        let added = ServerStore::new(path.clone())
            .add("  Home  ", "http://localhost:4000")
            .unwrap();

        assert_eq!(added.name, "Home");
        assert_eq!(added.url.as_str(), "http://localhost:4000/");
        assert_eq!(ServerStore::new(path).list().unwrap(), vec![added]);
    }

    #[test]
    fn a_blank_name_is_rejected_and_nothing_is_saved() {
        let store = ServerStore::new(scratch_path());

        let result = store.add("   ", "https://board.example.com");

        assert!(matches!(result, Err(ClientError::Name)));
        assert_eq!(store.list().unwrap(), Vec::new());
    }

    #[test]
    fn a_rejected_url_is_not_saved() {
        let store = ServerStore::new(scratch_path());

        let result = store.add("Office", "http://board.example.com");

        assert!(matches!(result, Err(ClientError::Url { .. })));
        assert_eq!(store.list().unwrap(), Vec::new());
    }

    #[test]
    fn removing_a_server_keeps_the_others() {
        let store = ServerStore::new(scratch_path());
        let home = store.add("Home", "http://localhost:4000").unwrap();
        let office = store.add("Office", "https://board.example.com").unwrap();

        store.remove(home.id, StoredData::Delete).unwrap();

        assert_eq!(store.list().unwrap(), vec![office]);
        assert!(matches!(store.find(home.id), Err(ClientError::NotFound)));
    }

    #[test]
    fn a_new_server_has_notifications_on() {
        let store = ServerStore::new(scratch_path());

        let home = store.add("Home", "http://localhost:4000").unwrap();

        assert!(home.notifications.enabled);
    }

    #[test]
    fn an_entry_saved_without_the_setting_has_notifications_on() {
        let path = scratch_path();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let id = Uuid::new_v4();
        let saved = format!(
            r#"{{"servers":[{{"id":"{id}","name":"Home","url":"http://localhost:4000/"}}]}}"#
        );
        fs::write(&path, saved).unwrap();

        let home = ServerStore::new(path).find(id).unwrap();

        assert!(home.notifications.enabled);
    }

    #[test]
    fn switching_notifications_off_is_read_back_by_a_new_store() {
        let path = scratch_path();
        let store = ServerStore::new(path.clone());
        let home = store.add("Home", "http://localhost:4000").unwrap();
        let office = store.add("Office", "https://board.example.com").unwrap();

        let updated = store.set_notifications(home.id, false).unwrap();

        assert!(!updated.notifications.enabled);
        let reread = ServerStore::new(path);
        assert!(!reread.find(home.id).unwrap().notifications.enabled);
        assert!(reread.find(office.id).unwrap().notifications.enabled);
    }

    #[test]
    fn switching_notifications_on_an_unknown_server_reports_it() {
        let store = ServerStore::new(scratch_path());

        assert!(matches!(
            store.set_notifications(Uuid::new_v4(), false),
            Err(ClientError::NotFound)
        ));
    }

    #[test]
    fn removing_an_unknown_server_reports_it() {
        let store = ServerStore::new(scratch_path());

        assert!(matches!(
            store.remove(Uuid::new_v4(), StoredData::Delete),
            Err(ClientError::NotFound)
        ));
    }

    #[test]
    fn a_server_removed_with_its_data_kept_gets_the_same_id_when_its_address_returns() {
        let path = scratch_path();
        let store = ServerStore::new(path.clone());
        let home = store.add("Home", "http://localhost:4000").unwrap();
        store.remove(home.id, StoredData::Keep).unwrap();
        assert_eq!(store.list().unwrap(), Vec::new());

        let returned = ServerStore::new(path)
            .add("Home again", "http://localhost:4000/")
            .unwrap();

        assert_eq!(returned.id, home.id);
    }

    #[test]
    fn kept_data_is_handed_to_one_returning_server_only() {
        let store = ServerStore::new(scratch_path());
        let home = store.add("Home", "http://localhost:4000").unwrap();
        store.remove(home.id, StoredData::Keep).unwrap();

        let first = store.add("First", "http://localhost:4000").unwrap();
        let second = store.add("Second", "http://localhost:4000").unwrap();

        assert_eq!(first.id, home.id);
        assert_ne!(second.id, home.id);
    }

    #[test]
    fn kept_data_is_not_handed_to_another_address() {
        let store = ServerStore::new(scratch_path());
        let home = store.add("Home", "http://localhost:4000").unwrap();
        store.remove(home.id, StoredData::Keep).unwrap();

        let other = store.add("Other", "http://localhost:4000/other").unwrap();

        assert_ne!(other.id, home.id);
    }

    #[test]
    fn a_server_removed_with_its_data_deleted_gets_a_new_id_when_its_address_returns() {
        let store = ServerStore::new(scratch_path());
        let home = store.add("Home", "http://localhost:4000").unwrap();
        store.remove(home.id, StoredData::Delete).unwrap();

        let returned = store.add("Home", "http://localhost:4000").unwrap();

        assert_ne!(returned.id, home.id);
    }

    #[test]
    fn an_unreadable_file_is_an_error_and_is_left_untouched() {
        let path = scratch_path();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "not json").unwrap();
        let store = ServerStore::new(path.clone());

        let result = store.add("Home", "http://localhost:4000");

        assert!(matches!(result, Err(ClientError::Storage { .. })));
        assert_eq!(fs::read_to_string(path).unwrap(), "not json");
    }
}
