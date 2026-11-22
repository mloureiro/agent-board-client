use tauri::menu::{
    Menu, MenuEvent, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu, WINDOW_SUBMENU_ID,
};
use tauri::{AppHandle, Manager, Wry};
use uuid::Uuid;

use crate::tab_session;

const CLOSE_TAB: &str = "tab-close";
const NEXT_TAB: &str = "tab-next";
const PREVIOUS_TAB: &str = "tab-previous";
const SELECT_PREFIX: &str = "tab-select-";
const LIST_LABEL: &str = "main";
const SERVER_LABEL_PREFIX: &str = "server-";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabAction {
    CloseTab,
    Next,
    Previous,
    Nth(u8),
}

pub fn action(id: &str) -> Option<TabAction> {
    match id {
        CLOSE_TAB => Some(TabAction::CloseTab),
        NEXT_TAB => Some(TabAction::Next),
        PREVIOUS_TAB => Some(TabAction::Previous),
        _ => {
            let n: u8 = id.strip_prefix(SELECT_PREFIX)?.parse().ok()?;
            (1..=9).contains(&n).then_some(TabAction::Nth(n))
        }
    }
}

fn item(app: &AppHandle, id: &str, text: &str, accelerator: &str) -> tauri::Result<MenuItem<Wry>> {
    MenuItem::with_id(app, id, text, true, Some(accelerator))
}

pub fn menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let menu = Menu::default(app)?;

    let mut close_tab_home: Option<Submenu<Wry>> = None;
    for entry in menu.items()? {
        let MenuItemKind::Submenu(submenu) = entry else {
            continue;
        };
        for child in submenu.items()? {
            if matches!(&child, MenuItemKind::Predefined(predefined) if predefined.text()? == "Close Window")
            {
                submenu.remove(&child)?;
                if close_tab_home.is_none() || submenu.text()? == "File" {
                    close_tab_home = Some(submenu.clone());
                }
            }
        }
    }
    let close_tab = item(app, CLOSE_TAB, "Close Tab", "CmdOrCtrl+W")?;
    match close_tab_home {
        Some(home) => home.insert(&close_tab, 0)?,
        None => {
            if let Some(MenuItemKind::Submenu(window)) = menu.get(WINDOW_SUBMENU_ID) {
                window.append(&close_tab)?;
            }
        }
    }

    let tabs = Submenu::with_id(app, "tabs", "Tabs", true)?;
    tabs.append(&item(app, NEXT_TAB, "Next Tab", "Ctrl+Tab")?)?;
    tabs.append(&item(app, PREVIOUS_TAB, "Previous Tab", "Ctrl+Shift+Tab")?)?;
    tabs.append(&PredefinedMenuItem::separator(app)?)?;
    for n in 1..=9 {
        tabs.append(&item(
            app,
            &format!("{SELECT_PREFIX}{n}"),
            &format!("Select Tab {n}"),
            &format!("CmdOrCtrl+{n}"),
        )?)?;
    }
    menu.append(&tabs)?;
    Ok(menu)
}

fn server_of(label: &str) -> Option<Uuid> {
    Uuid::parse_str(label.strip_prefix(SERVER_LABEL_PREFIX)?).ok()
}

pub fn handle(app: &AppHandle, event: MenuEvent) {
    let Some(action) = action(event.id().as_ref()) else {
        return;
    };
    let Some(window) = app
        .windows()
        .into_values()
        .find(|window| window.is_focused().unwrap_or(false))
    else {
        return;
    };

    if window.label() == LIST_LABEL {
        if action == TabAction::CloseTab {
            if let Err(error) = window.close() {
                eprintln!("the server list could not be closed: {error}");
            }
        }
        return;
    }

    let Some(server) = server_of(window.label()) else {
        return;
    };
    let result = tab_session::apply(app, server, |session| match action {
        TabAction::CloseTab => session.close_selected(),
        TabAction::Next => session.cycle(true),
        TabAction::Previous => session.cycle(false),
        TabAction::Nth(n) => session.select_nth(n),
    });
    if let Err(error) = result {
        eprintln!("server {server}: the tab shortcut could not be applied: {error:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board_window;

    #[test]
    fn maps_each_menu_item_id_to_its_action() {
        let mut cases = vec![
            ("tab-close", Some(TabAction::CloseTab)),
            ("tab-next", Some(TabAction::Next)),
            ("tab-previous", Some(TabAction::Previous)),
            ("tab-select-0", None),
            ("tab-select-10", None),
            ("tab-select-x", None),
            ("quit", None),
            ("", None),
        ];
        for n in 1..=9u8 {
            cases.push((
                Box::leak(format!("tab-select-{n}").into_boxed_str()),
                Some(TabAction::Nth(n)),
            ));
        }
        for (id, expected) in cases {
            assert_eq!(action(id), expected, "{id}");
        }
    }

    #[test]
    fn finds_the_server_behind_a_window_label() {
        let id = Uuid::new_v4();
        assert_eq!(server_of(&board_window::window_label(id)), Some(id));
        assert_eq!(server_of("main"), None);
        assert_eq!(server_of("server-nope"), None);
    }
}
