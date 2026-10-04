use std::time::Instant;

use super::CpuSensorReading;
use crate::metrics::pawnio::PawnIo;
use crate::metrics::topology::PhysicalCore;

static AMD_FAMILY_17_MODULE: &[u8] = include_bytes!("../../../resources/pawnio/AMDFamily17.bin");

/// SMN register with the reported (Tctl) temperature, common to Zen 1-5.
const THM_TCON_CUR_TMP: u32 = 0x0005_9800;
const CUR_TEMP_RANGE_SEL: u32 = 0x8_0000;
const CUR_TEMP_TJ_SEL: u32 = 0x3_0000;

/// Per-CCD temperature registers follow THM_TCON_CUR_TMP at a generation-specific offset (as in Linux k10temp).
const CCD_TEMP_OFFSET_ZEN2_3: u32 = 0x154;
const CCD_TEMP_OFFSET_ZEN4_5: u32 = 0x308;
const CCD_TEMP_VALID: u32 = 1 << 11;
const CCD_TEMP_MASK: u32 = 0x7FF;
const MAX_CCDS: u32 = 8;

const MSR_PWR_UNIT: u32 = 0xC001_0299;
const MSR_PKG_ENERGY_STAT: u32 = 0xC001_029B;
const MSR_MPERF: u32 = 0xC000_00E7;
const MSR_APERF: u32 = 0xC000_00E8;

/// AMD family 17h-1Ah (Zen 1-5) sensors read through PawnIO's AMDFamily17 module.
pub struct AmdSensors {
    pawnio: PawnIo,
    base_mhz: f64,
    energy_unit_joules: f64,
    last_energy: Option<(u32, Instant)>,
    last_perf: Vec<Option<(u64, u64)>>,
    /// SMN addresses of the CCD temperature registers that reported a valid value at startup.
    ccd_registers: Vec<u32>,
}

impl AmdSensors {
    pub fn new(base_mhz: u64, cores: &[PhysicalCore]) -> Option<Self> {
        let pawnio = PawnIo::load_module(AMD_FAMILY_17_MODULE)?;
        // Probe once so a driver that loads but cannot read falls back to ACPI.
        pawnio.read_smn(THM_TCON_CUR_TMP)?;

        let energy_unit_joules = pawnio
            .read_msr(MSR_PWR_UNIT)
            .map(|units| 1.0 / f64::from(1u32 << ((units >> 8) & 0x1F)))
            .unwrap_or(0.0);

        let ccd_registers = ccd_temp_offset()
            .map(|offset| {
                (0..MAX_CCDS)
                    .map(|ccd| THM_TCON_CUR_TMP + offset + ccd * 4)
                    .filter(|&register| pawnio.read_smn(register).and_then(decode_ccd_temp).is_some())
                    .collect()
            })
            .unwrap_or_default();

        Some(Self {
            pawnio,
            base_mhz: base_mhz as f64,
            energy_unit_joules,
            last_energy: None,
            last_perf: vec![None; cores.len()],
            ccd_registers,
        })
    }

    pub fn sample(&mut self, cores: &[PhysicalCore]) -> CpuSensorReading {
        let (core_clocks_mhz, average_clock_mhz) = self.core_clocks(cores);
        CpuSensorReading {
            temp_c: self.temperature(),
            power_w: self.package_power(),
            core_clocks_mhz,
            average_clock_mhz,
            ccd_temps_c: self.ccd_temperatures(),
        }
    }

    fn ccd_temperatures(&self) -> Vec<Option<f32>> {
        self.ccd_registers
            .iter()
            .map(|&register| self.pawnio.read_smn(register).and_then(decode_ccd_temp))
            .collect()
    }

    fn temperature(&self) -> Option<f32> {
        let raw = self.pawnio.read_smn(THM_TCON_CUR_TMP)?;
        let mut celsius = (raw >> 21) as f32 * 0.125;
        if raw & CUR_TEMP_RANGE_SEL != 0 || raw & CUR_TEMP_TJ_SEL == CUR_TEMP_TJ_SEL {
            celsius -= 49.0;
        }
        (0.0..150.0).contains(&celsius).then_some(celsius)
    }

    fn package_power(&mut self) -> Option<f32> {
        if self.energy_unit_joules == 0.0 {
            return None;
        }
        let energy = self.pawnio.read_msr(MSR_PKG_ENERGY_STAT)? as u32;
        let now = Instant::now();
        let previous = self.last_energy.replace((energy, now))?;

        let seconds = now.duration_since(previous.1).as_secs_f64();
        if seconds <= 0.0 {
            return None;
        }
        // The 32-bit counter wraps; wrapping_sub keeps the delta correct across one overflow.
        let joules = f64::from(energy.wrapping_sub(previous.0)) * self.energy_unit_joules;
        Some((joules / seconds) as f32)
    }

    /// Active clock per core from APERF/MPERF deltas; the average is weighted by active time.
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

fn decode_ccd_temp(raw: u32) -> Option<f32> {
    if raw & CCD_TEMP_VALID == 0 {
        return None;
    }
    let celsius = (raw & CCD_TEMP_MASK) as f32 * 0.125 - 49.0;
    (0.0..150.0).contains(&celsius).then_some(celsius)
}

/// CCD register offset for this CPU generation, or `None` for parts without per-CCD sensors (Zen 1).
fn ccd_temp_offset() -> Option<u32> {
    let (family, model) = cpu_family_model();
    match family {
        0x17 if model >= 0x30 => Some(CCD_TEMP_OFFSET_ZEN2_3),
        0x19 => match model {
            0x00..=0x0F | 0x20..=0x2F | 0x50..=0x5F => Some(CCD_TEMP_OFFSET_ZEN2_3),
            _ => Some(CCD_TEMP_OFFSET_ZEN4_5),
        },
        0x1A => Some(CCD_TEMP_OFFSET_ZEN4_5),
        _ => None,
    }
}

fn cpu_family_model() -> (u32, u32) {
    let eax = std::arch::x86_64::__cpuid(1).eax;
    let base_family = (eax >> 8) & 0xF;
    let base_model = (eax >> 4) & 0xF;
    if base_family == 0xF {
        (base_family + ((eax >> 20) & 0xFF), base_model | (((eax >> 16) & 0xF) << 4))
    } else {
        (base_family, base_model)
    }
}
