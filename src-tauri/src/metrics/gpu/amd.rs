use std::sync::Arc;

use adlx::{ffi, AdlxHelper, Gpu, GpuMetricsSupport, PerformanceMonitoringServices};
use serde::Serialize;

use super::{GpuMetrics, GpuProvider};

/// Which `IADLXGPUMetrics` values the driver reports for this GPU, queried once.
#[derive(Serialize, Clone, Copy, Default)]
#[serde(rename_all = "camelCase")]
struct Support {
    usage: bool,
    clock: bool,
    vram_clock: bool,
    temperature: bool,
    hotspot: bool,
    power: bool,
    board_power: bool,
    fan: bool,
    vram: bool,
}

impl Support {
    fn query(support: &GpuMetricsSupport) -> Self {
        let flag = |r: adlx::Result<bool>| r.unwrap_or(false);
        Self {
            usage: flag(support.is_supported_gpu_usage()),
            clock: flag(support.is_supported_gpu_clock_speed()),
            vram_clock: flag(support.is_supported_gpu_vram_clock_speed()),
            temperature: flag(support.is_supported_gpu_temperature()),
            hotspot: flag(support.is_supported_gpu_hotspot_temperature()),
            power: flag(support.is_supported_gpu_power()),
            board_power: flag(support.is_supported_gpu_total_board_power()),
            fan: flag(support.is_supported_gpu_fan_speed()),
            vram: flag(support.is_supported_gpu_vram()),
        }
    }
}

/// Shared ADLX session. GPU handles must be dropped before this (field order on [`AmdProvider`]).
struct AmdShared {
    services: PerformanceMonitoringServices,
    _helper: AdlxHelper,
}

/// AMD Radeon telemetry through ADLX (amdadlx64.dll ships with the Adrenalin driver).
pub struct AmdProvider {
    // GPU handle first so it drops before the shared ADLX session.
    gpu: Gpu,
    support: Support,
    name: String,
    integrated: bool,
    total_vram_bytes: Option<u64>,
    shared: Arc<AmdShared>,
}

impl AmdProvider {
    /// One provider per ADLX GPU, including the integrated part next to a discrete card.
    pub fn all() -> Vec<Self> {
        let Ok(helper) = AdlxHelper::new() else {
            return Vec::new();
        };
        let Ok(list) = helper.system().gpus() else {
            return Vec::new();
        };
        let Ok(services) = helper.system().performance_monitoring_services() else {
            return Vec::new();
        };

        // Walk the list before moving `helper` into the Arc. Relocating the helper first
        // leaves ADLX GPU list pointers dangling (access violation on this machine).
        let mut parts = Vec::new();
        for gpu in list.iter() {
            let Ok(metrics_support) = services.supported_gpu_metrics(&gpu) else {
                continue;
            };
            let Ok(name) = gpu.name() else {
                continue;
            };
            let name = name.trim().to_owned();
            if name.is_empty() {
                continue;
            }
            let integrated = gpu.type_().ok() == Some(ffi::ADLX_GPU_TYPE_GPUTYPE_INTEGRATED);
            let total_vram_bytes = gpu
                .total_vram()
                .ok()
                .filter(|mb| *mb > 0)
                .map(|mb| u64::from(mb) * 1024 * 1024);
            parts.push((gpu, Support::query(&metrics_support), name, integrated, total_vram_bytes));
        }
        drop(list);

        let shared = Arc::new(AmdShared { services, _helper: helper });
        parts
            .into_iter()
            .map(|(gpu, support, name, integrated, total_vram_bytes)| Self {
                gpu,
                support,
                name,
                integrated,
                total_vram_bytes,
                shared: shared.clone(),
            })
            .collect()
    }
}

fn read<T>(supported: bool, value: adlx::Result<T>) -> Option<T> {
    supported.then(|| value.ok()).flatten()
}

impl GpuProvider for AmdProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn vendor(&self) -> &'static str {
        "AMD"
    }

    fn sample(&mut self) -> Option<GpuMetrics> {
        let m = self.shared.services.current_gpu_metrics(&self.gpu).ok()?;
        let s = self.support;

        let temp_c = read(s.temperature, m.temperature())
            .or_else(|| read(s.hotspot, m.hotspot_temperature()))
            .filter(|t| *t > 0.0);
        let power_w = read(s.board_power, m.total_board_power())
            .or_else(|| read(s.power, m.power()))
            .filter(|w| *w > 0.0);

        Some(GpuMetrics {
            vendor: "AMD",
            name: self.name.clone(),
            usage: read(s.usage, m.usage()).map(|u| u.round().clamp(0.0, 100.0) as u32),
            temp_c: temp_c.map(|t| t.round() as u32),
            core_clock_mhz: read(s.clock, m.clock_speed()).and_then(|c| u32::try_from(c).ok()),
            mem_clock_mhz: read(s.vram_clock, m.vram_clock_speed())
                .and_then(|c| u32::try_from(c).ok())
                .filter(|c| *c > 0),
            vram_used_bytes: read(s.vram, m.vram())
                .and_then(|mb| u64::try_from(mb).ok())
                .map(|mb| mb * 1024 * 1024),
            vram_total_bytes: self.total_vram_bytes,
            power_w: power_w.map(|w| w as f32),
            fan_rpm: read(s.fan, m.fan_speed())
                .and_then(|rpm| u32::try_from(rpm).ok())
                .filter(|rpm| *rpm > 0),
            ..Default::default()
        })
    }

    /// On integrated Radeons ADLX reports instantaneous usage that flips between 0 and 100%.
    fn prefers_counter_usage(&self) -> bool {
        self.integrated
    }

    fn diagnostics(&self) -> serde_json::Value {
        serde_json::json!({
            "provider": "adlx",
            "name": self.name,
            "integrated": self.integrated,
            "totalVramBytes": self.total_vram_bytes,
            "supported": self.support,
        })
    }
}
