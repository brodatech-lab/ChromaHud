mod cpu_sensors;
mod disk;
mod display;
pub mod fps;
mod gpu;
mod memory;
mod pawnio;
mod system;
mod topology;

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::OVERLAY_LABEL;
use fps::FpsTracker;
use gpu::GpuMetrics;

const SAMPLE_INTERVAL: Duration = Duration::from_millis(500);
const HIDDEN_INTERVAL: Duration = Duration::from_millis(1500);

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CoreMetrics {
    pub index: usize,
    pub usage: f32,
    pub clock_mhz: Option<u32>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    pub fps: Option<f32>,
    pub fps_process: Option<String>,
    pub fps_capturing: bool,
    pub frame_time_ms: Option<f32>,
    pub gpu_busy_ms: Option<f32>,
    pub display_latency_ms: Option<f32>,
    pub bound: Option<fps::Bound>,
    pub cpu_usage: f32,
    pub cpu_clock_mhz: u32,
    pub cpu_temp_c: Option<f32>,
    pub cpu_power_w: Option<f32>,
    pub cores: Vec<CoreMetrics>,
    pub ccd_temps_c: Vec<Option<f32>>,
    pub ram_used_bytes: u64,
    pub ram_total_bytes: u64,
    pub disk_read_bps: Option<f64>,
    pub disk_write_bps: Option<f64>,
    pub gpu: Option<GpuMetrics>,
    pub screen_width: u32,
    pub screen_height: u32,
    pub refresh_hz: u32,
}

/// Static hardware description, available once the collector thread has probed the sensors.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub cpu_model: String,
    pub gpu_model: Option<String>,
    /// "pawnio" | "acpi"
    pub cpu_sensor_source: &'static str,
    pub physical_cores: usize,
    /// e.g. "DDR5-6000"
    pub ram_speed: Option<String>,
}

#[derive(Default)]
pub struct SystemInfoState(pub Mutex<Option<SystemInfo>>);

#[tauri::command]
pub fn get_system_info(state: tauri::State<'_, SystemInfoState>) -> Option<SystemInfo> {
    state.0.lock().unwrap().clone()
}

/// Samples every metric source on one background thread and pushes a `metrics` event to the overlay.
pub fn spawn_collector(app: AppHandle, fps_tracker: Arc<FpsTracker>) {
    let _ = thread::Builder::new()
        .name("metrics".into())
        .spawn(move || {
            let mut system = system::SystemSampler::new();
            let cores = topology::physical_cores(system.logical_count());
            let mut cpu_sensors = cpu_sensors::CpuSensors::new(system.base_mhz(), &cores);
            let mut gpu_provider = gpu::detect_provider();
            let mut disk = disk::DiskSampler::new();

            let info = SystemInfo {
                cpu_model: system.cpu_model(),
                gpu_model: gpu_provider.as_ref().map(|p| p.name().to_owned()),
                cpu_sensor_source: cpu_sensors.source().as_str(),
                physical_cores: cores.len(),
                ram_speed: memory::ram_speed_label(),
            };
            *app.state::<SystemInfoState>().0.lock().unwrap() = Some(info.clone());
            let _ = app.emit("system-info", &info);

            loop {
                let overlay_visible = app
                    .get_webview_window(OVERLAY_LABEL)
                    .and_then(|w| w.is_visible().ok())
                    .unwrap_or(false);
                if !overlay_visible {
                    thread::sleep(HIDDEN_INTERVAL);
                    continue;
                }

                let sys = system.sample();
                let sensors = cpu_sensors.sample(&cores);
                let fps = fps_tracker.reading();
                let display = display::primary_display_mode();
                let disk_sample = disk.as_mut().map(|d| d.sample());

                let core_metrics = cores
                    .iter()
                    .enumerate()
                    .map(|(index, core)| CoreMetrics {
                        index,
                        usage: core
                            .logical
                            .iter()
                            .filter_map(|&i| sys.logical_usage.get(i))
                            .sum::<f32>()
                            / core.logical.len() as f32,
                        clock_mhz: sensors.core_clocks_mhz.get(index).copied().flatten(),
                    })
                    .collect();

                let metrics = Metrics {
                    fps: fps.as_ref().map(|r| r.fps),
                    frame_time_ms: fps.as_ref().map(|r| r.frame_time_ms).filter(|ms| *ms > 0.0),
                    gpu_busy_ms: fps.as_ref().and_then(|r| r.gpu_busy_ms),
                    display_latency_ms: fps.as_ref().and_then(|r| r.display_latency_ms),
                    bound: fps.as_ref().and_then(|r| r.bound),
                    fps_process: fps.map(|r| r.application),
                    fps_capturing: fps_tracker.is_capturing(),
                    cpu_usage: sys.cpu_usage,
                    cpu_clock_mhz: sensors.average_clock_mhz.unwrap_or(sys.cpu_clock_mhz),
                    cpu_temp_c: sensors.temp_c,
                    cpu_power_w: sensors.power_w,
                    cores: core_metrics,
                    ccd_temps_c: sensors.ccd_temps_c,
                    ram_used_bytes: sys.ram_used_bytes,
                    ram_total_bytes: sys.ram_total_bytes,
                    disk_read_bps: disk_sample.as_ref().and_then(|d| d.read_bps),
                    disk_write_bps: disk_sample.as_ref().and_then(|d| d.write_bps),
                    gpu: gpu_provider.as_mut().and_then(|p| p.sample()),
                    screen_width: display.as_ref().map_or(0, |d| d.width),
                    screen_height: display.as_ref().map_or(0, |d| d.height),
                    refresh_hz: display.as_ref().map_or(0, |d| d.refresh_hz),
                };

                let _ = app.emit_to(OVERLAY_LABEL, "metrics", &metrics);
                thread::sleep(SAMPLE_INTERVAL);
            }
        });
}
