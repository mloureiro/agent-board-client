use std::time::Duration;

use reqwest::redirect::Policy;
use serde::Serialize;
use url::Url;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const ANSWER_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ConnectionFailure {
    Unreachable,
    Timeout,
    Status { status: u16 },
    Other { message: String },
}

impl From<reqwest::Error> for ConnectionFailure {
    fn from(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            return Self::Timeout;
        }
        if error.is_connect() {
            return Self::Unreachable;
        }
        let error = error.without_url();
        let mut message = error.to_string();
        let mut source = std::error::Error::source(&error);
        while let Some(cause) = source {
            message = format!("{message}: {cause}");
            source = cause.source();
        }
        Self::Other { message }
    }
}

// A hosted board answers through its sign-in proxy, so a redirect or a 4xx proves it is up.
pub async fn check(url: &Url) -> Result<(), ConnectionFailure> {
    let client = reqwest::Client::builder()
        .redirect(Policy::none())
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(ANSWER_TIMEOUT)
        .build()?;
    let response = client.get(url.clone()).send().await?;

    let status = response.status();
    if status.is_server_error() {
        return Err(ConnectionFailure::Status {
            status: status.as_u16(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    fn answer_once(status_line: &'static str) -> Url {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            let response = format!(
                "HTTP/1.1 {status_line}\r\nLocation: https://login.example.com/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            stream.write_all(response.as_bytes()).unwrap();
        });
        Url::parse(&format!("http://{address}/")).unwrap()
    }

    fn closed_port() -> Url {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        Url::parse(&format!("http://{address}/")).unwrap()
    }

    fn run(url: &Url) -> Result<(), ConnectionFailure> {
        tauri::async_runtime::block_on(check(url))
    }

    #[test]
    fn a_server_that_answers_is_reachable() {
        assert_eq!(run(&answer_once("200 OK")), Ok(()));
    }

    #[test]
    fn a_sign_in_redirect_counts_as_an_answer_and_is_not_followed() {
        assert_eq!(run(&answer_once("302 Found")), Ok(()));
    }

    #[test]
    fn a_server_error_reports_its_status() {
        assert_eq!(
            run(&answer_once("502 Bad Gateway")),
            Err(ConnectionFailure::Status { status: 502 })
        );
    }

    #[test]
    fn a_port_with_nothing_listening_is_unreachable() {
        assert_eq!(run(&closed_port()), Err(ConnectionFailure::Unreachable));
    }
}
