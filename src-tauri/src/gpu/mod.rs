//! GPU utilization, read through whichever OS facility is available.

#[cfg(target_os = "macos")]
mod macos;

/// Reads GPU utilization. Never panics: when the OS offers no reading,
/// `sample` returns `None`.
pub struct GpuSampler {}

impl GpuSampler {
    pub fn new() -> Self {
        GpuSampler {}
    }

    /// Current GPU utilization, 0.0..=100.0, or `None` when unavailable.
    pub fn sample(&mut self) -> Option<f32> {
        #[cfg(target_os = "macos")]
        {
            macos::sample()
        }
        #[cfg(not(target_os = "macos"))]
        {
            None
        }
    }
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

    /// Prints the live reading: `cargo test print_gpu -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn print_gpu() {
        println!("gpu = {:?}", GpuSampler::new().sample());
    }
}
