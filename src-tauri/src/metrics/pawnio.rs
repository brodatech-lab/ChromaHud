use std::ffi::{c_char, c_void};
use std::path::PathBuf;

use libloading::Library;
use windows::core::w;
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
use windows::Win32::System::Threading::{
    CreateMutexW, GetCurrentThread, ReleaseMutex, SetThreadAffinityMask, WaitForSingleObject,
};

type OpenFn = unsafe extern "system" fn(*mut *mut c_void) -> i32;
type LoadFn = unsafe extern "system" fn(*mut c_void, *const u8, usize) -> i32;
type ExecuteFn = unsafe extern "system" fn(
    *mut c_void,
    *const c_char,
    *const u64,
    usize,
    *mut u64,
    usize,
    *mut usize,
) -> i32;
type CloseFn = unsafe extern "system" fn(*mut c_void) -> i32;

const PCI_MUTEX_TIMEOUT_MS: u32 = 20;

/// A loaded PawnIO module (https://pawnio.eu), talking to the signed PawnIO kernel driver
/// through PawnIOLib.dll. Requires the driver to be installed and the process to be elevated.
pub struct PawnIo {
    handle: *mut c_void,
    execute: ExecuteFn,
    close: CloseFn,
    pci_mutex: Option<HANDLE>,
    _library: Library,
}

impl PawnIo {
    pub fn load_module(blob: &[u8]) -> Option<Self> {
        let library = unsafe { Library::new(library_path()?) }.ok()?;
        unsafe {
            let open = *library.get::<OpenFn>(b"pawnio_open\0").ok()?;
            let load = *library.get::<LoadFn>(b"pawnio_load\0").ok()?;
            let execute = *library.get::<ExecuteFn>(b"pawnio_execute\0").ok()?;
            let close = *library.get::<CloseFn>(b"pawnio_close\0").ok()?;

            let mut handle = std::ptr::null_mut();
            if open(&mut handle) < 0 || handle.is_null() {
                return None;
            }
            if load(handle, blob.as_ptr(), blob.len()) < 0 {
                close(handle);
                return None;
            }

            // Shared convention between hardware monitors for PCI config space access.
            let pci_mutex = CreateMutexW(None, false, w!("Global\\Access_PCI")).ok();

            Some(Self {
                handle,
                execute,
                close,
                pci_mutex,
                _library: library,
            })
        }
    }

    fn execute(&self, function: &[u8], input: u64) -> Option<u64> {
        let mut output = 0u64;
        let mut written = 0usize;
        let status = unsafe {
            (self.execute)(
                self.handle,
                function.as_ptr().cast(),
                &input,
                1,
                &mut output,
                1,
                &mut written,
            )
        };
        (status >= 0 && written == 1).then_some(output)
    }

    pub fn read_msr(&self, index: u32) -> Option<u64> {
        self.execute(b"ioctl_read_msr\0", index as u64)
    }

    /// Reads an MSR on a specific logical processor by pinning the calling thread to it.
    pub fn read_msr_on(&self, affinity_mask: usize, index: u32) -> Option<u64> {
        unsafe {
            let thread = GetCurrentThread();
            let previous = SetThreadAffinityMask(thread, affinity_mask);
            if previous == 0 {
                return None;
            }
            let value = self.read_msr(index);
            SetThreadAffinityMask(thread, previous);
            value
        }
    }

    pub fn read_smn(&self, offset: u32) -> Option<u32> {
        let locked = self.pci_mutex.map(|mutex| unsafe {
            let wait = WaitForSingleObject(mutex, PCI_MUTEX_TIMEOUT_MS);
            wait == WAIT_OBJECT_0 || wait == WAIT_ABANDONED
        });
        if locked == Some(false) {
            return None;
        }

        let value = self.execute(b"ioctl_read_smn\0", offset as u64);

        if let (Some(true), Some(mutex)) = (locked, self.pci_mutex) {
            unsafe {
                let _ = ReleaseMutex(mutex);
            }
        }
        value.map(|v| v as u32)
    }
}

impl Drop for PawnIo {
    fn drop(&mut self) {
        unsafe {
            (self.close)(self.handle);
            if let Some(mutex) = self.pci_mutex {
                let _ = CloseHandle(mutex);
            }
        }
    }
}

fn library_path() -> Option<PathBuf> {
    let from_registry = install_location().map(|dir| dir.join("PawnIOLib.dll"));
    let default = std::env::var_os("ProgramFiles")
        .map(|dir| PathBuf::from(dir).join("PawnIO").join("PawnIOLib.dll"));
    [from_registry, default].into_iter().flatten().find(|path| path.exists())
}

fn install_location() -> Option<PathBuf> {
    let mut buffer = [0u16; 512];
    let mut size = (buffer.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            w!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\PawnIO"),
            w!("InstallLocation"),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if status.is_err() {
        return None;
    }
    let len = (size as usize / 2).saturating_sub(1);
    let path = String::from_utf16_lossy(&buffer[..len]);
    (!path.is_empty()).then(|| PathBuf::from(path))
}
