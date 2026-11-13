//! Tray icon (Windows) / menu bar item (macOS): opens the widget and shows a
//! live summary.

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::snapshot::{tray_title, Snapshot};

pub const TRAY_ID: &str = "main";
const WINDOW_LABEL: &str = "main";
const MENU_OPEN: &str = "open";
const MENU_QUIT: &str = "quit";

/// Creates the tray icon with its `열기` / `종료` menu. Left click toggles the
/// widget window; the menu opens on right click.
pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, MENU_OPEN, "열기", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "종료", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("System Monitor")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            MENU_OPEN => show_window(app),
            MENU_QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// Shows the latest reading on the tray: as text beside the icon on macOS,
/// and as the hover tooltip everywhere.
pub fn update_title(app: &AppHandle, s: &Snapshot) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    let title = tray_title(s);
    // Failures here mean the tray is being torn down; nothing to recover.
    #[cfg(target_os = "macos")]
    let _ = tray.set_title(Some(&title));
    let _ = tray.set_tooltip(Some(&title));
}

/// Brings the widget window to the front, un-hiding it if needed.
pub fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn toggle_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window(WINDOW_LABEL) else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else {
        show_window(app);
    }
}
