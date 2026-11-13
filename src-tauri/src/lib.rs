use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::{Emitter, Manager, WindowEvent};

/// Diagnostics on stderr, for checking the app from outside (window created
/// or destroyed, each tray update). Off unless the app is started with
/// `SYSTEM_MONITOR_DIAG=1`.
macro_rules! diag {
    ($($arg:tt)*) => {
        if $crate::diag_enabled() {
            eprintln!("[diag {}] {}", $crate::diag_time(), format!($($arg)*));
        }
    };
}

mod collector;
mod gpu;
mod snapshot;
mod tray;
mod widget;

use collector::Collector;

/// How often a new snapshot is taken and sent to the UI.
const SAMPLE_INTERVAL: Duration = Duration::from_millis(1000);

/// Keeps the widget window above other windows, or lets it go behind them.
#[tauri::command]
fn set_pinned(window: tauri::WebviewWindow, pinned: bool) -> Result<(), String> {
    widget::set_pinned(&window, pinned).map_err(|e| e.to_string())
}

/// Whether the widget is pinned, for a newly opened widget's 📌 button.
#[tauri::command]
fn pinned(app: tauri::AppHandle) -> bool {
    widget::is_pinned(&app)
}

fn diag_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("SYSTEM_MONITOR_DIAG").is_ok_and(|v| v == "1"))
}

/// Wall-clock seconds with milliseconds, for diagnostics.
fn diag_time() -> String {
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:03}", t.as_secs(), t.subsec_millis())
}

fn spawn_sampler(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let mut collector = Collector::new();
        loop {
            std::thread::sleep(SAMPLE_INTERVAL);
            // With the widget closed only the tray summary is shown, so only
            // that is read, and nothing is sent to a window that is not there.
            if widget::is_open(&app) {
                let snapshot = collector.sample();
                // Emitting fails only while the app is shutting down.
                let _ = app.emit("snapshot", &snapshot);
                tray::update_title(&app, &snapshot);
            } else {
                tray::update_title(&app, &collector.sample_tray());
            }
        }
    });
}

pub fn run() {
    let app = tauri::Builder::default()
        // Must be the first plugin. Starting the app a second time brings the
        // running widget forward instead of adding another tray icon.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let action = widget::second_launch_action(&args, diag_enabled());
            diag!("second launch: {action:?}");
            widget::run_from_any_thread(app, action);
        }))
        .invoke_handler(tauri::generate_handler![set_pinned, pinned])
        // Closing the widget (✕, or the system closing the window) releases
        // its window and web view; the app lives on in the tray and quits from
        // the tray menu.
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { .. } => {
                if let Some(w) = window.app_handle().get_webview_window(window.label()) {
                    widget::remember_position(&w);
                }
            }
            WindowEvent::Destroyed => diag!("widget: window destroyed"),
            _ => {}
        })
        .setup(|app| {
            // Menu bar only: no Dock icon.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            app.manage(widget::WidgetState::load(app.handle()));
            // The first widget comes from tauri.conf.json; pin it if the
            // 📌 was on when the app last ran.
            if widget::is_pinned(app.handle()) {
                if let Some(w) = app.get_webview_window(widget::WINDOW_LABEL) {
                    let _ = w.set_always_on_top(true);
                }
            }
            tray::build(app.handle())?;
            spawn_sampler(app.handle().clone());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building System Monitor");

    app.run(|_app, event| match event {
        // The last window closing asks to exit; stay in the tray instead.
        tauri::RunEvent::ExitRequested { code, api, .. } if widget::stays_in_tray(code) => {
            api.prevent_exit();
        }
        // Opening the app again from Finder or Spotlight while it is running
        // opens the widget.
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Reopen { .. } => widget::show(_app),
        _ => {}
    });
}
