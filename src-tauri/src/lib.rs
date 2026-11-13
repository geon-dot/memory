mod collector;
mod gpu;
mod snapshot;

use std::time::Duration;

use tauri::Emitter;

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
        }
    });
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![set_pinned])
        .setup(|app| {
            spawn_sampler(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running System Monitor");
}
