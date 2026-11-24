use std::collections::HashMap;
use std::fmt;
use std::sync::{Mutex, MutexGuard, PoisonError};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::board_api::{self, classify, Answer, BoardFailure, BoardHttp, CallKind, Reply};
use crate::board_events::{emit_to_native, SIGNED_OUT};
use crate::board_window::open_in_system_browser;
use crate::diagnostics;
use crate::link_route::server_page;
use crate::session_store::{self, CookieName, StoredSession};
use crate::store::Server;

pub const SIGNED_IN: &str = "signed-in";
const SIGN_IN_BODY_CAP: usize = 64 * 1024;
const VERIFIER_BYTES: usize = 32;
const CODE_ALPHABET: &str = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

#[derive(Debug, Serialize)]
pub struct SignInStarted {
    pub code: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PollStatus {
    Pending,
    SignedIn,
    NoFlow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SignOutStatus {
    SignedOut,
}

#[derive(Clone)]
pub struct Verifier(String);

impl fmt::Debug for Verifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Verifier(<redacted>)")
    }
}

impl Verifier {
    pub fn generate() -> Result<Self, getrandom::Error> {
        let mut bytes = [0_u8; VERIFIER_BYTES];
        getrandom::fill(&mut bytes)?;
        Ok(Self(URL_SAFE_NO_PAD.encode(bytes)))
    }

    #[cfg(test)]
    pub fn from_text(text: &str) -> Option<Self> {
        let shaped = (32..=128).contains(&text.len())
            && text.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'~' | b'-')
            });
        shaped.then(|| Self(text.to_owned()))
    }

    pub fn challenge(&self) -> String {
        URL_SAFE_NO_PAD.encode(Sha256::digest(self.0.as_bytes()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub fn is_flow_id(text: &str) -> bool {
    (16..=128).contains(&text.len())
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

pub fn is_code(text: &str) -> bool {
    let bytes = text.as_bytes();
    let symbol = |byte: &u8| CODE_ALPHABET.as_bytes().contains(byte);
    bytes.len() == 9
        && bytes[4] == b'-'
        && bytes[..4].iter().all(symbol)
        && bytes[5..].iter().all(symbol)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CookieRefusal {
    None,
    Several,
    NotText,
    Name,
    Token,
    Domain,
    Clearing,
}

pub fn accept_session(set_cookies: &[&[u8]]) -> Result<StoredSession, CookieRefusal> {
    let value = match set_cookies {
        [] => return Err(CookieRefusal::None),
        [one] => one,
        _ => return Err(CookieRefusal::Several),
    };
    let text = std::str::from_utf8(value).map_err(|_| CookieRefusal::NotText)?;
    let mut parts = text.split(';');
    let pair = parts.next().unwrap_or_default();
    let (name, token) = pair.split_once('=').ok_or(CookieRefusal::Name)?;
    let name = match name {
        "__Host-board-session" => CookieName::Host,
        "board-session" => CookieName::Plain,
        _ => return Err(CookieRefusal::Name),
    };
    session_store::check_token(token).map_err(|_| CookieRefusal::Token)?;

    for attribute in parts {
        let (key, value) = attribute.split_once('=').unwrap_or((attribute, ""));
        let key = key.trim();
        if key.eq_ignore_ascii_case("domain") {
            return Err(CookieRefusal::Domain);
        }
        let ends = value
            .trim()
            .parse::<i64>()
            .is_ok_and(|seconds| seconds <= 0);
        if key.eq_ignore_ascii_case("max-age") && ends {
            return Err(CookieRefusal::Clearing);
        }
    }
    Ok(StoredSession {
        name,
        token: token.to_owned(),
    })
}

#[derive(Clone)]
pub struct PendingFlow {
    pub flow: String,
    pub code: String,
    pub verifier: Verifier,
}

impl fmt::Debug for PendingFlow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PendingFlow")
            .field("flow", &self.flow)
            .field("code", &self.code)
            .field("verifier", &self.verifier)
            .finish()
    }
}

#[derive(Default)]
pub struct SignInFlows(Mutex<HashMap<Uuid, PendingFlow>>);

impl SignInFlows {
    fn flows(&self) -> MutexGuard<'_, HashMap<Uuid, PendingFlow>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn replace(&self, server: Uuid, flow: PendingFlow) {
        self.flows().insert(server, flow);
    }

    // A second tab's start replaces the flow, so a stale code must stop matching and its tab must
    // fall back to idle instead of keeping a code the browser page no longer shows.
    pub fn current(&self, server: Uuid, code: &str) -> Option<PendingFlow> {
        self.flows()
            .get(&server)
            .filter(|pending| pending.code == code)
            .cloned()
    }

    pub fn remove_if(&self, server: Uuid, flow: &str) {
        let mut flows = self.flows();
        if flows
            .get(&server)
            .is_some_and(|pending| pending.flow == flow)
        {
            flows.remove(&server);
        }
    }

    pub fn cancel(&self, server: Uuid, code: &str) {
        let mut flows = self.flows();
        if flows
            .get(&server)
            .is_some_and(|pending| pending.code == code)
        {
            flows.remove(&server);
        }
    }

    pub fn forget(&self, server: Uuid) {
        self.flows().remove(&server);
    }
}

// A refusal here must never become signed-out: that class deletes the stored session, and these
// procedures answer UNAUTHORIZED for an expired sign-in, not for an expired session.
pub fn classify_sign_in(answer: Answer) -> Result<Value, BoardFailure> {
    match answer {
        Answer::Error(error) if error.code == "UNAUTHORIZED" => {
            Err(BoardFailure::server(&error.message))
        }
        other => classify(other),
    }
}

fn unexpected() -> BoardFailure {
    BoardFailure::server("The board gave an unexpected answer.")
}

fn text_field(data: &Value, name: &str) -> Option<String> {
    data.get(name).and_then(Value::as_str).map(str::to_owned)
}

pub async fn start(app: &AppHandle, server: &Server) -> Result<SignInStarted, BoardFailure> {
    let verifier = Verifier::generate()
        .map_err(|_| BoardFailure::server("A sign-in could not be prepared."))?;
    let input = json!({ "challenge": verifier.challenge() });
    let data = {
        let http = app.state::<BoardHttp>();
        let reply = board_api::exchange_reply(
            &http.calls,
            &server.url,
            CallKind::Mutation,
            "auth.handoff.start",
            Some(&input),
            None,
            SIGN_IN_BODY_CAP,
        )
        .await?;
        classify_sign_in(reply.answer)?
    };

    let flow = text_field(&data, "flow").filter(|flow| is_flow_id(flow));
    let code = text_field(&data, "code").filter(|code| is_code(code));
    let (Some(flow), Some(code)) = (flow, code) else {
        return Err(unexpected());
    };
    let url = server_page(&server.url, &format!("/handoff/{flow}")).ok_or_else(unexpected)?;

    app.state::<SignInFlows>().replace(
        server.id,
        PendingFlow {
            flow,
            code: code.clone(),
            verifier,
        },
    );
    open_in_system_browser(server.id, &url);
    Ok(SignInStarted { code })
}

pub async fn poll(
    app: &AppHandle,
    server: &Server,
    code: &str,
) -> Result<PollStatus, BoardFailure> {
    let Some(pending) = app.state::<SignInFlows>().current(server.id, code) else {
        return Ok(PollStatus::NoFlow);
    };
    let input = json!({ "flow": pending.flow, "verifier": pending.verifier.as_str() });
    let reply = {
        let http = app.state::<BoardHttp>();
        board_api::exchange_reply(
            &http.calls,
            &server.url,
            CallKind::Mutation,
            "auth.handoff.finish",
            Some(&input),
            None,
            SIGN_IN_BODY_CAP,
        )
        .await?
    };
    let Reply { answer, headers } = reply;

    let flows = app.state::<SignInFlows>();
    let data = match answer {
        Answer::NotTrpc { .. } => return classify(answer).map(|_| PollStatus::Pending),
        answer => classify_sign_in(answer).inspect_err(|_| {
            flows.remove_if(server.id, &pending.flow);
        })?,
    };

    match data.get("status").and_then(Value::as_str) {
        Some("pending") => Ok(PollStatus::Pending),
        Some("signed-in") => {
            flows.remove_if(server.id, &pending.flow);
            let cookies: Vec<&[u8]> = headers
                .get_all(reqwest::header::SET_COOKIE)
                .iter()
                .map(|value| value.as_bytes())
                .collect();
            keep_session(app, server.id, accept_session(&cookies)).await?;
            emit_to_native(app, server.id, SIGNED_IN, ());
            Ok(PollStatus::SignedIn)
        }
        _ => Err(unexpected()),
    }
}

async fn keep_session(
    app: &AppHandle,
    id: Uuid,
    accepted: Result<StoredSession, CookieRefusal>,
) -> Result<(), BoardFailure> {
    let session = accepted.map_err(|refusal| {
        diagnostics::record(
            app,
            &format!("server {id}: the board's sign-in cookie was refused: {refusal:?}"),
        );
        BoardFailure::server("The board's sign-in answer could not be used.")
    })?;
    let written =
        tauri::async_runtime::spawn_blocking(move || session_store::write(id, &session)).await;
    let failure = match written {
        Ok(Ok(())) => return Ok(()),
        Ok(Err(error)) => error.to_string(),
        Err(error) => error.to_string(),
    };
    diagnostics::record(
        app,
        &format!("server {id}: the session could not be written to the keychain: {failure}"),
    );
    Err(BoardFailure::server(
        "The session could not be saved in the keychain.",
    ))
}

pub fn cancel(app: &AppHandle, server: Uuid, code: &str) {
    app.state::<SignInFlows>().cancel(server, code);
}

pub async fn sign_out(app: &AppHandle, server: &Server) -> Result<SignOutStatus, BoardFailure> {
    if let Ok(Some(session)) = board_api::read_session(app, server.id).await {
        let http = app.state::<BoardHttp>();
        let _ = board_api::exchange_reply(
            &http.calls,
            &server.url,
            CallKind::Mutation,
            "auth.logout",
            None,
            Some(&session),
            SIGN_IN_BODY_CAP,
        )
        .await;
    }

    let id = server.id;
    let removed = tauri::async_runtime::spawn_blocking(move || session_store::delete(id)).await;
    let failure = match removed {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(error.to_string()),
        Err(error) => Some(error.to_string()),
    };
    if let Some(failure) = failure {
        diagnostics::record(
            app,
            &format!("server {id}: the session could not be removed from the keychain: {failure}"),
        );
        return Err(BoardFailure::server(
            "The session could not be removed from the keychain.",
        ));
    }

    emit_to_native(app, id, SIGNED_OUT, ());
    Ok(SignOutStatus::SignedOut)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board_api::{FailureClass, TrpcError};

    const TOKEN: &str = "abs_abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNO_-";
    const RFC_VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    const RFC_CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

    #[test]
    fn derives_the_challenge_the_way_rfc_7636_does() {
        let verifier = Verifier::from_text(RFC_VERIFIER).expect("a valid verifier");

        assert_eq!(verifier.challenge(), RFC_CHALLENGE);
    }

    #[test]
    fn generates_verifiers_the_board_accepts() {
        let first = Verifier::generate().unwrap();
        let second = Verifier::generate().unwrap();

        assert_eq!(first.as_str().len(), 43);
        assert!(Verifier::from_text(first.as_str()).is_some());
        let challenge = first.challenge();
        assert_eq!(challenge.len(), 43);
        assert!(is_flow_id(&challenge));
        assert_ne!(first.as_str(), second.as_str());
    }

    #[test]
    fn refuses_text_that_is_not_a_verifier() {
        let cases = [
            ("31 characters", "a".repeat(31), false),
            ("32 characters", "a".repeat(32), true),
            ("128 characters", "a".repeat(128), true),
            ("129 characters", "a".repeat(129), false),
            ("a slash", format!("{}/", "a".repeat(40)), false),
            (
                "the unreserved marks",
                format!("{}._~-", "a".repeat(40)),
                true,
            ),
        ];

        for (name, text, accepted) in cases {
            assert_eq!(Verifier::from_text(&text).is_some(), accepted, "{name}");
        }
    }

    #[test]
    fn checks_the_shape_of_a_flow_id() {
        let cases = [
            ("16 characters", "a".repeat(16), true),
            ("128 characters", "a".repeat(128), true),
            ("15 characters", "a".repeat(15), false),
            ("129 characters", "a".repeat(129), false),
            ("dash and underscore", format!("{}-_", "a".repeat(16)), true),
            ("a slash", format!("{}/", "a".repeat(16)), false),
            ("a dot", format!("{}.", "a".repeat(16)), false),
            ("a percent sign", format!("{}%", "a".repeat(16)), false),
        ];

        for (name, text, accepted) in cases {
            assert_eq!(is_flow_id(&text), accepted, "{name}");
        }
    }

    #[test]
    fn checks_the_shape_of_a_code() {
        let cases = [
            ("ABCD-EF23", true),
            ("ABCD-EF2O", false),
            ("ABCD-EF21", false),
            ("abcd-ef23", false),
            ("ABCDEF23", false),
            ("ABCD-EF234", false),
            ("ABCD_EF23", false),
            ("", false),
        ];

        for (text, accepted) in cases {
            assert_eq!(is_code(text), accepted, "{text}");
        }
    }

    fn accept(values: &[&[u8]]) -> Result<StoredSession, CookieRefusal> {
        accept_session(values)
    }

    fn session(name: CookieName) -> StoredSession {
        StoredSession {
            name,
            token: TOKEN.to_owned(),
        }
    }

    #[test]
    fn accepts_the_session_cookie_a_board_sets() {
        let host = format!(
            "__Host-board-session={TOKEN}; Path=/; HttpOnly; SameSite=Lax; Max-Age=34560000; Secure"
        );
        let plain =
            format!("board-session={TOKEN}; Path=/; HttpOnly; SameSite=Lax; Max-Age=34560000");

        assert_eq!(accept(&[host.as_bytes()]), Ok(session(CookieName::Host)));
        assert_eq!(accept(&[plain.as_bytes()]), Ok(session(CookieName::Plain)));
    }

    #[test]
    fn refuses_every_other_set_cookie() {
        let valid = format!("board-session={TOKEN}; Path=/");
        let cases: Vec<(&str, Vec<Vec<u8>>, CookieRefusal)> = vec![
            (
                "wrong name",
                vec![format!("session={TOKEN}; Path=/").into()],
                CookieRefusal::Name,
            ),
            (
                "name in another case",
                vec![format!("Board-Session={TOKEN}").into()],
                CookieRefusal::Name,
            ),
            (
                "no pair",
                vec![b"board-session".to_vec()],
                CookieRefusal::Name,
            ),
            (
                "Domain",
                vec![format!("board-session={TOKEN}; Path=/; Domain=example.com").into()],
                CookieRefusal::Domain,
            ),
            (
                "domain in lower case",
                vec![format!("board-session={TOKEN}; domain=example.com").into()],
                CookieRefusal::Domain,
            ),
            (
                "two cookies",
                vec![valid.clone().into(), valid.clone().into()],
                CookieRefusal::Several,
            ),
            ("none", vec![], CookieRefusal::None),
            (
                "malformed token",
                vec![b"board-session=abs_short; Path=/".to_vec()],
                CookieRefusal::Token,
            ),
            (
                "token with a space",
                vec![format!("board-session={} x", &TOKEN[..TOKEN.len() - 2]).into()],
                CookieRefusal::Token,
            ),
            (
                "clearing cookie",
                vec![b"board-session=; Path=/; Max-Age=0".to_vec()],
                CookieRefusal::Token,
            ),
            (
                "Max-Age zero with a valid token",
                vec![format!("board-session={TOKEN}; Max-Age=0").into()],
                CookieRefusal::Clearing,
            ),
            (
                "negative Max-Age",
                vec![format!("board-session={TOKEN}; max-age=-1").into()],
                CookieRefusal::Clearing,
            ),
            (
                "not UTF-8",
                vec![[b"board-session=".as_slice(), &[0xff]].concat()],
                CookieRefusal::NotText,
            ),
        ];

        for (name, values, expected) in cases {
            let borrowed: Vec<&[u8]> = values.iter().map(Vec::as_slice).collect();
            assert_eq!(accept(&borrowed), Err(expected), "{name}");
        }
    }

    fn pending(flow: &str, code: &str) -> PendingFlow {
        PendingFlow {
            flow: flow.to_owned(),
            code: code.to_owned(),
            verifier: Verifier::from_text(RFC_VERIFIER).unwrap(),
        }
    }

    #[test]
    fn keeps_one_pending_flow_per_server() {
        let (one, other) = (Uuid::new_v4(), Uuid::new_v4());
        let flows = SignInFlows::default();
        flows.replace(other, pending("flow-other", "ZZZZ-ZZZZ"));
        flows.replace(one, pending("flow-a", "AAAA-AAAA"));
        flows.replace(one, pending("flow-b", "BBBB-BBBB"));

        assert_eq!(flows.current(one, "BBBB-BBBB").unwrap().flow, "flow-b");
        assert!(flows.current(one, "AAAA-AAAA").is_none());

        flows.remove_if(one, "flow-a");
        assert!(flows.current(one, "BBBB-BBBB").is_some());
        flows.cancel(one, "AAAA-AAAA");
        assert!(flows.current(one, "BBBB-BBBB").is_some());

        flows.remove_if(one, "flow-b");
        assert!(flows.current(one, "BBBB-BBBB").is_none());
        flows.replace(one, pending("flow-c", "CCCC-CCCC"));
        flows.cancel(one, "CCCC-CCCC");
        assert!(flows.current(one, "CCCC-CCCC").is_none());

        assert!(flows.current(other, "ZZZZ-ZZZZ").is_some());
        flows.forget(other);
        assert!(flows.current(other, "ZZZZ-ZZZZ").is_none());
    }

    #[test]
    fn keeps_the_verifier_out_of_the_debug_output() {
        let text = format!("{:?}", pending("flow-a", "AAAA-AAAA"));

        assert!(!text.contains(RFC_VERIFIER), "{text}");
    }

    #[test]
    fn keeps_the_boards_message_for_a_sign_in_refusal() {
        let error = |code: &str, message: &str, upgrade: bool| {
            Answer::Error(TrpcError {
                code: code.to_owned(),
                message: message.to_owned(),
                upgrade,
            })
        };
        let expired = "This sign-in has expired. Start again.";

        assert_eq!(
            classify_sign_in(error("UNAUTHORIZED", expired, false)),
            Err(BoardFailure::server(expired))
        );
        assert_eq!(
            classify_sign_in(error("PRECONDITION_FAILED", "Update.", true))
                .unwrap_err()
                .class,
            FailureClass::UpgradeRequired
        );
        assert_eq!(
            classify_sign_in(Answer::Data(json!({ "status": "pending" }))),
            Ok(json!({ "status": "pending" }))
        );
        assert_eq!(
            classify_sign_in(Answer::NotTrpc { status: 502 }),
            Err(BoardFailure::server(
                "The board gave an unexpected answer (HTTP 502)."
            ))
        );
    }
}
