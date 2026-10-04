use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use windows::core::PWSTR;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

/// Frame times are summed until they cover this span; FPS = frames / span.
const WINDOW_MS: f32 = 1000.0;
/// A process that stopped presenting for this long is treated as having no FPS.
const STALE_AFTER: Duration = Duration::from_secs(2);
/// Processes not seen for this long are dropped from memory.
const FORGET_AFTER: Duration = Duration::from_secs(30);
/// Desktop Window Manager; its presents are the frames of the composed desktop.
const DESKTOP_COMPOSITOR: &str = "dwm.exe";
/// Share of the frame time the GPU must be busy for the frame to count as GPU-bound.
const GPU_BOUND_RATIO: f32 = 0.9;

/// Arguments for the PresentMon console sidecar. All processes are captured;
/// the foreground process is selected at read time.
pub const PRESENTMON_ARGS: &[&str] = &[
    "--output_stdout",
    "--no_console_stats",
    "--v2_metrics",
    "--no_track_input",
    "--session_name",
    "ChromaHUD",
    "--stop_existing_session",
];

#[derive(Clone, Copy)]
struct Columns {
    application: usize,
    process_id: usize,
    swap_chain: Option<usize>,
    frame_time: usize,
    gpu_busy: Option<usize>,
    display_latency: Option<usize>,
}

impl Columns {
    fn from_header(header: &str) -> Option<Self> {
        let names: Vec<&str> = header.split(',').map(str::trim).collect();
        let find = |name: &str| names.iter().position(|n| n.eq_ignore_ascii_case(name));

        Some(Self {
            application: find("Application")?,
            process_id: find("ProcessID")?,
            swap_chain: find("SwapChainAddress"),
            // v2 metrics name it FrameTime, v1 metrics MsBetweenPresents.
            frame_time: find("FrameTime").or_else(|| find("MsBetweenPresents"))?,
            gpu_busy: find("GPUBusy").or_else(|| find("MsGPUActive")),
            display_latency: find("DisplayLatency").or_else(|| find("MsUntilDisplayed")),
        })
    }
}

#[derive(Clone, Copy)]
struct Frame {
    time_ms: f32,
    /// NA when GPU tracking is unavailable.
    gpu_busy_ms: Option<f32>,
    /// NA for frames that were never displayed (dropped).
    display_latency_ms: Option<f32>,
}

/// Running sum and sample count of an optional per-frame value.
#[derive(Default)]
struct OptionalSum {
    total: f32,
    count: u32,
}

impl OptionalSum {
    fn add(&mut self, value: Option<f32>) {
        if let Some(v) = value {
            self.total += v;
            self.count += 1;
        }
    }

    fn remove(&mut self, value: Option<f32>) {
        if let Some(v) = value {
            self.total -= v;
            self.count -= 1;
        }
    }

    fn average(&self) -> Option<f32> {
        (self.count > 0).then(|| self.total / self.count as f32)
    }
}

#[derive(Default)]
struct SwapChainFrames {
    frames: VecDeque<Frame>,
    total_ms: f32,
    gpu_busy: OptionalSum,
    display_latency: OptionalSum,
}

impl SwapChainFrames {
    fn push(&mut self, frame: Frame) {
        self.frames.push_back(frame);
        self.total_ms += frame.time_ms;
        self.gpu_busy.add(frame.gpu_busy_ms);
        self.display_latency.add(frame.display_latency_ms);
        while self.total_ms > WINDOW_MS && self.frames.len() > 1 {
            if let Some(old) = self.frames.pop_front() {
                self.total_ms -= old.time_ms;
                self.gpu_busy.remove(old.gpu_busy_ms);
                self.display_latency.remove(old.display_latency_ms);
            }
        }
    }

    fn fps(&self) -> Option<f32> {
        (self.total_ms > 0.0).then(|| self.frames.len() as f32 * 1000.0 / self.total_ms)
    }

    fn reading(&self, application: &str) -> Option<FpsReading> {
        let fps = self.fps()?;
        let frame_time_ms = self.total_ms / self.frames.len() as f32;
        let gpu_busy_ms = self.gpu_busy.average();
        let bound = gpu_busy_ms.map(|busy| {
            if busy >= frame_time_ms * GPU_BOUND_RATIO {
                Bound::Gpu
            } else {
                Bound::Cpu
            }
        });
        Some(FpsReading {
            fps,
            application: application.to_owned(),
            frame_time_ms,
            gpu_busy_ms,
            display_latency_ms: self.display_latency.average(),
            bound,
        })
    }
}

struct ProcessFrames {
    application: String,
    last_seen: Instant,
    swap_chains: HashMap<String, SwapChainFrames>,
}

impl ProcessFrames {
    fn reading(&self) -> Option<FpsReading> {
        if self.last_seen.elapsed() > STALE_AFTER {
            return None;
        }
        // Games may own several swap chains (e.g. launcher + renderer); the busiest one is the game view.
        self.swap_chains
            .values()
            .filter_map(|frames| frames.reading(&self.application))
            .max_by(|a, b| a.fps.total_cmp(&b.fps))
    }
}

#[derive(Default)]
struct TrackerState {
    columns: Option<Columns>,
    processes: HashMap<u32, ProcessFrames>,
    last_cleanup: Option<Instant>,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Bound {
    Cpu,
    Gpu,
}

pub struct FpsReading {
    pub fps: f32,
    pub application: String,
    pub frame_time_ms: f32,
    pub gpu_busy_ms: Option<f32>,
    pub display_latency_ms: Option<f32>,
    pub bound: Option<Bound>,
}

/// Consumes PresentMon CSV lines and reports the FPS of the foreground process,
/// falling back to the desktop compositor when the foreground window renders nothing itself.
#[derive(Default)]
pub struct FpsTracker {
    state: Mutex<TrackerState>,
    capturing: AtomicBool,
}

impl FpsTracker {
    /// True once PresentMon has started streaming; false if it failed (usually missing admin rights).
    pub fn is_capturing(&self) -> bool {
        self.capturing.load(Ordering::Relaxed)
    }

    pub fn mark_stopped(&self) {
        self.capturing.store(false, Ordering::Relaxed);
    }

    pub fn ingest_line(&self, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }

        let mut state = self.state.lock().unwrap();
        if line.starts_with("Application,") {
            state.columns = Columns::from_header(line);
            self.capturing.store(state.columns.is_some(), Ordering::Relaxed);
            return;
        }
        let Some(columns) = state.columns else {
            return;
        };

        let fields: Vec<&str> = line.split(',').collect();
        let Some(pid) = fields.get(columns.process_id).and_then(|v| v.parse::<u32>().ok()) else {
            return;
        };
        let Some(frame_ms) = fields
            .get(columns.frame_time)
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|ms| *ms > 0.0 && *ms < 5000.0)
        else {
            return;
        };
        let swap_chain = columns
            .swap_chain
            .and_then(|i| fields.get(i))
            .copied()
            .unwrap_or_default()
            .to_owned();
        let application = fields.get(columns.application).copied().unwrap_or_default();
        let optional_ms = |index: Option<usize>| {
            index
                .and_then(|i| fields.get(i))
                .and_then(|v| v.parse::<f32>().ok())
                .filter(|ms| ms.is_finite() && *ms >= 0.0)
        };
        let frame = Frame {
            time_ms: frame_ms,
            gpu_busy_ms: optional_ms(columns.gpu_busy),
            display_latency_ms: optional_ms(columns.display_latency),
        };

        let now = Instant::now();
        let process = state.processes.entry(pid).or_insert_with(|| ProcessFrames {
            application: application.to_owned(),
            last_seen: now,
            swap_chains: HashMap::new(),
        });
        process.last_seen = now;
        process.swap_chains.entry(swap_chain).or_default().push(frame);

        let cleanup_due = state
            .last_cleanup
            .map_or(true, |t| now.duration_since(t) > FORGET_AFTER);
        if cleanup_due {
            state
                .processes
                .retain(|_, p| now.duration_since(p.last_seen) < FORGET_AFTER);
            state.last_cleanup = Some(now);
        }
    }

    pub fn reading(&self) -> Option<FpsReading> {
        if !self.is_capturing() {
            return None;
        }
        let state = self.state.lock().unwrap();

        if let Some(pid) = foreground_process_id() {
            if let Some(reading) = state.processes.get(&pid).and_then(ProcessFrames::reading) {
                return Some(reading);
            }

            // Multi-process apps (Chromium/Electron, some game launchers) own the window in one
            // process and present from a sibling process with the same executable name.
            if let Some(exe) = process_exe_name(pid) {
                let sibling = state
                    .processes
                    .values()
                    .filter(|p| p.application.eq_ignore_ascii_case(&exe))
                    .filter_map(ProcessFrames::reading)
                    .max_by(|a, b| a.fps.total_cmp(&b.fps));
                if sibling.is_some() {
                    return sibling;
                }
            }
        }

        // DWM only presents when something on screen changes, so an idle desktop really is ~0 FPS.
        // Composition is not a game workload, so the desktop gets no bound / latency verdict.
        let desktop_fps = state
            .processes
            .values()
            .find(|p| p.application.eq_ignore_ascii_case(DESKTOP_COMPOSITOR))
            .and_then(ProcessFrames::reading)
            .map_or(0.0, |r| r.fps);
        Some(FpsReading {
            fps: desktop_fps,
            application: "Desktop".to_owned(),
            frame_time_ms: if desktop_fps > 0.0 { 1000.0 / desktop_fps } else { 0.0 },
            gpu_busy_ms: None,
            display_latency_ms: None,
            bound: None,
        })
    }
}

fn foreground_process_id() -> Option<u32> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        (pid != 0).then_some(pid)
    }
}

/// Executable file name (e.g. "chrome.exe") of a process, matching PresentMon's Application column.
fn process_exe_name(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 1024];
        let mut len = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(handle);
        result.ok()?;

        let path = String::from_utf16_lossy(&buffer[..len as usize]);
        path.rsplit('\\').next().map(str::to_owned)
    }
}
