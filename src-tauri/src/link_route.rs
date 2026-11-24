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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> Url {
        Url::parse(input).expect("test url parses")
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
