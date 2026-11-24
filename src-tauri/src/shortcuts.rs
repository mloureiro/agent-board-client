use tauri::menu::{
    AboutMetadata, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu, HELP_SUBMENU_ID,
    WINDOW_SUBMENU_ID,
};
use tauri::{AppHandle, Manager, Wry};
use uuid::Uuid;

use crate::zoom::ZoomStep;
use crate::{board_window, list_window, tab_session};

const CLOSE_TAB: &str = "tab-close";
const NEXT_TAB: &str = "tab-next";
const PREVIOUS_TAB: &str = "tab-previous";
const SELECT_PREFIX: &str = "tab-select-";
const ZOOM_IN: &str = "zoom-in";
const ZOOM_OUT: &str = "zoom-out";
const ZOOM_RESET: &str = "zoom-reset";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabAction {
    CloseTab,
    Next,
    Previous,
    Nth(u8),
    Zoom(ZoomStep),
}

pub fn action(id: &str) -> Option<TabAction> {
    match id {
        CLOSE_TAB => Some(TabAction::CloseTab),
        NEXT_TAB => Some(TabAction::Next),
        PREVIOUS_TAB => Some(TabAction::Previous),
        ZOOM_IN => Some(TabAction::Zoom(ZoomStep::In)),
        ZOOM_OUT => Some(TabAction::Zoom(ZoomStep::Out)),
        ZOOM_RESET => Some(TabAction::Zoom(ZoomStep::Reset)),
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
    let about = AboutMetadata {
        name: Some(app.package_info().name.clone()),
        version: Some(app.package_info().version.to_string()),
        ..Default::default()
    };

    let close_tab = item(app, CLOSE_TAB, "Close Tab", "CmdOrCtrl+W")?;
    let file = Submenu::with_items(app, "File", true, &[&close_tab])?;
    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;
    let window = Submenu::with_id_and_items(
        app,
        WINDOW_SUBMENU_ID,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, None)?,
        ],
    )?;
    let help = Submenu::with_id(app, HELP_SUBMENU_ID, "Help", true)?;

    let menu = Menu::new(app)?;
    #[cfg(target_os = "macos")]
    {
        menu.append(&Submenu::with_items(
            app,
            app.package_info().name.clone(),
            true,
            &[
                &PredefinedMenuItem::about(app, None, Some(about))?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::services(app, None)?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::hide(app, None)?,
                &PredefinedMenuItem::hide_others(app, None)?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::quit(app, None)?,
            ],
        )?)?;
    }
    #[cfg(not(target_os = "macos"))]
    {
        file.append(&PredefinedMenuItem::quit(app, None)?)?;
        help.append(&PredefinedMenuItem::about(app, None, Some(about))?)?;
    }
    menu.append(&file)?;
    menu.append(&edit)?;
    let view = Submenu::with_items(
        app,
        "View",
        true,
        &[
            &item(app, ZOOM_RESET, "Actual Size", "CmdOrCtrl+0")?,
            &item(app, ZOOM_IN, "Zoom In", "CmdOrCtrl+=")?,
            &item(app, ZOOM_OUT, "Zoom Out", "CmdOrCtrl+-")?,
        ],
    )?;
    #[cfg(target_os = "macos")]
    {
        view.append(&PredefinedMenuItem::separator(app)?)?;
        view.append(&PredefinedMenuItem::fullscreen(app, None)?)?;
    }
    menu.append(&view)?;
    menu.append(&window)?;
    menu.append(&help)?;

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
    let id = Uuid::parse_str(label.strip_prefix("server-")?).ok()?;
    (board_window::window_label(id) == label).then_some(id)
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

    if window.label() == list_window::LABEL {
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
        TabAction::Zoom(step) => session.step_zoom(step),
    });
    if let Err(error) = result {
        eprintln!("server {server}: the tab shortcut could not be applied: {error:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_each_menu_item_id_to_its_action() {
        let mut cases = vec![
            ("tab-close", Some(TabAction::CloseTab)),
            ("tab-next", Some(TabAction::Next)),
            ("tab-previous", Some(TabAction::Previous)),
            ("zoom-in", Some(TabAction::Zoom(ZoomStep::In))),
            ("zoom-out", Some(TabAction::Zoom(ZoomStep::Out))),
            ("zoom-reset", Some(TabAction::Zoom(ZoomStep::Reset))),
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
        assert_eq!(server_of(&format!("server-{}", id.simple())), None);
    }
}
