use windows::Win32::System::SystemInformation::{
    GetLogicalProcessorInformationEx, RelationProcessorCore, SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX,
};

/// One physical core and the logical processors (SMT threads) that belong to it.
pub struct PhysicalCore {
    /// Logical processor indices, matching sysinfo's `cpus()` order.
    pub logical: Vec<usize>,
    /// Affinity mask of the first logical processor, used to run MSR reads on this core.
    pub affinity_mask: usize,
}

/// Physical cores of processor group 0. Falls back to one core per logical processor.
pub fn physical_cores(logical_count: usize) -> Vec<PhysicalCore> {
    let cores = query_cores();
    if !cores.is_empty() {
        return cores;
    }
    (0..logical_count.min(usize::BITS as usize))
        .map(|i| PhysicalCore {
            logical: vec![i],
            affinity_mask: 1 << i,
        })
        .collect()
}

fn query_cores() -> Vec<PhysicalCore> {
    let mut length = 0u32;
    unsafe {
        let _ = GetLogicalProcessorInformationEx(RelationProcessorCore, None, &mut length);
    }
    if length == 0 {
        return Vec::new();
    }

    // u64 storage keeps the variable-sized records suitably aligned.
    let mut buffer = vec![0u64; (length as usize).div_ceil(8)];
    let base = buffer.as_mut_ptr().cast::<u8>();
    let ok = unsafe {
        GetLogicalProcessorInformationEx(
            RelationProcessorCore,
            Some(base.cast::<SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX>()),
            &mut length,
        )
    };
    if ok.is_err() {
        return Vec::new();
    }

    let mut cores = Vec::new();
    let mut offset = 0usize;
    while offset < length as usize {
        let entry = unsafe {
            &*base
                .add(offset)
                .cast::<SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX>()
        };
        if entry.Size == 0 {
            break;
        }
        if entry.Relationship == RelationProcessorCore {
            let group = unsafe { entry.Anonymous.Processor.GroupMask[0] };
            if group.Group == 0 && group.Mask != 0 {
                let logical: Vec<usize> = (0..usize::BITS as usize)
                    .filter(|bit| group.Mask & (1 << bit) != 0)
                    .collect();
                cores.push(PhysicalCore {
                    affinity_mask: 1 << logical[0],
                    logical,
                });
            }
        }
        offset += entry.Size as usize;
    }

    cores.sort_by_key(|core| core.logical[0]);
    cores
}
