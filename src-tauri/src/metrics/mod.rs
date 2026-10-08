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
    pub fps_game: Option<String>,
    pub fps_capturing: bool,
    pub fps_paused: bool,
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
    pub gpus: Vec<GpuMetrics>,
    pub screen_width: u32,
    pub screen_height: u32,
    pub refresh_hz: u32,
}

/// One detected GPU, used by settings to build GPU 1 / GPU 2 groups.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub name: String,
    pub vendor: String,
}

/// Static hardware description, available once the collector thread has probed the sensors.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub cpu_model: String,
    pub gpu_model: Option<String>,
    pub gpus: Vec<GpuInfo>,
    /// "pawnio" | "acpi"
    pub cpu_sensor_source: &'static str,
    pub physical_cores: usize,
    /// e.g. "DDR5-6000"
    pub ram_speed: Option<String>,
    /// SPD manufacturer (not the module part number).
    pub ram_manufacturer: Option<String>,
}

#[derive(Default)]
pub struct SystemInfoState(pub Mutex<Option<SystemInfo>>);

#[tauri::command]
pub fn get_system_info(state: tauri::State<'_, SystemInfoState>) -> Option<SystemInfo> {
    state.0.lock().unwrap().clone()
}

/// Every metric source, probed once and then sampled on demand.
struct Collector {
    system: system::SystemSampler,
    cores: Vec<topology::PhysicalCore>,
    cpu_sensors: cpu_sensors::CpuSensors,
    gpu_providers: Vec<Box<dyn gpu::GpuProvider>>,
    disk: Option<disk::DiskSampler>,
}

impl Collector {
    fn new() -> (Self, SystemInfo) {
        let system = system::SystemSampler::new();
        let cores = topology::physical_cores(system.logical_count());
        let cpu_sensors = cpu_sensors::CpuSensors::new(system.base_mhz(), &cores);
        let gpu_providers = gpu::detect_gpus();
        let gpus: Vec<GpuInfo> = gpu_providers
            .iter()
            .map(|p| GpuInfo {
                name: p.name().to_owned(),
                vendor: p.vendor().to_owned(),
            })
            .collect();

        let ram = memory::probe();
        let info = SystemInfo {
            cpu_model: system.cpu_model(),
            gpu_model: gpus.first().map(|g| g.name.clone()),
            gpus,
            cpu_sensor_source: cpu_sensors.source().as_str(),
            physical_cores: cores.len(),
            ram_speed: ram.speed,
            ram_manufacturer: ram.manufacturer,
        };
        let collector = Self {
            system,
            cores,
            cpu_sensors,
            gpu_providers,
            disk: disk::DiskSampler::new(),
        };
        (collector, info)
    }

    fn sample(&mut self, fps_tracker: &FpsTracker) -> Metrics {
        let sys = self.system.sample();
        let sensors = self.cpu_sensors.sample(&self.cores);
        let fps = fps_tracker.reading();
        let display = display::primary_display_mode();
        let disk_sample = self.disk.as_mut().map(|d| d.sample());

        let core_metrics = self
            .cores
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
                clock_mhz: sensors
                    .core_clocks_mhz
                    .get(index)
                    .copied()
                    .flatten()
                    .or_else(|| average_logical_clock(&core.logical, &sys.logical_clock_mhz)),
            })
            .collect();

        Metrics {
            fps: fps.as_ref().map(|r| r.fps),
            frame_time_ms: fps.as_ref().map(|r| r.frame_time_ms).filter(|ms| *ms > 0.0),
            gpu_busy_ms: fps.as_ref().and_then(|r| r.gpu_busy_ms),
            display_latency_ms: fps.as_ref().and_then(|r| r.display_latency_ms),
            bound: fps.as_ref().and_then(|r| r.bound),
            fps_paused: fps.as_ref().is_some_and(|r| r.paused),
            fps_process: fps.as_ref().map(|r| r.application.clone()),
            fps_game: fps.as_ref().and_then(|r| r.game_name.clone()),
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
            gpus: self
                .gpu_providers
                .iter_mut()
                .enumerate()
                .filter_map(|(index, provider)| {
                    let mut metrics = provider.sample()?;
                    metrics.index = index as u32;
                    Some(metrics)
                })
                .collect(),
            screen_width: display.as_ref().map_or(0, |d| d.width),
            screen_height: display.as_ref().map_or(0, |d| d.height),
            refresh_hz: display.as_ref().map_or(0, |d| d.refresh_hz),
        }
    }
}

fn average_logical_clock(logical: &[usize], clocks: &[Option<u32>]) -> Option<u32> {
    let values: Vec<u32> = logical
        .iter()
        .filter_map(|&i| clocks.get(i).copied().flatten())
        .collect();
    if values.is_empty() {
        return None;
    }
    Some(values.iter().sum::<u32>() / values.len() as u32)
}

/// Samples every metric source on one background thread and pushes a `metrics` event to the overlay.
pub fn spawn_collector(app: AppHandle, fps_tracker: Arc<FpsTracker>) {
    let _ = thread::Builder::new()
        .name("metrics".into())
        .spawn(move || {
            let (mut collector, info) = Collector::new();
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

                let metrics = collector.sample(&fps_tracker);
                let _ = app.emit_to(OVERLAY_LABEL, "metrics", &metrics);
                thread::sleep(SAMPLE_INTERVAL);
            }
        });
}

/// Headless diagnostics: prints `samples` metric snapshots (one second apart) as JSON lines,
/// without windows or PresentMon, so sensors can be checked over SSH.
pub fn dump(samples: u32) {
    let (mut collector, info) = Collector::new();
    let fps_tracker = FpsTracker::default();
    let gpus: Vec<serde_json::Value> = collector.gpu_providers.iter().map(|p| p.diagnostics()).collect();
    println!("{}", serde_json::json!({ "systemInfo": info, "gpuProviders": gpus }));
    for _ in 0..samples {
        thread::sleep(Duration::from_secs(1));
        let metrics = collector.sample(&fps_tracker);
        println!("{}", serde_json::json!({ "metrics": metrics }));
    }
}
