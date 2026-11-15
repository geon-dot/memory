//! GPU utilization, read through whichever OS facility is available: the
//! whole GPU every sample, and per process while the GPU detail is shown.

use std::collections::HashMap;

#[cfg(target_os = "macos")]
mod macos;
mod windows;

/// Per-process GPU utilization, as `GpuSampler::processes` reports it.
#[derive(Clone, Debug, PartialEq)]
pub enum ProcGpu {
    /// This machine does not report GPU use per process (no GPU counters on
    /// Windows; an Intel Mac, whose GPU clients carry no `AppUsage`).
    Unavailable,
    /// The first reading after turning on: rates need a second one.
    Warming,
    /// `(pid, name, percent of the whole GPU 0.0..=100.0)`. The name is a
    /// fallback (macOS: the driver's 16-character process name; Windows:
    /// empty); the caller looks up the full one.
    Ready(Vec<(u32, String, f64)>),
}

/// Reads GPU utilization. Never panics: when the OS offers no reading,
/// `sample` returns `None`.
pub struct GpuSampler {
    #[cfg(target_os = "windows")]
    pdh: Option<windows::PdhGpu>,
    /// The last engine instances read, kept only while per-process use is
    /// wanted: per-process figures come from the same reading as the total,
    /// since every PDH collection restarts the counters' interval.
    #[cfg(target_os = "windows")]
    engines: Option<Vec<(String, f64)>>,
    #[cfg(target_os = "windows")]
    keep_engines: bool,
    /// The last per-client GPU times and when they were read.
    #[cfg(target_os = "macos")]
    clients: Option<(std::time::Instant, HashMap<u64, u64>)>,
}

impl GpuSampler {
    pub fn new() -> Self {
        GpuSampler {
            #[cfg(target_os = "windows")]
            pdh: windows::PdhGpu::new(),
            #[cfg(target_os = "windows")]
            engines: None,
            #[cfg(target_os = "windows")]
            keep_engines: false,
            #[cfg(target_os = "macos")]
            clients: None,
        }
    }

    /// Current GPU utilization, 0.0..=100.0, or `None` when unavailable.
    pub fn sample(&mut self) -> Option<f32> {
        #[cfg(target_os = "macos")]
        {
            macos::sample()
        }
        #[cfg(target_os = "windows")]
        {
            let engines = self.pdh.as_mut().and_then(|p| p.read());
            let total = engines.as_deref().and_then(windows::aggregate_engines);
            self.engines = if self.keep_engines { engines } else { None };
            total
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            None
        }
    }

    /// GPU use per process since the last call. On Windows, call it right
    /// after `sample`, which does the reading; the first call only turns the
    /// keeping of that reading on.
    pub fn processes(&mut self) -> ProcGpu {
        #[cfg(target_os = "macos")]
        {
            let Some(now) = macos::client_times() else {
                self.clients = None;
                return ProcGpu::Unavailable;
            };
            let at = std::time::Instant::now();
            let result = match self.clients.take() {
                Some((then, before)) => {
                    let elapsed = at.duration_since(then).as_nanos() as u64;
                    ProcGpu::Ready(client_rates(&before, &now, elapsed))
                }
                None => ProcGpu::Warming,
            };
            self.clients = Some((at, now.iter().map(|c| (c.id, c.gpu_ns)).collect()));
            result
        }
        #[cfg(target_os = "windows")]
        {
            if self.pdh.is_none() {
                return ProcGpu::Unavailable;
            }
            self.keep_engines = true;
            match self.engines.as_deref() {
                Some(engines) => ProcGpu::Ready(
                    windows::per_process_engines(engines)
                        .into_iter()
                        .map(|(pid, v)| (pid, String::new(), v as f64))
                        .collect(),
                ),
                None => ProcGpu::Warming,
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            ProcGpu::Unavailable
        }
    }

    /// Drops what `processes` kept, once the GPU detail is closed.
    pub fn forget_processes(&mut self) {
        #[cfg(target_os = "macos")]
        {
            self.clients = None;
        }
        #[cfg(target_os = "windows")]
        {
            self.keep_engines = false;
            self.engines = None;
        }
    }
}

/// One process's connection to the GPU driver and the GPU time it has used
/// so far (macOS: an `AGXDeviceUserClient`-like entry in the IORegistry).
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub struct ClientTime {
    /// IORegistry entry id, stable while the client lives.
    pub id: u64,
    pub pid: u32,
    pub name: String,
    /// Sum of `accumulatedGPUTime` over the client's `AppUsage`, nanoseconds.
    pub gpu_ns: u64,
}

/// Percent of the whole GPU each process used between two readings of its
/// clients' GPU times, `elapsed_ns` apart: the GPU time added in between
/// divided by the time passed, clamped to 0.0..=100.0. A client that was not
/// there before opened in between, so all of its time counts; a client that
/// closed in between is lost (its time with it). Every process with a
/// client is listed, idle ones at 0.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn client_rates(
    before: &HashMap<u64, u64>,
    now: &[ClientTime],
    elapsed_ns: u64,
) -> Vec<(u32, String, f64)> {
    let mut per_pid: HashMap<u32, (String, u64)> = HashMap::new();
    for c in now {
        let used = c.gpu_ns.saturating_sub(before.get(&c.id).copied().unwrap_or(0));
        let entry = per_pid.entry(c.pid).or_insert_with(|| (c.name.clone(), 0));
        entry.1 = entry.1.saturating_add(used);
    }
    let mut out: Vec<(u32, String, f64)> = per_pid
        .into_iter()
        .map(|(pid, (name, used))| {
            let pct = if elapsed_ns == 0 {
                0.0
            } else {
                (used as f64 / elapsed_ns as f64 * 100.0).clamp(0.0, 100.0)
            };
            (pid, name, pct)
        })
        .collect();
    out.sort_by_key(|(pid, _, _)| *pid);
    out
}

/// Reads an `IOUserClientCreator` value, `"pid 176, WindowServer"`.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn parse_creator(text: &str) -> Option<(u32, String)> {
    let (pid, name) = text.strip_prefix("pid ")?.split_once(',')?;
    Some((pid.trim().parse().ok()?, name.trim().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_is_none_or_in_range() {
        let mut g = GpuSampler::new();
        for _ in 0..3 {
            if let Some(v) = g.sample() {
                assert!((0.0..=100.0).contains(&v), "got {v}");
            }
        }
    }

    #[test]
    fn processes_warm_up_then_report_in_range() {
        let mut g = GpuSampler::new();
        g.sample();
        let first = g.processes();
        assert!(matches!(first, ProcGpu::Warming | ProcGpu::Unavailable), "{first:?}");
        std::thread::sleep(std::time::Duration::from_millis(200));
        g.sample();
        match g.processes() {
            ProcGpu::Ready(list) => {
                assert!(list.iter().all(|(_, _, v)| (0.0..=100.0).contains(v)));
            }
            ProcGpu::Unavailable => assert_eq!(first, ProcGpu::Unavailable),
            ProcGpu::Warming => panic!("still warming after a second reading"),
        }
        g.forget_processes();
        assert!(matches!(g.processes(), ProcGpu::Warming | ProcGpu::Unavailable));
    }

    fn client(id: u64, pid: u32, gpu_ns: u64) -> ClientTime {
        ClientTime { id, pid, name: format!("p{pid}"), gpu_ns }
    }

    #[test]
    fn rates_add_up_a_process_clients_over_the_time_passed() {
        let before: HashMap<u64, u64> = [(1, 1_000), (2, 5_000), (3, 0)].into();
        let now = vec![
            client(1, 10, 101_000), // +100 µs
            client(2, 10, 55_000),  // +50 µs, same process
            client(3, 20, 0),       // idle
        ];
        // 1 ms passed: pid 10 used 150 µs of it.
        let rates = client_rates(&before, &now, 1_000_000);
        assert_eq!(rates.len(), 2);
        assert_eq!(rates[0].0, 10);
        assert!((rates[0].2 - 15.0).abs() < 1e-9);
        assert_eq!(rates[1], (20, "p20".to_string(), 0.0));
    }

    #[test]
    fn a_new_client_counts_all_of_its_time_and_a_shrinking_one_none() {
        let before: HashMap<u64, u64> = [(1, 9_000)].into();
        let now = vec![client(1, 10, 5_000), client(2, 11, 200_000)];
        let rates = client_rates(&before, &now, 1_000_000);
        assert_eq!(rates[0].2, 0.0);
        assert!((rates[1].2 - 20.0).abs() < 1e-9);
    }

    #[test]
    fn rates_clamp_and_survive_zero_time() {
        let now = vec![client(1, 10, 3_000_000)];
        assert_eq!(client_rates(&HashMap::new(), &now, 1_000_000)[0].2, 100.0);
        assert_eq!(client_rates(&HashMap::new(), &now, 0)[0].2, 0.0);
    }

    #[test]
    fn parses_the_client_creator() {
        assert_eq!(parse_creator("pid 176, WindowServer"), Some((176, "WindowServer".into())));
        assert_eq!(
            parse_creator("pid 811, NotificationCent"),
            Some((811, "NotificationCent".into()))
        );
        assert_eq!(parse_creator("pid x, y"), None);
        assert_eq!(parse_creator("WindowServer"), None);
    }

    /// Prints the live reading: `cargo test print_gpu -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn print_gpu() {
        let mut g = GpuSampler::new();
        println!("gpu = {:?}", g.sample());
        g.processes();
        std::thread::sleep(std::time::Duration::from_secs(2));
        println!("gpu = {:?}", g.sample());
        if let ProcGpu::Ready(mut list) = g.processes() {
            list.sort_by(|a, b| b.2.total_cmp(&a.2));
            let sum: f64 = list.iter().map(|x| x.2).sum();
            println!("per-process sum = {sum:.2}");
            for x in list.iter().take(10) {
                println!("{x:?}");
            }
        }
    }
}
