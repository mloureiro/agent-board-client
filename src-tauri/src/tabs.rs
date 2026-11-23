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

use serde::Serialize;

use crate::tab_state::{SavedTab, SavedTabs};
use crate::zoom::{Zoom, ZoomStep};

pub const HOME: u64 = 0;
pub const MAX_TABS: usize = 20;
const HOME_LABEL: &str = "Board";
const BOARD_TITLE: &str = "Agent Board";
const TITLE_SUFFIX: &str = " · Agent Board";
const MAX_LABEL_CHARS: usize = 200;

pub fn label_from_title(title: &str) -> Option<String> {
    if title == BOARD_TITLE {
        return None;
    }
    let name = title.strip_suffix(TITLE_SUFFIX).unwrap_or(title);
    let label: String = name.trim().chars().take(MAX_LABEL_CHARS).collect();
    let label = label.trim_end();
    (!label.is_empty()).then(|| label.to_owned())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tab {
    pub id: u64,
    pub path: String,
    pub page: Page,
    pub label: String,
    pub loaded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Load(u64, String),
    Show(u64),
    Hide(u64),
    Destroy(u64),
    Zoom(Zoom),
    Changed,
    LimitReached,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TabView {
    pub id: u64,
    pub label: String,
    pub home: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TabsView {
    pub selected: u64,
    pub tabs: Vec<TabView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabSession {
    tabs: Vec<Tab>,
    selected: u64,
    next_id: u64,
    zoom: Zoom,
}

impl Default for TabSession {
    fn default() -> Self {
        Self {
            tabs: Vec::new(),
            selected: HOME,
            next_id: 1,
            zoom: Zoom::default(),
        }
    }
}

impl TabSession {
    /// Expects `saved` to have passed `tab_state::restore`. Every tab starts unloaded and 🏠 is
    /// selected; selecting the saved tab afterwards loads it. Saved tab `n` gets id `n`.
    pub fn restored(saved: &SavedTabs) -> Self {
        let tabs: Vec<Tab> = saved
            .tabs
            .iter()
            .zip(1..)
            .filter_map(|(entry, id)| match page_of(&entry.path) {
                Route::Page(page) => Some(Tab {
                    id,
                    path: entry.path.clone(),
                    page,
                    label: entry.label.clone(),
                    loaded: false,
                }),
                _ => None,
            })
            .collect();
        Self {
            next_id: tabs.len() as u64 + 1,
            tabs,
            selected: HOME,
            zoom: Zoom::default(),
        }
    }

    pub fn saved(&self) -> SavedTabs {
        let selected = self
            .tabs
            .iter()
            .position(|tab| tab.id == self.selected)
            .map_or(HOME, |index| index as u64 + 1);
        SavedTabs {
            selected,
            tabs: self
                .tabs
                .iter()
                .map(|tab| SavedTab {
                    path: tab.path.clone(),
                    label: tab.label.clone(),
                })
                .collect(),
        }
    }

    pub fn open(&mut self, path: &str) -> Vec<Effect> {
        let page = match page_of(path) {
            Route::Home => return self.select(HOME),
            Route::Other => return Vec::new(),
            Route::Page(page) => page,
        };

        if let Some(existing) = self.tabs.iter().find(|tab| tab.page == page) {
            let id = existing.id;
            return self.select(id);
        }

        if self.tabs.len() >= MAX_TABS {
            return vec![Effect::LimitReached];
        }

        let id = self.next_id;
        self.next_id += 1;
        self.tabs.push(Tab {
            id,
            path: path.to_owned(),
            label: page.path_label().to_owned(),
            page,
            loaded: true,
        });
        let previous = std::mem::replace(&mut self.selected, id);
        vec![
            Effect::Load(id, path.to_owned()),
            Effect::Hide(previous),
            Effect::Show(id),
            Effect::Changed,
        ]
    }

    pub fn route(&mut self, tab: u64, path: &str) -> Vec<Effect> {
        let Some(index) = self.tabs.iter().position(|candidate| candidate.id == tab) else {
            return Vec::new();
        };

        match page_of(path) {
            Route::Home => {
                self.tabs.remove(index);
                let previous = std::mem::replace(&mut self.selected, HOME);
                let mut effects = vec![Effect::Destroy(tab)];
                if previous != HOME && previous != tab {
                    effects.push(Effect::Hide(previous));
                }
                if previous != HOME {
                    effects.push(Effect::Show(HOME));
                }
                effects.push(Effect::Changed);
                effects
            }
            Route::Page(page) => {
                let current = &mut self.tabs[index];
                if current.path == path && current.page == page {
                    return Vec::new();
                }
                if current.page != page {
                    page.path_label().clone_into(&mut current.label);
                }
                current.path = path.to_owned();
                current.page = page;
                vec![Effect::Changed]
            }
            Route::Other => Vec::new(),
        }
    }

    pub fn title(&mut self, tab: u64, title: &str) -> Vec<Effect> {
        let Some(current) = self.tabs.iter_mut().find(|candidate| candidate.id == tab) else {
            return Vec::new();
        };
        let label = label_from_title(title).unwrap_or_else(|| current.page.path_label().to_owned());
        if current.label == label {
            return Vec::new();
        }
        current.label = label;
        vec![Effect::Changed]
    }

    pub fn select(&mut self, tab: u64) -> Vec<Effect> {
        if tab == self.selected {
            return Vec::new();
        }

        let mut effects = Vec::new();
        if tab != HOME {
            let Some(target) = self.tabs.iter_mut().find(|candidate| candidate.id == tab) else {
                return Vec::new();
            };
            if !target.loaded {
                target.loaded = true;
                effects.push(Effect::Load(tab, target.path.clone()));
            }
        }

        effects.push(Effect::Hide(self.selected));
        effects.push(Effect::Show(tab));
        effects.push(Effect::Changed);
        self.selected = tab;
        effects
    }

    pub fn activate(&mut self, tab: u64) -> Vec<Effect> {
        if tab == self.selected {
            return vec![Effect::Show(tab)];
        }

        self.select(tab)
    }

    pub fn close(&mut self, tab: u64) -> Vec<Effect> {
        let Some(index) = self.tabs.iter().position(|candidate| candidate.id == tab) else {
            return Vec::new();
        };

        self.tabs.remove(index);
        let mut effects = vec![Effect::Destroy(tab)];
        if self.selected == tab {
            let neighbour = if index < self.tabs.len() {
                Some(index)
            } else {
                index.checked_sub(1)
            };
            self.selected = match neighbour.map(|n| &mut self.tabs[n]) {
                Some(next) => {
                    if !next.loaded {
                        next.loaded = true;
                        effects.push(Effect::Load(next.id, next.path.clone()));
                    }
                    next.id
                }
                None => HOME,
            };
            effects.push(Effect::Show(self.selected));
        }
        effects.push(Effect::Changed);
        effects
    }

    pub fn close_selected(&mut self) -> Vec<Effect> {
        self.close(self.selected)
    }

    pub fn select_nth(&mut self, n: u8) -> Vec<Effect> {
        match usize::from(n) {
            0 => Vec::new(),
            1 => self.select(HOME),
            position => match self.tabs.get(position - 2) {
                Some(tab) => {
                    let id = tab.id;
                    self.select(id)
                }
                None => Vec::new(),
            },
        }
    }

    pub fn cycle(&mut self, next: bool) -> Vec<Effect> {
        let order: Vec<u64> = std::iter::once(HOME)
            .chain(self.tabs.iter().map(|tab| tab.id))
            .collect();
        let current = order
            .iter()
            .position(|id| *id == self.selected)
            .unwrap_or(0);
        let target = if next {
            (current + 1) % order.len()
        } else {
            (current + order.len() - 1) % order.len()
        };
        self.select(order[target])
    }

    pub fn zoom(&self) -> Zoom {
        self.zoom
    }

    pub fn step_zoom(&mut self, step: ZoomStep) -> Vec<Effect> {
        let zoom = self.zoom.step(step);
        if zoom == self.zoom {
            return Vec::new();
        }
        self.zoom = zoom;
        vec![Effect::Zoom(zoom)]
    }

    pub fn view(&self) -> TabsView {
        let home = TabView {
            id: HOME,
            label: HOME_LABEL.to_owned(),
            home: true,
        };
        let others = self.tabs.iter().map(|tab| TabView {
            id: tab.id,
            label: tab.label.clone(),
            home: false,
        });
        TabsView {
            selected: self.selected,
            tabs: std::iter::once(home).chain(others).collect(),
        }
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

    type ModelCase<'a> = (&'a str, TabSession, Op, Vec<Effect>, u64, Vec<&'a str>);

    enum Op {
        Open(&'static str),
        Route(u64, &'static str),
        Select(u64),
        Close(u64),
        CloseSelected,
        SelectNth(u8),
        Cycle(bool),
        Title(u64, &'static str),
    }

    fn session(paths: &[&str], selected: u64, unloaded: &[u64]) -> TabSession {
        let tabs: Vec<Tab> = paths
            .iter()
            .zip(1..)
            .map(|(path, id)| {
                let Route::Page(page) = page_of(path) else {
                    panic!("{path} is not a page");
                };
                Tab {
                    id,
                    path: (*path).to_owned(),
                    label: page.path_label().to_owned(),
                    page,
                    loaded: !unloaded.contains(&id),
                }
            })
            .collect();
        TabSession {
            next_id: tabs.len() as u64 + 1,
            tabs,
            selected,
            zoom: Zoom::default(),
        }
    }

    fn paths(session: &TabSession) -> Vec<&str> {
        session.tabs.iter().map(|tab| tab.path.as_str()).collect()
    }

    #[test]
    fn applies_each_operation_to_the_session_and_reports_its_effects() {
        use Effect::*;

        let full: Vec<String> = (0..MAX_TABS).map(|n| format!("/t/{n}")).collect();
        let full: Vec<&str> = full.iter().map(String::as_str).collect();

        let cases: Vec<ModelCase> = vec![
            (
                "open a page",
                session(&[], 0, &[]),
                Op::Open("/backlog"),
                vec![Load(1, "/backlog".into()), Hide(0), Show(1), Changed],
                1,
                vec!["/backlog"],
            ),
            (
                "open an open page again",
                session(&["/backlog"], 0, &[]),
                Op::Open("/backlog"),
                vec![Hide(0), Show(1), Changed],
                1,
                vec!["/backlog"],
            ),
            (
                "open a task with another query",
                session(&["/t/1"], 0, &[]),
                Op::Open("/t/1?transcript=1"),
                vec![Hide(0), Show(1), Changed],
                1,
                vec!["/t/1"],
            ),
            (
                "open a settings subpage",
                session(&["/settings"], 0, &[]),
                Op::Open("/settings/agents"),
                vec![Hide(0), Show(1), Changed],
                1,
                vec!["/settings"],
            ),
            (
                "open home",
                session(&["/backlog"], 1, &[]),
                Op::Open("/"),
                vec![Hide(1), Show(0), Changed],
                0,
                vec!["/backlog"],
            ),
            (
                "open another path",
                session(&[], 0, &[]),
                Op::Open("/login/oidc"),
                vec![],
                0,
                vec![],
            ),
            (
                "open at the cap",
                session(&full, 0, &[]),
                Op::Open("/backlog"),
                vec![LimitReached],
                0,
                full.clone(),
            ),
            (
                "route a tab to home",
                session(&["/backlog", "/closed"], 1, &[]),
                Op::Route(1, "/"),
                vec![Destroy(1), Show(0), Changed],
                0,
                vec!["/closed"],
            ),
            (
                "route a hidden tab to home",
                session(&["/backlog", "/closed"], 2, &[]),
                Op::Route(1, "/"),
                vec![Destroy(1), Hide(2), Show(0), Changed],
                0,
                vec!["/closed"],
            ),
            (
                "route a tab to another page",
                session(&["/t/1"], 1, &[]),
                Op::Route(1, "/backlog"),
                vec![Changed],
                1,
                vec!["/backlog"],
            ),
            (
                "route a tab to where it already is",
                session(&["/t/1"], 1, &[]),
                Op::Route(1, "/t/1"),
                vec![],
                1,
                vec!["/t/1"],
            ),
            (
                "route a tab to another path",
                session(&["/t/1"], 1, &[]),
                Op::Route(1, "/login/oidc"),
                vec![],
                1,
                vec!["/t/1"],
            ),
            (
                "route home",
                session(&["/t/1"], 0, &[]),
                Op::Route(0, "/backlog"),
                vec![],
                0,
                vec!["/t/1"],
            ),
            (
                "close home",
                session(&["/t/1"], 0, &[]),
                Op::Close(0),
                vec![],
                0,
                vec!["/t/1"],
            ),
            (
                "close the selected middle tab",
                session(&["/backlog", "/closed", "/schedules"], 2, &[]),
                Op::Close(2),
                vec![Destroy(2), Show(3), Changed],
                3,
                vec!["/backlog", "/schedules"],
            ),
            (
                "close the selected last tab",
                session(&["/backlog", "/closed", "/schedules"], 3, &[]),
                Op::Close(3),
                vec![Destroy(3), Show(2), Changed],
                2,
                vec!["/backlog", "/closed"],
            ),
            (
                "close the only tab",
                session(&["/backlog"], 1, &[]),
                Op::Close(1),
                vec![Destroy(1), Show(0), Changed],
                0,
                vec![],
            ),
            (
                "close a tab that is not selected",
                session(&["/backlog", "/closed"], 2, &[]),
                Op::Close(1),
                vec![Destroy(1), Changed],
                2,
                vec!["/closed"],
            ),
            (
                "close the selected tab before an unloaded one",
                session(&["/backlog", "/closed"], 1, &[2]),
                Op::Close(1),
                vec![Destroy(1), Load(2, "/closed".into()), Show(2), Changed],
                2,
                vec!["/closed"],
            ),
            (
                "select an unloaded tab",
                session(&["/backlog", "/closed"], 0, &[2]),
                Op::Select(2),
                vec![Load(2, "/closed".into()), Hide(0), Show(2), Changed],
                2,
                vec!["/backlog", "/closed"],
            ),
            (
                "select the selected tab",
                session(&["/backlog"], 1, &[]),
                Op::Select(1),
                vec![],
                1,
                vec!["/backlog"],
            ),
            (
                "select a tab that does not exist",
                session(&["/backlog"], 0, &[]),
                Op::Select(9),
                vec![],
                0,
                vec!["/backlog"],
            ),
            (
                "close the selected tab from the keyboard",
                session(&["/backlog", "/closed"], 1, &[]),
                Op::CloseSelected,
                vec![Destroy(1), Show(2), Changed],
                2,
                vec!["/closed"],
            ),
            (
                "close the selected tab from the keyboard with home selected",
                session(&["/backlog"], 0, &[]),
                Op::CloseSelected,
                vec![],
                0,
                vec!["/backlog"],
            ),
            (
                "select the first position",
                session(&["/backlog"], 1, &[]),
                Op::SelectNth(1),
                vec![Hide(1), Show(0), Changed],
                0,
                vec!["/backlog"],
            ),
            (
                "select the third position",
                session(&["/backlog", "/closed"], 0, &[]),
                Op::SelectNth(3),
                vec![Hide(0), Show(2), Changed],
                2,
                vec!["/backlog", "/closed"],
            ),
            (
                "select a position past the last tab",
                session(&["/backlog", "/closed"], 0, &[]),
                Op::SelectNth(9),
                vec![],
                0,
                vec!["/backlog", "/closed"],
            ),
            (
                "cycle forward from the last tab",
                session(&["/backlog", "/closed"], 2, &[]),
                Op::Cycle(true),
                vec![Hide(2), Show(0), Changed],
                0,
                vec!["/backlog", "/closed"],
            ),
            (
                "cycle back from home",
                session(&["/backlog", "/closed"], 0, &[]),
                Op::Cycle(false),
                vec![Hide(0), Show(2), Changed],
                2,
                vec!["/backlog", "/closed"],
            ),
            (
                "title a tab that does not exist",
                session(&["/t/1"], 1, &[]),
                Op::Title(9, "Fix login"),
                vec![],
                1,
                vec!["/t/1"],
            ),
            (
                "title home",
                session(&["/t/1"], 0, &[]),
                Op::Title(0, "Fix login"),
                vec![],
                0,
                vec!["/t/1"],
            ),
            (
                "title a tab with the static board title",
                session(&["/t/1"], 1, &[]),
                Op::Title(1, "Agent Board"),
                vec![],
                1,
                vec!["/t/1"],
            ),
            (
                "cycle with only home",
                session(&[], 0, &[]),
                Op::Cycle(true),
                vec![],
                0,
                vec![],
            ),
        ];

        for (name, mut session, op, effects, selected, remaining) in cases {
            let actual = match op {
                Op::Open(path) => session.open(path),
                Op::Route(tab, path) => session.route(tab, path),
                Op::Select(tab) => session.select(tab),
                Op::Close(tab) => session.close(tab),
                Op::CloseSelected => session.close_selected(),
                Op::SelectNth(n) => session.select_nth(n),
                Op::Cycle(next) => session.cycle(next),
                Op::Title(tab, title) => session.title(tab, title),
            };

            assert_eq!(actual, effects, "{name}: effects");
            assert_eq!(session.selected, selected, "{name}: selected");
            assert_eq!(paths(&session), remaining, "{name}: tabs");
        }
    }

    fn label_of(session: &TabSession, tab: u64) -> &str {
        &session.tabs.iter().find(|t| t.id == tab).unwrap().label
    }

    #[test]
    fn reports_a_title_once_and_falls_back_to_the_path_label() {
        let mut session = session(&["/t/1"], 1, &[]);

        assert_eq!(session.title(1, "Fix login"), vec![Effect::Changed]);
        assert_eq!(session.title(1, "Fix login · Agent Board"), vec![]);
        assert_eq!(label_of(&session, 1), "Fix login");

        assert_eq!(session.title(1, "Agent Board"), vec![Effect::Changed]);
        assert_eq!(label_of(&session, 1), "Task");
    }

    #[test]
    fn replaces_a_title_with_the_path_label_when_the_tab_goes_to_another_page() {
        let mut session = session(&["/t/1"], 1, &[]);
        session.title(1, "Fix login");

        session.route(1, "/backlog");

        assert_eq!(label_of(&session, 1), "Backlog");
    }

    #[test]
    fn drops_a_title_when_the_tab_moves_to_another_task() {
        let mut session = session(&["/t/1"], 1, &[]);
        session.title(1, "Fix login");

        assert_eq!(session.route(1, "/t/2"), vec![Effect::Changed]);
        assert_eq!(label_of(&session, 1), "Task");
    }

    #[test]
    fn keeps_a_title_when_the_same_page_finishes_loading() {
        let mut session = session(&["/t/1"], 1, &[]);
        session.title(1, "Fix login");

        assert_eq!(session.route(1, "/t/1"), vec![]);
        assert_eq!(session.route(1, "/t/1?transcript=1"), vec![Effect::Changed]);
        assert_eq!(label_of(&session, 1), "Fix login");
    }

    #[test]
    fn derives_a_label_from_a_page_title() {
        let long = "x".repeat(500);
        let cases: Vec<(&str, Option<String>)> = vec![
            ("Agent Board", None),
            ("", None),
            ("Settings · Agent Board", Some("Settings".into())),
            (
                "Fix the login · bug · Agent Board",
                Some("Fix the login · bug".into()),
            ),
            ("  Spaced  · Agent Board", Some("Spaced".into())),
            (" · Agent Board", None),
            ("   ", None),
            ("Some other site", Some("Some other site".into())),
            (&long, Some("x".repeat(200))),
        ];
        for (title, expected) in cases {
            assert_eq!(label_from_title(title), expected, "{title:?}");
        }
    }

    #[test]
    fn never_reuses_a_tab_id() {
        let mut session = TabSession::default();
        session.open("/backlog");
        session.close(1);

        assert_eq!(
            session.open("/backlog")[0],
            Effect::Load(2, "/backlog".into())
        );
    }

    #[test]
    fn shows_home_first_and_marks_only_it() {
        let view = session(&["/t/1"], 1, &[]).view();

        assert_eq!(view.selected, 1);
        assert_eq!(
            view.tabs,
            vec![
                TabView {
                    id: 0,
                    label: "Board".into(),
                    home: true
                },
                TabView {
                    id: 1,
                    label: "Task".into(),
                    home: false
                },
            ]
        );
    }

    #[test]
    fn saves_the_selection_by_position_and_restores_it_lazily() {
        let mut open = session(&["/backlog", "/closed", "/settings"], HOME, &[]);
        open.close(1);
        open.select(3);

        let saved = open.saved();
        assert_eq!(saved.selected, 2);
        assert_eq!(
            saved
                .tabs
                .iter()
                .map(|tab| tab.path.as_str())
                .collect::<Vec<_>>(),
            ["/closed", "/settings"]
        );

        let mut restored = TabSession::restored(&saved);
        assert_eq!(restored.view().selected, HOME);
        assert_eq!(
            restored.select(saved.selected),
            vec![
                Effect::Load(2, "/settings".to_owned()),
                Effect::Hide(HOME),
                Effect::Show(2),
                Effect::Changed,
            ]
        );
        assert_eq!(restored.saved(), saved);
        assert_eq!(
            restored.open("/t/9").first(),
            Some(&Effect::Load(3, "/t/9".to_owned()))
        );
    }

    #[test]
    fn activating_the_selected_tab_shows_it_without_changing_selection() {
        let mut session = TabSession::default();
        session.open("/t/1");
        let selected = session.view().selected;

        assert_eq!(session.activate(selected), vec![Effect::Show(selected)]);
        assert_eq!(session.view().selected, selected);
    }

    #[test]
    fn activating_another_tab_selects_it() {
        let mut activated = TabSession::default();
        activated.open("/t/1");
        activated.open("/t/2");
        let mut selected = TabSession::default();
        selected.open("/t/1");
        selected.open("/t/2");

        assert_eq!(activated.activate(HOME), selected.select(HOME));
    }

    #[test]
    fn zooms_every_tab_and_stops_at_actual_size() {
        let mut model = session(&["/settings"], 1, &[]);
        let zoomed = Zoom::default().step(ZoomStep::In);

        assert_eq!(model.step_zoom(ZoomStep::In), vec![Effect::Zoom(zoomed)]);
        assert_eq!(model.zoom(), zoomed);

        assert_eq!(
            model.step_zoom(ZoomStep::Reset),
            vec![Effect::Zoom(Zoom::default())]
        );
        assert_eq!(model.step_zoom(ZoomStep::Reset), vec![]);
    }
}
