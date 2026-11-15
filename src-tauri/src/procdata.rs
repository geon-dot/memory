//! Reads per-process CPU, memory and GPU use for the detail panel. A
//! `ProcReader` exists only while the widget shows a detail; nothing here
//! runs otherwise.
//!
//! Where the numbers come from:
//! - macOS CPU: `ps -axo pid=,time=,rss=,comm=`. `/bin/ps` is set-uid root,
//!   so it reads every process's CPU time, including root-owned ones such as
//!   WindowServer that this app (an ordinary user) cannot read itself. The
//!   rate is the change of that total time between two readings. (`ps`'s own
//!   `%cpu` column is a decaying average and is not used.)
//! - macOS memory: the process's physical footprint (`proc_pid_rusage`,
//!   `ri_phys_footprint`), which is what Activity Monitor shows as "메모리".
//!   It can be read only for this user's processes; for the others the
//!   resident size (RSS) from `ps` stands in and the program is marked `rss`.
//! - macOS GPU: per-client GPU time from the IORegistry (`gpu::macos`).
//! - Windows (and other systems): `sysinfo` for CPU and memory (working set),
//!   the `GPU Engine` counters for GPU (`gpu::windows`).

use crate::gpu::ProcGpu;
use crate::procs::{Detail, DetailList, ProcValue};

pub struct ProcReader {
    #[cfg(target_os = "macos")]
    mac: mac::MacProcs,
    #[cfg(not(target_os = "macos"))]
    sys: portable::SysinfoProcs,
}

impl ProcReader {
    pub fn new() -> Self {
        ProcReader {
            #[cfg(target_os = "macos")]
            mac: mac::MacProcs::default(),
            #[cfg(not(target_os = "macos"))]
            sys: portable::SysinfoProcs::new(),
        }
    }

    /// The CPU list, or `None` on the first call (a rate needs two readings)
    /// or when the processes could not be listed.
    pub fn cpu(&mut self) -> Option<DetailList> {
        #[cfg(target_os = "macos")]
        let read = self.mac.cpu();
        #[cfg(not(target_os = "macos"))]
        let read = self.sys.cpu();
        read.map(|(procs, unreadable)| DetailList::build(Detail::Cpu, &procs, unreadable))
    }

    /// The memory list, or `None` when the processes could not be listed.
    pub fn mem(&mut self) -> Option<DetailList> {
        #[cfg(target_os = "macos")]
        let read = self.mac.mem();
        #[cfg(not(target_os = "macos"))]
        let read = self.sys.mem();
        read.map(|(procs, unreadable)| DetailList::build(Detail::Mem, &procs, unreadable))
    }

    /// The GPU list from what `GpuSampler::processes` returned, with full
    /// program names looked up; `None` while warming up.
    pub fn gpu(&mut self, reading: ProcGpu) -> Option<DetailList> {
        match reading {
            ProcGpu::Unavailable => Some(DetailList::unavailable(Detail::Gpu)),
            ProcGpu::Warming => None,
            ProcGpu::Ready(list) => {
                #[cfg(not(target_os = "macos"))]
                let names = self
                    .sys
                    .names(&list.iter().map(|(pid, _, _)| *pid).collect::<Vec<_>>());
                let procs: Vec<ProcValue> = list
                    .into_iter()
                    .map(|(pid, fallback, value)| {
                        #[cfg(target_os = "macos")]
                        let name = mac::program_name(pid);
                        #[cfg(not(target_os = "macos"))]
                        let name = names.get(&pid).cloned();
                        let name = name
                            .filter(|n| !n.is_empty())
                            .or_else(|| Some(fallback).filter(|n| !n.is_empty()))
                            .unwrap_or_else(|| format!("pid {pid}"));
                        ProcValue { name, value, rss: false }
                    })
                    .collect();
                Some(DetailList::build(Detail::Gpu, &procs, 0))
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use std::collections::HashMap;
    use std::process::Command;
    use std::time::Instant;

    use crate::procs::{cpu_percent, parse_ps, ProcValue, PsRow};

    #[derive(Default)]
    pub struct MacProcs {
        /// Total CPU time per pid at the last CPU reading, and when.
        cpu_before: Option<(Instant, HashMap<u32, u64>)>,
    }

    impl MacProcs {
        pub fn cpu(&mut self) -> Option<(Vec<ProcValue>, u32)> {
            let rows = ps()?;
            let now = Instant::now();
            let before = self
                .cpu_before
                .replace((now, rows.iter().map(|r| (r.pid, r.cpu_cs)).collect()));
            let (then, before) = before?;
            let elapsed = now.duration_since(then).as_secs_f64();
            let (readable, unreadable) = split_readable(rows);
            let procs = readable
                .into_iter()
                .map(|r| ProcValue {
                    value: cpu_percent(before.get(&r.pid).copied(), r.cpu_cs, elapsed),
                    name: r.name,
                    rss: false,
                })
                .collect();
            Some((procs, unreadable))
        }

        pub fn mem(&mut self) -> Option<(Vec<ProcValue>, u32)> {
            let (readable, unreadable) = split_readable(ps()?);
            let procs = readable
                .into_iter()
                .map(|r| match footprint(r.pid) {
                    Some(bytes) => ProcValue { name: r.name, value: bytes as f64, rss: false },
                    None => ProcValue { name: r.name, value: r.rss_bytes as f64, rss: true },
                })
                .collect();
            Some((procs, unreadable))
        }
    }

    /// Every process as `ps` sees it, without the `ps` run itself.
    fn ps() -> Option<Vec<PsRow>> {
        let child = Command::new("/bin/ps")
            .args(["-axo", "pid=,time=,rss=,comm="])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .ok()?;
        let own = child.id();
        let out = child.wait_with_output().ok()?;
        if !out.status.success() {
            return None;
        }
        let mut rows = parse_ps(&String::from_utf8_lossy(&out.stdout));
        rows.retain(|r| r.pid != own);
        Some(rows)
    }

    /// Splits off the processes even `ps` reads nothing about (no CPU time
    /// and no memory: protected processes such as some security agents).
    fn split_readable(rows: Vec<PsRow>) -> (Vec<PsRow>, u32) {
        let (readable, unreadable): (Vec<PsRow>, Vec<PsRow>) =
            rows.into_iter().partition(|r| r.cpu_cs > 0 || r.rss_bytes > 0);
        (readable, unreadable.len() as u32)
    }

    /// Physical footprint in bytes, the "메모리" column of Activity Monitor;
    /// `None` for processes of other users, which only root may read.
    fn footprint(pid: u32) -> Option<u64> {
        let mut info = std::mem::MaybeUninit::<libc::rusage_info_v2>::zeroed();
        // SAFETY: RUSAGE_INFO_V2 makes the kernel fill a rusage_info_v2, the
        // size of the buffer passed.
        let status = unsafe {
            libc::proc_pid_rusage(
                pid as libc::c_int,
                libc::RUSAGE_INFO_V2,
                info.as_mut_ptr() as *mut libc::rusage_info_t,
            )
        };
        // SAFETY: zero-initialized, and filled on success.
        (status == 0).then(|| unsafe { info.assume_init() }.ri_phys_footprint)
    }

    /// The executable's file name, the same name `ps` (and so the CPU and
    /// memory lists) uses. Readable for every user's processes.
    pub fn program_name(pid: u32) -> Option<String> {
        let mut buf = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
        // SAFETY: `buf` is as long as the size passed.
        let n = unsafe {
            libc::proc_pidpath(pid as libc::c_int, buf.as_mut_ptr().cast(), buf.len() as u32)
        };
        if n <= 0 {
            return None;
        }
        buf.truncate(n as usize);
        let path = String::from_utf8_lossy(&buf);
        path.rsplit('/').next().map(str::to_string)
    }
}

/// `sysinfo`-based reading for Windows and other systems. Compiled on macOS
/// too, only so its tests run on the development machine.
#[cfg_attr(target_os = "macos", allow(dead_code))]
mod portable {
    use std::collections::HashMap;

    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

    use crate::procs::ProcValue;

    pub struct SysinfoProcs {
        system: Option<System>,
        /// Whether CPU times were read once already (rates need two).
        cpu_primed: bool,
    }

    impl SysinfoProcs {
        pub fn new() -> Self {
            SysinfoProcs { system: System::new().ok(), cpu_primed: false }
        }

        /// Percent of one core per process (sysinfo's `cpu_usage`, which is
        /// per core and can exceed 100).
        pub fn cpu(&mut self) -> Option<(Vec<ProcValue>, u32)> {
            let sys = self.system.as_mut()?;
            let kind = ProcessRefreshKind::nothing().with_cpu().with_memory();
            sys.refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
            if !std::mem::replace(&mut self.cpu_primed, true) {
                return None;
            }
            Some(Self::collect(sys, |p| p.cpu_usage() as f64))
        }

        /// Bytes per process (Windows: the working set).
        pub fn mem(&mut self) -> Option<(Vec<ProcValue>, u32)> {
            let sys = self.system.as_mut()?;
            // Memory needs no second reading; a later CPU list does.
            self.cpu_primed = false;
            let kind = ProcessRefreshKind::nothing().with_memory();
            sys.refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
            Some(Self::collect(sys, |p| p.memory() as f64))
        }

        /// A process without any memory reading is one the OS hid from this
        /// user; it is counted as unreadable rather than listed at zero.
        fn collect(sys: &System, value: impl Fn(&sysinfo::Process) -> f64) -> (Vec<ProcValue>, u32) {
            let mut unreadable = 0;
            let mut procs = Vec::new();
            for p in sys.processes().values() {
                if p.memory() == 0 {
                    unreadable += 1;
                    continue;
                }
                procs.push(ProcValue {
                    name: p.name().to_string_lossy().into_owned(),
                    value: value(p),
                    rss: false,
                });
            }
            (procs, unreadable)
        }

        /// Names of these processes, refreshing only them.
        pub fn names(&mut self, pids: &[u32]) -> HashMap<u32, String> {
            let Some(sys) = self.system.as_mut() else {
                return HashMap::new();
            };
            let pids: Vec<Pid> = pids.iter().map(|p| Pid::from_u32(*p)).collect();
            sys.refresh_processes_specifics(
                ProcessesToUpdate::Some(&pids),
                true,
                ProcessRefreshKind::nothing(),
            );
            pids.iter()
                .filter_map(|pid| {
                    let name = sys.process(*pid)?.name().to_string_lossy().into_owned();
                    Some((pid.as_u32(), name))
                })
                .collect()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn sysinfo_lists_warm_up_and_find_this_process() {
            let mut s = SysinfoProcs::new();
            assert!(s.cpu().is_none(), "first CPU reading is the baseline");
            std::thread::sleep(std::time::Duration::from_millis(200));
            let (procs, _) = s.cpu().expect("second CPU reading");
            assert!(!procs.is_empty());
            assert!(procs.iter().all(|p| p.value >= 0.0));
            let (mem, _) = s.mem().unwrap();
            assert!(mem.iter().any(|p| p.value > 0.0));
            let me = std::process::id();
            assert!(s.names(&[me]).contains_key(&me));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_list_needs_two_readings_then_ranks_programs() {
        let mut r = ProcReader::new();
        assert!(r.cpu().is_none());
        // Burn some CPU so this test process shows up.
        let start = std::time::Instant::now();
        let mut x = 0u64;
        while start.elapsed() < std::time::Duration::from_millis(300) {
            x = x.wrapping_mul(31).wrapping_add(7);
        }
        std::hint::black_box(x);
        let list = r.cpu().expect("second reading");
        assert_eq!(list.kind, "cpu");
        assert!(list.available);
        assert!(!list.programs.is_empty());
        assert!(list.processes > 10);
        let values: Vec<f64> = list.programs.iter().map(|p| p.value).collect();
        assert!(values.windows(2).all(|w| w[0] >= w[1]), "{values:?}");
    }

    #[test]
    fn mem_list_has_bytes_and_counts_everything() {
        let list = ProcReader::new().mem().expect("memory list");
        assert_eq!(list.kind, "mem");
        assert!(list.programs[0].value > 1024.0 * 1024.0);
        assert!(list.total >= list.programs.iter().map(|p| p.value).sum::<f64>());
    }

    #[test]
    fn gpu_list_names_programs_and_reports_unavailable() {
        let mut r = ProcReader::new();
        let me = std::process::id();
        let list = r
            .gpu(ProcGpu::Ready(vec![(me, "fallback".into(), 12.5), (0x7fff_fff0, String::new(), 1.0)]))
            .unwrap();
        assert_eq!(list.programs.len(), 2);
        assert_eq!(list.programs[0].value, 12.5);
        assert_ne!(list.programs[0].name, "fallback");
        assert_eq!(list.programs[1].name, format!("pid {}", 0x7fff_fff0u32));
        assert!(r.gpu(ProcGpu::Warming).is_none());
        assert!(!r.gpu(ProcGpu::Unavailable).unwrap().available);
    }

    /// Prints live lists: `cargo test print_detail -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn print_detail() {
        let mut r = ProcReader::new();
        r.cpu();
        std::thread::sleep(std::time::Duration::from_secs(2));
        let t = std::time::Instant::now();
        let cpu = r.cpu().unwrap();
        println!("cpu read {:?}\n{}", t.elapsed(), cpu.summary());
        let t = std::time::Instant::now();
        let mem = r.mem().unwrap();
        println!("mem read {:?}\n{}", t.elapsed(), mem.summary());
    }
}
