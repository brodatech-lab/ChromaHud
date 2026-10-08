export interface GpuMetrics {
  index: number;
  vendor: "NVIDIA" | "AMD" | "Intel" | "GPU";
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
export interface GpuInfo {
  name: string;
  vendor: string;
}

export interface SystemInfo {
  cpuModel: string;
  gpuModel: string | null;
  gpus: GpuInfo[];
  cpuSensorSource: "pawnio" | "acpi";
  physicalCores: number;
  /** e.g. "DDR5-6000" */
  ramSpeed: string | null;
  /** SPD manufacturer, without the module SKU. */
  ramManufacturer: string | null;
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
  /** One entry per detected CCD (AMD + PawnIO only; empty on Intel). */
  ccdTempsC: (number | null)[];
  ramUsedBytes: number;
  ramTotalBytes: number;
  diskReadBps: number | null;
  diskWriteBps: number | null;
  gpus: GpuMetrics[];
  screenWidth: number;
  screenHeight: number;
  refreshHz: number;
}

export type HudFont =
  | "Inter"
  | "Rajdhani"
  | "Oswald"
  | "Chakra Petch"
  | "Orbitron"
  | "JetBrains Mono"
  | "Share Tech Mono"
  | "Bangers"
  | "system-ui";

/** Comic starburst, large comic lettering, or a plain row in the stats table. */
export type FpsStyle = "splash" | "big" | "row";

export type FpsProcessSource = "exe" | "game";

export type OverlayLayout = "vertical" | "horizontal";

export interface GpuSlotSettings {
  /** Master switch for this card. Individual toggles stay as they were. */
  enabled: boolean;
  showUsage: boolean;
  showClock: boolean;
  showTemp: boolean;
  showPower: boolean;
  showGauge: boolean;
  showModel: boolean;
  showFan: boolean;
  showLimit: boolean;
  showVram: boolean;
  showMemClock: boolean;
}

/** Reorderable HUD sections; in the vertical layout consecutive table blocks share one panel. */
export type HudBlock =
  | "fps"
  | "latency"
  | "cpu"
  | "gpu1"
  | "gpu2"
  | "vram1"
  | "vram2"
  | "ram"
  | "disk"
  | "display";

export interface HudSettings {
  opacity: number;
  primaryColor: string;
  secondaryColor: string;
  /** CPU / GPU / RAM model captions. */
  modelColor: string;
  /** Usage percentages, clocks, temperatures and other numeric values. */
  valueColor: string;
  fontFamily: HudFont;
  fontSize: number;
  /** 0 = HUD touches the left edge, 100 = right edge. */
  posX: number;
  /** 0 = HUD touches the top edge, 100 = bottom edge. */
  posY: number;
  /** Vertical stack (default) or a single horizontal row of blocks. */
  overlayLayout: OverlayLayout;
  /** Horizontal: later blocks wrap to the next row at 10 px from the screen edge. */
  horizontalWrap: boolean;
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
  /** Per-GPU overlay toggles; missing slots use the first slot / defaults. */
  gpuSlots: GpuSlotSettings[];
  showRam: boolean;
  showRamSpeed: boolean;
  showRamManufacturer: boolean;
  showDisk: boolean;
  showDisplay: boolean;
}
