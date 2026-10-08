# Changelog

All notable changes to ChromaHUD are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Multi-GPU overlay: every NVIDIA, AMD and leftover Windows-counter adapter is listed as GPU 1, GPU 2, …
  (Sensors tab and HUD). VRAM follows the same numbering. A single GPU still uses the unnumbered labels.
- Order lists GPU 1, GPU 2, VRAM 1 and VRAM 2 as separate blocks so each card can be placed independently.
- Intel CPU sensors through PawnIO (`IntelMSR.bin`): package temperature (DTS), RAPL package power and
  per-core clocks (APERF/MPERF). Without PawnIO, per-core clocks fall back to Windows `% Processor
  Performance` counters.
- RAM manufacturer from Windows (`Win32_PhysicalMemory`), shown as a caption; Sensors has a dedicated
  RAM group with usage, speed and manufacturer toggles. Several kits are listed together (`G Skill Intl, Kingston`).
- Per-GPU Show checkbox hides that card (and its VRAM) without clearing the other sensor toggles.
  A single tracked GPU is labelled GPU / VRAM; two tracked cards stay GPU 1 / GPU 2.
- Overlay text colors for model names and numeric values (Look > Colors). The font size slider
  scales the stats panel evenly (labels, values, padding).
- **Wrap to next row** (Look > Size): in the horizontal layout, blocks that would cross 10 px
  from the screen edge move onto the next row instead of staying in one line.
- HUD fonts in Look > Text: Rajdhani, Oswald, Chakra Petch, Orbitron, and Share Tech Mono,
  alongside Inter, JetBrains Mono, and Bangers. The same face is used for the FPS splash
  and big-text styles, including the process name.

### Changed

- Look: Size is its own control (not under Text). It still scales the whole HUD.
- The horizontal layout stays 10 px from the screen edges. Without wrap, further Size increases
  stop once the HUD fills that area; with wrap, extra blocks go below until the same 10 px limit.
- Horizontal fit no longer measures the same element it scales. The previous loop could freeze the desktop.

## [0.3.0] - 2026-10-06

### Added

- Horizontal overlay layout (Layout > Arrangement): blocks sit in one row and can be moved to any
  screen edge. The per-core CPU panel is hidden in this layout.
- Gap slider between HUD blocks in the horizontal layout (Layout > Arrangement, 0–48 px).
- Show / hide the name under FPS, and choose the exe file name or the game name from the executable's
  version resource (FileDescription / ProductName).
- "Keep tracking the game after Alt+Tab" option (Layout > FPS): the overlay keeps showing the last
  full-screen game's FPS and usage while other windows are focused. Another full-screen game takes over;
  the game is dropped when its process exits.

### Changed

- A focused game that stops presenting frames (e.g. a pause or graphics menu) now shows 0 FPS labelled
  "paused" instead of switching to the desktop.
- GPU and VRAM are separate reorderable HUD blocks. Older layouts keep VRAM next to GPU.

## [0.2.0] - 2026-10-06

### Added

- AMD Radeon GPU telemetry through ADLX (`amdadlx64.dll` from the Adrenalin driver): usage, core and
  VRAM clock, temperature, power, VRAM usage and fan RPM. A discrete Radeon is preferred over the
  integrated one. On integrated Radeons the usage comes from the Windows GPU counters, because ADLX
  reports instantaneous values that flip between 0 and 100%.
- Vendor-neutral fallback from the Windows GPU counters (usage and memory, like Task Manager) for GPUs
  without a vendor library, e.g. Intel Arc, and for values ADLX does not report.
- `chromahud.exe --dump-metrics <n>`: prints `n` metric samples as JSON lines without opening any window,
  for checking sensors over SSH.

### Changed

- GPU rows without data (FAN, LIMIT, VRAM) are hidden instead of showing `--`. VRAM without a known total
  shows only the used amount; small totals (integrated GPUs) keep one decimal.
- GitHub release pages list the changes of that version ("What's new") taken from this changelog;
  install instructions live in the README.

## [0.1.1] - 2026-10-05

### Changed

- The installer now installs for all users to `C:\Program Files\ChromaHUD` (administrator prompt during
  setup) instead of `%LOCALAPPDATA%\ChromaHUD`.
- The installer is English-only. The Polish page had no translations in Tauri, which left the
  "Create desktop shortcut" checkbox on the finish page without a label.
- New comic-burst app icon (installer, Start menu, taskbar and tray) instead of the default Tauri logo.
- The one-line installer (`install.ps1`) launches ChromaHUD from Program Files.
- README: hero screenshot, a hardware support table (NVIDIA GPU only; AMD Ryzen full, Intel CPU partial)
  and install location notes.

### Upgrading from 0.1.0

Uninstall 0.1.0 first (**Settings > Apps > Installed apps > ChromaHUD**). It was installed per user, so
the new per-machine installer does not replace it.

## [0.1.0] - 2026-10-05

### Added

- Click-through, always-on-top overlay with FPS (PresentMon), frame time, GPU busy, display latency and a
  CPU-bound / GPU-bound verdict.
- FPS styles: comic splash, big comic text and a table row, with size sliders.
- CPU usage, clocks, temperature, package power, per-core details and CCD temperatures (AMD Zen via PawnIO).
- NVIDIA GPU usage, clocks, VRAM, temperature, power, fan % and RPM, P-state, power limit and throttle reason.
- RAM usage and speed, disk throughput, resolution and refresh rate.
- Settings window with Look / Layout / Sensors tabs, drag-and-drop block order and panel color / opacity.
- Windows release pipeline: NSIS installer, portable zip and a one-command PowerShell installer.

[Unreleased]: https://github.com/brodatech-lab/ChromaHud/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/brodatech-lab/ChromaHud/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/brodatech-lab/ChromaHud/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/brodatech-lab/ChromaHud/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/brodatech-lab/ChromaHud/releases/tag/v0.1.0
