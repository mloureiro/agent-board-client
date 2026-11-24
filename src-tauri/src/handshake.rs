use std::future::Future;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::board_api::{
    self, classify, Answer, BoardFailure, BoardHttp, CallKind, FailureClass, BODY_CAP,
};
use crate::store::Server;
use crate::ui_api::UiApi;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BoardState {
    Ready,
    SignedOut,
    BoardOnly,
    BoardTooOld,
    UpgradeRequired,
    Offline,
    Server,
}

impl BoardState {
    pub fn board_only(self) -> bool {
        !matches!(self, Self::Ready | Self::SignedOut)
    }

    pub fn retry_after_recovery(self) -> bool {
        matches!(self, Self::Offline | Self::Server)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handshake {
    pub state: BoardState,
    pub events: bool,
}

impl Handshake {
    fn without_events(state: BoardState) -> Self {
        Self {
            state,
            events: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct VersionAnswer {
    pub api: u32,
    pub features: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LoginConfig {
    login: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Probe<T> {
    Answer(T),
    Missing,
    Failed(BoardFailure),
}

fn unreachable_or_server(failure: &BoardFailure) -> BoardState {
    match failure.class {
        FailureClass::Offline => BoardState::Offline,
        _ => BoardState::Server,
    }
}

pub fn decide(
    version: Probe<VersionAnswer>,
    login: Option<Probe<bool>>,
    has_token: bool,
) -> Handshake {
    let ui_api = UiApi::bundled();
    let version = match version {
        Probe::Answer(version) => version,
        Probe::Missing => return Handshake::without_events(BoardState::BoardOnly),
        Probe::Failed(failure) if failure.class == FailureClass::UpgradeRequired => {
            return Handshake::without_events(BoardState::UpgradeRequired)
        }
        Probe::Failed(failure) => {
            return Handshake::without_events(unreachable_or_server(&failure))
        }
    };
    if version.api < ui_api.min_ui_server {
        return Handshake::without_events(BoardState::BoardTooOld);
    }

    let login = match login {
        Some(Probe::Answer(login)) => login,
        Some(Probe::Failed(failure)) => {
            return Handshake::without_events(unreachable_or_server(&failure))
        }
        Some(Probe::Missing) | None => return Handshake::without_events(BoardState::Server),
    };
    let offers = |feature: &str| version.features.iter().any(|offered| offered == feature);
    let events = offers(&ui_api.events_feature);

    if !login {
        return Handshake {
            state: BoardState::Ready,
            events,
        };
    }
    if !offers(&ui_api.handoff_feature) {
        return Handshake::without_events(BoardState::BoardOnly);
    }
    if has_token {
        return Handshake {
            state: BoardState::Ready,
            events,
        };
    }
    Handshake::without_events(BoardState::SignedOut)
}

const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
const RETRY_DELAYS: [Duration; 2] = [Duration::from_secs(1), Duration::from_secs(2)];

pub fn into_probe<T: DeserializeOwned>(answer: Result<Answer, BoardFailure>) -> Probe<T> {
    let answer = match answer {
        Ok(Answer::NotTrpc { status }) if status < 500 => return Probe::Missing,
        Ok(Answer::Error(error)) if error.code == "NOT_FOUND" => return Probe::Missing,
        Ok(answer) => answer,
        Err(failure) => return Probe::Failed(failure),
    };
    match classify(answer).map(serde_json::from_value::<T>) {
        Ok(Ok(value)) => Probe::Answer(value),
        Ok(Err(_)) => Probe::Failed(BoardFailure::server(
            "The board's answer was not understood.",
        )),
        Err(failure) => Probe::Failed(failure),
    }
}

async fn within(
    limit: Duration,
    answer: impl Future<Output = Result<Answer, BoardFailure>>,
) -> Result<Answer, BoardFailure> {
    tokio::time::timeout(limit, answer)
        .await
        .unwrap_or_else(|_| Err(BoardFailure::offline()))
}

async fn probe<T: DeserializeOwned>(app: &AppHandle, server: &Server, path: &str) -> Probe<T> {
    let http = app.state::<BoardHttp>();
    let answer = within(
        PROBE_TIMEOUT,
        board_api::exchange(
            &http.calls,
            &server.url,
            CallKind::Query,
            path,
            None,
            None,
            BODY_CAP,
        ),
    )
    .await;
    into_probe(answer)
}

fn offers_hand_off(version: &VersionAnswer) -> bool {
    version
        .features
        .iter()
        .any(|feature| *feature == UiApi::bundled().handoff_feature)
}

// The probes carry no session: both procedures are public, and a refused probe must never
// delete a stored token.
pub async fn run(app: &AppHandle, server: &Server) -> Handshake {
    let version = probe::<VersionAnswer>(app, server, "ui.version").await;
    let decided_by_version = !matches!(
        &version,
        Probe::Answer(answer) if answer.api >= UiApi::bundled().min_ui_server
    );
    if decided_by_version {
        return decide(version, None, false);
    }

    let login = probe::<LoginConfig>(app, server, "auth.config").await;
    let login = match login {
        Probe::Answer(config) => Probe::Answer(config.login),
        Probe::Missing => Probe::Missing,
        Probe::Failed(failure) => Probe::Failed(failure),
    };

    let needs_token = matches!(
        (&version, &login),
        (Probe::Answer(answer), Probe::Answer(true)) if offers_hand_off(answer)
    );
    let has_token = if needs_token {
        match board_api::read_session(app, server.id).await {
            Ok(session) => session.is_some(),
            Err(_) => return Handshake::without_events(BoardState::Server),
        }
    } else {
        false
    };
    decide(version, Some(login), has_token)
}

async fn retrying<F, Fut>(delays: &[Duration], mut attempt: F) -> Handshake
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Handshake>,
{
    let mut handshake = attempt().await;
    for delay in delays {
        if !handshake.state.retry_after_recovery() {
            break;
        }
        tokio::time::sleep(*delay).await;
        handshake = attempt().await;
    }
    handshake
}

pub async fn rerun(app: &AppHandle, server: &Server) -> Handshake {
    retrying(&RETRY_DELAYS, || run(app, server)).await
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::*;
    use crate::board_api::TrpcError;
    use serde_json::json;

    fn version(api: u32, features: &[&str]) -> Probe<VersionAnswer> {
        Probe::Answer(VersionAnswer {
            api,
            features: features
                .iter()
                .map(|feature| (*feature).to_owned())
                .collect(),
        })
    }

    fn failed(class: FailureClass) -> BoardFailure {
        BoardFailure {
            class,
            message: None,
        }
    }

    fn ready(events: bool) -> Handshake {
        Handshake {
            state: BoardState::Ready,
            events,
        }
    }

    #[test]
    fn decides_the_state_of_a_board() {
        use BoardState::*;
        type Row = (
            &'static str,
            Probe<VersionAnswer>,
            Option<Probe<bool>>,
            bool,
            Handshake,
        );
        let rows: Vec<Row> = vec![
            (
                "version offline",
                Probe::Failed(failed(FailureClass::Offline)),
                None,
                false,
                Handshake::without_events(Offline),
            ),
            (
                "version refused for an old client",
                Probe::Failed(failed(FailureClass::UpgradeRequired)),
                None,
                false,
                Handshake::without_events(UpgradeRequired),
            ),
            (
                "version missing",
                Probe::Missing,
                None,
                false,
                Handshake::without_events(BoardOnly),
            ),
            (
                "version failed otherwise",
                Probe::Failed(failed(FailureClass::Server)),
                None,
                false,
                Handshake::without_events(Server),
            ),
            (
                "version signed out",
                Probe::Failed(failed(FailureClass::SignedOut)),
                None,
                false,
                Handshake::without_events(Server),
            ),
            (
                "api below the minimum",
                version(0, &["events"]),
                Some(Probe::Answer(false)),
                false,
                Handshake::without_events(BoardTooOld),
            ),
            (
                "login probe offline",
                version(1, &["events"]),
                Some(Probe::Failed(failed(FailureClass::Offline))),
                false,
                Handshake::without_events(Offline),
            ),
            (
                "login probe failed",
                version(1, &["events"]),
                Some(Probe::Failed(failed(FailureClass::Server))),
                false,
                Handshake::without_events(Server),
            ),
            (
                "login probe missing",
                version(1, &["events"]),
                Some(Probe::Missing),
                false,
                Handshake::without_events(Server),
            ),
            (
                "no login, events offered",
                version(1, &["events"]),
                Some(Probe::Answer(false)),
                false,
                ready(true),
            ),
            (
                "no login, no events",
                version(2, &[]),
                Some(Probe::Answer(false)),
                false,
                ready(false),
            ),
            (
                "login without a hand-off",
                version(1, &["events"]),
                Some(Probe::Answer(true)),
                true,
                Handshake::without_events(BoardOnly),
            ),
            (
                "login with a hand-off and no token",
                version(1, &["events", "handoff"]),
                Some(Probe::Answer(true)),
                false,
                Handshake::without_events(SignedOut),
            ),
            (
                "login with a hand-off and a token",
                version(1, &["events", "handoff"]),
                Some(Probe::Answer(true)),
                true,
                ready(true),
            ),
            (
                "login with a hand-off, a token, no events",
                version(1, &["handoff"]),
                Some(Probe::Answer(true)),
                true,
                ready(false),
            ),
        ];

        for (name, version, login, has_token, expected) in rows {
            assert_eq!(decide(version, login, has_token), expected, "{name}");
        }
    }

    #[test]
    fn says_which_states_keep_the_board_pages() {
        use BoardState::*;
        let rows = [
            (Ready, false, false),
            (SignedOut, false, false),
            (BoardOnly, true, false),
            (BoardTooOld, true, false),
            (UpgradeRequired, true, false),
            (Offline, true, true),
            (Server, true, true),
        ];

        for (state, board_only, retry) in rows {
            assert_eq!(state.board_only(), board_only, "{state:?}");
            assert_eq!(state.retry_after_recovery(), retry, "{state:?}");
        }
    }

    #[test]
    fn turns_an_answer_into_a_probe() {
        let error = |code: &str| {
            Ok(Answer::Error(TrpcError {
                code: code.to_owned(),
                message: "text".to_owned(),
                upgrade: false,
            }))
        };
        let good = json!({ "api": 1, "minClient": 1, "features": ["events"] });

        assert_eq!(
            into_probe::<VersionAnswer>(Ok(Answer::Data(good))),
            version(1, &["events"])
        );
        for status in [404, 499] {
            assert_eq!(
                into_probe::<VersionAnswer>(Ok(Answer::NotTrpc { status })),
                Probe::Missing,
                "{status}"
            );
        }
        for status in [500, 502, 503] {
            assert_eq!(
                into_probe::<VersionAnswer>(Ok(Answer::NotTrpc { status })),
                Probe::Failed(BoardFailure::server(&format!(
                    "The board gave an unexpected answer (HTTP {status})."
                ))),
                "{status}"
            );
        }
        assert_eq!(
            into_probe::<VersionAnswer>(error("NOT_FOUND")),
            Probe::Missing
        );
        assert_eq!(
            into_probe::<VersionAnswer>(error("INTERNAL_SERVER_ERROR")),
            Probe::Failed(BoardFailure::server("text"))
        );
        assert_eq!(
            into_probe::<VersionAnswer>(Err(BoardFailure::offline())),
            Probe::Failed(BoardFailure::offline())
        );
        assert_eq!(
            into_probe::<VersionAnswer>(Ok(Answer::Data(json!({ "api": "one" })))),
            Probe::Failed(BoardFailure::server(
                "The board's answer was not understood."
            ))
        );
    }

    #[test]
    fn gives_up_on_a_probe_that_does_not_answer() {
        let pending = tauri::async_runtime::block_on(within(
            Duration::from_millis(20),
            std::future::pending(),
        ));
        let ready = tauri::async_runtime::block_on(within(
            Duration::from_millis(20),
            std::future::ready(Ok(Answer::Data(json!(1)))),
        ));

        assert_eq!(pending, Err(BoardFailure::offline()));
        assert_eq!(ready, Ok(Answer::Data(json!(1))));
    }

    #[test]
    fn retries_only_a_transient_failure_and_only_so_often() {
        use BoardState::*;
        let rows: [(&[BoardState], BoardState, usize); 6] = [
            (&[Ready], Ready, 1),
            (&[BoardOnly], BoardOnly, 1),
            (&[Offline, Ready], Ready, 2),
            (&[Server, Offline, Ready], Ready, 3),
            (&[Server, Server, Server, Ready], Server, 3),
            (&[Offline, SignedOut], SignedOut, 2),
        ];

        for (script, expected, attempts) in rows {
            let mut script: VecDeque<BoardState> = script.iter().copied().collect();
            let mut calls = 0;
            let result = tauri::async_runtime::block_on(retrying(&[Duration::ZERO; 2], || {
                calls += 1;
                let state = script.pop_front().unwrap();
                async move { Handshake::without_events(state) }
            }));

            assert_eq!(result.state, expected, "{expected:?}");
            assert_eq!(calls, attempts, "{expected:?}");
        }
    }
}
