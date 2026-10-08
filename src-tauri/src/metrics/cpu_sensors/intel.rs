use std::time::Instant;

use super::CpuSensorReading;
use crate::metrics::pawnio::PawnIo;
use crate::metrics::topology::PhysicalCore;

static INTEL_MSR_MODULE: &[u8] = include_bytes!("../../../resources/pawnio/IntelMSR.bin");

/// Digital Thermal Sensor: package temp = TjMax − readout (Intel SDM).
const MSR_TEMPERATURE_TARGET: u32 = 0x1A2;
const MSR_PACKAGE_THERM_STATUS: u32 = 0x1B1;

const MSR_RAPL_POWER_UNIT: u32 = 0x606;
const MSR_PKG_ENERGY_STATUS: u32 = 0x611;

const MSR_MPERF: u32 = 0xE7;
const MSR_APERF: u32 = 0xE8;

/// Intel package sensors through PawnIO's IntelMSR module (same ioctl as AMDFamily17).
pub struct IntelSensors {
    pawnio: PawnIo,
    base_mhz: f64,
    tjmax_c: f32,
    energy_unit_joules: f64,
    last_energy: Option<(u32, Instant)>,
    last_perf: Vec<Option<(u64, u64)>>,
}

impl IntelSensors {
    pub fn new(base_mhz: u64, cores: &[PhysicalCore]) -> Option<Self> {
        let pawnio = PawnIo::load_module(INTEL_MSR_MODULE)?;
        let tjmax_c = tjmax(&pawnio)?;

        let energy_unit_joules = pawnio
            .read_msr(MSR_RAPL_POWER_UNIT)
            .map(|units| 1.0 / f64::from(1u32 << ((units >> 8) & 0x1F)))
            .unwrap_or(0.0);

        Some(Self {
            pawnio,
            base_mhz: base_mhz as f64,
            tjmax_c,
            energy_unit_joules,
            last_energy: None,
            last_perf: vec![None; cores.len()],
        })
    }

    pub fn sample(&mut self, cores: &[PhysicalCore]) -> CpuSensorReading {
        let (core_clocks_mhz, average_clock_mhz) = self.core_clocks(cores);
        CpuSensorReading {
            temp_c: self.temperature(),
            power_w: self.package_power(),
            core_clocks_mhz,
            average_clock_mhz,
            ccd_temps_c: Vec::new(),
        }
    }

    fn temperature(&self) -> Option<f32> {
        let status = self.pawnio.read_msr(MSR_PACKAGE_THERM_STATUS)?;
        let dts = ((status >> 16) & 0x7F) as f32;
        let celsius = self.tjmax_c - dts;
        (0.0..150.0).contains(&celsius).then_some(celsius)
    }

    fn package_power(&mut self) -> Option<f32> {
        if self.energy_unit_joules == 0.0 {
            return None;
        }
        let energy = self.pawnio.read_msr(MSR_PKG_ENERGY_STATUS)? as u32;
        let now = Instant::now();
        let previous = self.last_energy.replace((energy, now))?;

        let seconds = now.duration_since(previous.1).as_secs_f64();
        if seconds <= 0.0 {
            return None;
        }
        let joules = f64::from(energy.wrapping_sub(previous.0)) * self.energy_unit_joules;
        Some((joules / seconds) as f32)
    }

    /// Active clock per core from architectural APERF/MPERF deltas.
    fn core_clocks(&mut self, cores: &[PhysicalCore]) -> (Vec<Option<u32>>, Option<u32>) {
        if self.last_perf.len() != cores.len() {
            self.last_perf = vec![None; cores.len()];
        }

        let mut total_aperf = 0u64;
        let mut total_mperf = 0u64;
        let clocks = cores
            .iter()
            .zip(self.last_perf.iter_mut())
            .map(|(core, last)| {
                let mperf = self.pawnio.read_msr_on(core.affinity_mask, MSR_MPERF)?;
                let aperf = self.pawnio.read_msr_on(core.affinity_mask, MSR_APERF)?;
                let (last_mperf, last_aperf) = last.replace((mperf, aperf))?;

                let d_mperf = mperf.wrapping_sub(last_mperf);
                let d_aperf = aperf.wrapping_sub(last_aperf);
                if d_mperf == 0 {
                    return None;
                }
                total_mperf += d_mperf;
                total_aperf += d_aperf;
                Some((self.base_mhz * d_aperf as f64 / d_mperf as f64).round() as u32)
            })
            .collect();

        let average = (total_mperf > 0)
            .then(|| (self.base_mhz * total_aperf as f64 / total_mperf as f64).round() as u32);
        (clocks, average)
    }
}

fn tjmax(pawnio: &PawnIo) -> Option<f32> {
    let raw = pawnio.read_msr(MSR_TEMPERATURE_TARGET)?;
    let celsius = ((raw >> 16) & 0xFF) as f32;
    (50.0..=120.0).contains(&celsius).then_some(celsius)
}
