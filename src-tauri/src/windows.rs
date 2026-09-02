use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::AppState;

/// Remembers the instance whose window was interacted with most recently so
/// the tray double-click can reopen exactly that profile page.
fn record_focus(app: &AppHandle, instance_id: &str) {
    if let Some(state) = app.try_state::<AppState>() {
        *state.last_focused_instance.lock().unwrap() = Some(instance_id.to_string());
    }
}

/// Opens (or focuses) the webview window hosting the instance's DSH Web GUI.
pub fn open_instance_window(
    app: &AppHandle,
    instance_id: &str,
    name: &str,
    url: &str,
) -> Result<(), String> {
    let label = format!("instance-{instance_id}");
    if let Some(win) = app.get_webview_window(&label) {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
        record_focus(app, instance_id);
        return Ok(());
    }
    let parsed = WebviewUrl::External(url.parse().map_err(|e| format!("无效的 URL {url}: {e}"))?);
    // Instance windows host the *remote* DSH Web GUI, which is not
    // Tauri-aware: it cannot declare a drag region or call startDragging.
    // Keep the standard macOS title bar (draggable, traffic lights above the
    // content) instead of an Overlay that would cover the page's top-left
    // corner and make the window undraggable.
    let win = WebviewWindowBuilder::new(app, label, parsed)
        .title(format!("{name} — DSH"))
        .inner_size(1024.0, 576.0)
        .min_inner_size(800.0, 500.0)
        .center()
        .build()
        .map_err(|e| e.to_string())?;
    record_focus(app, instance_id);

    // Track focus so the tray knows which profile page the user used last.
    let handle = app.clone();
    let id = instance_id.to_string();
    win.on_window_event(move |event| {
        if let WindowEvent::Focused(true) = event {
            record_focus(&handle, &id);
        }
    });
    Ok(())
}

/// Closes the instance's webview window if it is open.
pub fn close_instance_window(app: &AppHandle, instance_id: &str) {
    let label = format!("instance-{instance_id}");
    if let Some(win) = app.get_webview_window(&label) {
        let _ = win.close();
    }
}

/// Attaches the main window's close behavior: with `minimize_to_tray` the
/// close button hides the window instead of closing it; otherwise the window
/// is really destroyed, but the app itself stays alive (see the
/// `ExitRequested` handler in lib.rs) so instances keep running and the
/// window can be recreated from the Dock or the tray.
pub fn attach_close_behavior(app: &AppHandle, win: &tauri::WebviewWindow) {
    let handle = app.clone();
    let win2 = win.clone();
    win.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            let minimize = handle
                .state::<AppState>()
                .config
                .lock()
                .unwrap()
                .settings
                .minimize_to_tray;
            if minimize {
                api.prevent_close();
                let _ = win2.hide();
            }
        }
    });
}

/// Recreates the main launcher window (with its close handler) after it was
/// destroyed by closing with `minimize_to_tray` off.
fn create_main_window(app: &AppHandle) -> Result<(), String> {
    let win = tauri::WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("DSH Launcher")
        .inner_size(1080.0, 760.0)
        .min_inner_size(900.0, 620.0)
        .center()
        .visible(true)
        .decorations(true)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .build()
        .map_err(|e| e.to_string())?;
    attach_close_behavior(app, &win);
    Ok(())
}

/// Shows the main launcher window, recreating it when it no longer exists.
/// Used by the Dock `Reopen` event and the tray's "打开启动器" entry.
pub fn show_or_create_main(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    } else if let Err(e) = create_main_window(app) {
        crate::log_warn!("重建主窗口失败: {e}");
    }
}
