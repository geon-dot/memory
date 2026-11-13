//! Tray icon (Windows) / menu bar item (macOS): opens the widget and shows a
//! live summary.

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::AppHandle;

use crate::snapshot::{tray_title, Snapshot};
use crate::widget;

pub const TRAY_ID: &str = "main";
const MENU_OPEN: &str = "open";
const MENU_QUIT: &str = "quit";

/// Creates the tray icon with its `열기` / `종료` menu. Left click toggles the
/// widget window; the menu opens on right click.
pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, MENU_OPEN, "열기", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "종료", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;

    let builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("System Monitor")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            MENU_OPEN => widget::show(app),
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
                widget::toggle(tray.app_handle());
            }
        });
    // The mascot's face, drawn for the tray (sources in icons/src). The menu
    // bar gets a one-colour template image, which macOS tints for a light or
    // a dark bar; other trays get the colour drawing.
    #[cfg(target_os = "macos")]
    let builder = builder
        .icon(tauri::include_image!("icons/tray/template.png"))
        .icon_as_template(true);
    #[cfg(not(target_os = "macos"))]
    let builder = builder.icon(tauri::include_image!("icons/tray/color.png"));
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
    diag!("tray: {title}");
}
