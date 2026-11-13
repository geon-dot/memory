//! Snapshot of system resource usage and the pure calculations behind it.

/// One sample of system resource usage, sent to the UI every second.
#[derive(serde::Serialize, Clone, Debug, PartialEq)]
pub struct Snapshot {
    /// Average CPU usage across cores, 0.0..=100.0.
    pub cpu_total: f32,
    /// Per-core CPU usage, each 0.0..=100.0.
    pub cpu_cores: Vec<f32>,
    /// GPU utilization, 0.0..=100.0, or `None` when it cannot be read.
    pub gpu: Option<f32>,
    /// Memory in bytes.
    pub mem_used: u64,
    pub mem_total: u64,
    /// System disk in bytes. `disk_total == 0` means the disk was not found.
    pub disk_used: u64,
    pub disk_total: u64,
}

/// A mounted disk as reported by the OS.
pub struct DiskInfo {
    pub mount: String,
    pub total: u64,
    pub available: u64,
}

/// Clamps a percentage into 0.0..=100.0. NaN becomes 0.0.
pub fn clamp_pct(v: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, 100.0)
    }
}

/// Arithmetic mean; 0.0 for an empty slice.
pub fn mean(values: &[f32]) -> f32 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f32>() / values.len() as f32
    }
}

/// Returns `(used, total)` bytes of the disk mounted at `root`, or `(0, 0)`
/// when no such disk exists.
pub fn pick_system_disk(disks: &[DiskInfo], root: &str) -> (u64, u64) {
    disks
        .iter()
        .find(|d| d.mount.eq_ignore_ascii_case(root))
        .map(|d| (d.total.saturating_sub(d.available), d.total))
        .unwrap_or((0, 0))
}

/// Short summary shown next to the tray icon, e.g. `CPU 37%  MEM 58%`.
pub fn tray_title(s: &Snapshot) -> String {
    let mem = if s.mem_total == 0 {
        0.0
    } else {
        s.mem_used as f64 / s.mem_total as f64 * 100.0
    };
    format!("CPU {:.0}%  MEM {:.0}%", clamp_pct(s.cpu_total), mem.min(100.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_handles_nan_and_range() {
        assert_eq!(clamp_pct(f32::NAN), 0.0);
        assert_eq!(clamp_pct(-3.0), 0.0);
        assert_eq!(clamp_pct(140.0), 100.0);
        assert_eq!(clamp_pct(42.5), 42.5);
    }

    #[test]
    fn mean_of_empty_is_zero() {
        assert_eq!(mean(&[]), 0.0);
        assert_eq!(mean(&[10.0, 30.0]), 20.0);
    }

    #[test]
    fn picks_disk_by_mount_case_insensitive() {
        let d = vec![
            DiskInfo { mount: "/System/Volumes/Data".into(), total: 500, available: 100 },
            DiskInfo { mount: "/".into(), total: 500, available: 188 },
        ];
        assert_eq!(pick_system_disk(&d, "/"), (312, 500));
        let w = vec![DiskInfo { mount: "C:\\".into(), total: 1000, available: 400 }];
        assert_eq!(pick_system_disk(&w, "c:\\"), (600, 1000));
    }

    #[test]
    fn missing_disk_is_zero() {
        assert_eq!(pick_system_disk(&[], "/"), (0, 0));
    }

    #[test]
    fn available_larger_than_total_does_not_underflow() {
        let d = vec![DiskInfo { mount: "/".into(), total: 10, available: 20 }];
        assert_eq!(pick_system_disk(&d, "/"), (0, 10));
    }

    #[test]
    fn tray_title_formats_and_survives_zero_total() {
        let mut s = Snapshot {
            cpu_total: 36.6,
            cpu_cores: vec![],
            gpu: None,
            mem_used: 58,
            mem_total: 100,
            disk_used: 0,
            disk_total: 0,
        };
        assert_eq!(tray_title(&s), "CPU 37%  MEM 58%");
        s.mem_total = 0;
        assert_eq!(tray_title(&s), "CPU 37%  MEM 0%");
    }
}
