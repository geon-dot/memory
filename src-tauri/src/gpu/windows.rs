//! GPU utilization on Windows, read from the `GPU Engine` performance
//! counters — the same source Task Manager uses.

use std::collections::HashMap;

use crate::snapshot::clamp_pct;

/// Reduces per-process `GPU Engine` counter instances to one utilization
/// figure the way Task Manager does: sum the instances of each engine type
/// (`3D`, `VideoDecode`, ...) and report the busiest type.
///
/// Instance names look like `pid_1_luid_0x0_0x1_phys_0_eng_0_engtype_3D`.
/// Names without an `engtype_` part are ignored; `None` means nothing usable
/// was reported.
pub fn aggregate_engines(samples: &[(String, f64)]) -> Option<f32> {
    let mut per_type: HashMap<&str, f64> = HashMap::new();
    for (name, value) in samples {
        if let Some((_, engine_type)) = name.rsplit_once("engtype_") {
            if value.is_finite() {
                *per_type.entry(engine_type).or_insert(0.0) += value;
            }
        }
    }
    per_type
        .values()
        .copied()
        .reduce(f64::max)
        .map(|v| clamp_pct(v as f32))
}

#[cfg(target_os = "windows")]
pub use pdh::PdhGpu;

#[cfg(target_os = "windows")]
mod pdh {
    use windows::core::{w, PCWSTR};
    use windows::Win32::System::Performance::{
        PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData,
        PdhGetFormattedCounterArrayW, PdhOpenQueryW, PDH_CSTATUS_NEW_DATA,
        PDH_CSTATUS_VALID_DATA, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER,
        PDH_HQUERY, PDH_MORE_DATA,
    };

    use super::aggregate_engines;

    const ERROR_SUCCESS: u32 = 0;

    /// An open PDH query over every `GPU Engine` instance.
    pub struct PdhGpu {
        query: PDH_HQUERY,
        counter: PDH_HCOUNTER,
    }

    impl PdhGpu {
        /// Opens the query, or `None` when the counters do not exist (no
        /// WDDM 2.0 GPU driver, or performance counters disabled).
        pub fn new() -> Option<Self> {
            let mut query = PDH_HQUERY::default();
            // SAFETY: `query` is a valid out-pointer.
            if unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut query) } != ERROR_SUCCESS {
                return None;
            }
            // From here on `this` owns the query and closes it on drop.
            let mut this = PdhGpu { query, counter: PDH_HCOUNTER::default() };
            // SAFETY: the query handle is open and `counter` is a valid out-pointer.
            let status = unsafe {
                PdhAddEnglishCounterW(
                    this.query,
                    w!("\\GPU Engine(*)\\Utilization Percentage"),
                    0,
                    &mut this.counter,
                )
            };
            if status != ERROR_SUCCESS {
                return None;
            }
            // Rate counters need two collections; this is the baseline.
            // SAFETY: the query handle is open.
            unsafe { PdhCollectQueryData(this.query) };
            Some(this)
        }

        pub fn sample(&mut self) -> Option<f32> {
            // SAFETY: the query handle is open.
            if unsafe { PdhCollectQueryData(self.query) } != ERROR_SUCCESS {
                return None;
            }

            let mut bytes: u32 = 0;
            let mut count: u32 = 0;
            // SAFETY: a null buffer with size 0 asks PDH for the needed size.
            let status = unsafe {
                PdhGetFormattedCounterArrayW(
                    self.counter,
                    PDH_FMT_DOUBLE,
                    &mut bytes,
                    &mut count,
                    None,
                )
            };
            if status != PDH_MORE_DATA || bytes == 0 {
                return None;
            }

            // u64 storage keeps the buffer aligned for the item structs; the
            // instance-name strings PDH writes live in the same buffer.
            let mut buffer = vec![0u64; (bytes as usize).div_ceil(8)];
            let items = buffer.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
            // SAFETY: `buffer` is at least `bytes` long and suitably aligned.
            let status = unsafe {
                PdhGetFormattedCounterArrayW(
                    self.counter,
                    PDH_FMT_DOUBLE,
                    &mut bytes,
                    &mut count,
                    Some(items),
                )
            };
            if status != ERROR_SUCCESS {
                return None;
            }

            let mut samples = Vec::with_capacity(count as usize);
            for i in 0..count as usize {
                // SAFETY: PDH filled `count` items at the start of `buffer`,
                // and each `szName` points at a NUL-terminated string inside it.
                let item = unsafe { &*items.add(i) };
                let status = item.FmtValue.CStatus;
                if status != PDH_CSTATUS_VALID_DATA && status != PDH_CSTATUS_NEW_DATA {
                    continue;
                }
                if item.szName.is_null() {
                    continue;
                }
                // SAFETY: see above; PDH_FMT_DOUBLE selects `doubleValue`.
                let (name, value) = unsafe {
                    (item.szName.to_string(), item.FmtValue.Anonymous.doubleValue)
                };
                if let Ok(name) = name {
                    samples.push((name, value));
                }
            }
            aggregate_engines(&samples)
        }
    }

    impl Drop for PdhGpu {
        fn drop(&mut self) {
            // SAFETY: the query handle was opened by PdhOpenQueryW and is
            // closed exactly once; closing it frees its counters too.
            unsafe { PdhCloseQuery(self.query) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_per_engine_type_and_takes_max() {
        let s = vec![
            ("pid_1_luid_0x0_0x1_phys_0_eng_0_engtype_3D".to_string(), 20.0),
            ("pid_2_luid_0x0_0x1_phys_0_eng_0_engtype_3D".to_string(), 15.5),
            ("pid_1_luid_0x0_0x1_phys_0_eng_1_engtype_VideoDecode".to_string(), 30.0),
        ];
        assert_eq!(aggregate_engines(&s), Some(35.5));
    }

    #[test]
    fn clamps_over_100() {
        let s = vec![
            ("a_engtype_3D".to_string(), 80.0),
            ("b_engtype_3D".to_string(), 70.0),
        ];
        assert_eq!(aggregate_engines(&s), Some(100.0));
    }

    #[test]
    fn empty_or_unparseable_is_none() {
        assert_eq!(aggregate_engines(&[]), None);
        assert_eq!(aggregate_engines(&[("garbage".to_string(), 50.0)]), None);
    }
}
