use std::sync::LazyLock;

use serde::Deserialize;

use crate::tabs::{Page, Route};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PageKind {
    Home,
    Backlog,
    Closed,
    Schedules,
    Task,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabKind {
    Board,
    Native,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct NativePages(Vec<PageKind>);

impl PageKind {
    pub fn of(route: &Route) -> Option<PageKind> {
        match route {
            Route::Home => Some(PageKind::Home),
            Route::Page(Page::Backlog) => Some(PageKind::Backlog),
            Route::Page(Page::Schedules) => Some(PageKind::Schedules),
            Route::Page(Page::Closed) => Some(PageKind::Closed),
            Route::Page(Page::Settings) => Some(PageKind::Settings),
            Route::Page(Page::Task(_)) => Some(PageKind::Task),
            Route::Other => None,
        }
    }
}

impl NativePages {
    pub fn bundled() -> &'static NativePages {
        static BUNDLED: LazyLock<NativePages> = LazyLock::new(|| {
            serde_json::from_str(include_str!("../../fixtures/native-pages.json"))
                .expect("fixtures/native-pages.json lists page kinds")
        });
        &BUNDLED
    }

    pub fn kind_of(&self, route: &Route, board_only: bool) -> TabKind {
        match PageKind::of(route) {
            Some(kind) if !board_only && self.0.contains(&kind) => TabKind::Native,
            _ => TabKind::Board,
        }
    }

    #[cfg(test)]
    pub fn listing(kinds: &[PageKind]) -> Self {
        Self(kinds.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;
    use crate::tabs::page_of;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Case {
        pages: NativePages,
        board_only: bool,
        path: String,
        kind: String,
    }

    #[test]
    fn parses_the_bundled_table() {
        let parsed: NativePages =
            serde_json::from_str(include_str!("../../fixtures/native-pages.json")).unwrap();

        assert_eq!(NativePages::bundled(), &parsed);
    }

    #[test]
    fn refuses_a_table_with_an_unknown_kind_or_shape() {
        for input in [r#"["home","nope"]"#, "{}", r#""home""#] {
            assert!(
                serde_json::from_str::<NativePages>(input).is_err(),
                "{input}"
            );
        }
    }

    #[test]
    fn obeys_the_case_table_shared_with_the_native_app() {
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("../../fixtures/tab-kind-cases.json")).unwrap();

        for case in cases {
            let kind = case.pages.kind_of(&page_of(&case.path), case.board_only);
            let expected = match case.kind.as_str() {
                "native" => TabKind::Native,
                "board" => TabKind::Board,
                other => panic!("unknown kind {other}"),
            };

            assert_eq!(
                kind, expected,
                "{} with {:?}, board only {}",
                case.path, case.pages, case.board_only
            );
        }
    }
}
