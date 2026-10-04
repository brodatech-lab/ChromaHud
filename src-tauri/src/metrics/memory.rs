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
}

/// Memory type and configured data rate, e.g. "DDR5-6000". Read once at startup.
pub fn ram_speed_label() -> Option<String> {
    let connection = WMIConnection::new().ok()?;
    let modules: Vec<PhysicalMemory> = connection.query().ok()?;

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
