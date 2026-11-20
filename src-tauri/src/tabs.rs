#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Page {
    Backlog,
    Schedules,
    Closed,
    Settings,
    Task(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Home,
    Page(Page),
    Other,
}

impl Page {
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "read by the tab bar from iteration 3")
    )]
    pub fn path_label(&self) -> &'static str {
        match self {
            Page::Backlog => "Backlog",
            Page::Schedules => "Schedules",
            Page::Closed => "Closed",
            Page::Settings => "Settings",
            Page::Task(_) => "Task",
        }
    }
}

pub fn page_of(path: &str) -> Route {
    let pathname = path.split(['?', '#']).next().unwrap_or_default();
    let pathname = match pathname.strip_suffix('/') {
        Some(rest) if !rest.is_empty() => rest,
        _ => pathname,
    };

    match pathname {
        "/" => Route::Home,
        "/backlog" => Route::Page(Page::Backlog),
        "/schedules" => Route::Page(Page::Schedules),
        "/closed" => Route::Page(Page::Closed),
        _ if pathname == "/settings" || pathname.starts_with("/settings/") => {
            Route::Page(Page::Settings)
        }
        _ => match pathname.strip_prefix("/t/") {
            Some(id) if !id.is_empty() && !id.contains('/') => {
                Route::Page(Page::Task(id.to_owned()))
            }
            _ => Route::Other,
        },
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    struct Case {
        path: String,
        route: String,
        page: Option<String>,
        label: Option<String>,
    }

    fn page_key(page: &Page) -> String {
        match page {
            Page::Backlog => "backlog".to_owned(),
            Page::Schedules => "schedules".to_owned(),
            Page::Closed => "closed".to_owned(),
            Page::Settings => "settings".to_owned(),
            Page::Task(id) => format!("task:{id}"),
        }
    }

    #[test]
    fn obeys_the_case_table_shared_with_the_page_script() {
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("../../fixtures/page-cases.json")).unwrap();

        for case in cases {
            let (route, page, label) = match page_of(&case.path) {
                Route::Home => ("home", None, None),
                Route::Page(page) => (
                    "page",
                    Some(page_key(&page)),
                    Some(page.path_label().to_owned()),
                ),
                Route::Other => ("other", None, None),
            };

            assert_eq!(route, case.route, "route for {:?}", case.path);
            assert_eq!(page, case.page, "page for {:?}", case.path);
            assert_eq!(label, case.label, "label for {:?}", case.path);
        }
    }
}
