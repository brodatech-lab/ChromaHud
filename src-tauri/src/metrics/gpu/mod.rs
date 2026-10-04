mod amd;
mod nvidia;

use serde::Serialize;

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GpuMetrics {
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
    fn sample(&mut self) -> Option<GpuMetrics>;
}

/// Picks the first vendor library that loads on this machine.
pub fn detect_provider() -> Option<Box<dyn GpuProvider>> {
    if let Some(provider) = nvidia::NvidiaProvider::new() {
        return Some(Box::new(provider));
    }
    if let Some(provider) = amd::AmdProvider::new() {
        return Some(Box::new(provider));
    }
    None
}
