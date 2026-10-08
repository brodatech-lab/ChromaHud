import { load, type Store } from "@tauri-apps/plugin-store";
import { emit } from "@tauri-apps/api/event";
import type { GpuSlotSettings, HudBlock, HudSettings } from "../types";

export const SETTINGS_EVENT = "settings-changed";

const STORE_FILE = "settings.json";
const STORE_KEY = "hud";

export const HUD_BLOCKS: HudBlock[] = [
  "fps",
  "latency",
  "cpu",
  "gpu1",
  "gpu2",
  "vram1",
  "vram2",
  "ram",
  "disk",
  "display",
];

export const DEFAULT_GPU_SLOT: GpuSlotSettings = {
  enabled: true,
  showUsage: true,
  showClock: true,
  showTemp: true,
  showPower: true,
  showGauge: false,
  showModel: true,
  showFan: true,
  showLimit: false,
  showVram: true,
  showMemClock: true,
};

/** Slot `index`, or the first saved slot, or the factory default. */
export function gpuSlotAt(settings: HudSettings, index: number): GpuSlotSettings {
  return settings.gpuSlots[index] ?? settings.gpuSlots[0] ?? DEFAULT_GPU_SLOT;
}

export const DEFAULT_SETTINGS: HudSettings = {
  opacity: 0.9,
  primaryColor: "#00e5ff",
  secondaryColor: "#ff2e88",
  modelColor: "#9ca3af",
  valueColor: "#ffffff",
  fontFamily: "Inter",
  fontSize: 15,
  posX: 0,
  posY: 0,
  overlayLayout: "vertical",
  horizontalWrap: false,
  blockGap: 12,
  fpsStyle: "splash",
  keepGameTracked: false,
  showFpsProcess: true,
  fpsProcessSource: "exe",
  fpsBigSize: 72,
  fpsSplashSize: 150,
  blockOrder: HUD_BLOCKS,
  panelColor: "#000000",
  panelOpacity: 0.55,
  showLatency: false,
  showCpu: true,
  showCpuClock: true,
  showCpuTemp: true,
  showCpuPower: true,
  showCpuGauge: false,
  showCpuModel: true,
  showCpuCores: false,
  showCpuCcdTemp: true,
  gpuSlots: [DEFAULT_GPU_SLOT],
  showRam: true,
  showRamSpeed: true,
  showRamManufacturer: true,
  showDisk: false,
  showDisplay: true,
};

let storePromise: Promise<Store> | null = null;

function getStore(): Promise<Store> {
  storePromise ??= load(STORE_FILE, { autoSave: 300, defaults: {} });
  return storePromise;
}

export async function loadSettings(): Promise<HudSettings> {
  const store = await getStore();
  const saved = await store.get<StoredSettings>(STORE_KEY);
  return migrate({ ...DEFAULT_SETTINGS, ...saved }, saved);
}

/** Settings as persisted by any app version; `fpsSplash` predates `fpsStyle` / `blockOrder`. */
type StoredSettings = Partial<HudSettings> & {
  fpsSplash?: "top" | "bottom" | "hidden";
  showGpu?: boolean;
  showGpuClock?: boolean;
  showGpuTemp?: boolean;
  showGpuPower?: boolean;
  showGpuGauge?: boolean;
  showGpuModel?: boolean;
  showGpuFan?: boolean;
  showGpuLimit?: boolean;
  showVram?: boolean;
  showGpuMemClock?: boolean;
};

function slotFromLegacy(saved: StoredSettings | undefined): GpuSlotSettings {
  return {
    enabled: true,
    showUsage: saved?.showGpu ?? DEFAULT_GPU_SLOT.showUsage,
    showClock: saved?.showGpuClock ?? DEFAULT_GPU_SLOT.showClock,
    showTemp: saved?.showGpuTemp ?? DEFAULT_GPU_SLOT.showTemp,
    showPower: saved?.showGpuPower ?? DEFAULT_GPU_SLOT.showPower,
    showGauge: saved?.showGpuGauge ?? DEFAULT_GPU_SLOT.showGauge,
    showModel: saved?.showGpuModel ?? DEFAULT_GPU_SLOT.showModel,
    showFan: saved?.showGpuFan ?? DEFAULT_GPU_SLOT.showFan,
    showLimit: saved?.showGpuLimit ?? DEFAULT_GPU_SLOT.showLimit,
    showVram: saved?.showVram ?? DEFAULT_GPU_SLOT.showVram,
    showMemClock: saved?.showGpuMemClock ?? DEFAULT_GPU_SLOT.showMemClock,
  };
}

function migrate(settings: HudSettings, saved: StoredSettings | undefined): HudSettings {
  let { fpsStyle, blockOrder } = settings;
  if (saved?.fpsSplash && !saved.fpsStyle) {
    fpsStyle = saved.fpsSplash === "hidden" ? "row" : "splash";
    if (saved.fpsSplash === "bottom" && !saved.blockOrder) {
      blockOrder = [...HUD_BLOCKS.filter((block) => block !== "fps"), "fps"];
    }
  }
  const gpuSlots =
    saved?.gpuSlots && saved.gpuSlots.length > 0
      ? saved.gpuSlots.map((slot) => ({ ...DEFAULT_GPU_SLOT, ...slot }))
      : [slotFromLegacy(saved)];
  const { fpsSplash: _legacy, ...current } = settings as HudSettings & Pick<StoredSettings, "fpsSplash">;
  return {
    ...current,
    fpsStyle,
    gpuSlots,
    blockOrder: normalizeBlockOrder(blockOrder),
    blockGap: Math.min(48, Math.max(0, settings.blockGap)),
  };
}

/** Older saves used a single `gpu` / `vram` entry that the overlay expanded to every card. */
const LEGACY_BLOCKS: Record<string, HudBlock> = { gpu: "gpu1", vram: "vram1" };

/** Drops unknown entries and duplicates, then appends blocks missing from older saves. */
export function normalizeBlockOrder(order: readonly string[] | undefined): HudBlock[] {
  const known = new Set<string>(HUD_BLOCKS);
  const mapped = (order ?? []).map((block) => LEGACY_BLOCKS[block] ?? block);
  const result = [...new Set(mapped)].filter((block): block is HudBlock => known.has(block));
  for (const block of HUD_BLOCKS) {
    if (result.includes(block)) continue;
    const after =
      block === "gpu2" ? "gpu1" : block === "vram1" ? "gpu1" : block === "vram2" ? "vram1" : null;
    if (after) {
      const idx = result.indexOf(after);
      if (idx >= 0) {
        result.splice(idx + 1, 0, block);
        continue;
      }
    }
    result.push(block);
  }
  return result;
}

/** Persists the settings and broadcasts them so the overlay updates live. */
export async function saveSettings(settings: HudSettings): Promise<void> {
  await emit(SETTINGS_EVENT, settings);
  const store = await getStore();
  await store.set(STORE_KEY, settings);
}
