use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use crate::error::ClientError;
use crate::server_url::check_server_url;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Server {
    pub id: Uuid,
    pub name: String,
    pub url: Url,
}

#[derive(Default, Serialize, Deserialize)]
struct ServerFile {
    servers: Vec<Server>,
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
        Ok(read(&self.path())?)
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
        let server = Server {
            id: Uuid::new_v4(),
            name: name.to_owned(),
            url: check_server_url(url)?,
        };

        let path = self.path();
        let mut servers = read(&path)?;
        servers.push(server.clone());
        write(&path, servers)?;
        Ok(server)
    }

    pub fn remove(&self, id: Uuid) -> Result<(), ClientError> {
        let path = self.path();
        let servers = read(&path)?;
        let kept: Vec<Server> = servers
            .iter()
            .filter(|server| server.id != id)
            .cloned()
            .collect();
        if kept.len() == servers.len() {
            return Err(ClientError::NotFound);
        }

        write(&path, kept)?;
        Ok(())
    }
}

fn read(path: &PathBuf) -> io::Result<Vec<Server>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let file: ServerFile = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
    Ok(file.servers)
}

fn write(path: &PathBuf, servers: Vec<Server>) -> io::Result<()> {
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory)?;
    }
    let bytes = serde_json::to_vec_pretty(&ServerFile { servers }).map_err(io::Error::other)?;
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

        store.remove(home.id).unwrap();

        assert_eq!(store.list().unwrap(), vec![office]);
        assert!(matches!(store.find(home.id), Err(ClientError::NotFound)));
    }

    #[test]
    fn removing_an_unknown_server_reports_it() {
        let store = ServerStore::new(scratch_path());

        assert!(matches!(
            store.remove(Uuid::new_v4()),
            Err(ClientError::NotFound)
        ));
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
