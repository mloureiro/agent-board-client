use url::Url;

const ENTRY: &str = "board.html";

pub fn entry_path(server_name: &str, path: &str) -> String {
    let name: String = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("server", server_name)
        .finish();
    format!("{ENTRY}?{name}#{path}")
}

pub fn entry_of(tabbar_url: &Url) -> Option<Url> {
    tabbar_url.join(ENTRY).ok()
}

pub fn is_app_entry(entry: &Url, requested: &Url) -> bool {
    // `Url::origin()` is opaque for `tauri://localhost`, and two opaque origins never compare equal.
    requested.scheme() == entry.scheme()
        && requested.host_str() == entry.host_str()
        && requested.port_or_known_default() == entry.port_or_known_default()
        && requested.path() == entry.path()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> Url {
        Url::parse(input).expect("test url parses")
    }

    #[test]
    fn accepts_only_the_apps_own_entry_page() {
        let cases = [
            (
                "tauri://localhost/board.html",
                "tauri://localhost/board.html",
                true,
            ),
            (
                "tauri://localhost/board.html",
                "tauri://localhost/board.html?server=A#/t/1",
                true,
            ),
            (
                "tauri://localhost/board.html",
                "tauri://localhost/index.html",
                false,
            ),
            (
                "tauri://localhost/board.html",
                "tauri://localhost/tabbar.html",
                false,
            ),
            (
                "tauri://localhost/board.html",
                "tauri://evil/board.html",
                false,
            ),
            (
                "tauri://localhost/board.html",
                "https://localhost/board.html",
                false,
            ),
            (
                "tauri://localhost/board.html",
                "https://board.example.com/board.html",
                false,
            ),
            ("tauri://localhost/board.html", "about:blank", false),
            (
                "http://localhost:1420/board.html",
                "http://localhost:1420/board.html#/",
                true,
            ),
            (
                "http://localhost:1420/board.html",
                "http://localhost:1421/board.html",
                false,
            ),
        ];

        for (entry, requested, expected) in cases {
            assert_eq!(
                is_app_entry(&parse(entry), &parse(requested)),
                expected,
                "{requested} against {entry}"
            );
        }
    }

    #[test]
    fn puts_the_server_name_in_the_query_and_the_path_in_the_fragment() {
        let cases = [
            ("Work", "/", "board.html?server=Work#/"),
            (
                "My board & co #1",
                "/t/k3J9?transcript=1",
                "board.html?server=My+board+%26+co+%231#/t/k3J9?transcript=1",
            ),
            ("Zé", "/settings", "board.html?server=Z%C3%A9#/settings"),
        ];

        for (name, path, expected) in cases {
            assert_eq!(entry_path(name, path), expected, "{name} {path}");
        }
    }

    #[test]
    fn finds_the_entry_next_to_the_tab_bar() {
        let cases = [
            (
                "tauri://localhost/tabbar.html",
                "tauri://localhost/board.html",
            ),
            (
                "http://localhost:1420/tabbar.html",
                "http://localhost:1420/board.html",
            ),
            (
                "http://tauri.localhost/tabbar.html",
                "http://tauri.localhost/board.html",
            ),
        ];

        for (tabbar, expected) in cases {
            assert_eq!(entry_of(&parse(tabbar)), Some(parse(expected)), "{tabbar}");
        }
    }
}
