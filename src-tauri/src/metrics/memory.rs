use serde::Deserialize;
use wmi::WMIConnection;

#[derive(Deserialize)]
#[serde(rename = "Win32_PhysicalMemory")]
#[serde(rename_all = "PascalCase")]
struct PhysicalMemory {
    /// Data rate in MT/s as configured by the firmware (e.g. 6000 for DDR5-6000 with EXPO).
    configured_clock_speed: Option<u32>,
    #[serde(rename = "SMBIOSMemoryType")]
    smbios_memory_type: Option<u32>,
    manufacturer: Option<String>,
}

pub struct RamInfo {
    /// e.g. "DDR5-6000"
    pub speed: Option<String>,
    /// SPD manufacturer only — not the module SKU / part number.
    pub manufacturer: Option<String>,
}

/// Memory type, configured data rate and manufacturer. Read once at startup.
pub fn probe() -> RamInfo {
    let Some(modules) = query() else {
        return RamInfo { speed: None, manufacturer: None };
    };
    RamInfo {
        speed: speed_label(&modules),
        manufacturer: manufacturer_label(&modules),
    }
}

fn query() -> Option<Vec<PhysicalMemory>> {
    let connection = WMIConnection::new().ok()?;
    connection.query().ok()
}

fn speed_label(modules: &[PhysicalMemory]) -> Option<String> {
    let speed = modules
        .iter()
        .filter_map(|m| m.configured_clock_speed)
        .filter(|mts| *mts > 0)
        .min()?;
    let kind = modules
        .iter()
        .find_map(|m| m.smbios_memory_type)
        .and_then(memory_type_name);

    Some(match kind {
        Some(kind) => format!("{kind}-{speed}"),
        None => format!("{speed} MT/s"),
    })
}

fn manufacturer_label(modules: &[PhysicalMemory]) -> Option<String> {
    let mut names: Vec<String> = Vec::new();
    for module in modules {
        let Some(name) = module.manufacturer.as_deref().and_then(clean_manufacturer) else {
            continue;
        };
        if !names.iter().any(|existing| existing.eq_ignore_ascii_case(&name)) {
            names.push(name);
        }
    }
    match names.len() {
        0 => None,
        1 => names.pop(),
        _ => Some(names.join(", ")),
    }
}

fn clean_manufacturer(raw: &str) -> Option<String> {
    let name = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return None;
    }
    let lower = name.to_ascii_lowercase();
    let placeholder = matches!(
        lower.as_str(),
        "unknown"
            | "undefined"
            | "none"
            | "null"
            | "oem"
            | "to be filled by o.e.m."
            | "to be filled by oem"
    );
    if placeholder || name.bytes().all(|b| b == b'0') {
        return None;
    }
    Some(name)
}

/// SMBIOS type 17 "Memory Type" codes.
fn memory_type_name(code: u32) -> Option<&'static str> {
    match code {
        24 => Some("DDR3"),
        26 => Some("DDR4"),
        27 => Some("LPDDR"),
        28 => Some("LPDDR2"),
        29 => Some("LPDDR3"),
        30 => Some("LPDDR4"),
        34 => Some("DDR5"),
        35 => Some("LPDDR5"),
        _ => None,
    }
}
