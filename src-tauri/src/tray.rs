//! Tray icon (Windows) / menu bar item (macOS): opens the widget and shows a
//! live summary.

use tauri::menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::mascot::Mascot;
use crate::snapshot::{tray_title, Snapshot};
use crate::widget;

pub const TRAY_ID: &str = "main";
const MENU_OPEN: &str = "open";
const MENU_QUIT: &str = "quit";
/// A mascot's menu item has this before the mascot's id.
const MENU_MASCOT: &str = "mascot:";

/// The `마스코트` menu's items, kept to move the check mark.
struct MascotItems(Vec<(Mascot, CheckMenuItem<Wry>)>);

/// Creates the tray icon with its `열기` / `마스코트` / `종료` menu. Left click
/// toggles the widget window; the menu opens on right click.
pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, MENU_OPEN, "열기", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "종료", true, None::<&str>)?;
    let chosen = widget::mascot(app);
    let mut mascot_items = Vec::new();
    for mascot in Mascot::ALL {
        let id = format!("{MENU_MASCOT}{}", mascot.id());
        let checked = mascot == chosen;
        let item = CheckMenuItem::with_id(app, id, mascot.label(), true, checked, None::<&str>)?;
        mascot_items.push((mascot, item));
    }
    let mascot_refs: Vec<&dyn IsMenuItem<Wry>> = mascot_items
        .iter()
        .map(|(_, item)| item as &dyn IsMenuItem<Wry>)
        .collect();
    let mascots = Submenu::with_items(app, "마스코트", true, &mascot_refs)?;
    let menu = Menu::with_items(app, &[&open, &mascots, &quit])?;
    app.manage(MascotItems(mascot_items));

    let builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("먼작귀모니터링")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            MENU_OPEN => widget::show(app),
            MENU_QUIT => app.exit(0),
            id => {
                if let Some(mascot) = id.strip_prefix(MENU_MASCOT).and_then(Mascot::from_id) {
                    widget::set_mascot(app, mascot);
                }
            }
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
    // The three characters in their own colours (sources in icons/src), not
    // a one-colour template image: their dark outline and light bodies show
    // on a light menu bar and on a dark one. The menu bar draws the icon
    // 18pt high, so macOS gets it at its own shape (48x36px); other trays
    // want a square.
    #[cfg(target_os = "macos")]
    let builder = builder.icon(tauri::include_image!("icons/tray/menubar.png"));
    #[cfg(not(target_os = "macos"))]
    let builder = builder.icon(tauri::include_image!("icons/tray/color.png"));
    builder.build(app)?;
    Ok(())
}

/// Puts the `마스코트` menu's check mark on `chosen` and nowhere else. (A
/// clicked item toggles its own mark, so choosing the checked one again
/// would otherwise uncheck it.)
pub fn show_mascot(app: &AppHandle, chosen: Mascot) {
    let Some(items) = app.try_state::<MascotItems>() else {
        return;
    };
    for (mascot, item) in &items.0 {
        let _ = item.set_checked(*mascot == chosen);
    }
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
