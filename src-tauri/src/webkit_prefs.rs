use objc2::runtime::{AnyObject, Bool};
use objc2::{msg_send, sel};

pub fn disable_backspace_navigation(preferences: &AnyObject) -> bool {
    // The setter is private WebKit API a future macOS may remove, so it is checked before sending; KVC would throw on an unknown key and abort the release build.
    let setter = sel!(_setBackspaceKeyNavigationEnabled:);
    let responds: Bool = unsafe { msg_send![preferences, respondsToSelector: setter] };
    if !responds.as_bool() {
        return false;
    }
    let _: () = unsafe { msg_send![preferences, _setBackspaceKeyNavigationEnabled: Bool::NO] };
    true
}
