use windows::core::PCWSTR;
use windows::core::w;
use windows::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterValue,
    PdhOpenQueryW, PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY,
};

const ERROR_SUCCESS: u32 = 0;

pub struct DiskSample {
    pub read_bps: Option<f64>,
    pub write_bps: Option<f64>,
}

/// Combined read / write throughput of all physical disks.
pub struct DiskSampler {
    query: PDH_HQUERY,
    read: Option<PDH_HCOUNTER>,
    write: Option<PDH_HCOUNTER>,
}

impl DiskSampler {
    pub fn new() -> Option<Self> {
        unsafe {
            let mut query = PDH_HQUERY::default();
            if PdhOpenQueryW(None, 0, &mut query) != ERROR_SUCCESS {
                return None;
            }
            let read = add_counter(query, w!("\\PhysicalDisk(_Total)\\Disk Read Bytes/sec"));
            let write = add_counter(query, w!("\\PhysicalDisk(_Total)\\Disk Write Bytes/sec"));
            if read.is_none() && write.is_none() {
                PdhCloseQuery(query);
                return None;
            }

            // Rate counters need a first sample before they can be formatted.
            PdhCollectQueryData(query);
            Some(Self { query, read, write })
        }
    }

    pub fn sample(&mut self) -> DiskSample {
        unsafe {
            if PdhCollectQueryData(self.query) != ERROR_SUCCESS {
                return DiskSample { read_bps: None, write_bps: None };
            }
        }
        DiskSample {
            read_bps: self.read.and_then(read_counter),
            write_bps: self.write.and_then(read_counter),
        }
    }
}

impl Drop for DiskSampler {
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

fn read_counter(counter: PDH_HCOUNTER) -> Option<f64> {
    unsafe {
        let mut value = PDH_FMT_COUNTERVALUE::default();
        if PdhGetFormattedCounterValue(counter, PDH_FMT_DOUBLE, None, &mut value) != ERROR_SUCCESS {
            return None;
        }
        let bytes_per_second = value.Anonymous.doubleValue;
        (bytes_per_second >= 0.0).then_some(bytes_per_second)
    }
}
