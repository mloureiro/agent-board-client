use url::Url;

use crate::notify::Notice;

const SCHEME: &str = "agent-board-client";
const MAX_PATH_BYTES: usize = 2048;
const MAX_TITLE_CHARS: usize = 200;
const MAX_BODY_CHARS: usize = 500;
const MAX_TAG_CHARS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    Open(String),
    Route(String),
    Cycle(bool),
    ZoomIn,
    Notify(Notice),
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

    if url.path() == "cycle" {
        return match url.query_pairs().find(|(key, _)| key == "dir")?.1.as_ref() {
            "next" => Some(Signal::Cycle(true)),
            "previous" => Some(Signal::Cycle(false)),
            _ => None,
        };
    }

    if url.path() == "zoom" {
        let step = url.query_pairs().find(|(key, _)| key == "step")?.1;
        return (step == "in").then_some(Signal::ZoomIn);
    }

    if url.path() == "notify" {
        return parse_notify(url).map(Signal::Notify);
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

fn parse_notify(url: &Url) -> Option<Notice> {
    let mut title = None;
    let mut body = None;
    let mut tag = None;
    let mut path = None;
    for (key, value) in url.query_pairs() {
        let slot = match key.as_ref() {
            "title" => &mut title,
            "body" => &mut body,
            "tag" => &mut tag,
            "path" => &mut path,
            _ => continue,
        };
        slot.get_or_insert(value.into_owned());
    }

    let within = |value: &String, max: usize| value.chars().count() <= max;
    let title = title.filter(|title| within(title, MAX_TITLE_CHARS))?;
    let body = body.unwrap_or_default();
    if !within(&body, MAX_BODY_CHARS) {
        return None;
    }
    if tag.as_ref().is_some_and(|tag| !within(tag, MAX_TAG_CHARS)) {
        return None;
    }
    if path.as_ref().is_some_and(|path| !is_plain_path(path)) {
        return None;
    }

    Some(Notice {
        title,
        body,
        tag,
        path,
    })
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
            (
                "agent-board-client:cycle?dir=next",
                Some(Signal::Cycle(true)),
            ),
            (
                "agent-board-client:cycle?dir=previous",
                Some(Signal::Cycle(false)),
            ),
            ("agent-board-client:cycle?dir=sideways", None),
            ("agent-board-client:cycle", None),
            ("agent-board-client:zoom?step=in", Some(Signal::ZoomIn)),
            ("agent-board-client:zoom?step=out", None),
            ("agent-board-client:zoom", None),
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

    fn notice(title: &str, body: &str, tag: Option<&str>, path: Option<&str>) -> Option<Signal> {
        Some(Signal::Notify(Notice {
            title: title.to_owned(),
            body: body.to_owned(),
            tag: tag.map(str::to_owned),
            path: path.map(str::to_owned),
        }))
    }

    #[test]
    fn parses_notify_signals_within_the_caps() {
        let title_200 = format!("agent-board-client:notify?title={}", "%C3%A9".repeat(200));
        let title_201 = format!("agent-board-client:notify?title={}", "%C3%A9".repeat(201));
        let body_501 = format!("agent-board-client:notify?title=x&body={}", "b".repeat(501));
        let tag_201 = format!("agent-board-client:notify?title=x&tag={}", "t".repeat(201));
        let cases = [
            (
                "agent-board-client:notify?title=Needs%20you&body=Fix%20it&tag=task-1&path=%2Ft%2Fk3J9",
                notice("Needs you", "Fix it", Some("task-1"), Some("/t/k3J9")),
            ),
            (
                "agent-board-client:notify?title=Needs%20you",
                notice("Needs you", "", None, None),
            ),
            (
                "agent-board-client:notify?title=&body=x",
                notice("", "x", None, None),
            ),
            ("agent-board-client:notify?body=x", None),
            (title_200.as_str(), notice(&"\u{e9}".repeat(200), "", None, None)),
            (title_201.as_str(), None),
            (body_501.as_str(), None),
            (tag_201.as_str(), None),
            ("agent-board-client:notify?title=x&path=%2F%2Fevil.test", None),
            ("agent-board-client:notify?title=x&path=%2F%5Cx", None),
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
