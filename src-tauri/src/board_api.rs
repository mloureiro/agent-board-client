use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE, COOKIE};
use reqwest::redirect::Policy;
use reqwest::{Client, Method, RequestBuilder};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager};
use url::Url;
use uuid::Uuid;

use crate::diagnostics;
use crate::error::ClientError;
use crate::session_store::{self, StoredSession};
use crate::store::Server;
use crate::ui_api::UiApi;

pub const UI_API_HEADER: &str = "x-board-ui-api";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const CALL_TIMEOUT: Duration = Duration::from_secs(30);
const STREAM_READ_TIMEOUT: Duration = Duration::from_secs(60);
pub const BODY_CAP: usize = 32 * 1024 * 1024;
const MESSAGE_CHARS: usize = 300;
const PATH_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FailureClass {
    Offline,
    SignedOut,
    UpgradeRequired,
    Server,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BoardFailure {
    pub class: FailureClass,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl BoardFailure {
    pub fn offline() -> Self {
        Self {
            class: FailureClass::Offline,
            message: None,
        }
    }

    pub fn signed_out() -> Self {
        Self {
            class: FailureClass::SignedOut,
            message: None,
        }
    }

    pub fn server(message: &str) -> Self {
        Self {
            class: FailureClass::Server,
            message: Some(bounded(message)),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum BoardCallError {
    Board(BoardFailure),
    Client(ClientError),
}

impl From<BoardFailure> for BoardCallError {
    fn from(failure: BoardFailure) -> Self {
        Self::Board(failure)
    }
}

impl From<ClientError> for BoardCallError {
    fn from(error: ClientError) -> Self {
        Self::Client(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CallKind {
    Query,
    Mutation,
}

pub fn bounded(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_control())
        .take(MESSAGE_CHARS)
        .collect()
}

fn is_segment(segment: &str) -> bool {
    let mut bytes = segment.bytes();
    bytes.next().is_some_and(|first| first.is_ascii_lowercase())
        && bytes.all(|byte| byte.is_ascii_alphabetic())
}

// Public auth procedures answer UNAUTHORIZED for a failed sign-in, which would read as an
// expired session and delete a valid token.
pub fn checked_path(path: &str) -> Option<&str> {
    let allowed = path.len() <= PATH_BYTES
        && path.split('.').all(is_segment)
        && path.split('.').next() != Some("auth");
    allowed.then_some(path)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrpcError {
    pub code: String,
    pub message: String,
    pub upgrade: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    Data(Value),
    Error(TrpcError),
    NotTrpc { status: u16 },
}

pub fn parse_answer(status: u16, body: &[u8]) -> Answer {
    let not_trpc = Answer::NotTrpc { status };
    let Ok(Value::Object(root)) = serde_json::from_slice::<Value>(body) else {
        return not_trpc;
    };

    if let Some(Value::Object(result)) = root.get("result") {
        return Answer::Data(result.get("data").cloned().unwrap_or(Value::Null));
    }

    let Some(Value::Object(error)) = root.get("error") else {
        return not_trpc;
    };
    let message = error.get("message").and_then(Value::as_str);
    let data = error.get("data").and_then(Value::as_object);
    let code = data
        .and_then(|data| data.get("code"))
        .and_then(Value::as_str);
    match (message, code) {
        (Some(message), Some(code)) => Answer::Error(TrpcError {
            code: code.to_owned(),
            message: message.to_owned(),
            upgrade: data
                .and_then(|data| data.get("upgrade"))
                .is_some_and(Value::is_object),
        }),
        _ => not_trpc,
    }
}

pub fn classify(answer: Answer) -> Result<Value, BoardFailure> {
    match answer {
        Answer::Data(value) => Ok(value),
        Answer::Error(error) if error.code == "UNAUTHORIZED" => Err(BoardFailure::signed_out()),
        Answer::Error(error) if error.code == "PRECONDITION_FAILED" && error.upgrade => {
            Err(BoardFailure {
                class: FailureClass::UpgradeRequired,
                message: Some(bounded(&error.message)),
            })
        }
        Answer::Error(error) => Err(BoardFailure::server(&error.message)),
        Answer::NotTrpc { status } => Err(BoardFailure::server(&format!(
            "The board gave an unexpected answer (HTTP {status})."
        ))),
    }
}

pub fn classify_transport(error: &reqwest::Error) -> BoardFailure {
    if error.is_timeout() || error.is_connect() || error.is_request() || error.is_body() {
        return BoardFailure::offline();
    }
    BoardFailure::server("The request to the board could not be made.")
}

pub struct BoardHttp {
    pub calls: Client,
    pub streams: Client,
}

impl BoardHttp {
    pub fn new() -> Result<Self, reqwest::Error> {
        Self::with_timeouts(CALL_TIMEOUT, STREAM_READ_TIMEOUT)
    }

    fn with_timeouts(call: Duration, stream_read: Duration) -> Result<Self, reqwest::Error> {
        let calls = Client::builder()
            .redirect(Policy::none())
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(call)
            .build()?;
        // A total timeout would cut a stream that is healthy; only silence ends it.
        let streams = Client::builder()
            .redirect(Policy::none())
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(stream_read)
            .build()?;
        Ok(Self { calls, streams })
    }
}

pub fn with_board_headers(
    builder: RequestBuilder,
    session: Option<&StoredSession>,
) -> Result<RequestBuilder, BoardFailure> {
    let version = UiApi::bundled().ui_api_version.to_string();
    let builder = builder.header(UI_API_HEADER, version);
    let Some(session) = session else {
        return Ok(builder);
    };
    let mut cookie = HeaderValue::from_str(&format!("{}={}", session.name.as_str(), session.token))
        .map_err(|_| BoardFailure::server("The stored session cannot be sent."))?;
    cookie.set_sensitive(true);
    Ok(builder.header(COOKIE, cookie))
}

fn request(
    client: &Client,
    base: &Url,
    kind: CallKind,
    path: &str,
    input: Option<&Value>,
    session: Option<&StoredSession>,
) -> Result<RequestBuilder, BoardFailure> {
    let failure = || BoardFailure::server("The request to the board could not be made.");
    let mut url = base
        .join("trpc/")
        .and_then(|trpc| trpc.join(path))
        .map_err(|_| failure())?;

    let builder = match kind {
        CallKind::Query => {
            if let Some(input) = input {
                let encoded = serde_json::to_string(input).map_err(|_| failure())?;
                url.query_pairs_mut().append_pair("input", &encoded);
            }
            client.request(Method::GET, url)
        }
        CallKind::Mutation => {
            // The board refuses a write that is not application/json, and an empty body with that
            // type fails to parse, so a mutation without input sends a JSON null.
            let body = match input {
                Some(input) => serde_json::to_vec(input).map_err(|_| failure())?,
                None => b"null".to_vec(),
            };
            client
                .request(Method::POST, url)
                .header(CONTENT_TYPE, "application/json")
                .body(body)
        }
    };
    with_board_headers(builder, session)
}

pub async fn read_capped(
    mut response: reqwest::Response,
    cap: usize,
) -> Result<Vec<u8>, BoardFailure> {
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| classify_transport(&error))?
    {
        if body.len() + chunk.len() > cap {
            return Err(BoardFailure::server("The board's answer is too large."));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

pub struct Reply {
    pub answer: Answer,
    pub headers: HeaderMap,
}

pub async fn exchange_reply(
    client: &Client,
    base: &Url,
    kind: CallKind,
    path: &str,
    input: Option<&Value>,
    session: Option<&StoredSession>,
    cap: usize,
) -> Result<Reply, BoardFailure> {
    let builder = request(client, base, kind, path, input, session)?;
    let response = builder
        .send()
        .await
        .map_err(|error| classify_transport(&error))?;
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let body = read_capped(response, cap).await?;
    Ok(Reply {
        answer: parse_answer(status, &body),
        headers,
    })
}

pub async fn exchange(
    client: &Client,
    base: &Url,
    kind: CallKind,
    path: &str,
    input: Option<&Value>,
    session: Option<&StoredSession>,
    cap: usize,
) -> Result<Answer, BoardFailure> {
    exchange_reply(client, base, kind, path, input, session, cap)
        .await
        .map(|reply| reply.answer)
}

pub async fn perform(
    client: &Client,
    base: &Url,
    kind: CallKind,
    path: &str,
    input: Option<&Value>,
    session: Option<&StoredSession>,
    cap: usize,
) -> Result<Value, BoardFailure> {
    classify(exchange(client, base, kind, path, input, session, cap).await?)
}

pub async fn read_session(
    app: &AppHandle,
    server: Uuid,
) -> Result<Option<StoredSession>, BoardFailure> {
    let read = tauri::async_runtime::spawn_blocking(move || session_store::read(server)).await;
    match read {
        Ok(Ok((session, discarded))) => {
            if discarded {
                diagnostics::record(
                    app,
                    &format!("server {server}: the stored session was malformed and was deleted"),
                );
            }
            Ok(session)
        }
        Ok(Err(error)) => {
            diagnostics::record(
                app,
                &format!(
                    "server {server}: the session could not be read from the keychain: {error}"
                ),
            );
            Err(keychain_failure())
        }
        Err(error) => {
            diagnostics::record(
                app,
                &format!("server {server}: the keychain read did not finish: {error}"),
            );
            Err(keychain_failure())
        }
    }
}

fn keychain_failure() -> BoardFailure {
    BoardFailure::server("The session could not be read from the keychain.")
}

pub async fn delete_session_if(app: &AppHandle, server: Uuid, token: String) {
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        session_store::delete_if_token(server, &token)
    })
    .await;
    let failure = match outcome {
        Ok(Ok(())) => return,
        Ok(Err(error)) => error.to_string(),
        Err(error) => error.to_string(),
    };
    diagnostics::record(
        app,
        &format!("server {server}: the refused session could not be deleted: {failure}"),
    );
}

pub async fn call(
    app: &AppHandle,
    server: &Server,
    kind: CallKind,
    path: &str,
    input: Option<Value>,
) -> Result<Value, BoardFailure> {
    let session = read_session(app, server.id).await?;
    let http = app.state::<BoardHttp>();
    let result = perform(
        &http.calls,
        &server.url,
        kind,
        path,
        input.as_ref(),
        session.as_ref(),
        BODY_CAP,
    )
    .await;

    if let (Err(failure), Some(sent)) = (&result, session) {
        if failure.class == FailureClass::SignedOut {
            delete_session_if(app, server.id, sent.token).await;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::mpsc;
    use std::thread;

    use serde_json::json;

    use super::*;
    use crate::session_store::CookieName;

    const TOKEN: &str = "abs_abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNO_-";

    #[test]
    fn accepts_only_procedure_paths_outside_the_auth_router() {
        let long = "a".repeat(129);
        let cases: [(&str, bool); 20] = [
            ("me", true),
            ("tasks.list", true),
            ("ui.version", true),
            ("transcript.events", true),
            ("auth.config", false),
            ("auth.passkey.login", false),
            ("auth", false),
            ("", false),
            ("Me", false),
            ("tasks.", false),
            (".tasks", false),
            ("tasks..list", false),
            ("tasks/list", false),
            ("tasks.list?x=1", false),
            ("tasks-list", false),
            ("tasks.1st", false),
            ("authors.list", true),
            (&long, false),
            ("tasks.List", false),
            ("tasks.list ", false),
        ];

        for (path, accepted) in cases {
            assert_eq!(checked_path(path).is_some(), accepted, "{path:?}");
        }
    }

    #[test]
    fn parses_what_a_board_answers() {
        let error = |code: &str, upgrade: bool| {
            Answer::Error(TrpcError {
                code: code.to_owned(),
                message: "no".to_owned(),
                upgrade,
            })
        };
        let cases: Vec<(&str, u16, &str, Answer)> = vec![
            (
                "data",
                200,
                r#"{"result":{"data":[1]}}"#,
                Answer::Data(json!([1])),
            ),
            ("void", 200, r#"{"result":{}}"#, Answer::Data(Value::Null)),
            (
                "an error",
                500,
                r#"{"error":{"message":"no","data":{"code":"INTERNAL_SERVER_ERROR"}}}"#,
                error("INTERNAL_SERVER_ERROR", false),
            ),
            (
                "an upgrade",
                412,
                r#"{"error":{"message":"no","data":{"code":"PRECONDITION_FAILED","upgrade":{"minClient":2}}}}"#,
                error("PRECONDITION_FAILED", true),
            ),
            (
                "html",
                502,
                "<html></html>",
                Answer::NotTrpc { status: 502 },
            ),
            ("empty", 204, "", Answer::NotTrpc { status: 204 }),
            ("an array", 200, "[]", Answer::NotTrpc { status: 200 }),
            (
                "a string error",
                401,
                r#"{"error":"Sign in to use the board."}"#,
                Answer::NotTrpc { status: 401 },
            ),
            (
                "an error without a code",
                500,
                r#"{"error":{"message":"no"}}"#,
                Answer::NotTrpc { status: 500 },
            ),
        ];

        for (name, status, body, expected) in cases {
            assert_eq!(parse_answer(status, body.as_bytes()), expected, "{name}");
        }
    }

    #[test]
    fn classifies_an_answer_by_what_the_client_does_next() {
        let error = |code: &str, upgrade: bool| {
            Answer::Error(TrpcError {
                code: code.to_owned(),
                message: "text".to_owned(),
                upgrade,
            })
        };
        let failure = |class, message: Option<&str>| {
            Err(BoardFailure {
                class,
                message: message.map(str::to_owned),
            })
        };
        let cases: Vec<(&str, Answer, Result<Value, BoardFailure>)> = vec![
            ("data", Answer::Data(json!(1)), Ok(json!(1))),
            (
                "unauthorized",
                error("UNAUTHORIZED", false),
                failure(FailureClass::SignedOut, None),
            ),
            (
                "an upgrade",
                error("PRECONDITION_FAILED", true),
                failure(FailureClass::UpgradeRequired, Some("text")),
            ),
            (
                "a precondition without an upgrade",
                error("PRECONDITION_FAILED", false),
                failure(FailureClass::Server, Some("text")),
            ),
            (
                "another code",
                error("INTERNAL_SERVER_ERROR", false),
                failure(FailureClass::Server, Some("text")),
            ),
            (
                "not trpc",
                Answer::NotTrpc { status: 502 },
                failure(
                    FailureClass::Server,
                    Some("The board gave an unexpected answer (HTTP 502)."),
                ),
            ),
        ];

        for (name, answer, expected) in cases {
            assert_eq!(classify(answer), expected, "{name}");
        }
    }

    #[test]
    fn bounds_a_text_from_the_board() {
        let long = "x".repeat(301);
        let multibyte = "é".repeat(301);

        assert_eq!(bounded(&long).chars().count(), 300);
        assert_eq!(bounded(&multibyte), "é".repeat(300));
        assert_eq!(bounded("a\u{0}b\nc\u{1b}[31md"), "abc[31md");
    }

    fn client(call_timeout: Duration) -> Client {
        BoardHttp::with_timeouts(call_timeout, call_timeout)
            .unwrap()
            .calls
    }

    fn run(
        base: &Url,
        kind: CallKind,
        cap: usize,
        call_timeout: Duration,
    ) -> Result<Value, BoardFailure> {
        tauri::async_runtime::block_on(perform(
            &client(call_timeout),
            base,
            kind,
            "tasks.list",
            None,
            None,
            cap,
        ))
    }

    fn query(base: &Url) -> Result<Value, BoardFailure> {
        run(base, CallKind::Query, BODY_CAP, Duration::from_secs(5))
    }

    fn read_request(stream: &mut TcpStream) -> String {
        let mut received = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let read = stream.read(&mut buffer).unwrap_or(0);
            if read == 0 {
                break;
            }
            received.extend_from_slice(&buffer[..read]);
            let text = String::from_utf8_lossy(&received).to_lowercase();
            let Some(end) = text.find("\r\n\r\n") else {
                continue;
            };
            let length = text
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .and_then(|value| value.trim().parse::<usize>().ok())
                .unwrap_or(0);
            if received.len() >= end + 4 + length {
                break;
            }
        }
        String::from_utf8_lossy(&received).into_owned()
    }

    fn answer(status_line: &str, extra: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {status_line}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    fn serve(response: String) -> (Url, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            while let Ok((mut stream, _)) = listener.accept() {
                let request = read_request(&mut stream);
                let _ = sender.send(request);
                let _ = stream.write_all(response.as_bytes());
            }
        });
        (Url::parse(&format!("http://{address}/")).unwrap(), receiver)
    }

    #[test]
    fn a_port_with_nothing_listening_is_offline() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let base = Url::parse(&format!("http://{address}/")).unwrap();

        assert_eq!(query(&base), Err(BoardFailure::offline()));
    }

    #[test]
    fn a_board_that_never_answers_is_offline() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let held = listener.accept();
            thread::sleep(Duration::from_secs(4));
            drop(held);
        });
        let base = Url::parse(&format!("http://{address}/")).unwrap();

        let result = run(&base, CallKind::Query, BODY_CAP, Duration::from_secs(1));

        assert_eq!(result, Err(BoardFailure::offline()));
    }

    #[test]
    fn a_tls_failure_is_offline() {
        let (base, _requests) = serve("garbage\r\n\r\n".to_owned());
        let secure = Url::parse(&format!("https://127.0.0.1:{}/", base.port().unwrap())).unwrap();

        assert_eq!(query(&secure), Err(BoardFailure::offline()));
    }

    #[test]
    fn a_trpc_error_is_a_server_failure_with_the_boards_text() {
        let body = r#"{"error":{"message":"It broke.","data":{"code":"INTERNAL_SERVER_ERROR"}}}"#;
        let (base, _requests) = serve(answer("500 Internal Server Error", "", body));

        assert_eq!(query(&base), Err(BoardFailure::server("It broke.")));
    }

    #[test]
    fn a_redirect_is_not_followed() {
        let (base, requests) = serve(answer(
            "302 Found",
            "Location: http://127.0.0.1:1/elsewhere\r\n",
            "",
        ));

        let result = query(&base);

        assert_eq!(
            result,
            Err(BoardFailure::server(
                "The board gave an unexpected answer (HTTP 302)."
            ))
        );
        assert_eq!(requests.try_iter().count(), 1);
    }

    #[test]
    fn an_answer_over_the_cap_is_refused() {
        let body = r#"{"result":{"data":"0123456789"}}"#;
        let (base, _requests) = serve(answer("200 OK", "", body));

        let result = run(&base, CallKind::Query, 10, Duration::from_secs(5));

        assert_eq!(
            result,
            Err(BoardFailure::server("The board's answer is too large."))
        );
    }

    #[test]
    fn sends_a_query_and_a_mutation_as_the_board_expects() {
        let body = r#"{"result":{"data":1}}"#;
        let (base, requests) = serve(answer("200 OK", "", body));
        let session = StoredSession {
            name: CookieName::Plain,
            token: TOKEN.to_owned(),
        };
        let input = json!({ "id": "a b" });
        let http = client(Duration::from_secs(5));

        let queried = tauri::async_runtime::block_on(perform(
            &http,
            &base,
            CallKind::Query,
            "tasks.get",
            Some(&input),
            Some(&session),
            BODY_CAP,
        ));
        let mutated = tauri::async_runtime::block_on(perform(
            &http,
            &base,
            CallKind::Mutation,
            "tasks.stop",
            None,
            None,
            BODY_CAP,
        ));

        assert_eq!(queried, Ok(json!(1)));
        assert_eq!(mutated, Ok(json!(1)));
        let query = requests.recv().unwrap().to_lowercase();
        let mutation = requests.recv().unwrap().to_lowercase();
        let encoded = url::form_urlencoded::byte_serialize(br#"{"id":"a b"}"#)
            .collect::<String>()
            .to_lowercase();
        assert!(
            query.starts_with(&format!("get /trpc/tasks.get?input={encoded} http/1.1")),
            "{query}"
        );
        assert!(query.contains("x-board-ui-api: 1\r\n"), "{query}");
        assert!(
            query.contains(&format!(
                "cookie: board-session={}\r\n",
                TOKEN.to_lowercase()
            )),
            "{query}"
        );
        assert!(
            mutation.starts_with("post /trpc/tasks.stop http/1.1"),
            "{mutation}"
        );
        assert!(
            mutation.contains("content-type: application/json\r\n"),
            "{mutation}"
        );
        assert!(mutation.ends_with("\r\n\r\nnull"), "{mutation}");
        assert!(!mutation.contains("cookie:"), "{mutation}");
    }

    #[test]
    fn keeps_every_set_cookie_header_of_a_reply() {
        let body = r#"{"result":{"data":{"status":"signed-in"}}}"#;
        let (base, requests) = serve(answer(
            "200 OK",
            "Set-Cookie: board-session=one\r\nSet-Cookie: other=two\r\n",
            body,
        ));
        let http = client(Duration::from_secs(5));
        let input = json!({ "flow": "f" });

        let reply = tauri::async_runtime::block_on(exchange_reply(
            &http,
            &base,
            CallKind::Mutation,
            "auth.handoff.finish",
            Some(&input),
            None,
            BODY_CAP,
        ))
        .unwrap();

        assert_eq!(reply.answer, Answer::Data(json!({ "status": "signed-in" })));
        let cookies: Vec<&[u8]> = reply
            .headers
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .map(HeaderValue::as_bytes)
            .collect();
        assert_eq!(cookies, [&b"board-session=one"[..], &b"other=two"[..]]);
        let request = requests.recv().unwrap().to_lowercase();
        assert!(request.contains("x-board-ui-api: 1\r\n"), "{request}");
        assert!(
            request.contains("content-type: application/json\r\n"),
            "{request}"
        );
    }
}
