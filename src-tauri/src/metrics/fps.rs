use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND, RECT, STILL_ACTIVE};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
use windows::Win32::System::Threading::{
    GetExitCodeProcess, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId};

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
            paused: false,
            game_name: None,
        })
    }
}

struct ProcessFrames {
    application: String,
    last_seen: Instant,
    swap_chains: HashMap<String, SwapChainFrames>,
}

impl ProcessFrames {
    fn reading(&self, now: Instant) -> Option<FpsReading> {
        if now.saturating_duration_since(self.last_seen) > STALE_AFTER {
            return None;
        }
        // Games may own several swap chains (e.g. launcher + renderer); the busiest one is the game view.
        self.swap_chains
            .values()
            .filter_map(|frames| frames.reading(&self.application))
            .max_by(|a, b| a.fps.total_cmp(&b.fps))
    }
}

/// The game the overlay follows: the last process that presented frames while in the foreground.
struct Target {
    pid: u32,
    application: String,
}

#[derive(Default)]
struct TrackerState {
    columns: Option<Columns>,
    processes: HashMap<u32, ProcessFrames>,
    last_cleanup: Option<Instant>,
    target: Option<Target>,
    /// FileDescription / ProductName of a process image, looked up once per pid.
    game_names: HashMap<u32, Option<String>>,
}

/// The process owning the foreground window.
struct Foreground {
    pid: u32,
    /// Executable file name, used to match sibling processes (Chromium, launchers).
    exe: Option<String>,
    /// The window covers its whole monitor (exclusive or borderless full screen).
    covers_monitor: bool,
}

impl TrackerState {
    /// Best fresh reading of the foreground process or of a sibling with the same executable.
    fn foreground_reading(&self, foreground: &Foreground, now: Instant) -> Option<(u32, FpsReading)> {
        if let Some(reading) = self.processes.get(&foreground.pid).and_then(|p| p.reading(now)) {
            return Some((foreground.pid, reading));
        }
        let exe = foreground.exe.as_deref()?;
        self.processes
            .iter()
            .filter(|(_, p)| p.application.eq_ignore_ascii_case(exe))
            .filter_map(|(pid, p)| p.reading(now).map(|r| (*pid, r)))
            .max_by(|a, b| a.1.fps.total_cmp(&b.1.fps))
    }

    fn is_target(&self, foreground: &Foreground) -> bool {
        self.target.as_ref().is_some_and(|target| {
            target.pid == foreground.pid
                || foreground
                    .exe
                    .as_deref()
                    .is_some_and(|exe| exe.eq_ignore_ascii_case(&target.application))
        })
    }

    /// The target's live reading, or 0 FPS flagged as paused when it stopped presenting.
    fn target_reading(&self, now: Instant) -> Option<FpsReading> {
        let target = self.target.as_ref()?;
        let live = self.processes.get(&target.pid).and_then(|p| p.reading(now));
        Some(live.unwrap_or_else(|| FpsReading {
            fps: 0.0,
            application: target.application.clone(),
            frame_time_ms: 0.0,
            gpu_busy_ms: None,
            display_latency_ms: None,
            bound: None,
            paused: true,
            game_name: None,
        }))
    }

    /// Chooses whose FPS to show. `alive` reports whether a process is still running.
    fn select(
        &mut self,
        foreground: Option<&Foreground>,
        keep_target: bool,
        now: Instant,
        alive: impl Fn(u32) -> bool,
    ) -> FpsReading {
        if self.target.as_ref().is_some_and(|t| !alive(t.pid)) {
            self.target = None;
        }

        if let Some(foreground) = foreground {
            if let Some((pid, reading)) = self.foreground_reading(foreground, now) {
                let is_desktop = reading.application.eq_ignore_ascii_case(DESKTOP_COMPOSITOR);
                // A kept game is only replaced by another full-screen app, not by a browser after Alt+Tab.
                let may_retarget = !keep_target
                    || self.target.is_none()
                    || foreground.covers_monitor
                    || self.is_target(foreground);
                if !is_desktop && may_retarget {
                    self.target = Some(Target {
                        pid,
                        application: reading.application.clone(),
                    });
                    return reading;
                }
                if !keep_target {
                    return reading;
                }
            } else if self.is_target(foreground) {
                // The game is focused but stopped presenting, e.g. a pause menu.
                if let Some(reading) = self.target_reading(now) {
                    return reading;
                }
            }
        }

        if keep_target {
            if let Some(reading) = self.target_reading(now) {
                return reading;
            }
        }

        // DWM only presents when something on screen changes, so an idle desktop really is ~0 FPS.
        // Composition is not a game workload, so the desktop gets no bound / latency verdict.
        let desktop_fps = self
            .processes
            .values()
            .find(|p| p.application.eq_ignore_ascii_case(DESKTOP_COMPOSITOR))
            .and_then(|p| p.reading(now))
            .map_or(0.0, |r| r.fps);
        FpsReading {
            fps: desktop_fps,
            application: "Desktop".to_owned(),
            frame_time_ms: if desktop_fps > 0.0 { 1000.0 / desktop_fps } else { 0.0 },
            gpu_busy_ms: None,
            display_latency_ms: None,
            bound: None,
            paused: false,
            game_name: None,
        }
    }

    fn resolve_game_name(&mut self, pid: Option<u32>, application: &str) -> Option<String> {
        if application.eq_ignore_ascii_case("Desktop") || application.eq_ignore_ascii_case(DESKTOP_COMPOSITOR) {
            return None;
        }
        let pid = pid?;
        if let Some(cached) = self.game_names.get(&pid) {
            return cached.clone();
        }
        let name = process_image_path(pid).and_then(|path| file_version_game_name(&path, application));
        self.game_names.insert(pid, name.clone());
        name
    }
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
    /// The followed game is focused (or kept) but presents no frames, e.g. in a pause menu.
    pub paused: bool,
    /// FileDescription or ProductName of the exe, when it differs from the file name.
    pub game_name: Option<String>,
}

/// Consumes PresentMon CSV lines and reports the FPS of the foreground process,
/// falling back to the desktop compositor when the foreground window renders nothing itself.
#[derive(Default)]
pub struct FpsTracker {
    state: Mutex<TrackerState>,
    capturing: AtomicBool,
    /// Keep following the last game while other windows are focused.
    keep_target: AtomicBool,
}

impl FpsTracker {
    /// True once PresentMon has started streaming; false if it failed (usually missing admin rights).
    pub fn is_capturing(&self) -> bool {
        self.capturing.load(Ordering::Relaxed)
    }

    pub fn mark_stopped(&self) {
        self.capturing.store(false, Ordering::Relaxed);
    }

    pub fn set_keep_target(&self, on: bool) {
        self.keep_target.store(on, Ordering::Relaxed);
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
            // A paused game presents nothing, but it must not be forgotten while it is the target.
            let target_pid = state.target.as_ref().map(|t| t.pid);
            state
                .processes
                .retain(|pid, p| Some(*pid) == target_pid || now.duration_since(p.last_seen) < FORGET_AFTER);
            let live: HashSet<u32> = state.processes.keys().copied().collect();
            state.game_names.retain(|pid, _| live.contains(pid));
            state.last_cleanup = Some(now);
        }
    }

    pub fn reading(&self) -> Option<FpsReading> {
        if !self.is_capturing() {
            return None;
        }
        let foreground = foreground_window();
        let keep_target = self.keep_target.load(Ordering::Relaxed);
        let mut state = self.state.lock().unwrap();
        let mut reading = state.select(foreground.as_ref(), keep_target, Instant::now(), process_alive);
        let pid = state.target.as_ref().and_then(|t| {
            reading
                .application
                .eq_ignore_ascii_case(&t.application)
                .then_some(t.pid)
        });
        reading.game_name = state.resolve_game_name(pid, &reading.application);
        Some(reading)
    }
}

fn foreground_window() -> Option<Foreground> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        Some(Foreground {
            pid,
            exe: process_exe_name(pid),
            covers_monitor: covers_monitor(hwnd),
        })
    }
}

/// True for exclusive and borderless full-screen windows; maximized windows leave the taskbar visible.
unsafe fn covers_monitor(hwnd: HWND) -> bool {
    let mut window = RECT::default();
    if GetWindowRect(hwnd, &mut window).is_err() {
        return false;
    }
    let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !GetMonitorInfoW(monitor, &mut info).as_bool() {
        return false;
    }
    let screen = info.rcMonitor;
    window.left <= screen.left
        && window.top <= screen.top
        && window.right >= screen.right
        && window.bottom >= screen.bottom
}

fn process_alive(pid: u32) -> bool {
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let mut code = 0u32;
        let running = GetExitCodeProcess(handle, &mut code).is_ok() && code == STILL_ACTIVE.0 as u32;
        let _ = CloseHandle(handle);
        running
    }
}

/// Full image path of a process, used to read the version resource.
fn process_image_path(pid: u32) -> Option<String> {
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
        Some(String::from_utf16_lossy(&buffer[..len as usize]))
    }
}

/// Executable file name (e.g. "chrome.exe") of a process, matching PresentMon's Application column.
fn process_exe_name(pid: u32) -> Option<String> {
    process_image_path(pid)?.rsplit('\\').next().map(str::to_owned)
}

/// Uses FileDescription, then ProductName. Ignores empty values and the exe file name itself.
fn game_name_from_version(value: &str, exe: &str) -> Option<String> {
    let name = value.trim();
    if name.is_empty() {
        return None;
    }
    let stem = exe.strip_suffix(".exe").or_else(|| exe.strip_suffix(".EXE")).unwrap_or(exe);
    if name.eq_ignore_ascii_case(exe) || name.eq_ignore_ascii_case(stem) {
        return None;
    }
    Some(name.to_owned())
}

fn file_version_game_name(path: &str, exe: &str) -> Option<String> {
    let data = file_version_block(path)?;
    let translations = version_translations(&data);
    for (lang, codepage) in translations {
        for key in ["FileDescription", "ProductName"] {
            if let Some(name) = query_version_string(&data, lang, codepage, key).and_then(|v| game_name_from_version(&v, exe))
            {
                return Some(name);
            }
        }
    }
    None
}

fn file_version_block(path: &str) -> Option<Vec<u8>> {
    let wide: Vec<u16> = OsStr::new(path).encode_wide().chain(Some(0)).collect();
    unsafe {
        let size = GetFileVersionInfoSizeW(PCWSTR(wide.as_ptr()), None);
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        GetFileVersionInfoW(PCWSTR(wide.as_ptr()), Some(0), size, data.as_mut_ptr().cast()).ok()?;
        Some(data)
    }
}

fn version_translations(data: &[u8]) -> Vec<(u16, u16)> {
    let mut out = Vec::new();
    unsafe {
        let mut ptr = std::ptr::null_mut();
        let mut len = 0u32;
        let path: Vec<u16> = "\\VarFileInfo\\Translation".encode_utf16().chain(Some(0)).collect();
        if VerQueryValueW(data.as_ptr().cast(), PCWSTR(path.as_ptr()), &mut ptr, &mut len).as_bool()
            && !ptr.is_null()
            && len >= 4
        {
            let pairs = (len as usize) / 4;
            let trans = ptr as *const u16;
            for i in 0..pairs {
                out.push((*trans.add(i * 2), *trans.add(i * 2 + 1)));
            }
        }
    }
    // US English Unicode / Windows Latin-1, used when the translation table is missing.
    if out.is_empty() {
        out.extend([(0x0409, 0x04b0), (0x0409, 0x04e4)]);
    }
    out
}

fn query_version_string(data: &[u8], lang: u16, codepage: u16, key: &str) -> Option<String> {
    let sub = format!("\\StringFileInfo\\{lang:04x}{codepage:04x}\\{key}");
    let wide: Vec<u16> = sub.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let mut ptr = std::ptr::null_mut();
        let mut len = 0u32;
        if !VerQueryValueW(data.as_ptr().cast(), PCWSTR(wide.as_ptr()), &mut ptr, &mut len).as_bool()
            || ptr.is_null()
            || len == 0
        {
            return None;
        }
        let chars = std::slice::from_raw_parts(ptr as *const u16, len as usize);
        let end = chars.iter().position(|&c| c == 0).unwrap_or(chars.len());
        let value = String::from_utf16_lossy(&chars[..end]);
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GAME: u32 = 100;
    const BROWSER: u32 = 200;
    const OTHER_GAME: u32 = 300;
    const DWM: u32 = 10;

    fn tracker() -> FpsTracker {
        let tracker = FpsTracker::default();
        tracker.ingest_line("Application,ProcessID,SwapChainAddress,FrameTime,GPUBusy,DisplayLatency");
        tracker
    }

    fn present(tracker: &FpsTracker, application: &str, pid: u32, frame_ms: f32) {
        for _ in 0..(1000.0 / frame_ms) as usize + 2 {
            tracker.ingest_line(&format!("{application},{pid},0x1,{frame_ms},{},20", frame_ms * 0.5));
        }
    }

    fn focused(pid: u32, exe: &str, covers_monitor: bool) -> Foreground {
        Foreground {
            pid,
            exe: Some(exe.to_owned()),
            covers_monitor,
        }
    }

    fn select(tracker: &FpsTracker, foreground: &Foreground, keep: bool, now: Instant) -> FpsReading {
        tracker.state.lock().unwrap().select(Some(foreground), keep, now, |_| true)
    }

    fn game() -> Foreground {
        focused(GAME, "witcher3.exe", true)
    }

    #[test]
    fn running_game_is_reported_live() {
        let tracker = tracker();
        present(&tracker, "witcher3.exe", GAME, 10.0);

        let reading = select(&tracker, &game(), false, Instant::now());
        assert_eq!(reading.application, "witcher3.exe");
        assert!(!reading.paused);
        assert!((reading.fps - 100.0).abs() < 1.0);
    }

    #[test]
    fn paused_game_reports_zero_fps_instead_of_desktop() {
        let tracker = tracker();
        present(&tracker, "dwm.exe", DWM, 16.0);
        present(&tracker, "witcher3.exe", GAME, 10.0);
        select(&tracker, &game(), false, Instant::now());

        let later = Instant::now() + STALE_AFTER * 2;
        let reading = select(&tracker, &game(), false, later);
        assert_eq!(reading.application, "witcher3.exe");
        assert!(reading.paused);
        assert_eq!(reading.fps, 0.0);
    }

    #[test]
    fn alt_tab_without_option_follows_the_focused_window() {
        let tracker = tracker();
        present(&tracker, "witcher3.exe", GAME, 10.0);
        present(&tracker, "chrome.exe", BROWSER, 16.0);
        select(&tracker, &game(), false, Instant::now());

        let reading = select(&tracker, &focused(BROWSER, "chrome.exe", false), false, Instant::now());
        assert_eq!(reading.application, "chrome.exe");

        let reading = select(&tracker, &focused(1, "explorer.exe", false), false, Instant::now());
        assert_eq!(reading.application, "Desktop");
    }

    #[test]
    fn alt_tab_with_option_keeps_the_game() {
        let tracker = tracker();
        present(&tracker, "witcher3.exe", GAME, 10.0);
        present(&tracker, "chrome.exe", BROWSER, 16.0);
        select(&tracker, &game(), true, Instant::now());

        let reading = select(&tracker, &focused(BROWSER, "chrome.exe", false), true, Instant::now());
        assert_eq!(reading.application, "witcher3.exe");
        assert!(!reading.paused);

        let reading = select(&tracker, &focused(1, "explorer.exe", false), true, Instant::now());
        assert_eq!(reading.application, "witcher3.exe");

        let later = Instant::now() + STALE_AFTER * 2;
        let reading = select(&tracker, &focused(1, "explorer.exe", false), true, later);
        assert_eq!(reading.application, "witcher3.exe");
        assert!(reading.paused);
    }

    #[test]
    fn full_screen_game_takes_over_a_kept_target() {
        let tracker = tracker();
        present(&tracker, "witcher3.exe", GAME, 10.0);
        present(&tracker, "cyberpunk2077.exe", OTHER_GAME, 16.0);
        select(&tracker, &game(), true, Instant::now());

        let other = focused(OTHER_GAME, "cyberpunk2077.exe", true);
        let reading = select(&tracker, &other, true, Instant::now());
        assert_eq!(reading.application, "cyberpunk2077.exe");

        let reading = select(&tracker, &focused(BROWSER, "chrome.exe", false), true, Instant::now());
        assert_eq!(reading.application, "cyberpunk2077.exe");
    }

    #[test]
    fn exited_game_is_dropped() {
        let tracker = tracker();
        present(&tracker, "witcher3.exe", GAME, 10.0);
        select(&tracker, &game(), true, Instant::now());

        let reading = tracker.state.lock().unwrap().select(
            Some(&focused(1, "explorer.exe", false)),
            true,
            Instant::now(),
            |pid| pid != GAME,
        );
        assert_eq!(reading.application, "Desktop");
        assert!(tracker.state.lock().unwrap().target.is_none());
    }

    #[test]
    fn desktop_compositor_never_becomes_the_target() {
        let tracker = tracker();
        present(&tracker, "dwm.exe", DWM, 16.0);
        select(&tracker, &focused(DWM, "dwm.exe", true), true, Instant::now());
        assert!(tracker.state.lock().unwrap().target.is_none());
    }

    #[test]
    fn game_name_from_version_skips_empty_and_exe() {
        assert_eq!(
            game_name_from_version("The Witcher 3", "witcher3.exe").as_deref(),
            Some("The Witcher 3")
        );
        assert_eq!(game_name_from_version("witcher3.exe", "witcher3.exe"), None);
        assert_eq!(game_name_from_version("witcher3", "witcher3.exe"), None);
        assert_eq!(game_name_from_version("  ", "witcher3.exe"), None);
    }

    #[test]
    fn cached_game_name_is_attached_to_reading() {
        let tracker = tracker();
        present(&tracker, "witcher3.exe", GAME, 10.0);
        {
            let mut state = tracker.state.lock().unwrap();
            state.game_names.insert(GAME, Some("The Witcher 3".into()));
            let mut reading = state.select(Some(&game()), false, Instant::now(), |_| true);
            let pid = state.target.as_ref().map(|t| t.pid);
            let application = reading.application.clone();
            reading.game_name = state.resolve_game_name(pid, &application);
            assert_eq!(reading.game_name.as_deref(), Some("The Witcher 3"));
        }
    }

    #[test]
    fn missing_metadata_leaves_game_name_empty() {
        let tracker = tracker();
        present(&tracker, "witcher3.exe", GAME, 10.0);
        {
            let mut state = tracker.state.lock().unwrap();
            state.game_names.insert(GAME, None);
            let mut reading = state.select(Some(&game()), false, Instant::now(), |_| true);
            let pid = state.target.as_ref().map(|t| t.pid);
            let application = reading.application.clone();
            reading.game_name = state.resolve_game_name(pid, &application);
            assert_eq!(reading.application, "witcher3.exe");
            assert_eq!(reading.game_name, None);
        }
    }

    #[test]
    fn desktop_reading_has_no_game_name() {
        let tracker = tracker();
        present(&tracker, "dwm.exe", DWM, 16.0);
        let mut state = tracker.state.lock().unwrap();
        let mut reading = state.select(Some(&focused(1, "explorer.exe", false)), false, Instant::now(), |_| true);
        let pid = state.target.as_ref().map(|t| t.pid);
        let application = reading.application.clone();
        reading.game_name = state.resolve_game_name(pid, &application);
        assert_eq!(reading.application, "Desktop");
        assert_eq!(reading.game_name, None);
    }
}
