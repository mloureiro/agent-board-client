use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use tauri::{AppHandle, Emitter, EventTarget, Manager};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use uuid::Uuid;

use crate::board_window;
use crate::error::ClientError;
use crate::store::ServerStore;
use crate::tabs::{Effect, TabSession, TabsView};

const TABS_CHANGED: &str = "tabs-changed";

struct Batch {
    server: Uuid,
    effects: Vec<Effect>,
    view: TabsView,
}

pub struct TabSessions {
    sessions: Mutex<HashMap<Uuid, TabSession>>,
    queue: UnboundedSender<Batch>,
}

impl TabSessions {
    fn sessions(&self) -> MutexGuard<'_, HashMap<Uuid, TabSession>> {
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn insert(&self, server: Uuid) {
        self.sessions().insert(server, TabSession::default());
    }

    pub fn remove(&self, server: Uuid) {
        self.sessions().remove(&server);
    }

    pub fn view(&self, server: Uuid) -> Result<TabsView, ClientError> {
        self.sessions()
            .get(&server)
            .map(TabSession::view)
            .ok_or(ClientError::NotFound)
    }
}

pub fn tabbar_label(server: Uuid) -> String {
    format!("tabbar-{server}")
}

pub fn start(app: &AppHandle) -> TabSessions {
    let (queue, mut receiver) = unbounded_channel::<Batch>();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(batch) = receiver.recv().await {
            perform(&app, batch);
        }
    });
    TabSessions {
        sessions: Mutex::new(HashMap::new()),
        queue,
    }
}

/// Mutates the session under the lock and queues the resulting effects while still holding it,
/// so effects reach the webviews in the order the model changed. The queue is drained on the
/// async runtime, never inside a WebKit callback.
pub fn apply(
    app: &AppHandle,
    server: Uuid,
    change: impl FnOnce(&mut TabSession) -> Vec<Effect>,
) -> Result<(), ClientError> {
    let sessions = app.state::<TabSessions>();
    let mut guard = sessions.sessions();
    let session = guard.get_mut(&server).ok_or(ClientError::NotFound)?;
    let effects = change(session);
    if effects.is_empty() {
        return Ok(());
    }

    let view = session.view();
    let queued = sessions.queue.send(Batch {
        server,
        effects,
        view,
    });
    if queued.is_err() {
        eprintln!("server {server}: tab effects could not be queued");
    }
    Ok(())
}

fn perform(app: &AppHandle, batch: Batch) {
    let Batch {
        server,
        effects,
        view,
    } = batch;

    for effect in effects {
        match effect {
            Effect::Load(tab, path) => {
                if let Err(error) = load(app, server, tab, &path) {
                    eprintln!("server {server}: tab {tab} could not be loaded: {error:?}");
                }
            }
            Effect::Show(tab) => {
                if let Some(webview) = app.get_webview(&board_window::tab_label(server, tab)) {
                    if let Err(error) = webview.show().and_then(|()| webview.set_focus()) {
                        eprintln!("server {server}: tab {tab} could not be shown: {error}");
                    }
                }
            }
            Effect::Hide(tab) => {
                if let Some(webview) = app.get_webview(&board_window::tab_label(server, tab)) {
                    if let Err(error) = webview.hide() {
                        eprintln!("server {server}: tab {tab} could not be hidden: {error}");
                    }
                }
            }
            Effect::Destroy(tab) => {
                if let Some(webview) = app.get_webview(&board_window::tab_label(server, tab)) {
                    if let Err(error) = webview.close() {
                        eprintln!("server {server}: tab {tab} could not be closed: {error}");
                    }
                }
            }
            Effect::Changed => {
                let target = EventTarget::webview(tabbar_label(server));
                if let Err(error) = app.emit_to(target, TABS_CHANGED, &view) {
                    eprintln!("server {server}: the tab bar could not be updated: {error}");
                }
            }
            Effect::LimitReached => eprintln!("server {server}: tab limit reached"),
        }
    }
}

fn load(app: &AppHandle, server_id: Uuid, tab: u64, path: &str) -> Result<(), ClientError> {
    let Some(window) = app.get_window(&board_window::window_label(server_id)) else {
        return Ok(());
    };
    let server = app.state::<ServerStore>().find(server_id)?;
    let target =
        crate::sentinel::same_origin_url(&server.url, path).ok_or(ClientError::NotFound)?;
    board_window::add_board_webview(app, &window, server_id, &server.url, tab, target)
}
