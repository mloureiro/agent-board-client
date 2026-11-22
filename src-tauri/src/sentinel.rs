use url::Url;

const SCHEME: &str = "agent-board-client";
const MAX_PATH_BYTES: usize = 2048;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    Open(String),
    Route(String),
}

pub fn is_sentinel(url: &Url) -> bool {
    url.scheme() == SCHEME
}

pub fn is_plain_path(path: &str) -> bool {
    path.starts_with('/')
        && !path.starts_with("//")
        && !path.contains('\\')
        && path.len() <= MAX_PATH_BYTES
}

pub fn parse(url: &Url) -> Option<Signal> {
    if !is_sentinel(url) {
        return None;
    }

    let path = url
        .query_pairs()
        .find(|(key, _)| key == "path")
        .map(|(_, value)| value.into_owned())
        .filter(|path| is_plain_path(path))?;

    match url.path() {
        "open" => Some(Signal::Open(path)),
        "route" => Some(Signal::Route(path)),
        _ => None,
    }
}

pub fn same_origin_url(server: &Url, path: &str) -> Option<Url> {
    let url = server.join(path).ok()?;
    (url.origin() == server.origin()).then_some(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(input: &str) -> Url {
        Url::parse(input).expect("test url parses")
    }

    #[test]
    fn parses_only_well_formed_signals() {
        let long = format!("agent-board-client:open?path=%2F{}", "a".repeat(2048));
        let cases = [
            (
                "agent-board-client:open?path=%2Fsettings",
                Some(Signal::Open("/settings".to_owned())),
            ),
            (
                "agent-board-client:route?path=%2Ft%2Fk3J9%3Ftranscript%3D1",
                Some(Signal::Route("/t/k3J9?transcript=1".to_owned())),
            ),
            ("agent-board-client:close?path=%2Fsettings", None),
            ("agent-board-client:open", None),
            ("agent-board-client:open?other=%2Fsettings", None),
            ("agent-board-client:open?path=", None),
            ("agent-board-client:open?path=settings", None),
            ("agent-board-client:open?path=%2F%2Fevil.test%2Fx", None),
            ("agent-board-client:open?path=%2F%5Cevil", None),
            (long.as_str(), None),
            ("https://board.example.com/open?path=%2Fsettings", None),
        ];

        for (input, expected) in cases {
            assert_eq!(parse(&url(input)), expected, "{input}");
        }
    }

    #[test]
    fn recognises_the_scheme_even_when_the_signal_is_malformed() {
        assert!(is_sentinel(&url("agent-board-client:nonsense")));
        assert!(!is_sentinel(&url("https://board.example.com/")));
    }

    #[test]
    fn rebuilds_only_urls_on_the_server_origin() {
        let server = url("https://board.example.com/");
        let cases = [
            ("/settings", Some("https://board.example.com/settings")),
            (
                "/t/k3J9?transcript=1",
                Some("https://board.example.com/t/k3J9?transcript=1"),
            ),
            ("//evil.test/x", None),
            ("/\\evil", None),
            ("https://evil.test/x", None),
        ];

        for (path, expected) in cases {
            assert_eq!(
                same_origin_url(&server, path).as_ref().map(Url::as_str),
                expected,
                "{path}"
            );
        }
    }
}
