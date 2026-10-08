mod acpi;
mod amd;
mod intel;

use super::topology::PhysicalCore;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CpuSensorSource {
    /// AMD Zen or Intel sensors through the PawnIO driver: temperature, package power, per-core clocks.
    PawnIo,
    /// ACPI thermal zone via WMI: temperature only, and only on some boards.
    Acpi,
}

impl CpuSensorSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PawnIo => "pawnio",
            Self::Acpi => "acpi",
        }
    }
}

#[derive(Default)]
pub struct CpuSensorReading {
    pub temp_c: Option<f32>,
    pub power_w: Option<f32>,
    /// Per physical core, same order as the topology passed to `CpuSensors::new`.
    pub core_clocks_mhz: Vec<Option<u32>>,
    pub average_clock_mhz: Option<u32>,
    /// One entry per detected CCD, in die order.
    pub ccd_temps_c: Vec<Option<f32>>,
}

/// CPU temperature, power and clocks, preferring PawnIO and falling back to ACPI for temperature.
pub enum CpuSensors {
    Amd(amd::AmdSensors),
    Intel(intel::IntelSensors),
    Acpi(acpi::SharedTemperature),
}

impl CpuSensors {
    pub fn new(base_mhz: u64, cores: &[PhysicalCore]) -> Self {
        if let Some(sensors) = amd::AmdSensors::new(base_mhz, cores) {
            return Self::Amd(sensors);
        }
        if let Some(sensors) = intel::IntelSensors::new(base_mhz, cores) {
            return Self::Intel(sensors);
        }
        Self::Acpi(acpi::spawn_reader())
    }

    pub fn source(&self) -> CpuSensorSource {
        match self {
            Self::Amd(_) | Self::Intel(_) => CpuSensorSource::PawnIo,
            Self::Acpi(_) => CpuSensorSource::Acpi,
        }
    }

    pub fn sample(&mut self, cores: &[PhysicalCore]) -> CpuSensorReading {
        match self {
            Self::Amd(sensors) => sensors.sample(cores),
            Self::Intel(sensors) => sensors.sample(cores),
            Self::Acpi(temperature) => CpuSensorReading {
                temp_c: *temperature.lock().unwrap(),
                ..Default::default()
            },
        }
    }
}
