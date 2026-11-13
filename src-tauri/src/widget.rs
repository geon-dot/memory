//! The widget window.
//!
//! Hiding the widget destroys its window and web view instead of only hiding
//! them: WebKit then shuts down the helper processes (WebContent, GPU,
//! Networking) that draw it, which hold most of the app's memory. Opening the
//! widget builds a fresh window from the `main` entry in `tauri.conf.json` at
//! the place the old one was.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use tauri::{AppHandle, LogicalPosition, Manager, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

pub const WINDOW_LABEL: &str = "main";

/// What a new widget window takes over from the last one.
pub struct WidgetState {
    /// Where the widget was when it was last closed, so it reopens there.
    /// Kept in logical units: a new window can start on a display with
    /// another scale factor, which would misread a physical position.
    position: Mutex<Option<LogicalPosition<f64>>>,
    /// 📌 always on top. Kept here, and in `pin_file` across restarts, because
    /// the web view has no storage that outlives it (see `incognito` in
    /// tauri.conf.json).
    pinned: Mutex<bool>,
    pin_file: Option<PathBuf>,
}

impl WidgetState {
    pub fn load(app: &AppHandle) -> Self {
        let pin_file = app.path().app_config_dir().ok().map(|d| d.join("pinned"));
        let pinned = pin_file
            .as_ref()
            .and_then(|f| std::fs::read_to_string(f).ok())
            .is_some_and(|t| parse_pinned(&t));
        WidgetState {
            position: Mutex::new(None),
            pinned: Mutex::new(pinned),
            pin_file,
        }
    }
}

fn parse_pinned(text: &str) -> bool {
    text.trim() == "1"
}

fn pinned_text(pinned: bool) -> &'static str {
    if pinned {
        "1"
    } else {
        "0"
    }
}

/// Whether the 📌 is on.
pub fn is_pinned(app: &AppHandle) -> bool {
    app.try_state::<WidgetState>()
        .is_some_and(|s| *s.pinned.lock().unwrap())
}

/// Turns 📌 on or off for this window and the ones after it.
pub fn set_pinned(window: &WebviewWindow, pinned: bool) -> tauri::Result<()> {
    window.set_always_on_top(pinned)?;
    if let Some(state) = window.try_state::<WidgetState>() {
        *state.pinned.lock().unwrap() = pinned;
        if let Some(file) = &state.pin_file {
            // A preference only: if it cannot be saved, it lasts this session.
            let _ = std::fs::create_dir_all(file.parent().unwrap_or(file));
            let _ = std::fs::write(file, pinned_text(pinned));
        }
    }
    Ok(())
}

/// Whether the app should keep running when it is asked to exit with `code`.
///
/// Tauri asks to exit with no code when the last window closes; the app lives
/// on in the tray then. Quitting from the tray menu passes an explicit code.
pub fn stays_in_tray(code: Option<i32>) -> bool {
    code.is_none()
}

/// What starting the app again does to the running one.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Action {
    Show,
    Hide,
    Toggle,
    /// Tell the user the app is already running.
    Notice,
}

/// Starting the app again does not start a second copy; the running one says
/// it is already running. With diagnostics on, `--hide`, `--toggle` and
/// `--show` instead drive the widget the way the tray does, so the memory of
/// each state can be measured without clicking.
pub fn second_launch_action(args: &[String], diagnostics: bool) -> Action {
    if !diagnostics {
        return Action::Notice;
    }
    args.iter()
        .skip(1)
        .find_map(|a| match a.as_str() {
            "--hide" => Some(Action::Hide),
            "--toggle" => Some(Action::Toggle),
            "--show" => Some(Action::Show),
            _ => None,
        })
        .unwrap_or(Action::Notice)
}

/// Set while the "already running" dialog is up, so launching the app several
/// times shows one dialog, not a stack of them.
static NOTICE_OPEN: AtomicBool = AtomicBool::new(false);

/// Shows the "already running" dialog. Does not block the caller.
pub fn notice(app: &AppHandle) {
    if NOTICE_OPEN.swap(true, Ordering::SeqCst) {
        return;
    }
    diag!("notice: already running");
    app.dialog()
        .message("System Monitor가 이미 실행 중입니다.\n메뉴 막대(트레이)의 아이콘을 눌러 위젯을 여세요.")
        .title("System Monitor")
        .kind(MessageDialogKind::Info)
        .show(|ok| {
            diag!("notice: closed ({ok})");
            NOTICE_OPEN.store(false, Ordering::SeqCst);
        });
}

/// Runs `action` on the main thread; for callers off it, such as the
/// single-instance callback, where creating and showing a window can race
/// with the event loop.
pub fn run_from_any_thread(app: &AppHandle, action: Action) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || match action {
        Action::Show => show(&handle),
        Action::Hide => hide(&handle),
        Action::Toggle => toggle(&handle),
        Action::Notice => notice(&handle),
    });
}

/// Whether the widget window exists (it exists only while it is open).
pub fn is_open(app: &AppHandle) -> bool {
    app.get_webview_window(WINDOW_LABEL).is_some()
}

/// Opens the widget, or brings it to the front if it is already open.
///
/// Call on the main thread (tray and app events run there); from another
/// thread use `run_from_any_thread`.
pub fn show(app: &AppHandle) {
    let window = match app.get_webview_window(WINDOW_LABEL) {
        Some(window) => window,
        None => match create(app) {
            Ok(window) => window,
            Err(e) => {
                eprintln!("System Monitor: could not open the widget: {e}");
                return;
            }
        },
    };
    let _ = window.show();
    let _ = window.set_focus();
}

/// Closes the widget, releasing its window and web view.
pub fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        remember_position(&window);
        // `destroy` skips CloseRequested; the window is gone afterwards.
        let _ = window.destroy();
    }
}

/// Tray left click: closes an open widget, opens a closed one.
pub fn toggle(app: &AppHandle) {
    match app.get_webview_window(WINDOW_LABEL) {
        Some(window) if window.is_visible().unwrap_or(false) => hide(app),
        _ => show(app),
    }
}

/// Records where the widget is, for the next time it opens.
pub fn remember_position(window: &WebviewWindow) {
    let (Ok(pos), Ok(scale)) = (window.outer_position(), window.scale_factor()) else {
        return;
    };
    let pos = pos.to_logical::<f64>(scale);
    if let Some(state) = window.try_state::<WidgetState>() {
        *state.position.lock().unwrap() = Some(pos);
        diag!("widget: remembered position {},{}", pos.x, pos.y);
    }
}

fn create(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == WINDOW_LABEL)
        .ok_or(tauri::Error::WindowNotFound)?
        .clone();
    let position = app
        .try_state::<WidgetState>()
        .and_then(|s| *s.position.lock().unwrap());
    let mut builder =
        WebviewWindowBuilder::from_config(app, &config)?.always_on_top(is_pinned(app));
    if let Some(pos) = position {
        builder = builder.position(pos.x, pos.y);
    }
    let window = builder.build()?;
    diag!("widget: window created at {:?}", position);
    Ok(window)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_window_closing_keeps_the_app_in_the_tray() {
        assert!(stays_in_tray(None));
    }

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn second_launch_says_the_app_is_already_running() {
        let plain = args(&["system-monitor"]);
        assert_eq!(second_launch_action(&plain, false), Action::Notice);
        assert_eq!(second_launch_action(&plain, true), Action::Notice);
    }

    #[test]
    fn launch_flags_work_only_with_diagnostics_on() {
        let hide = args(&["system-monitor", "--hide"]);
        assert_eq!(second_launch_action(&hide, false), Action::Notice);
        assert_eq!(second_launch_action(&hide, true), Action::Hide);
        let toggle = args(&["system-monitor", "--toggle"]);
        assert_eq!(second_launch_action(&toggle, true), Action::Toggle);
        let show = args(&["system-monitor", "--show"]);
        assert_eq!(second_launch_action(&show, true), Action::Show);
        let other = args(&["system-monitor", "--whatever"]);
        assert_eq!(second_launch_action(&other, true), Action::Notice);
    }

    #[test]
    fn pin_file_round_trips() {
        assert!(parse_pinned(pinned_text(true)));
        assert!(!parse_pinned(pinned_text(false)));
    }

    #[test]
    fn missing_or_odd_pin_file_means_not_pinned() {
        assert!(!parse_pinned(""));
        assert!(!parse_pinned("yes"));
        assert!(parse_pinned("1\n"));
    }

    #[test]
    fn explicit_exit_quits() {
        assert!(!stays_in_tray(Some(0)));
        assert!(!stays_in_tray(Some(1)));
    }
}
