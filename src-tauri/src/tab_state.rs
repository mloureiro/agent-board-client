use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::error::ClientError;
use crate::sentinel::is_plain_path;
use crate::store::ServerStore;
use crate::tabs::{page_of, Route, MAX_TABS};
use crate::webview_store;
use crate::zoom::Zoom;

const MAX_LABEL_CHARS: usize = 200;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedTab {
    pub path: String,
    pub label: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedTabs {
    pub selected: u64,
    pub tabs: Vec<SavedTab>,
    #[serde(default)]
    pub zoom: Zoom,
}

fn directory(app: &AppHandle) -> tauri::Result<PathBuf> {
    Ok(app.path().app_data_dir()?.join("tabs"))
}

fn file(app: &AppHandle, id: Uuid) -> Result<PathBuf, ClientError> {
    Ok(directory(app)?.join(format!("{id}.json")))
}

fn load(path: &Path) -> io::Result<SavedTabs> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(SavedTabs::default()),
        Err(error) => return Err(error),
    };
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

fn store(path: &Path, saved: &SavedTabs) -> io::Result<()> {
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory)?;
    }
    let bytes = serde_json::to_vec_pretty(saved).map_err(io::Error::other)?;
    let staged = path.with_extension("json.tmp");
    fs::write(&staged, bytes)?;
    fs::rename(staged, path)
}

fn remove(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

pub fn read(app: &AppHandle, id: Uuid) -> SavedTabs {
    let loaded = file(app, id)
        .map_err(|_| io::Error::from(io::ErrorKind::Other))
        .and_then(|path| load(&path));
    match loaded {
        Ok(saved) => saved,
        Err(error) => {
            eprintln!(
                "server {id}: saved tabs could not be read: {:?}",
                error.kind()
            );
            SavedTabs::default()
        }
    }
}

pub fn write(app: &AppHandle, id: Uuid, saved: &SavedTabs) -> Result<(), ClientError> {
    Ok(store(&file(app, id)?, saved)?)
}

pub fn delete(app: &AppHandle, id: Uuid) -> Result<(), ClientError> {
    Ok(remove(&file(app, id)?)?)
}

pub fn restore(saved: SavedTabs) -> SavedTabs {
    let total = saved.tabs.len() as u64;
    let mut pages = Vec::new();
    let mut tabs = Vec::new();
    let mut selected = 0;

    for (index, tab) in saved.tabs.into_iter().enumerate() {
        let Route::Page(page) = page_of(&tab.path) else {
            continue;
        };
        if !is_plain_path(&tab.path) || pages.contains(&page) || tabs.len() >= MAX_TABS {
            continue;
        }
        pages.push(page);
        tabs.push(SavedTab {
            path: tab.path,
            label: tab.label.chars().take(MAX_LABEL_CHARS).collect(),
        });
        if saved.selected == index as u64 + 1 {
            selected = tabs.len() as u64;
        }
    }

    if saved.selected > total {
        selected = tabs.len() as u64;
    }
    SavedTabs {
        selected,
        tabs,
        zoom: saved.zoom.restored(),
    }
}

pub fn tab_file_ids(names: &[OsString]) -> Vec<Uuid> {
    names
        .iter()
        .filter_map(|name| {
            let path = Path::new(name);
            if path.extension()? != "json" {
                return None;
            }
            let stem = path.file_stem()?.to_str()?;
            let id = Uuid::parse_str(stem).ok()?;
            (id.to_string() == stem).then_some(id)
        })
        .collect()
}

fn file_names(app: &AppHandle) -> Result<Vec<OsString>, ClientError> {
    let entries = match fs::read_dir(directory(app)?) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut names = Vec::new();
    for entry in entries {
        names.push(entry?.file_name());
    }
    Ok(names)
}

pub async fn sweep(app: &AppHandle) {
    let servers = match app.state::<ServerStore>().list() {
        Ok(servers) => servers,
        Err(error) => {
            eprintln!("skipped the tab sweep, the server list could not be read: {error:?}");
            return;
        }
    };
    let names = match file_names(app) {
        Ok(names) => names,
        Err(error) => {
            eprintln!("skipped the tab sweep, the saved tabs could not be listed: {error:?}");
            return;
        }
    };

    for id in webview_store::orphans(&tab_file_ids(&names), &servers) {
        if let Err(error) = delete(app, id) {
            eprintln!("server {id}: saved tabs could not be deleted: {error:?}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved(selected: u64, entries: &[(&str, &str)]) -> SavedTabs {
        SavedTabs {
            selected,
            tabs: entries
                .iter()
                .map(|(path, label)| SavedTab {
                    path: (*path).to_owned(),
                    label: (*label).to_owned(),
                })
                .collect(),
            zoom: Zoom::default(),
        }
    }

    #[test]
    fn restores_only_valid_distinct_pages_within_the_cap() {
        let many: Vec<(String, String)> = (1..=25)
            .map(|n| (format!("/t/{n}"), "Task".to_owned()))
            .collect();
        let many: Vec<(&str, &str)> = many.iter().map(|(p, l)| (p.as_str(), l.as_str())).collect();
        let long = "x".repeat(500);
        let cut = "x".repeat(200);

        let cases = [
            (
                "a valid file passes unchanged",
                saved(
                    2,
                    &[("/settings/machines", "Settings"), ("/t/k3J9", "Task")],
                ),
                saved(
                    2,
                    &[("/settings/machines", "Settings"), ("/t/k3J9", "Task")],
                ),
            ),
            (
                "an Other path is dropped",
                saved(0, &[("/login/oidc", "Sign in"), ("/backlog", "Backlog")]),
                saved(0, &[("/backlog", "Backlog")]),
            ),
            (
                "a Home path is dropped",
                saved(0, &[("/", "Board"), ("/backlog", "Backlog")]),
                saved(0, &[("/backlog", "Backlog")]),
            ),
            (
                "an implausible path is dropped",
                saved(0, &[("/backlog\\x", "Backlog"), ("/closed", "Closed")]),
                saved(0, &[("/closed", "Closed")]),
            ),
            (
                "later duplicates of a page are dropped",
                saved(
                    0,
                    &[
                        ("/t/1", "Task"),
                        ("/t/1", "Task"),
                        ("/t/1?transcript=1", "Task"),
                    ],
                ),
                saved(0, &[("/t/1", "Task")]),
            ),
            (
                "25 tabs are cut to 20",
                saved(0, &many),
                saved(0, &many[..20]),
            ),
            (
                "the selection follows its tab when earlier tabs are dropped",
                saved(
                    3,
                    &[("/login/oidc", "x"), ("/", "x"), ("/closed", "Closed")],
                ),
                saved(1, &[("/closed", "Closed")]),
            ),
            (
                "a selection past the end clamps to the last tab",
                saved(9, &[("/backlog", "Backlog"), ("/closed", "Closed")]),
                saved(2, &[("/backlog", "Backlog"), ("/closed", "Closed")]),
            ),
            (
                "a selection on a dropped tab falls back to home",
                saved(1, &[("/login/oidc", "x"), ("/closed", "Closed")]),
                saved(0, &[("/closed", "Closed")]),
            ),
            (
                "a selection on a tab cut by the cap falls back to home",
                saved(22, &many[..22]),
                saved(0, &many[..20]),
            ),
            (
                "a 500-character label is cut",
                saved(0, &[("/backlog", long.as_str())]),
                saved(0, &[("/backlog", cut.as_str())]),
            ),
        ];

        for (name, input, expected) in cases {
            assert_eq!(restore(input), expected, "{name}");
        }
    }

    #[test]
    fn keeps_only_canonical_uuid_json_names() {
        let id = Uuid::from_u128(0xabcdef);
        let names: Vec<OsString> = [
            format!("{id}.json"),
            format!("{id}.json.tmp"),
            format!("{id}"),
            "notes.json".to_owned(),
            format!("{}.json", id.to_string().to_uppercase()),
            format!("{}.json", id.simple()),
        ]
        .into_iter()
        .map(OsString::from)
        .collect();

        assert_eq!(tab_file_ids(&names), vec![id]);
    }

    #[test]
    fn a_written_file_is_read_back_and_leaves_no_staging_file() {
        let directory =
            std::env::temp_dir().join(format!("agent-board-client-test-{}", Uuid::new_v4()));
        let path = directory
            .join("tabs")
            .join(format!("{}.json", Uuid::new_v4()));
        let tabs = saved(1, &[("/t/1", "Task")]);

        assert_eq!(load(&path).unwrap(), SavedTabs::default());
        store(&path, &saved(0, &[("/closed", "Closed")])).unwrap();
        store(&path, &tabs).unwrap();

        assert_eq!(load(&path).unwrap(), tabs);
        assert!(!path.with_extension("json.tmp").exists());

        remove(&path).unwrap();
        remove(&path).unwrap();
        assert_eq!(load(&path).unwrap(), SavedTabs::default());
    }
}
