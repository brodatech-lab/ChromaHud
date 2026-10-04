# ChromaHUD

Lightweight, click-through performance overlay for Windows games with a comic-book look.
Built with Tauri v2 (Rust) + React + Vite + TypeScript + Tailwind CSS v4.

> **This project is vibecoded.** It was built almost entirely through conversation with an AI coding
> assistant (Cursor agent): the human described what they wanted, tested it on real hardware and gave
> feedback; the AI wrote the code. Expect rough edges, treat hardware-level parts (driver access,
> sensor decoding) with care, and review before reusing anything in production.

## Features

- Transparent, always-on-top, click-through overlay (does not steal focus or mouse input).
- FPS of the foreground app in three styles: comic starburst splash, big comic lettering, or a plain table row.
- Display latency, GPU busy time and a CPU-bound / GPU-bound verdict.
- CPU: usage, effective clock, temperature (Tctl), package power, model name, per-core usage and clocks, CCD temperatures.
- GPU (NVIDIA): usage, core / VRAM clock, temperature, power, VRAM usage, fan % and RPM, P-state, power limit and throttle reason.
- RAM usage and speed (e.g. DDR5-6000), disk throughput, screen resolution and refresh rate.
- Color-changing temperature gauges for CPU and GPU.
- Settings window: colors, panel background and opacity, font and size, position sliders and presets,
  drag-and-drop block order, and a toggle for every single value.

## Metrics sources

| Metric | Source |
| --- | --- |
| FPS, frame time, GPU busy, display latency | PresentMon console sidecar (ETW), needs admin |
| CPU usage, per-core usage, model | sysinfo |
| CPU temperature (Tctl), CCD temperatures, package power, per-core clocks (AMD Zen) | PawnIO driver + signed `AMDFamily17` module, needs admin |
| CPU clock / temperature fallback | PDH `% Processor Performance` + WMI `MSAcpi_ThermalZoneTemperature` (not exposed on every board) |
| RAM usage / speed | sysinfo / WMI `Win32_PhysicalMemory` |
| Disk throughput | PDH `PhysicalDisk(_Total)` counters |
| GPU (NVIDIA) | NVML (`nvml.dll` from the driver); AMD Radeon support is planned |
| Resolution / refresh rate | `EnumDisplaySettingsW` |

## Requirements

- Windows 10/11 x64
- Node.js 20+ and the Rust toolchain (MSVC)
- Optional: [PawnIO](https://pawnio.eu/) driver for AMD CPU temperature, power and per-core clocks

## Setup

```powershell
npm install
npm run fetch:presentmon   # downloads PresentMon into src-tauri/binaries
npm run fetch:pawnio       # downloads the signed PawnIO modules into src-tauri/resources/pawnio
winget install namazso.PawnIO --source winget   # PawnIO driver (optional)
```

Both fetch steps are required before building: PresentMon is bundled as a sidecar and the PawnIO
module is embedded into the binary. Without the PawnIO driver installed the app still runs; CPU
temperature, power and per-core clocks fall back to ACPI/PDH and the settings window shows a
"limited" sensor status.

## Development

```powershell
npm run tauri dev
```

Run the terminal **as administrator** to get FPS and CPU sensors during development
(`scripts/dev-admin.ps1` opens an elevated dev session for you). Release builds request elevation
automatically through the app manifest.

## Release build

```powershell
npm run tauri build
```

## Controls

- `Ctrl+Shift+H` - show / hide the overlay
- `Ctrl+Shift+O` - open settings (also: left-click the tray icon)
- `Ctrl+Shift+C` - expand / collapse per-core CPU details (also: tray menu, settings)
- Tray menu - show/hide overlay, show CPU cores, settings, quit

## Project layout

```
src/                     React frontend (overlay + settings windows share one bundle)
  overlay/               HUD components (FPS splash, big text, stat rows, gauges, cores panel)
  settings/              settings window with Look / Layout / Sensors tabs
src-tauri/src/           Rust backend
  metrics/               collectors: fps (PresentMon), gpu (NVML), cpu_sensors (PawnIO / ACPI), disk, memory, display
  tray.rs, lib.rs        tray menu, global shortcuts, window setup
scripts/                 PresentMon / PawnIO download helpers, elevated dev launcher
```

## Notes

- The overlay is visible over windowed and borderless-fullscreen games, not exclusive fullscreen.
- If Windows Controlled Folder Access blocks cargo from writing `target/` (e.g. when the project lives on the
  Desktop), point the build directory elsewhere in a local `src-tauri/.cargo/config.toml`:
  ```toml
  [build]
  target-dir = "C:/Users/<you>/AppData/Local/ChromaHUD-target"
  ```

## Third-party components

- [PresentMon](https://github.com/GameTechDev/PresentMon) (MIT) - frame timing via ETW
- [PawnIO](https://github.com/namazso/PawnIO) and [PawnIO.Modules](https://github.com/namazso/PawnIO.Modules) - kernel access for CPU sensors
