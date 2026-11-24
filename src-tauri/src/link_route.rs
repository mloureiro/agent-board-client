use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkRoute {
    SameWindow,
    SystemBrowser,
    Drop,
}

// The board's sign-in hand-off page, which must run in the default browser where passkeys work.
const HANDOFF_PATH_PREFIX: &str = "/handoff/";

pub fn route(server: &Url, requested: &Url) -> LinkRoute {
    let is_web = matches!(requested.scheme(), "http" | "https");
    let is_same_origin = is_web && requested.origin() == server.origin();

    if is_same_origin && requested.path().starts_with(HANDOFF_PATH_PREFIX) {
        return LinkRoute::SystemBrowser;
    }

    if is_same_origin {
        return LinkRoute::SameWindow;
    }

    if is_web {
        return LinkRoute::SystemBrowser;
    }

    LinkRoute::Drop
}

pub fn route_from_native(requested: &Url) -> LinkRoute {
    if matches!(requested.scheme(), "http" | "https") {
        LinkRoute::SystemBrowser
    } else {
        LinkRoute::Drop
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> Url {
        Url::parse(input).expect("test url parses")
    }

    #[test]
    fn sends_a_link_from_a_native_page_to_the_system_browser_or_drops_it() {
        let cases = [
            ("https://github.com/o/r/pull/1", LinkRoute::SystemBrowser),
            ("http://localhost:4000/t/1", LinkRoute::SystemBrowser),
            (
                "https://board.example.com/handoff/abc",
                LinkRoute::SystemBrowser,
            ),
            ("mailto:a@example.com", LinkRoute::Drop),
            ("file:///etc/hosts", LinkRoute::Drop),
            ("tauri://localhost/board.html", LinkRoute::Drop),
            ("javascript:alert(1)", LinkRoute::Drop),
        ];

        for (requested, expected) in cases {
            assert_eq!(
                route_from_native(&parse(requested)),
                expected,
                "{requested}"
            );
        }
    }

    #[test]
    fn routes_a_requested_url_by_its_origin_and_scheme() {
        let cases = [
            (
                "https://board.example.com/",
                "https://board.example.com/t/1",
                LinkRoute::SameWindow,
            ),
            (
                "https://board.example.com/",
                "https://board.example.com:443/x",
                LinkRoute::SameWindow,
            ),
            (
                "https://board.example.com/",
                "https://board.example.com:8443/",
                LinkRoute::SystemBrowser,
            ),
            (
                "https://board.example.com/",
                "http://board.example.com/",
                LinkRoute::SystemBrowser,
            ),
            (
                "https://board.example.com/",
                "https://auth.example.com/",
                LinkRoute::SystemBrowser,
            ),
            (
                "https://board.example.com/",
                "https://board.example.com.evil.test/",
                LinkRoute::SystemBrowser,
            ),
            (
                "https://board.example.com/",
                "https://github.com/o/r/pull/1",
                LinkRoute::SystemBrowser,
            ),
            (
                "https://board.example.com/",
                "mailto:a@example.com",
                LinkRoute::Drop,
            ),
            (
                "https://board.example.com/",
                "file:///etc/hosts",
                LinkRoute::Drop,
            ),
            ("https://board.example.com/", "about:blank", LinkRoute::Drop),
            (
                "http://[::1]:4000/",
                "http://[::1]:4000/x",
                LinkRoute::SameWindow,
            ),
            (
                "https://board.example.com/",
                "https://board.example.com/handoff/abc",
                LinkRoute::SystemBrowser,
            ),
            (
                "https://board.example.com/",
                "https://board.example.com/handoff/abc?x=1",
                LinkRoute::SystemBrowser,
            ),
            (
                "https://board.example.com/",
                "https://board.example.com/handoff",
                LinkRoute::SameWindow,
            ),
            (
                "https://board.example.com/",
                "https://board.example.com/handoffs/abc",
                LinkRoute::SameWindow,
            ),
            (
                "https://board.example.com/",
                "https://board.example.com/t/handoff/abc",
                LinkRoute::SameWindow,
            ),
            (
                "https://board.example.com/",
                "https://other.example.com/handoff/abc",
                LinkRoute::SystemBrowser,
            ),
            (
                "http://[::1]:4000/",
                "http://[::1]:4000/handoff/abc",
                LinkRoute::SystemBrowser,
            ),
        ];

        for (server, requested, expected) in cases {
            assert_eq!(
                route(&parse(server), &parse(requested)),
                expected,
                "{requested} against {server}"
            );
        }
    }
}
