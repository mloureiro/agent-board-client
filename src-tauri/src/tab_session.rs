use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use tauri::{AppHandle, Emitter, EventTarget, Manager, Webview};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use uuid::Uuid;

use crate::board_events;
use crate::board_window;
use crate::error::ClientError;
use crate::handshake;
use crate::health::Banner;
use crate::store::ServerStore;
use crate::tab_kind::TabKind;
use crate::tab_state::{self, SavedTabs};
use crate::tabs::{Effect, TabSession, TabsView};
use crate::zoom::Zoom;

const TABS_CHANGED: &str = "tabs-changed";

struct Batch {
    server: Uuid,
    effects: Vec<Effect>,
    view: TabsView,
    saved: SavedTabs,
    kinds: Vec<(u64, TabKind)>,
    relay: Option<bool>,
}

pub struct TabSessions {
    sessions: Mutex<HashMap<Uuid, TabSession>>,
    queue: UnboundedSender<Batch>,
}

impl TabSessions {
    fn sessions(&self) -> MutexGuard<'_, HashMap<Uuid, TabSession>> {
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn insert(&self, server: Uuid, session: TabSession) {
        self.sessions().insert(server, session);
    }

    /// Holds the lock across the write so that removing a session, then deleting its file,
    /// cannot be followed by a late write recreating the file.
    fn persist(&self, server: Uuid, app: &AppHandle, saved: &SavedTabs) {
        let sessions = self.sessions();
        if !sessions.contains_key(&server) {
            return;
        }
        if let Err(error) = tab_state::write(app, server, saved) {
            eprintln!("server {server}: saved tabs could not be written: {error:?}");
        }
    }

    pub fn remove(&self, server: Uuid) {
        self.sessions().remove(&server);
    }

    pub fn zoom(&self, server: Uuid) -> Option<Zoom> {
        self.sessions().get(&server).map(TabSession::zoom)
    }

    pub fn home_kind(&self, server: Uuid) -> Option<TabKind> {
        self.sessions().get(&server).map(TabSession::home_kind)
    }

    pub fn banner(&self, server: Uuid) -> Option<Banner> {
        self.sessions().get(&server).and_then(TabSession::banner)
    }

    pub fn relay_wanted(&self, server: Uuid) -> bool {
        self.sessions()
            .get(&server)
            .is_some_and(TabSession::relay_wanted)
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
    let relay_before = session.relay_wanted();
    let effects = change(session);
    let relay_after = session.relay_wanted();
    if effects.is_empty() && relay_before == relay_after {
        return Ok(());
    }

    let view = session.view();
    let saved = session.saved();
    let kinds = session.tab_kinds();
    let queued = sessions.queue.send(Batch {
        server,
        effects,
        view,
        saved,
        kinds,
        relay: (relay_before != relay_after).then_some(relay_after),
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
        saved,
        kinds,
        relay,
    } = batch;

    for effect in effects {
        match effect {
            Effect::Load(tab, path, kind) => {
                if let Err(error) = load(app, server, tab, &path, kind) {
                    eprintln!("server {server}: tab {tab} could not be loaded: {error:?}");
                }
            }
            Effect::Rebuild(tab, path, kind) => {
                rebuild(app, server, tab, &path, kind, view.selected);
            }
            Effect::Show(tab) => {
                for webview in webviews_of(app, server, tab, &kinds) {
                    if let Err(error) = webview.show().and_then(|()| webview.set_focus()) {
                        eprintln!("server {server}: tab {tab} could not be shown: {error}");
                    }
                }
            }
            Effect::Hide(tab) => {
                for webview in webviews_of(app, server, tab, &kinds) {
                    if let Err(error) = webview.hide() {
                        eprintln!("server {server}: tab {tab} could not be hidden: {error}");
                    }
                }
            }
            Effect::Destroy(tab) => {
                for webview in webviews_of(app, server, tab, &kinds) {
                    if let Err(error) = webview.close() {
                        eprintln!("server {server}: tab {tab} could not be closed: {error}");
                    }
                }
            }
            Effect::Zoom(zoom) => board_window::set_zoom(app, server, zoom),
            Effect::Changed => {
                let target = EventTarget::webview(tabbar_label(server));
                if let Err(error) = app.emit_to(target, TABS_CHANGED, &view) {
                    eprintln!("server {server}: the tab bar could not be updated: {error}");
                }
                app.state::<TabSessions>().persist(server, app, &saved);
            }
            Effect::Banner => {
                board_window::relayout(app, server);
                let target = EventTarget::webview(tabbar_label(server));
                if let Err(error) = app.emit_to(target, TABS_CHANGED, &view) {
                    eprintln!("server {server}: the tab bar could not be updated: {error}");
                }
            }
            Effect::Handshake => rehandshake(app, server),
            Effect::LimitReached => eprintln!("server {server}: tab limit reached"),
        }
    }

    if let Some(wanted) = relay {
        board_events::sync(app, server, wanted);
    }
}

fn rehandshake(app: &AppHandle, id: Uuid) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Ok(server) = app.state::<ServerStore>().find(id) else {
            return;
        };
        let handshake = handshake::run(&app, &server).await;
        let _ = apply(&app, id, |session| session.set_handshake(&handshake));
    });
}

fn webviews_of(app: &AppHandle, server: Uuid, tab: u64, kinds: &[(u64, TabKind)]) -> Vec<Webview> {
    let labels = match kinds.iter().find(|(id, _)| *id == tab) {
        Some((_, kind)) => vec![board_window::tab_label(server, tab, *kind)],
        None => vec![
            board_window::tab_label(server, tab, TabKind::Board),
            board_window::tab_label(server, tab, TabKind::Native),
        ],
    };
    labels
        .iter()
        .filter_map(|label| app.get_webview(label))
        .collect()
}

fn load(
    app: &AppHandle,
    server_id: Uuid,
    tab: u64,
    path: &str,
    kind: TabKind,
) -> Result<(), ClientError> {
    let Some(window) = app.get_window(&board_window::window_label(server_id)) else {
        return Ok(());
    };
    let server = app.state::<ServerStore>().find(server_id)?;
    match kind {
        TabKind::Board => {
            let target =
                crate::sentinel::same_origin_url(&server.url, path).ok_or(ClientError::NotFound)?;
            board_window::add_board_webview(app, &window, server_id, &server.url, tab, target)
        }
        TabKind::Native => board_window::add_native_webview(app, &window, &server, tab, path),
    }
}

fn rebuild(app: &AppHandle, server: Uuid, tab: u64, path: &str, kind: TabKind, selected: u64) {
    let replaced = match kind {
        TabKind::Board => TabKind::Native,
        TabKind::Native => TabKind::Board,
    };
    if let Some(webview) = app.get_webview(&board_window::tab_label(server, tab, replaced)) {
        if let Err(error) = webview.close() {
            eprintln!("server {server}: tab {tab} could not be closed: {error}");
        }
    }

    if let Err(error) = load(app, server, tab, path, kind) {
        eprintln!("server {server}: tab {tab} could not be rebuilt: {error:?}");
        return;
    }

    let Some(webview) = app.get_webview(&board_window::tab_label(server, tab, kind)) else {
        return;
    };
    let outcome = if selected == tab {
        webview.set_focus()
    } else {
        webview.hide()
    };
    if let Err(error) = outcome {
        eprintln!("server {server}: tab {tab} could not be settled after a rebuild: {error}");
    }
}
