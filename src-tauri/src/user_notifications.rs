use tauri::AppHandle;
use uuid::Uuid;

use crate::notify::Notice;

#[cfg(target_os = "macos")]
mod native {
    use std::sync::atomic::{AtomicBool, Ordering};

    use block2::{DynBlock, RcBlock};
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, Bool, ProtocolObject};
    use objc2::{define_class, msg_send, AnyThread, DefinedClass};
    use objc2_foundation::{NSBundle, NSDictionary, NSError, NSObject, NSObjectProtocol, NSString};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNMutableNotificationContent, UNNotification,
        UNNotificationPresentationOptions, UNNotificationRequest, UNNotificationResponse,
        UNNotificationSound, UNUserNotificationCenter, UNUserNotificationCenterDelegate,
    };
    use tauri::AppHandle;
    use uuid::Uuid;

    use crate::board_window;
    use crate::notify::{self, Notice};

    static NOT_ALLOWED_LOGGED: AtomicBool = AtomicBool::new(false);

    define_class!(
        #[unsafe(super(NSObject))]
        #[name = "AgentBoardNotificationDelegate"]
        #[ivars = AppHandle]
        struct Delegate;

        unsafe impl NSObjectProtocol for Delegate {}

        unsafe impl UNUserNotificationCenterDelegate for Delegate {
            #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
            fn will_present(
                &self,
                _center: &UNUserNotificationCenter,
                _notification: &UNNotification,
                completion_handler: &DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
            ) {
                completion_handler.call((UNNotificationPresentationOptions::Banner
                    | UNNotificationPresentationOptions::List
                    | UNNotificationPresentationOptions::Sound,));
            }

            #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
            fn did_receive(
                &self,
                _center: &UNUserNotificationCenter,
                response: &UNNotificationResponse,
                completion_handler: &DynBlock<dyn Fn()>,
            ) {
                let info = response.notification().request().content().userInfo();
                let server = string_in(&info, "server");
                let path = string_in(&info, "path");
                completion_handler.call(());

                let app = self.ivars().clone();
                tauri::async_runtime::spawn(async move {
                    board_window::open_from_notification(&app, &server, &path);
                });
            }
        }
    );

    fn string_in(info: &NSDictionary<AnyObject, AnyObject>, key: &str) -> String {
        let key = NSString::from_str(key);
        info.objectForKey(&key)
            .and_then(|value| value.downcast::<NSString>().ok())
            .map(|value| value.to_string())
            .unwrap_or_default()
    }

    impl Delegate {
        fn new(app: AppHandle) -> Retained<Self> {
            let this = Self::alloc().set_ivars(app);
            // SAFETY: `NSObject`'s `init` takes no arguments and returns the initialised receiver.
            unsafe { msg_send![super(this), init] }
        }
    }

    fn has_bundle_identifier() -> bool {
        NSBundle::mainBundle().bundleIdentifier().is_some()
    }

    pub fn install(app: &AppHandle) {
        if !has_bundle_identifier() {
            eprintln!("notifications are off: the app has no bundle identifier");
            return;
        }

        let delegate = Delegate::new(app.clone());
        UNUserNotificationCenter::currentNotificationCenter()
            .setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        // The center holds its delegate weakly, so it must outlive the process.
        std::mem::forget(delegate);
    }

    fn content(
        server: Uuid,
        server_name: &str,
        notice: &Notice,
    ) -> Retained<UNMutableNotificationContent> {
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(&notice.title));
        content.setSubtitle(&NSString::from_str(server_name));
        content.setBody(&NSString::from_str(&notice.body));
        content.setThreadIdentifier(&NSString::from_str(&server.to_string()));
        content.setSound(Some(&UNNotificationSound::defaultSound()));

        let server_key = NSString::from_str("server");
        let path_key = NSString::from_str("path");
        let server_value = NSString::from_str(&server.to_string());
        let path_value = NSString::from_str(notice.path.as_deref().unwrap_or("/"));
        let info =
            NSDictionary::from_slices(&[&*server_key, &*path_key], &[&*server_value, &*path_value]);
        // SAFETY: every key and value is an `NSString`, which is a property list type.
        unsafe {
            content.setUserInfo(
                &*(&*info as *const NSDictionary<NSString, NSString>
                    as *const NSDictionary<AnyObject, AnyObject>),
            )
        };
        content
    }

    fn add(center: &UNUserNotificationCenter, server: Uuid, server_name: &str, notice: &Notice) {
        let identifier =
            NSString::from_str(&notify::request_identifier(server, notice.tag.as_deref()));
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
            &identifier,
            &content(server, server_name, notice),
            None,
        );
        let completion = RcBlock::new(move |error: *mut NSError| {
            // SAFETY: the system passes either null or a valid `NSError` for the call's duration.
            if let Some(error) = unsafe { error.as_ref() } {
                eprintln!("server {server}: notification failed: {}", error.code());
            }
        });
        center.addNotificationRequest_withCompletionHandler(&request, Some(&completion));
    }

    pub fn deliver(server: Uuid, server_name: &str, notice: Notice) {
        if !has_bundle_identifier() {
            return;
        }

        let server_name = server_name.to_owned();
        let center = UNUserNotificationCenter::currentNotificationCenter();
        let target = center.clone();
        let completion = RcBlock::new(move |granted: Bool, error: *mut NSError| {
            // SAFETY: the system passes either null or a valid `NSError` for the call's duration.
            if let Some(error) = unsafe { error.as_ref() } {
                eprintln!(
                    "server {server}: notification permission failed: {}",
                    error.code()
                );
            }
            if granted.as_bool() {
                add(&target, server, &server_name, &notice);
            } else if !NOT_ALLOWED_LOGGED.swap(true, Ordering::Relaxed) {
                eprintln!("notifications are not allowed for this app");
            }
        });
        center.requestAuthorizationWithOptions_completionHandler(
            UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
            &completion,
        );
    }
}

#[cfg(target_os = "macos")]
pub fn install(app: &AppHandle) {
    native::install(app);
}

#[cfg(target_os = "macos")]
pub fn deliver(server: Uuid, server_name: &str, notice: Notice) {
    native::deliver(server, server_name, notice);
}

#[cfg(not(target_os = "macos"))]
pub fn install(_app: &AppHandle) {}

#[cfg(not(target_os = "macos"))]
pub fn deliver(_server: Uuid, _server_name: &str, _notice: Notice) {}
