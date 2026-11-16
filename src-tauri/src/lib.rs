use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
mod mascot;
mod procdata;
mod procs;
mod snapshot;
mod tray;
mod widget;

use collector::Collector;
use procdata::ProcReader;
use procs::Detail;

/// How often a new snapshot is taken and sent to the UI.
const SAMPLE_INTERVAL: Duration = Duration::from_millis(1000);

/// How often each per-program list is read while it is shown, in samples
/// (`SAMPLE_INTERVAL`s). CPU and memory run `/bin/ps` on macOS, which costs
/// about 20–25 ms of CPU per run, so they are read every 2 seconds; the GPU
/// list only reads the IORegistry and keeps up with the 1-second sample.
/// See README "프로그램별 사용량" for the measurements.
fn detail_every(kind: Detail) -> u32 {
    match kind {
        Detail::Gpu => 1,
        Detail::Cpu | Detail::Mem | Detail::Off => 2,
    }
}

/// The per-program list the widget shows. Off unless the widget asks, and
/// turned off whenever the widget window goes away.
static DETAIL: Mutex<Detail> = Mutex::new(Detail::Off);

fn detail_kind() -> Detail {
    *DETAIL.lock().unwrap()
}

fn set_detail_kind(kind: Detail) {
    let mut current = DETAIL.lock().unwrap();
    if *current != kind {
        *current = kind;
        diag!("detail: {}", kind.id());
    }
}

/// Tells the app which per-program list the widget shows: `"cpu"`, `"gpu"`,
/// `"mem"` or `"off"`. While one is on (and the widget is open), the app
/// reads only that kind and sends it as the `detail` event; see
/// `procs::DetailList` for its shape. Closing the widget turns it off.
#[tauri::command]
fn set_detail(window: tauri::WebviewWindow, kind: String) -> Result<(), String> {
    let kind = Detail::from_id(&kind).ok_or_else(|| format!("unknown detail {kind:?}"))?;
    set_detail_kind(kind);
    // The list is drawn in a panel to the right of the widget.
    widget::set_panel(&window, kind != Detail::Off).map_err(|e| e.to_string())
}

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

/// The chosen mascot's id, for a newly opened widget to draw.
#[tauri::command]
fn mascot(app: tauri::AppHandle) -> &'static str {
    widget::mascot(&app).id()
}

/// A line from the widget about what it shows (which mascot and picture, in
/// which mood), printed with the diagnostics. The web view cannot be looked
/// at from outside, so this is how a check sees what it drew.
#[tauri::command]
fn diag(line: String) {
    diag!("widget: {line}");
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
        let mut detail = DetailSampler::default();
        loop {
            std::thread::sleep(SAMPLE_INTERVAL);
            // With the widget closed only the tray summary is shown, so only
            // that is read, and nothing is sent to a window that is not there.
            if widget::is_open(&app) {
                let snapshot = collector.sample();
                // Emitting fails only while the app is shutting down.
                let _ = app.emit("snapshot", &snapshot);
                tray::update_title(&app, &snapshot);
                detail.tick(&app, &mut collector);
            } else {
                set_detail_kind(Detail::Off);
                detail.stop(&mut collector);
                tray::update_title(&app, &collector.sample_tray());
            }
        }
    });
}

/// Reads the per-program list the widget shows, if any, on the sampler
/// thread. Holds nothing while no list is shown: the process reader is
/// created when a list opens and dropped when it closes.
#[derive(Default)]
struct DetailSampler {
    /// The kind `reader` serves; `Off` when there is none.
    kind: Option<Detail>,
    reader: Option<ProcReader>,
    /// Samples until the next reading.
    wait: u32,
}

impl DetailSampler {
    fn tick(&mut self, app: &tauri::AppHandle, collector: &mut Collector) {
        let want = detail_kind();
        if self.kind != Some(want) {
            self.stop(collector);
            self.kind = Some(want);
        }
        if want == Detail::Off {
            return;
        }
        if self.wait > 0 {
            self.wait -= 1;
            return;
        }
        let reader = self.reader.get_or_insert_with(ProcReader::new);
        let started = Instant::now();
        let child_cpu = children_cpu_ms();
        let list = match want {
            Detail::Cpu => reader.cpu(),
            Detail::Mem => reader.mem(),
            Detail::Gpu => reader.gpu(collector.gpu_processes()),
            Detail::Off => None,
        };
        let cost = format!(
            "{:.1} ms, child cpu {:.1} ms",
            started.elapsed().as_secs_f64() * 1000.0,
            children_cpu_ms() - child_cpu
        );
        match list {
            Some(list) => {
                diag!("detail {} ({cost})", list.summary());
                let _ = app.emit("detail", &list);
                self.wait = detail_every(want) - 1;
            }
            // A rate needs a second reading: take it on the next sample.
            None => diag!("detail {}: first reading ({cost})", want.id()),
        }
    }

    fn stop(&mut self, collector: &mut Collector) {
        if self.reader.take().is_some() || self.kind == Some(Detail::Gpu) {
            collector.forget_gpu_processes();
        }
        self.kind = None;
        self.wait = 0;
    }
}

/// CPU time used by this app's finished child processes (the `ps` runs), for
/// the diagnostics' cost line. Zero where it is not measured.
fn children_cpu_ms() -> f64 {
    if !diag_enabled() {
        return 0.0;
    }
    #[cfg(target_os = "macos")]
    {
        let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
        // SAFETY: `usage` is a valid out-pointer for one rusage.
        if unsafe { libc::getrusage(libc::RUSAGE_CHILDREN, usage.as_mut_ptr()) } != 0 {
            return 0.0;
        }
        // SAFETY: zero-initialized, and filled on success.
        let u = unsafe { usage.assume_init() };
        let ms = |t: libc::timeval| t.tv_sec as f64 * 1000.0 + t.tv_usec as f64 / 1000.0;
        ms(u.ru_utime) + ms(u.ru_stime)
    }
    #[cfg(not(target_os = "macos"))]
    {
        0.0
    }
}

pub fn run() {
    let app = tauri::Builder::default()
        // Must be the first plugin. Starting the app a second time adds no
        // second tray icon; the running app says it is already running.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let action = widget::second_launch_action(&args, diag_enabled());
            diag!("second launch: {action:?}");
            widget::run_from_any_thread(app, action);
        }))
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            set_pinned, pinned, mascot, diag, set_detail
        ])
        // Closing the widget (✕, or the system closing the window) releases
        // its window and web view; the app lives on in the tray and quits from
        // the tray menu.
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { .. } => {
                if let Some(w) = window.app_handle().get_webview_window(window.label()) {
                    widget::remember_position(&w);
                }
            }
            WindowEvent::Destroyed => {
                diag!("widget: window destroyed");
                // The next widget starts with no list open.
                set_detail_kind(Detail::Off);
                widget::panel_closed(window.app_handle());
            }
            _ => {}
        })
        .setup(|app| {
            // Menu bar only: no Dock icon.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            app.manage(widget::WidgetState::load(app.handle()));
            diag!("mascot: {} (loaded)", widget::mascot(app.handle()).id());
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
        .expect("error while building 먼작귀모니터링");

    app.run(|_app, event| match event {
        // The last window closing asks to exit; stay in the tray instead.
        tauri::RunEvent::ExitRequested { code, api, .. } if widget::stays_in_tray(code) => {
            api.prevent_exit();
        }
        // Opening the app again from Finder or Spotlight while it is running
        // arrives here on macOS, not as a second launch.
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Reopen { .. } => widget::notice(_app),
        _ => {}
    });
}
