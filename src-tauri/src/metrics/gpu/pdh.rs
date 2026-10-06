use std::collections::HashMap;

use serde::Deserialize;
use windows::core::{w, PCWSTR};
use windows::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
    PdhOpenQueryW, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY,
};
use wmi::WMIConnection;

use super::{GpuMetrics, GpuProvider};

const ERROR_SUCCESS: u32 = 0;
const PDH_MORE_DATA: u32 = 0x8000_07D2;

pub struct PdhSample {
    pub usage: Option<u32>,
    pub vram_used_bytes: Option<u64>,
}

/// Vendor-neutral GPU load and memory from the Windows "GPU Engine" / "GPU Adapter Memory" counters
/// (the same data Task Manager shows).
pub struct PdhGpu {
    query: PDH_HQUERY,
    engine: PDH_HCOUNTER,
    dedicated: PDH_HCOUNTER,
    shared: PDH_HCOUNTER,
}

impl PdhGpu {
    pub fn new() -> Option<Self> {
        unsafe {
            let mut query = PDH_HQUERY::default();
            if PdhOpenQueryW(None, 0, &mut query) != ERROR_SUCCESS {
                return None;
            }
            let counters = (
                add_counter(query, w!("\\GPU Engine(*engtype_3D)\\Utilization Percentage")),
                add_counter(query, w!("\\GPU Adapter Memory(*)\\Dedicated Usage")),
                add_counter(query, w!("\\GPU Adapter Memory(*)\\Shared Usage")),
            );
            let (Some(engine), Some(dedicated), Some(shared)) = counters else {
                PdhCloseQuery(query);
                return None;
            };
            // Utilization is a rate counter and needs a first sample before it can be formatted.
            PdhCollectQueryData(query);
            Some(Self { query, engine, dedicated, shared })
        }
    }

    pub fn sample(&mut self) -> PdhSample {
        let empty = PdhSample { usage: None, vram_used_bytes: None };
        if unsafe { PdhCollectQueryData(self.query) } != ERROR_SUCCESS {
            return empty;
        }

        let dedicated = per_adapter(self.dedicated);
        let shared = per_adapter(self.shared);
        // The adapter holding the most memory is the one rendering; software adapters hold almost none.
        let Some(luid) = dedicated
            .keys()
            .chain(shared.keys())
            .max_by_key(|luid| {
                let total = dedicated.get(*luid).unwrap_or(&0.0) + shared.get(*luid).unwrap_or(&0.0);
                total as u64
            })
            .cloned()
        else {
            return empty;
        };

        let usage = per_adapter(self.engine).get(&luid).map(|u| u.round().clamp(0.0, 100.0) as u32);
        // Integrated GPUs keep almost everything in shared system memory.
        let local = dedicated.get(&luid).copied().unwrap_or(0.0);
        let memory = if local >= 64.0 * 1024.0 * 1024.0 { local } else { local + shared.get(&luid).copied().unwrap_or(0.0) };

        PdhSample {
            usage,
            vram_used_bytes: (memory > 0.0).then_some(memory as u64),
        }
    }
}

impl Drop for PdhGpu {
    fn drop(&mut self) {
        unsafe {
            PdhCloseQuery(self.query);
        }
    }
}

unsafe fn add_counter(query: PDH_HQUERY, path: PCWSTR) -> Option<PDH_HCOUNTER> {
    let mut counter = PDH_HCOUNTER::default();
    (PdhAddEnglishCounterW(query, path, 0, &mut counter) == ERROR_SUCCESS).then_some(counter)
}

/// Sums a wildcard counter per adapter; instance names look like
/// `pid_1234_luid_0x00000000_0x0000E215_phys_0_eng_0_engtype_3D` or `luid_0x00000000_0x0000E215_phys_0`.
fn per_adapter(counter: PDH_HCOUNTER) -> HashMap<String, f64> {
    let mut totals = HashMap::new();
    unsafe {
        let mut size = 0u32;
        let mut count = 0u32;
        if PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, None) != PDH_MORE_DATA {
            return totals;
        }
        let items = (size as usize).div_ceil(std::mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>());
        let mut buffer = vec![PDH_FMT_COUNTERVALUE_ITEM_W::default(); items];
        if PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, Some(buffer.as_mut_ptr()))
            != ERROR_SUCCESS
        {
            return totals;
        }
        for item in &buffer[..count as usize] {
            if item.FmtValue.CStatus != ERROR_SUCCESS {
                continue;
            }
            let Ok(name) = item.szName.to_string() else { continue };
            if let Some(luid) = adapter_luid(&name) {
                *totals.entry(luid.to_owned()).or_insert(0.0) += item.FmtValue.Anonymous.doubleValue;
            }
        }
    }
    totals
}

fn adapter_luid(instance: &str) -> Option<&str> {
    let start = instance.find("luid_")?;
    let rest = &instance[start..];
    let end = rest.find("_phys").unwrap_or(rest.len());
    Some(&rest[..end])
}

#[derive(Deserialize)]
#[serde(rename = "Win32_VideoController")]
#[serde(rename_all = "PascalCase")]
struct VideoController {
    name: Option<String>,
}

/// GPU usage and memory for adapters without a vendor library (e.g. Intel Arc / UHD).
pub struct PdhProvider {
    pdh: PdhGpu,
    name: String,
    vendor: &'static str,
}

impl PdhProvider {
    pub fn new() -> Option<Self> {
        let pdh = PdhGpu::new()?;
        let controllers: Vec<VideoController> = WMIConnection::new().ok()?.query().ok()?;
        let name = controllers
            .into_iter()
            .filter_map(|c| c.name)
            .find(|name| !name.starts_with("Microsoft"))?;
        let lower = name.to_lowercase();
        let vendor = if lower.contains("intel") {
            "Intel"
        } else if lower.contains("amd") || lower.contains("radeon") {
            "AMD"
        } else if lower.contains("nvidia") {
            "NVIDIA"
        } else {
            "GPU"
        };
        Some(Self { pdh, name, vendor })
    }
}

impl GpuProvider for PdhProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn sample(&mut self) -> Option<GpuMetrics> {
        let sample = self.pdh.sample();
        Some(GpuMetrics {
            vendor: self.vendor,
            name: self.name.clone(),
            usage: sample.usage,
            vram_used_bytes: sample.vram_used_bytes,
            ..Default::default()
        })
    }

    fn diagnostics(&self) -> serde_json::Value {
        serde_json::json!({ "provider": "pdh", "name": self.name })
    }
}
