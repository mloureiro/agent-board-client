use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, EventTarget, Manager};
use uuid::Uuid;

use crate::board_api::{self, BoardHttp};
use crate::commands::server_of_native;
use crate::diagnostics;
use crate::store::ServerStore;

const BOARD_CHANGED: &str = "board-changed";
const BOARD_EVENTS: &str = "board-events";
pub const SIGNED_OUT: &str = "signed-out";
const MAX_LINE_BYTES: usize = 4 * 1024;
const FIRST_BACKOFF: Duration = Duration::from_secs(1);
const LONGEST_BACKOFF: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SseItem {
    Connected,
    Changed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SseTooLong;

#[derive(Debug, Default)]
pub struct SseParser {
    buffer: Vec<u8>,
    event: Option<String>,
}

impl SseParser {
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<SseItem>, SseTooLong> {
        let mut items = Vec::new();
        for &byte in chunk {
            if byte == b'\n' {
                let line = std::mem::take(&mut self.buffer);
                items.extend(self.line(&line));
            } else if self.buffer.len() + 1 >= MAX_LINE_BYTES {
                return Err(SseTooLong);
            } else {
                self.buffer.push(byte);
            }
        }
        Ok(items)
    }

    fn line(&mut self, bytes: &[u8]) -> Option<SseItem> {
        let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
        let line = String::from_utf8_lossy(bytes);
        if line.is_empty() {
            let event = self.event.take();
            return (event.as_deref() == Some("changed")).then_some(SseItem::Changed);
        }
        if line.starts_with(':') {
            return (line == ": connected").then_some(SseItem::Connected);
        }
        if let Some(name) = line.strip_prefix("event:") {
            self.event = Some(name.strip_prefix(' ').unwrap_or(name).to_owned());
        }
        None
    }
}

struct Relay {
    generation: u64,
    up: bool,
    task: JoinHandle<()>,
}

#[derive(Default)]
pub struct Relays {
    running: Mutex<HashMap<Uuid, Relay>>,
    generations: AtomicU64,
}

impl Relays {
    fn running(&self) -> MutexGuard<'_, HashMap<Uuid, Relay>> {
        self.running.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn is_up(&self, server: Uuid) -> bool {
        self.running().get(&server).is_some_and(|relay| relay.up)
    }

    fn set_up(&self, server: Uuid, generation: u64, up: bool) -> bool {
        let mut running = self.running();
        match running.get_mut(&server) {
            Some(relay) if relay.generation == generation && relay.up != up => {
                relay.up = up;
                true
            }
            _ => false,
        }
    }

    fn finish(&self, server: Uuid, generation: u64) {
        let mut running = self.running();
        if running
            .get(&server)
            .is_some_and(|relay| relay.generation == generation)
        {
            running.remove(&server);
        }
    }
}

pub fn sync(app: &AppHandle, server: Uuid, wanted: bool) {
    if !wanted {
        stop(app, server);
        return;
    }

    let relays = app.state::<Relays>();
    let mut running = relays.running();
    if running.contains_key(&server) {
        return;
    }
    let generation = relays.generations.fetch_add(1, Ordering::Relaxed);
    let task = tauri::async_runtime::spawn(relay(app.clone(), server, generation));
    running.insert(
        server,
        Relay {
            generation,
            up: false,
            task,
        },
    );
}

pub fn stop(app: &AppHandle, server: Uuid) {
    let removed = app.state::<Relays>().running().remove(&server);
    if let Some(relay) = removed {
        relay.task.abort();
    }
}

#[derive(Serialize, Clone, Copy)]
struct EventsUp {
    up: bool,
}

pub fn emit_to_native<S: Serialize + Clone>(app: &AppHandle, server: Uuid, name: &str, payload: S) {
    for label in app.webviews().into_keys() {
        if server_of_native(&label) != Some(server) {
            continue;
        }
        if let Err(error) = app.emit_to(EventTarget::webview(label), name, payload.clone()) {
            diagnostics::record(
                app,
                &format!("server {server}: {name} could not be delivered: {error}"),
            );
        }
    }
}

fn mark(app: &AppHandle, server: Uuid, generation: u64, up: bool) {
    if app.state::<Relays>().set_up(server, generation, up) {
        emit_to_native(app, server, BOARD_EVENTS, EventsUp { up });
    }
}

enum Ended {
    SignedOut,
    ServerRemoved,
    Retry,
}

async fn relay(app: AppHandle, server: Uuid, generation: u64) {
    let mut backoff = FIRST_BACKOFF;
    loop {
        match stream_once(&app, server, generation, &mut backoff).await {
            Ended::SignedOut => {
                mark(&app, server, generation, false);
                emit_to_native(&app, server, SIGNED_OUT, ());
                break;
            }
            Ended::ServerRemoved => break,
            Ended::Retry => {
                mark(&app, server, generation, false);
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(LONGEST_BACKOFF);
            }
        }
    }
    app.state::<Relays>().finish(server, generation);
}

async fn stream_once(
    app: &AppHandle,
    server_id: Uuid,
    generation: u64,
    backoff: &mut Duration,
) -> Ended {
    let Ok(server) = app.state::<ServerStore>().find(server_id) else {
        return Ended::ServerRemoved;
    };
    let Ok(session) = board_api::read_session(app, server_id).await else {
        return Ended::Retry;
    };
    let Ok(url) = server.url.join("api/ui/events") else {
        return Ended::Retry;
    };
    let http = app.state::<BoardHttp>();
    let Ok(request) = board_api::with_board_headers(http.streams.get(url), session.as_ref()) else {
        return Ended::Retry;
    };
    let mut response = match request.send().await {
        Ok(response) => response,
        Err(error) => {
            diagnostics::record(
                app,
                &format!(
                    "server {server_id}: the event stream could not be reached: {}",
                    error.without_url()
                ),
            );
            return Ended::Retry;
        }
    };

    if response.status().as_u16() == 401 {
        if let Some(sent) = session {
            board_api::delete_session_if(app, server_id, sent.token).await;
        }
        return Ended::SignedOut;
    }
    if response.status().as_u16() != 200 {
        diagnostics::record(
            app,
            &format!(
                "server {server_id}: the event stream answered HTTP {}",
                response.status().as_u16()
            ),
        );
        return Ended::Retry;
    }

    let mut parser = SseParser::default();
    loop {
        let chunk = match response.chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(error) => {
                diagnostics::record(
                    app,
                    &format!(
                        "server {server_id}: the event stream broke: {}",
                        error.without_url()
                    ),
                );
                break;
            }
        };
        let Ok(items) = parser.push(&chunk) else {
            break;
        };
        for item in items {
            match item {
                SseItem::Connected => {
                    *backoff = FIRST_BACKOFF;
                    mark(app, server_id, generation, true);
                }
                SseItem::Changed => emit_to_native(app, server_id, BOARD_CHANGED, ()),
            }
        }
    }
    Ended::Retry
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(chunks: &[&[u8]]) -> Result<Vec<SseItem>, SseTooLong> {
        let mut parser = SseParser::default();
        let mut items = Vec::new();
        for chunk in chunks {
            items.extend(parser.push(chunk)?);
        }
        Ok(items)
    }

    fn at_every_split(input: &[u8]) -> Vec<Result<Vec<SseItem>, SseTooLong>> {
        (0..=input.len())
            .map(|at| feed(&[&input[..at], &input[at..]]))
            .collect()
    }

    #[test]
    fn reads_the_events_the_board_sends() {
        let cases: [(&str, &[u8], Vec<SseItem>); 8] = [
            ("connected", b": connected\n\n", vec![SseItem::Connected]),
            (
                "changed",
                b"event: changed\ndata:\n\n",
                vec![SseItem::Changed],
            ),
            ("ping", b": ping\n\n", vec![]),
            (
                "crlf",
                b": connected\r\n\r\nevent: changed\r\ndata:\r\n\r\n",
                vec![SseItem::Connected, SseItem::Changed],
            ),
            ("unknown event", b"event: other\ndata:\n\n", vec![]),
            (
                "no space after the colon",
                b"event:changed\n\n",
                vec![SseItem::Changed],
            ),
            ("no blank line yet", b"event: changed\ndata:\n", vec![]),
            (
                "two events",
                b"event: changed\n\nevent: changed\n\n",
                vec![SseItem::Changed, SseItem::Changed],
            ),
        ];

        for (name, input, expected) in cases {
            assert_eq!(feed(&[input]), Ok(expected.clone()), "{name}");
            for (at, split) in at_every_split(input).into_iter().enumerate() {
                assert_eq!(split, Ok(expected.clone()), "{name} split at {at}");
            }
        }
    }

    #[test]
    fn forgets_an_event_name_once_it_is_dispatched() {
        assert_eq!(feed(&[b"event: changed\n\n\n"]), Ok(vec![SseItem::Changed]));
    }

    #[test]
    fn refuses_a_line_without_an_end() {
        let line = vec![b'x'; MAX_LINE_BYTES];

        assert_eq!(feed(&[&line]), Err(SseTooLong));
        assert_eq!(feed(&[&line[..MAX_LINE_BYTES - 2], b"\n"]), Ok(vec![]));
    }

    #[test]
    fn keeps_a_multibyte_character_split_across_chunks() {
        let input = "event: changed\ndata: é\n\n".as_bytes();
        let split = input.iter().position(|byte| *byte == 0xC3).unwrap() + 1;

        assert_eq!(
            feed(&[&input[..split], &input[split..]]),
            Ok(vec![SseItem::Changed])
        );
    }
}
