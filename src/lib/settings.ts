import { load, type Store } from "@tauri-apps/plugin-store";
import { emit } from "@tauri-apps/api/event";
import type { HudBlock, HudSettings } from "../types";

export const SETTINGS_EVENT = "settings-changed";

const STORE_FILE = "settings.json";
const STORE_KEY = "hud";

export const HUD_BLOCKS: HudBlock[] = ["fps", "latency", "cpu", "gpu", "ram", "disk", "display"];

export const DEFAULT_SETTINGS: HudSettings = {
  opacity: 0.9,
  primaryColor: "#00e5ff",
  secondaryColor: "#ff2e88",
  fontFamily: "Inter",
  fontSize: 15,
  posX: 0,
  posY: 0,
  fpsStyle: "splash",
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
  showGpu: true,
  showGpuClock: true,
  showGpuMemClock: true,
  showGpuTemp: true,
  showGpuPower: true,
  showGpuGauge: false,
  showGpuModel: true,
  showGpuFan: true,
  showGpuLimit: false,
  showRam: true,
  showRamSpeed: true,
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
type StoredSettings = Partial<HudSettings> & { fpsSplash?: "top" | "bottom" | "hidden" };

function migrate(settings: HudSettings, saved: StoredSettings | undefined): HudSettings {
  let { fpsStyle, blockOrder } = settings;
  if (saved?.fpsSplash && !saved.fpsStyle) {
    fpsStyle = saved.fpsSplash === "hidden" ? "row" : "splash";
    if (saved.fpsSplash === "bottom" && !saved.blockOrder) {
      blockOrder = [...HUD_BLOCKS.filter((block) => block !== "fps"), "fps"];
    }
  }
  const { fpsSplash: _legacy, ...current } = settings as HudSettings & Pick<StoredSettings, "fpsSplash">;
  return { ...current, fpsStyle, blockOrder: normalizeBlockOrder(blockOrder) };
}

/** Drops unknown entries and duplicates, then appends blocks missing from older saves. */
export function normalizeBlockOrder(order: readonly string[] | undefined): HudBlock[] {
  const known = new Set<string>(HUD_BLOCKS);
  const result = [...new Set(order ?? [])].filter((block): block is HudBlock => known.has(block));
  return [...result, ...HUD_BLOCKS.filter((block) => !result.includes(block))];
}

/** Persists the settings and broadcasts them so the overlay updates live. */
export async function saveSettings(settings: HudSettings): Promise<void> {
  await emit(SETTINGS_EVENT, settings);
  const store = await getStore();
  await store.set(STORE_KEY, settings);
}
