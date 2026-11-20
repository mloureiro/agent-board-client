use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::Serialize;
use tauri::{AppHandle, Manager};
use url::Url;

use crate::board_window;
use crate::diagnostics;
use crate::error::ClientError;
use crate::list_window;
use crate::server_url::{check_server_url, is_loopback};
use crate::store::ServerStore;

const SERVERS_CHANGED: &str = "servers-changed";
const OPEN_FAILED: &str = "open-failed";
const ADDED_SERVER_NAME: &str = "Local";

#[derive(Debug, Clone, Serialize)]
pub struct OpenFailure {
    pub name: String,
    pub error: ClientError,
}

#[derive(Default)]
pub struct OpenFailures {
    last: Mutex<Option<OpenFailure>>,
}

impl OpenFailures {
    fn last(&self) -> MutexGuard<'_, Option<OpenFailure>> {
        self.last.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn take(&self) -> Option<OpenFailure> {
        self.last().take()
    }

    fn record(&self, failure: OpenFailure) {
        *self.last() = Some(failure);
    }
}

fn local_board(urls: &[Url]) -> Option<Url> {
    urls.iter()
        .filter(|url| is_loopback(url))
        .find_map(|url| check_server_url(url.as_str()).ok())
}

fn report(app: &AppHandle, name: String, error: ClientError) {
    diagnostics::record(
        app,
        &format!("could not open the board of {name} from an opened link: {error:?}"),
    );
    app.state::<OpenFailures>()
        .record(OpenFailure { name, error });
    if let Err(error) = list_window::show(app) {
        eprintln!("could not show the server list: {error}");
    }
    list_window::announce(app, OPEN_FAILED, ());
}

async fn open(app: &AppHandle, url: Url) {
    let server = match app
        .state::<ServerStore>()
        .find_or_add(ADDED_SERVER_NAME, &url)
    {
        Ok(server) => server,
        Err(error) => return report(app, url.origin().ascii_serialization(), error),
    };
    list_window::announce(app, SERVERS_CHANGED, ());

    match board_window::connect_and_open(app, &server).await {
        Ok(()) => diagnostics::record(
            app,
            &format!("opened server {} from an opened link", server.id),
        ),
        Err(error) => report(app, server.name, error),
    }
}

pub fn handle(app: &AppHandle, urls: &[Url]) {
    let accepted = local_board(urls);
    diagnostics::record(
        app,
        &format!(
            "received {} opened link(s), accepted: {}",
            urls.len(),
            accepted
                .as_ref()
                .map_or("none".to_owned(), |url| url.origin().ascii_serialization())
        ),
    );
    let Some(url) = accepted else {
        if let Err(error) = list_window::show(app) {
            eprintln!("could not show the server list: {error}");
        }
        return;
    };

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        open(&app, url).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(inputs: &[&str]) -> Vec<Url> {
        inputs
            .iter()
            .map(|input| Url::parse(input).unwrap())
            .collect()
    }

    #[test]
    fn a_loopback_board_is_accepted() {
        assert_eq!(
            local_board(&parse(&["http://127.0.0.1:4785/"])),
            Some(Url::parse("http://127.0.0.1:4785/").unwrap())
        );
    }

    #[test]
    fn a_remote_board_is_not_added_from_an_opened_link() {
        assert_eq!(local_board(&parse(&["https://board.example.com/"])), None);
    }

    #[test]
    fn a_loopback_address_with_credentials_is_refused() {
        assert_eq!(
            local_board(&parse(&["http://user:secret@localhost:4785/"])),
            None
        );
    }

    #[test]
    fn the_first_acceptable_link_wins() {
        assert_eq!(
            local_board(&parse(&[
                "https://board.example.com/",
                "http://[::1]:4785/",
                "http://localhost:4786/",
            ])),
            Some(Url::parse("http://[::1]:4785/").unwrap())
        );
    }
}
