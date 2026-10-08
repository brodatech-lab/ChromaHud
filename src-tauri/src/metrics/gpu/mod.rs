mod amd;
mod nvidia;
mod pdh;

use serde::Serialize;

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GpuMetrics {
    pub index: u32,
    pub vendor: &'static str,
    pub name: String,
    pub usage: Option<u32>,
    pub temp_c: Option<u32>,
    pub core_clock_mhz: Option<u32>,
    pub mem_clock_mhz: Option<u32>,
    pub vram_used_bytes: Option<u64>,
    pub vram_total_bytes: Option<u64>,
    pub power_w: Option<f32>,
    pub power_limit_w: Option<f32>,
    pub fan_percent: Option<u32>,
    pub fan_rpm: Option<u32>,
    /// "Thermal" | "Power" | "Sync" while clocks are being limited.
    pub throttle_reason: Option<&'static str>,
    pub pstate: Option<u8>,
}

/// A vendor-specific source of GPU telemetry.
pub trait GpuProvider {
    fn name(&self) -> &str;
    fn vendor(&self) -> &'static str;
    fn sample(&mut self) -> Option<GpuMetrics>;
    /// Extra detail for `--dump-metrics`.
    fn diagnostics(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
    /// True when the vendor usage value is less reliable than the Windows counters.
    fn prefers_counter_usage(&self) -> bool {
        false
    }
}

/// A vendor provider whose missing usage / memory values are filled from the Windows GPU counters.
struct WithPdhFallback<P> {
    inner: P,
    pdh: Option<pdh::PdhGpu>,
}

impl<P: GpuProvider> WithPdhFallback<P> {
    fn new(inner: P) -> Self {
        Self { inner, pdh: pdh::PdhGpu::new() }
    }
}

impl<P: GpuProvider> GpuProvider for WithPdhFallback<P> {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn vendor(&self) -> &'static str {
        self.inner.vendor()
    }

    fn sample(&mut self) -> Option<GpuMetrics> {
        let mut metrics = self.inner.sample()?;
        let prefer_counters = self.inner.prefers_counter_usage();
        if prefer_counters || metrics.usage.is_none() || metrics.vram_used_bytes.is_none() {
            if let Some(sample) = self.pdh.as_mut().map(|p| {
                if prefer_counters {
                    p.sample_integrated()
                } else {
                    p.sample()
                }
            }) {
                metrics.usage = if prefer_counters {
                    sample.usage.or(metrics.usage)
                } else {
                    metrics.usage.or(sample.usage)
                };
                metrics.vram_used_bytes = metrics.vram_used_bytes.or(sample.vram_used_bytes);
            }
        }
        Some(metrics)
    }

    fn diagnostics(&self) -> serde_json::Value {
        self.inner.diagnostics()
    }
}

/// Every vendor GPU, then Windows-counter adapters whose names are not already covered.
pub fn detect_gpus() -> Vec<Box<dyn GpuProvider>> {
    let mut providers: Vec<Box<dyn GpuProvider>> = Vec::new();
    for nvidia in nvidia::NvidiaProvider::all() {
        providers.push(Box::new(nvidia));
    }
    for amd in amd::AmdProvider::all() {
        providers.push(Box::new(WithPdhFallback::new(amd)));
    }
    let covered: Vec<String> = providers.iter().map(|p| p.name().to_owned()).collect();
    for leftover in pdh::PdhProvider::leftover(&covered) {
        providers.push(Box::new(leftover));
    }
    providers
}
