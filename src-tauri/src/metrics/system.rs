use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
use windows::core::w;
use windows::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterValue,
    PdhOpenQueryW, PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY,
};

const ERROR_SUCCESS: u32 = 0;

pub struct SystemSample {
    pub cpu_usage: f32,
    pub cpu_clock_mhz: u32,
    /// Usage per logical processor, in `cpus()` order.
    pub logical_usage: Vec<f32>,
    pub ram_used_bytes: u64,
    pub ram_total_bytes: u64,
}

/// CPU usage, effective CPU clock and RAM usage.
pub struct SystemSampler {
    sys: System,
    base_mhz: u64,
    perf_counter: Option<ProcessorPerformanceCounter>,
}

impl SystemSampler {
    pub fn new() -> Self {
        let sys = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::nothing().with_cpu_usage().with_frequency())
                .with_memory(MemoryRefreshKind::nothing().with_ram()),
        );
        let base_mhz = sys.cpus().first().map(|cpu| cpu.frequency()).unwrap_or(0);

        Self {
            sys,
            base_mhz,
            perf_counter: ProcessorPerformanceCounter::open(),
        }
    }

    pub fn base_mhz(&self) -> u64 {
        self.base_mhz
    }

    pub fn logical_count(&self) -> usize {
        self.sys.cpus().len()
    }

    pub fn cpu_model(&self) -> String {
        self.sys
            .cpus()
            .first()
            .map(|cpu| cpu.brand().split_whitespace().collect::<Vec<_>>().join(" "))
            .unwrap_or_default()
    }

    pub fn sample(&mut self) -> SystemSample {
        self.sys.refresh_cpu_usage();
        self.sys
            .refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());

        // On Windows sysinfo only reports the nominal clock; "% Processor Performance"
        // scales it to the real (boost / power-saving) frequency, like Task Manager does.
        let perf_ratio = self
            .perf_counter
            .as_ref()
            .and_then(|counter| counter.read_percent())
            .map(|percent| percent / 100.0)
            .unwrap_or(1.0);

        SystemSample {
            cpu_usage: self.sys.global_cpu_usage(),
            cpu_clock_mhz: (self.base_mhz as f64 * perf_ratio).round() as u32,
            logical_usage: self.sys.cpus().iter().map(|cpu| cpu.cpu_usage()).collect(),
            ram_used_bytes: self.sys.used_memory(),
            ram_total_bytes: self.sys.total_memory(),
        }
    }
}

struct ProcessorPerformanceCounter {
    query: PDH_HQUERY,
    counter: PDH_HCOUNTER,
}

impl ProcessorPerformanceCounter {
    fn open() -> Option<Self> {
        unsafe {
            let mut query = PDH_HQUERY::default();
            if PdhOpenQueryW(None, 0, &mut query) != ERROR_SUCCESS {
                return None;
            }

            let mut counter = PDH_HCOUNTER::default();
            let status = PdhAddEnglishCounterW(
                query,
                w!("\\Processor Information(_Total)\\% Processor Performance"),
                0,
                &mut counter,
            );
            if status != ERROR_SUCCESS {
                PdhCloseQuery(query);
                return None;
            }

            // Rate counters need a first sample before they can be formatted.
            PdhCollectQueryData(query);
            Some(Self { query, counter })
        }
    }

    fn read_percent(&self) -> Option<f64> {
        unsafe {
            if PdhCollectQueryData(self.query) != ERROR_SUCCESS {
                return None;
            }
            let mut value = PDH_FMT_COUNTERVALUE::default();
            if PdhGetFormattedCounterValue(self.counter, PDH_FMT_DOUBLE, None, &mut value)
                != ERROR_SUCCESS
            {
                return None;
            }
            let percent = value.Anonymous.doubleValue;
            (percent > 0.0).then_some(percent)
        }
    }
}

impl Drop for ProcessorPerformanceCounter {
    fn drop(&mut self) {
        unsafe {
            PdhCloseQuery(self.query);
        }
    }
}
