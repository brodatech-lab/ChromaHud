use std::sync::Arc;

use nvml_wrapper::bitmasks::device::ThrottleReasons;
use nvml_wrapper::enum_wrappers::device::{Clock, PerformanceState, TemperatureSensor};
use nvml_wrapper::Nvml;

use super::{GpuMetrics, GpuProvider};

/// NVIDIA telemetry through NVML (nvml.dll ships with the display driver, no elevation needed).
pub struct NvidiaProvider {
    nvml: Arc<Nvml>,
    index: u32,
    name: String,
}

impl NvidiaProvider {
    /// One provider per NVML device, sharing a single library handle.
    pub fn all() -> Vec<Self> {
        let Ok(nvml) = Nvml::init() else {
            return Vec::new();
        };
        let Ok(count) = nvml.device_count() else {
            return Vec::new();
        };
        let nvml = Arc::new(nvml);
        (0..count)
            .filter_map(|index| {
                let name = nvml.device_by_index(index).ok()?.name().ok()?;
                Some(Self { nvml: nvml.clone(), index, name })
            })
            .collect()
    }
}

impl GpuProvider for NvidiaProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn vendor(&self) -> &'static str {
        "NVIDIA"
    }

    fn sample(&mut self) -> Option<GpuMetrics> {
        // `Device` borrows `Nvml`, so the handle is looked up per sample (a cheap driver call).
        let device = self.nvml.device_by_index(self.index).ok()?;
        let memory = device.memory_info().ok();

        Some(GpuMetrics {
            vendor: "NVIDIA",
            name: self.name.clone(),
            usage: device.utilization_rates().ok().map(|u| u.gpu),
            temp_c: device.temperature(TemperatureSensor::Gpu).ok(),
            core_clock_mhz: device.clock_info(Clock::Graphics).ok(),
            mem_clock_mhz: device.clock_info(Clock::Memory).ok(),
            vram_used_bytes: memory.as_ref().map(|m| m.used),
            vram_total_bytes: memory.as_ref().map(|m| m.total),
            power_w: device.power_usage().ok().map(|mw| mw as f32 / 1000.0),
            power_limit_w: device.enforced_power_limit().ok().map(|mw| mw as f32 / 1000.0),
            fan_percent: device.fan_speed(0).ok(),
            fan_rpm: device.fan_speed_rpm(0).ok().filter(|rpm| *rpm > 0),
            throttle_reason: device.current_throttle_reasons().ok().and_then(throttle_label),
            pstate: device.performance_state().ok().and_then(pstate_number),
            ..Default::default()
        })
    }

    fn diagnostics(&self) -> serde_json::Value {
        serde_json::json!({ "provider": "nvml", "index": self.index, "name": self.name })
    }
}

/// The most significant active limiter; idle and clock-setting reasons are not throttling.
fn throttle_label(reasons: ThrottleReasons) -> Option<&'static str> {
    let thermal = ThrottleReasons::SW_THERMAL_SLOWDOWN
        | ThrottleReasons::HW_THERMAL_SLOWDOWN
        | ThrottleReasons::HW_SLOWDOWN;
    let power = ThrottleReasons::SW_POWER_CAP | ThrottleReasons::HW_POWER_BRAKE_SLOWDOWN;

    if reasons.intersects(thermal) {
        Some("Thermal")
    } else if reasons.intersects(power) {
        Some("Power")
    } else if reasons.contains(ThrottleReasons::SYNC_BOOST) {
        Some("Sync")
    } else {
        None
    }
}

fn pstate_number(state: PerformanceState) -> Option<u8> {
    use PerformanceState::*;
    Some(match state {
        Zero => 0,
        One => 1,
        Two => 2,
        Three => 3,
        Four => 4,
        Five => 5,
        Six => 6,
        Seven => 7,
        Eight => 8,
        Nine => 9,
        Ten => 10,
        Eleven => 11,
        Twelve => 12,
        Thirteen => 13,
        Fourteen => 14,
        Fifteen => 15,
        _ => return None,
    })
}
