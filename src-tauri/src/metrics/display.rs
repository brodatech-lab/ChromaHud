use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::{EnumDisplaySettingsW, DEVMODEW, ENUM_CURRENT_SETTINGS};

pub struct DisplayMode {
    pub width: u32,
    pub height: u32,
    pub refresh_hz: u32,
}

/// Current mode of the primary display in physical pixels (independent of DPI scaling).
pub fn primary_display_mode() -> Option<DisplayMode> {
    let mut mode = DEVMODEW {
        dmSize: std::mem::size_of::<DEVMODEW>() as u16,
        ..Default::default()
    };

    let ok = unsafe { EnumDisplaySettingsW(PCWSTR::null(), ENUM_CURRENT_SETTINGS, &mut mode) };
    ok.as_bool().then(|| DisplayMode {
        width: mode.dmPelsWidth,
        height: mode.dmPelsHeight,
        refresh_hz: mode.dmDisplayFrequency,
    })
}
