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

use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewWindow, WebviewWindowBuilder,
};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

use crate::mascot::Mascot;
use crate::procs::Detail;

pub const WINDOW_LABEL: &str = "main";

/// How much wider the window is while the per-program panel is open, in
/// logical px. The panel in `ui/style.css` fills exactly this.
pub const PANEL_WIDTH: f64 = 250.0;

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
    /// The mascot chosen in the tray menu. Kept the same way, in
    /// `mascot_file`, and handed to each new widget.
    mascot: Mutex<Mascot>,
    mascot_file: Option<PathBuf>,
    /// `Some` while the per-program panel is open: how far left the window
    /// was moved to keep the panel on the screen (0 when it fitted).
    panel: Mutex<Option<f64>>,
}

impl WidgetState {
    pub fn load(app: &AppHandle) -> Self {
        let config_dir = app.path().app_config_dir().ok();
        let pin_file = config_dir.as_ref().map(|d| d.join("pinned"));
        let pinned = pin_file
            .as_ref()
            .and_then(|f| std::fs::read_to_string(f).ok())
            .is_some_and(|t| parse_pinned(&t));
        let mascot_file = config_dir.map(|d| d.join("mascot"));
        let saved = mascot_file
            .as_ref()
            .and_then(|f| std::fs::read_to_string(f).ok());
        WidgetState {
            position: Mutex::new(None),
            pinned: Mutex::new(pinned),
            pin_file,
            mascot: Mutex::new(Mascot::from_saved(saved.as_deref())),
            mascot_file,
            panel: Mutex::new(None),
        }
    }
}

/// Saves a preference. If it cannot be saved, it lasts this session only.
fn save(file: &Option<PathBuf>, text: &str) {
    if let Some(file) = file {
        let _ = std::fs::create_dir_all(file.parent().unwrap_or(file));
        let _ = std::fs::write(file, text);
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
        save(&state.pin_file, pinned_text(pinned));
    }
    Ok(())
}

/// The mascot the widget shows.
pub fn mascot(app: &AppHandle) -> Mascot {
    app.try_state::<WidgetState>()
        .map(|s| *s.mascot.lock().unwrap())
        .unwrap_or_default()
}

/// Chooses the mascot: an open widget changes at once, and every widget
/// opened later (this run or the next) starts with it.
pub fn set_mascot(app: &AppHandle, mascot: Mascot) {
    if let Some(state) = app.try_state::<WidgetState>() {
        *state.mascot.lock().unwrap() = mascot;
        save(&state.mascot_file, mascot.id());
    }
    crate::tray::show_mascot(app, mascot);
    // Reaches the widget only if one is open; a closed one has no web view.
    let _ = app.emit("mascot", mascot.id());
    diag!("mascot: {}", mascot.id());
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
    /// Choose a mascot, as the tray menu does.
    Mascot(Mascot),
    /// Show a per-program list (or none), as the widget's `set_detail` does.
    Detail(Detail),
}

/// Starting the app again does not start a second copy; the running one says
/// it is already running. With diagnostics on, `--hide`, `--toggle`,
/// `--show`, `--mascot=<id>` and `--detail=cpu|gpu|mem|off` instead drive the widget the way the tray
/// does, so each state can be measured and checked without clicking.
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
            other => other
                .strip_prefix("--mascot=")
                .and_then(Mascot::from_id)
                .map(Action::Mascot)
                .or_else(|| {
                    other
                        .strip_prefix("--detail=")
                        .and_then(Detail::from_id)
                        .map(Action::Detail)
                }),
        })
        .unwrap_or(Action::Notice)
}

/// Set while the "already running" dialog is up, so launching the app several
/// times shows one dialog, not a stack of them.
static NOTICE_OPEN: AtomicBool = AtomicBool::new(false);

/// Opens the widget and shows the "already running" dialog. Does not block
/// the caller.
pub fn notice(app: &AppHandle) {
    show(app);
    if NOTICE_OPEN.swap(true, Ordering::SeqCst) {
        return;
    }
    diag!("notice: already running");
    app.dialog()
        .message("먼작귀모니터링이 이미 실행 중입니다.\n위젯을 열었습니다. 메뉴 막대(트레이)의 아이콘으로 여닫을 수 있습니다.")
        .title("먼작귀모니터링")
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
        Action::Mascot(mascot) => set_mascot(&handle, mascot),
        Action::Detail(detail) => crate::set_detail_kind(detail),
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
                eprintln!("먼작귀모니터링: could not open the widget: {e}");
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

/// How far left a window at `x` has to move for `width` to end inside a
/// screen that spans `screen_x..screen_x + screen_width`: nothing when it
/// fits, and never past the screen's left edge.
fn panel_shift(x: f64, width: f64, screen_x: f64, screen_width: f64) -> f64 {
    let over = x + width - (screen_x + screen_width);
    over.clamp(0.0, (x - screen_x).max(0.0))
}

/// Opens or closes the per-program panel: the window grows to the right by
/// `PANEL_WIDTH` and shrinks back. Near the right edge of the screen it
/// moves left first so the panel stays visible, and returns on closing.
pub fn set_panel(window: &WebviewWindow, open: bool) -> tauri::Result<()> {
    let Some(state) = window.try_state::<WidgetState>() else {
        return Ok(());
    };
    let mut panel = state.panel.lock().unwrap();
    if open == panel.is_some() {
        return Ok(());
    }
    let config = window.app_handle().config();
    let Some(base) = config.app.windows.iter().find(|w| w.label == WINDOW_LABEL) else {
        return Ok(());
    };
    let scale = window.scale_factor()?;
    let pos = window.outer_position()?.to_logical::<f64>(scale);
    if open {
        let width = base.width + PANEL_WIDTH;
        let shift = match window.current_monitor()? {
            Some(m) => {
                let screen_x = m.position().to_logical::<f64>(m.scale_factor()).x;
                let screen_width = m.size().to_logical::<f64>(m.scale_factor()).width;
                panel_shift(pos.x, width, screen_x, screen_width)
            }
            None => 0.0,
        };
        if shift > 0.0 {
            window.set_position(LogicalPosition::new(pos.x - shift, pos.y))?;
        }
        window.set_size(LogicalSize::new(width, base.height))?;
        *panel = Some(shift);
    } else {
        let shift = panel.take().unwrap_or(0.0);
        window.set_size(LogicalSize::new(base.width, base.height))?;
        if shift > 0.0 {
            window.set_position(LogicalPosition::new(pos.x + shift, pos.y))?;
        }
    }
    diag!("panel: open={open}");
    Ok(())
}

/// The widget window is gone; the next one starts without the panel.
pub fn panel_closed(app: &AppHandle) {
    if let Some(state) = app.try_state::<WidgetState>() {
        *state.panel.lock().unwrap() = None;
    }
}

/// Records where the widget is, for the next time it opens.
pub fn remember_position(window: &WebviewWindow) {
    let (Ok(pos), Ok(scale)) = (window.outer_position(), window.scale_factor()) else {
        return;
    };
    let mut pos = pos.to_logical::<f64>(scale);
    if let Some(state) = window.try_state::<WidgetState>() {
        // Where it would be with the panel closed, as it reopens that way.
        pos.x += state.panel.lock().unwrap().unwrap_or(0.0);
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
    fn mascot_flag_chooses_a_mascot_only_with_diagnostics_on() {
        let usagi = args(&["system-monitor", "--mascot=usagi"]);
        assert_eq!(second_launch_action(&usagi, false), Action::Notice);
        assert_eq!(
            second_launch_action(&usagi, true),
            Action::Mascot(Mascot::Usagi)
        );
        let chip = args(&["system-monitor", "--mascot=chip"]);
        assert_eq!(
            second_launch_action(&chip, true),
            Action::Mascot(Mascot::Chip)
        );
        let nobody = args(&["system-monitor", "--mascot=nobody"]);
        assert_eq!(second_launch_action(&nobody, true), Action::Notice);
    }

    #[test]
    fn detail_flag_chooses_a_list_only_with_diagnostics_on() {
        let cpu = args(&["app", "--detail=cpu"]);
        assert_eq!(second_launch_action(&cpu, false), Action::Notice);
        assert_eq!(second_launch_action(&cpu, true), Action::Detail(Detail::Cpu));
        let off = args(&["app", "--detail=off"]);
        assert_eq!(second_launch_action(&off, true), Action::Detail(Detail::Off));
        let odd = args(&["app", "--detail=disk"]);
        assert_eq!(second_launch_action(&odd, true), Action::Notice);
    }

    #[test]
    fn panel_fits_without_moving_when_there_is_room() {
        assert_eq!(panel_shift(100.0, 550.0, 0.0, 1440.0), 0.0);
        assert_eq!(panel_shift(890.0, 550.0, 0.0, 1440.0), 0.0);
    }

    #[test]
    fn panel_near_the_right_edge_moves_the_window_left() {
        assert_eq!(panel_shift(1100.0, 550.0, 0.0, 1440.0), 210.0);
        // A second screen to the right starts at its own x.
        assert_eq!(panel_shift(2700.0, 550.0, 1440.0, 1440.0), 370.0);
    }

    #[test]
    fn panel_never_moves_the_window_past_the_left_edge() {
        assert_eq!(panel_shift(100.0, 550.0, 0.0, 400.0), 100.0);
        assert_eq!(panel_shift(-20.0, 550.0, 0.0, 400.0), 0.0);
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
