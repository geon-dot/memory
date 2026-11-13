//! Gathers one `Snapshot` from the OS.

use sysinfo::{Disks, System};

use crate::gpu::GpuSampler;
use crate::snapshot::{clamp_pct, mean, pick_system_disk, DiskInfo, Snapshot};

/// The disk list changes slowly and is the costliest thing to read, so it is
/// refreshed once every this many samples.
const DISK_REFRESH_EVERY: u32 = 5;

/// Reads CPU, memory, disk and GPU usage. Only those are refreshed — never
/// the process list — so sampling once a second stays cheap.
///
/// Any source the OS refuses to open reads as zero (or `None` for the GPU)
/// instead of failing.
pub struct Collector {
    system: Option<System>,
    disks: Option<Disks>,
    gpu: GpuSampler,
    root: String,
    samples: u32,
    disk: (u64, u64),
}

impl Collector {
    pub fn new() -> Self {
        let mut system = System::new().ok();
        if let Some(sys) = system.as_mut() {
            // CPU usage is a delta between two refreshes; this is the first.
            sys.refresh_cpu_usage();
        }
        Collector {
            system,
            disks: Disks::new_with_refreshed_list().ok(),
            gpu: GpuSampler::new(),
            root: system_root(),
            samples: 0,
            disk: (0, 0),
        }
    }

    pub fn sample(&mut self) -> Snapshot {
        let (cpu_cores, mem_used, mem_total) = match self.system.as_mut() {
            Some(sys) => {
                sys.refresh_cpu_usage();
                sys.refresh_memory();
                let cores: Vec<f32> =
                    sys.cpus().iter().map(|c| clamp_pct(c.usage())).collect();
                let total = sys.total_memory();
                (cores, sys.used_memory().min(total), total)
            }
            None => (Vec::new(), 0, 0),
        };

        if self.samples % DISK_REFRESH_EVERY == 0 {
            self.disk = self.read_disk();
        }
        self.samples = self.samples.wrapping_add(1);

        Snapshot {
            cpu_total: mean(&cpu_cores),
            cpu_cores,
            gpu: self.gpu.sample(),
            mem_used,
            mem_total,
            disk_used: self.disk.0,
            disk_total: self.disk.1,
        }
    }

    fn read_disk(&mut self) -> (u64, u64) {
        let Some(disks) = self.disks.as_mut() else {
            return (0, 0);
        };
        disks.refresh(true);
        let infos: Vec<DiskInfo> = disks
            .iter()
            .map(|d| DiskInfo {
                mount: d.mount_point().to_string_lossy().into_owned(),
                total: d.total_space(),
                available: d.available_space(),
            })
            .collect();
        pick_system_disk(&infos, &self.root)
    }
}

/// Mount point of the disk the OS lives on.
pub fn system_root() -> String {
    if cfg!(target_os = "windows") {
        let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
        format!("{drive}\\")
    } else {
        "/".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_is_sane_on_this_machine() {
        let mut c = Collector::new();
        std::thread::sleep(std::time::Duration::from_millis(250));
        let s = c.sample();
        assert!(!s.cpu_cores.is_empty());
        assert!((0.0..=100.0).contains(&s.cpu_total));
        assert!(s.cpu_cores.iter().all(|v| (0.0..=100.0).contains(v)));
        assert!(s.mem_total > 0 && s.mem_used <= s.mem_total);
        assert!(s.disk_total > 0 && s.disk_used <= s.disk_total);
    }
}
