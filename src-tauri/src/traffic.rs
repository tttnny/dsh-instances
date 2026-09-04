//! Native macOS traffic-light buttons are hidden so the sidebar can draw its
//! own set, vertically aligned with the sidebar controls. AppKit locks the
//! native buttons to the title bar's own vertical position, and raising them
//! (trafficLightPosition) inflates the title-bar hit-test area which then
//! swallows real clicks on the whole web strip — so instead the buttons are
//! hidden and their behavior is re-implemented in the webview.

/// Hides the three standard window buttons of the main NSWindow. Safe to call
/// repeatedly: AppKit re-shows them after e.g. fullscreen transitions, and the
/// window resize hook re-asserts the hidden state.
#[cfg(target_os = "macos")]
pub fn hide_native_traffic_lights(win: &tauri::WebviewWindow) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;

    let Ok(ns_window) = win.ns_window() else {
        return;
    };
    if ns_window.is_null() {
        return;
    }
    unsafe {
        let window = ns_window as *mut AnyObject;
        // NSWindowButton enum values (stable AppKit ABI):
        // close = 0, miniaturize = 1, zoom = 2.
        for kind in [0i64, 1, 2] {
            let button: *mut AnyObject = msg_send![window, standardWindowButton: kind];
            if !button.is_null() {
                let _: () = msg_send![button, setHidden: true];
            }
        }
    }
}

/// Hides the native traffic lights and keeps them hidden across window
/// lifecycle events (recreated windows and fullscreen transitions included).
#[cfg(target_os = "macos")]
pub fn attach(win: &tauri::WebviewWindow) {
    use tauri::WindowEvent;

    hide_native_traffic_lights(win);
    let win2 = win.clone();
    win.on_window_event(move |event| {
        if matches!(event, WindowEvent::Resized(_)) {
            hide_native_traffic_lights(&win2);
        }
    });
}

#[cfg(not(target_os = "macos"))]
pub fn attach(_win: &tauri::WebviewWindow) {}
