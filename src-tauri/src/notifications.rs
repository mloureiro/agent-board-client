use serde::Serialize;
use tauri::{AppHandle, Manager, State, Webview};

use crate::board_window;
use crate::store::{Server, ServerStore};

const MAX_TITLE_CHARS: usize = 200;
const MAX_BODY_CHARS: usize = 500;
const MAX_TAG_CHARS: usize = 200;

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum NotifyError {
    Payload,
    Server,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Request {
    title: String,
    body: String,
    tag: String,
}

impl Request {
    pub fn new(title: &str, body: &str, tag: &str) -> Result<Self, NotifyError> {
        let title = title.trim();
        let fits = |text: &str, limit: usize| text.chars().count() <= limit;
        let is_valid = !title.is_empty()
            && !tag.is_empty()
            && fits(title, MAX_TITLE_CHARS)
            && fits(body, MAX_BODY_CHARS)
            && fits(tag, MAX_TAG_CHARS);
        if !is_valid {
            return Err(NotifyError::Payload);
        }

        Ok(Self {
            title: title.to_owned(),
            body: body.to_owned(),
            tag: tag.to_owned(),
        })
    }
}

#[tauri::command]
pub fn notify(
    webview: Webview,
    store: State<'_, ServerStore>,
    title: String,
    body: String,
    tag: String,
) -> Result<(), NotifyError> {
    let request = Request::new(&title, &body, &tag)?;
    let server = board_window::server_id(webview.label())
        .and_then(|id| store.find(id).ok())
        .ok_or(NotifyError::Server)?;

    if server.notifications.enabled {
        show(webview.app_handle(), &server, request);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn identify_as(bundle_identifier: &str) {
    if let Err(error) = mac_notification_sys::set_application(bundle_identifier) {
        eprintln!("notifications will not carry the app's identity: {error}");
    }
}

#[cfg(target_os = "macos")]
fn show(app: &AppHandle, server: &Server, request: Request) {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use mac_notification_sys::{Notification, NotificationResponse};

    // mac-notification-sys polls Notification Center twice a second on the main thread for every
    // notification it waits on, so only a few may wait for a click at once.
    const MAX_WAITING_FOR_CLICK: usize = 8;
    static WAITING_FOR_CLICK: AtomicUsize = AtomicUsize::new(0);

    let app = app.clone();
    let server_id = server.id;
    let server_name = server.name.clone();
    std::thread::spawn(move || {
        let waits = WAITING_FOR_CLICK.fetch_add(1, Ordering::SeqCst) < MAX_WAITING_FOR_CLICK;
        let response = Notification::new()
            .title(&request.title)
            .subtitle(&server_name)
            .message(&request.body)
            .wait_for_click(waits)
            .send();
        WAITING_FOR_CLICK.fetch_sub(1, Ordering::SeqCst);

        match response {
            Ok(NotificationResponse::Click) => {
                board_window::activate_notification(&app, server_id, &request.tag);
            }
            Ok(_) => {}
            Err(error) => eprintln!("could not show a notification: {error}"),
        }
    });
}

#[cfg(not(target_os = "macos"))]
fn show(_app: &AppHandle, _server: &Server, _request: Request) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_task_notification_is_accepted_with_its_title_trimmed() {
        let request = Request::new("  Fix the build  ", "Needs you", "task-1").unwrap();

        assert_eq!(request.title, "Fix the build");
        assert_eq!(request.body, "Needs you");
        assert_eq!(request.tag, "task-1");
    }

    #[test]
    fn an_empty_body_is_accepted() {
        assert!(Request::new("Fix the build", "", "task-1").is_ok());
    }

    #[test]
    fn a_blank_title_is_rejected() {
        assert_eq!(
            Request::new("   ", "Needs you", "task-1"),
            Err(NotifyError::Payload)
        );
    }

    #[test]
    fn a_missing_tag_is_rejected() {
        assert_eq!(
            Request::new("Fix the build", "Needs you", ""),
            Err(NotifyError::Payload)
        );
    }

    #[test]
    fn each_field_is_bounded_by_characters_not_bytes() {
        let title = "é".repeat(MAX_TITLE_CHARS);
        let body = "é".repeat(MAX_BODY_CHARS);
        let tag = "é".repeat(MAX_TAG_CHARS);

        assert!(Request::new(&title, &body, &tag).is_ok());
        assert_eq!(
            Request::new(&format!("{title}é"), &body, &tag),
            Err(NotifyError::Payload)
        );
        assert_eq!(
            Request::new(&title, &format!("{body}é"), &tag),
            Err(NotifyError::Payload)
        );
        assert_eq!(
            Request::new(&title, &body, &format!("{tag}é")),
            Err(NotifyError::Payload)
        );
    }
}
