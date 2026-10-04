use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::Deserialize;
use wmi::WMIConnection;

const POLL_INTERVAL: Duration = Duration::from_secs(5);
const RETRY_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Deserialize)]
#[serde(rename = "MSAcpi_ThermalZoneTemperature")]
#[serde(rename_all = "PascalCase")]
struct ThermalZone {
    /// Tenths of a Kelvin.
    current_temperature: u32,
}

pub type SharedTemperature = Arc<Mutex<Option<f32>>>;

/// Polls ACPI thermal zones on a dedicated thread (WMI connections are bound to their COM thread).
/// Requires administrator rights; many boards expose no zones at all, in which case the value stays `None`.
pub fn spawn_reader() -> SharedTemperature {
    let shared: SharedTemperature = Arc::new(Mutex::new(None));
    let writer = shared.clone();

    let _ = thread::Builder::new()
        .name("thermal".into())
        .spawn(move || loop {
            let reading = WMIConnection::with_namespace_path("ROOT\\WMI")
                .ok()
                .and_then(|connection| read_max_celsius(&connection).map(|t| (connection, t)));

            let Some((connection, first)) = reading else {
                *writer.lock().unwrap() = None;
                thread::sleep(RETRY_INTERVAL);
                continue;
            };

            *writer.lock().unwrap() = Some(first);
            loop {
                thread::sleep(POLL_INTERVAL);
                let value = read_max_celsius(&connection);
                *writer.lock().unwrap() = value;
                if value.is_none() {
                    break;
                }
            }
        });

    shared
}

fn read_max_celsius(connection: &WMIConnection) -> Option<f32> {
    let zones: Vec<ThermalZone> = connection.query().ok()?;
    zones
        .iter()
        .map(|zone| zone.current_temperature as f32 / 10.0 - 273.15)
        .filter(|celsius| (1.0..150.0).contains(celsius))
        .reduce(f32::max)
}
