export interface GpuMetrics {
  vendor: "NVIDIA" | "AMD";
  name: string;
  usage: number | null;
  tempC: number | null;
  coreClockMhz: number | null;
  memClockMhz: number | null;
  vramUsedBytes: number | null;
  vramTotalBytes: number | null;
  powerW: number | null;
  powerLimitW: number | null;
  fanPercent: number | null;
  fanRpm: number | null;
  /** Set while clocks are being limited. */
  throttleReason: "Thermal" | "Power" | "Sync" | null;
  pstate: number | null;
}

export interface CoreMetrics {
  index: number;
  usage: number;
  clockMhz: number | null;
}

/** Mirrors `SystemInfo` in src-tauri/src/metrics/mod.rs. */
export interface SystemInfo {
  cpuModel: string;
  gpuModel: string | null;
  cpuSensorSource: "pawnio" | "acpi";
  physicalCores: number;
  /** e.g. "DDR5-6000" */
  ramSpeed: string | null;
}

/** Mirrors `Metrics` in src-tauri/src/metrics/mod.rs. */
export interface Metrics {
  fps: number | null;
  fpsProcess: string | null;
  /** FileDescription / ProductName of the tracked exe, when it differs from the file name. */
  fpsGame: string | null;
  /** False when PresentMon could not start (typically: app not running as administrator). */
  fpsCapturing: boolean;
  /** The followed game stopped presenting (pause menu) or is kept in the background. */
  fpsPaused: boolean;
  frameTimeMs: number | null;
  gpuBusyMs: number | null;
  displayLatencyMs: number | null;
  /** Which side limits the frame rate; null on the desktop or without GPU tracking. */
  bound: "cpu" | "gpu" | null;
  cpuUsage: number;
  cpuClockMhz: number;
  cpuTempC: number | null;
  cpuPowerW: number | null;
  /** One entry per physical core. */
  cores: CoreMetrics[];
  /** One entry per detected CCD (AMD + PawnIO only). */
  ccdTempsC: (number | null)[];
  ramUsedBytes: number;
  ramTotalBytes: number;
  diskReadBps: number | null;
  diskWriteBps: number | null;
  gpu: GpuMetrics | null;
  screenWidth: number;
  screenHeight: number;
  refreshHz: number;
}

export type HudFont = "Inter" | "JetBrains Mono" | "Bangers" | "system-ui";

/** Comic starburst, large comic lettering, or a plain row in the stats table. */
export type FpsStyle = "splash" | "big" | "row";

export type FpsProcessSource = "exe" | "game";

export type OverlayLayout = "vertical" | "horizontal";

/** Reorderable HUD sections; in the vertical layout consecutive table blocks share one panel. */
export type HudBlock = "fps" | "latency" | "cpu" | "gpu" | "vram" | "ram" | "disk" | "display";

export interface HudSettings {
  opacity: number;
  primaryColor: string;
  secondaryColor: string;
  fontFamily: HudFont;
  fontSize: number;
  /** 0 = HUD touches the left edge, 100 = right edge. */
  posX: number;
  /** 0 = HUD touches the top edge, 100 = bottom edge. */
  posY: number;
  /** Vertical stack (default) or a single horizontal row of blocks. */
  overlayLayout: OverlayLayout;
  /** Gap in px between HUD blocks. */
  blockGap: number;
  fpsStyle: FpsStyle;
  /** Keep showing the last full-screen game's FPS while other windows are focused. */
  keepGameTracked: boolean;
  /** Show the process / game name under FPS. */
  showFpsProcess: boolean;
  /** Exe file name, or the version-resource game name when available. */
  fpsProcessSource: FpsProcessSource;
  /** Font size of the large FPS lettering, in px. */
  fpsBigSize: number;
  /** Width and height of the FPS starburst, in px. */
  fpsSplashSize: number;
  blockOrder: HudBlock[];
  /** Stats panel background. */
  panelColor: string;
  panelOpacity: number;
  /** Display latency, GPU busy time and CPU/GPU-bound verdict. */
  showLatency: boolean;
  /** Whole CPU row. */
  showCpu: boolean;
  showCpuClock: boolean;
  showCpuTemp: boolean;
  showCpuPower: boolean;
  /** Color-changing temperature slider under the CPU row. */
  showCpuGauge: boolean;
  showCpuModel: boolean;
  /** Per-core panel; also toggled from the tray menu and Ctrl+Shift+C. */
  showCpuCores: boolean;
  /** CCD temperatures in the per-core panel header. */
  showCpuCcdTemp: boolean;
  /** GPU usage, clocks, fan, limit and temperature. */
  showGpu: boolean;
  showGpuClock: boolean;
  /** VRAM used / total row. */
  showVram: boolean;
  showGpuMemClock: boolean;
  showGpuTemp: boolean;
  showGpuPower: boolean;
  showGpuGauge: boolean;
  showGpuModel: boolean;
  showGpuFan: boolean;
  /** P-state, share of the power limit and throttle reason. */
  showGpuLimit: boolean;
  showRam: boolean;
  showRamSpeed: boolean;
  showDisk: boolean;
  showDisplay: boolean;
}
