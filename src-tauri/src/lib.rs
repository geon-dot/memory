mod collector;
mod gpu;
mod snapshot;
mod tray;

use std::time::Duration;

use tauri::{Emitter, WindowEvent};

use collector::Collector;

/// How often a new snapshot is taken and sent to the UI.
const SAMPLE_INTERVAL: Duration = Duration::from_millis(1000);

/// Keeps the widget window above other windows, or lets it go behind them.
#[tauri::command]
fn set_pinned(window: tauri::WebviewWindow, pinned: bool) -> Result<(), String> {
    window.set_always_on_top(pinned).map_err(|e| e.to_string())
}

fn spawn_sampler(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let mut collector = Collector::new();
        loop {
            std::thread::sleep(SAMPLE_INTERVAL);
            let snapshot = collector.sample();
            // Emitting fails only while the app is shutting down.
            let _ = app.emit("snapshot", &snapshot);
            tray::update_title(&app, &snapshot);
        }
    });
}

pub fn run() {
    let app = tauri::Builder::default()
        // Must be the first plugin. Starting the app a second time brings the
        // running widget forward instead of adding another tray icon.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_window(app);
        }))
        .invoke_handler(tauri::generate_handler![set_pinned])
        // Closing the widget only hides it; the app lives on in the tray and
        // quits from the tray menu.
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            // Menu bar only: no Dock icon.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            tray::build(app.handle())?;
            spawn_sampler(app.handle().clone());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building System Monitor");

    app.run(|_app, _event| {
        // Opening the app again from Finder or Spotlight while it is running
        // re-shows a hidden widget.
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Reopen { .. } = _event {
            tray::show_window(_app);
        }
    });
}
