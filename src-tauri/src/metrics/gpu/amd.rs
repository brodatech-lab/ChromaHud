use super::{GpuMetrics, GpuProvider};

/// Placeholder for AMD Radeon telemetry (planned: ADLX). Not detected yet.
pub struct AmdProvider;

impl AmdProvider {
    pub fn new() -> Option<Self> {
        None
    }
}

impl GpuProvider for AmdProvider {
    fn name(&self) -> &str {
        ""
    }

    fn sample(&mut self) -> Option<GpuMetrics> {
        None
    }
}
