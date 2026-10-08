import { useState, type ReactNode } from "react";
import BlockOrderList from "./BlockOrderList";
import FpsSplash from "../overlay/FpsSplash";
import { useHudSettings } from "../hooks/useHudSettings";
import { useSystemInfo } from "../hooks/useSystemInfo";
import { DEFAULT_GPU_SLOT, DEFAULT_SETTINGS, gpuSlotAt } from "../lib/settings";
import type {
  FpsProcessSource,
  FpsStyle,
  GpuSlotSettings,
  HudBlock,
  HudFont,
  HudSettings,
  OverlayLayout,
  SystemInfo,
} from "../types";

type Tab = "look" | "layout" | "sensors";

const TABS: { value: Tab; label: string }[] = [
  { value: "look", label: "Look" },
  { value: "layout", label: "Layout" },
  { value: "sensors", label: "Sensors" },
];

const FONTS: HudFont[] = [
  "Inter",
  "Rajdhani",
  "Oswald",
  "Chakra Petch",
  "Orbitron",
  "JetBrains Mono",
  "Share Tech Mono",
  "Bangers",
  "system-ui",
];

const POSITION_PRESETS: { label: string; posX: number; posY: number }[] = [
  { label: "Top left", posX: 0, posY: 0 },
  { label: "Top center", posX: 50, posY: 0 },
  { label: "Top right", posX: 100, posY: 0 },
  { label: "Middle left", posX: 0, posY: 50 },
  { label: "Center", posX: 50, posY: 50 },
  { label: "Middle right", posX: 100, posY: 50 },
  { label: "Bottom left", posX: 0, posY: 100 },
  { label: "Bottom center", posX: 50, posY: 100 },
  { label: "Bottom right", posX: 100, posY: 100 },
];

const FPS_STYLES: { value: FpsStyle; label: string }[] = [
  { value: "splash", label: "Splash" },
  { value: "big", label: "Big text" },
  { value: "row", label: "Row" },
];

const FPS_PROCESS_SOURCES: { value: FpsProcessSource; label: string }[] = [
  { value: "exe", label: "Exe" },
  { value: "game", label: "Game name" },
];

const LAYOUTS: { value: OverlayLayout; label: string }[] = [
  { value: "vertical", label: "Vertical" },
  { value: "horizontal", label: "Horizontal" },
];

const BLOCK_LABELS: Record<HudBlock, string> = {
  fps: "FPS",
  latency: "Latency",
  cpu: "CPU",
  gpu1: "GPU 1",
  gpu2: "GPU 2",
  vram1: "VRAM 1",
  vram2: "VRAM 2",
  ram: "RAM",
  disk: "Disk",
  display: "Resolution",
};

type ToggleKey = { [K in keyof HudSettings]: HudSettings[K] extends boolean ? K : never }[keyof HudSettings];

interface ToggleDef {
  key: ToggleKey;
  label: string;
  /** Part of a row that only renders when this parent toggle is on. */
  parent?: ToggleKey;
}

const SENSOR_GROUPS: { title: string; toggles: ToggleDef[] }[] = [
  {
    title: "Frame",
    toggles: [{ key: "showLatency", label: "Latency / GPU busy" }],
  },
  {
    title: "CPU",
    toggles: [
      { key: "showCpu", label: "Usage row" },
      { key: "showCpuClock", label: "Clock", parent: "showCpu" },
      { key: "showCpuTemp", label: "Temperature", parent: "showCpu" },
      { key: "showCpuPower", label: "Power", parent: "showCpu" },
      { key: "showCpuGauge", label: "Temperature gauge" },
      { key: "showCpuModel", label: "Model name" },
      { key: "showCpuCores", label: "Per-core details" },
      { key: "showCpuCcdTemp", label: "CCD temperature", parent: "showCpuCores" },
    ],
  },
  {
    title: "RAM",
    toggles: [
      { key: "showRam", label: "Usage row" },
      { key: "showRamSpeed", label: "RAM speed", parent: "showRam" },
      { key: "showRamManufacturer", label: "Manufacturer" },
    ],
  },
  {
    title: "Other",
    toggles: [
      { key: "showDisk", label: "Disk" },
      { key: "showDisplay", label: "Resolution" },
    ],
  },
];

const GPU_SLOT_TOGGLES: { key: keyof GpuSlotSettings; label: string; parent?: keyof GpuSlotSettings }[] = [
  { key: "showUsage", label: "Usage row" },
  { key: "showClock", label: "Clock", parent: "showUsage" },
  { key: "showTemp", label: "Temperature", parent: "showUsage" },
  { key: "showPower", label: "Power", parent: "showUsage" },
  { key: "showGauge", label: "Temperature gauge" },
  { key: "showModel", label: "Model name" },
  { key: "showFan", label: "Fan" },
  { key: "showLimit", label: "Power limit / throttling" },
  { key: "showVram", label: "VRAM" },
  { key: "showMemClock", label: "VRAM clock", parent: "showVram" },
];

const SENSOR_SOURCE_TEXT: Record<SystemInfo["cpuSensorSource"], string> = {
  pawnio: "CPU sensors: PawnIO (temperature, power, per-core clocks)",
  acpi: "CPU sensors: limited. Install PawnIO for temperature, power and per-core clocks: winget install namazso.PawnIO",
};

function gpuBlockEnabled(slot: GpuSlotSettings): boolean {
  return slot.enabled && (slot.showUsage || slot.showModel || slot.showGauge || slot.showFan || slot.showLimit);
}

/** Whether a block renders anything with the current sensor toggles and detected GPU count. */
function isBlockEnabled(settings: HudSettings, block: HudBlock, gpuCount: number): boolean {
  switch (block) {
    case "fps":
      return true;
    case "latency":
      return settings.showLatency;
    case "cpu":
      return settings.showCpu || settings.showCpuModel || settings.showCpuGauge || settings.showCpuCores;
    case "gpu1":
      return gpuBlockEnabled(gpuSlotAt(settings, 0));
    case "gpu2":
      return gpuCount > 1 && gpuBlockEnabled(gpuSlotAt(settings, 1));
    case "vram1":
      return gpuSlotAt(settings, 0).enabled && gpuSlotAt(settings, 0).showVram;
    case "vram2":
      return gpuCount > 1 && gpuSlotAt(settings, 1).enabled && gpuSlotAt(settings, 1).showVram;
    case "ram":
      return settings.showRam || settings.showRamManufacturer;
    case "disk":
      return settings.showDisk;
    case "display":
      return settings.showDisplay;
  }
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-2.5 rounded-xl border border-white/5 bg-white/[0.03] p-3">
      <h2 className="text-[11px] font-semibold uppercase tracking-[0.2em] text-white/50">{title}</h2>
      {children}
    </section>
  );
}

function ChoiceButton({ active, color, onClick, children }: { active: boolean; color: string; onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="rounded-md border px-2 py-1 text-xs transition-colors hover:bg-white/5"
      style={active ? { borderColor: color, color } : { borderColor: "rgba(255,255,255,0.1)" }}
    >
      {children}
    </button>
  );
}

function Field({ label, value, children }: { label: string; value?: string; children: ReactNode }) {
  return (
    <label className="flex flex-col gap-1 text-sm">
      <span className="flex justify-between text-white/80">
        {label}
        {value && <span className="tabular-nums text-white/50">{value}</span>}
      </span>
      {children}
    </label>
  );
}

function Slider({ value, min, max, step, onChange }: { value: number; min: number; max: number; step: number; onChange: (value: number) => void }) {
  return <input type="range" min={min} max={max} step={step} value={value} onChange={(e) => onChange(Number(e.target.value))} />;
}

function ColorInput({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  return (
    <input
      type="color"
      className="h-8 w-full cursor-pointer rounded border border-white/10 bg-transparent"
      value={value}
      onChange={(e) => onChange(e.target.value)}
    />
  );
}

export default function Settings() {
  const { settings, update } = useHudSettings();
  const systemInfo = useSystemInfo();
  const [tab, setTab] = useState<Tab>("look");
  const accent = settings.primaryColor;
  const gpuCount = Math.max(1, systemInfo?.gpus.length ?? settings.gpuSlots.length);
  const gpuGroups = gpuCount > 1 ? Array.from({ length: gpuCount }, (_, i) => `GPU ${i + 1}`) : ["GPU"];

  return (
    <div className="flex min-h-full flex-col gap-3 p-4" style={{ accentColor: accent }}>
      <header className="flex items-center gap-3">
        <FpsSplash
          fps={144}
          primaryColor={settings.primaryColor}
          secondaryColor={settings.secondaryColor}
          fontFamily={settings.fontFamily}
          size={56}
        />
        <div className="min-w-0 flex-1">
          <div className="flex items-baseline justify-between">
            <h1 className="font-comic text-2xl tracking-wider" style={{ color: accent }}>
              ChromaHUD
            </h1>
            <button type="button" onClick={() => update(DEFAULT_SETTINGS)} className="text-xs text-white/40 hover:text-white/80">
              Reset to defaults
            </button>
          </div>
          <p className="truncate text-[11px] text-white/45">Ctrl+Shift+H overlay · Ctrl+Shift+O settings · Ctrl+Shift+C CPU cores</p>
        </div>
      </header>

      <nav className="grid grid-cols-3 gap-1.5">
        {TABS.map(({ value, label }) => (
          <ChoiceButton key={value} active={tab === value} color={accent} onClick={() => setTab(value)}>
            {label}
          </ChoiceButton>
        ))}
      </nav>

      {tab === "look" && (
        <>
          <Section title="Colors">
            <div className="grid grid-cols-2 gap-3">
              <Field label="Primary" value={settings.primaryColor}>
                <ColorInput value={settings.primaryColor} onChange={(primaryColor) => update({ primaryColor })} />
              </Field>
              <Field label="Secondary" value={settings.secondaryColor}>
                <ColorInput value={settings.secondaryColor} onChange={(secondaryColor) => update({ secondaryColor })} />
              </Field>
              <Field label="Model names" value={settings.modelColor}>
                <ColorInput value={settings.modelColor} onChange={(modelColor) => update({ modelColor })} />
              </Field>
              <Field label="Values" value={settings.valueColor}>
                <ColorInput value={settings.valueColor} onChange={(valueColor) => update({ valueColor })} />
              </Field>
              <Field label="Panel background" value={settings.panelColor}>
                <ColorInput value={settings.panelColor} onChange={(panelColor) => update({ panelColor })} />
              </Field>
              <Field label="Panel opacity" value={`${Math.round(settings.panelOpacity * 100)}%`}>
                <Slider value={settings.panelOpacity} min={0} max={1} step={0.05} onChange={(panelOpacity) => update({ panelOpacity })} />
              </Field>
            </div>
            <Field label="HUD opacity" value={`${Math.round(settings.opacity * 100)}%`}>
              <Slider value={settings.opacity} min={0.2} max={1} step={0.05} onChange={(opacity) => update({ opacity })} />
            </Field>
          </Section>

          <Section title="Size">
            <Field label="Size" value={`${settings.fontSize}px`}>
              <Slider value={settings.fontSize} min={11} max={32} step={1} onChange={(fontSize) => update({ fontSize })} />
            </Field>
            <label className="flex items-center gap-2 text-sm text-white/80">
              <input
                type="checkbox"
                checked={settings.horizontalWrap}
                onChange={(e) => update({ horizontalWrap: e.target.checked })}
              />
              Wrap to next row
            </label>
            <p className="-mt-1 text-[11px] text-white/40">
              Horizontal layout: blocks that would cross 10 px from the screen edge move to the next row.
            </p>
          </Section>

          <Section title="Text">
            <Field label="Font">
              <select
                className="rounded-md border border-white/10 bg-[#161922] px-2 py-1.5 text-sm"
                value={settings.fontFamily}
                onChange={(e) => update({ fontFamily: e.target.value as HudFont })}
              >
                {FONTS.map((font) => (
                  <option key={font} value={font} style={{ fontFamily: font }}>
                    {font}
                  </option>
                ))}
              </select>
            </Field>
          </Section>
        </>
      )}

      {tab === "layout" && (
        <>
          <Section title="Arrangement">
            <div className="grid grid-cols-2 gap-1.5">
              {LAYOUTS.map(({ value, label }) => (
                <ChoiceButton
                  key={value}
                  active={settings.overlayLayout === value}
                  color={accent}
                  onClick={() => {
                    if (
                      value === "horizontal" &&
                      settings.overlayLayout !== "horizontal" &&
                      settings.posX === 0 &&
                      settings.posY === 0
                    ) {
                      update({ overlayLayout: value, posX: 50, posY: 0 });
                    } else {
                      update({ overlayLayout: value });
                    }
                  }}
                >
                  {label}
                </ChoiceButton>
              ))}
            </div>
            {settings.overlayLayout === "horizontal" && (
              <>
                <p className="-mt-1 text-[11px] text-white/40">
                  The per-core CPU panel is hidden in this layout. The HUD stays 10 px from the screen
                  edges.
                </p>
                <Field label="Gap between blocks" value={`${settings.blockGap}px`}>
                  <Slider
                    value={settings.blockGap}
                    min={0}
                    max={48}
                    step={2}
                    onChange={(blockGap) => update({ blockGap })}
                  />
                </Field>
              </>
            )}
          </Section>

          <Section title="Position">
            <div className="grid grid-cols-2 gap-3">
              <Field label="Horizontal" value={`${settings.posX}%`}>
                <Slider value={settings.posX} min={0} max={100} step={0.5} onChange={(posX) => update({ posX })} />
              </Field>
              <Field label="Vertical" value={`${settings.posY}%`}>
                <Slider value={settings.posY} min={0} max={100} step={0.5} onChange={(posY) => update({ posY })} />
              </Field>
            </div>
            <div className="grid grid-cols-3 gap-1">
              {POSITION_PRESETS.map(({ label, posX, posY }) => (
                <ChoiceButton
                  key={label}
                  active={settings.posX === posX && settings.posY === posY}
                  color={accent}
                  onClick={() => update({ posX, posY })}
                >
                  {label}
                </ChoiceButton>
              ))}
            </div>
          </Section>

          <Section title="FPS">
            <div className="grid grid-cols-3 gap-1.5">
              {FPS_STYLES.map(({ value, label }) => (
                <ChoiceButton key={value} active={settings.fpsStyle === value} color={accent} onClick={() => update({ fpsStyle: value })}>
                  {label}
                </ChoiceButton>
              ))}
            </div>
            {settings.fpsStyle === "splash" && (
              <Field label="Splash size" value={`${settings.fpsSplashSize}px`}>
                <Slider value={settings.fpsSplashSize} min={100} max={260} step={5} onChange={(fpsSplashSize) => update({ fpsSplashSize })} />
              </Field>
            )}
            {settings.fpsStyle === "big" && (
              <Field label="Text size" value={`${settings.fpsBigSize}px`}>
                <Slider value={settings.fpsBigSize} min={32} max={160} step={2} onChange={(fpsBigSize) => update({ fpsBigSize })} />
              </Field>
            )}
            <label className="flex items-center gap-2 text-sm text-white/80">
              <input
                type="checkbox"
                checked={settings.keepGameTracked}
                onChange={(e) => update({ keepGameTracked: e.target.checked })}
              />
              Keep tracking the game after Alt+Tab
            </label>
            <p className="-mt-1 text-[11px] text-white/40">
              Overlay keeps showing the last full-screen game while other windows are focused.
            </p>
            <label className="flex items-center gap-2 text-sm text-white/80">
              <input
                type="checkbox"
                checked={settings.showFpsProcess}
                onChange={(e) => update({ showFpsProcess: e.target.checked })}
              />
              Show process name
            </label>
            <div className={`grid grid-cols-2 gap-1.5 ${settings.showFpsProcess ? "" : "pointer-events-none opacity-40"}`}>
              {FPS_PROCESS_SOURCES.map(({ value, label }) => (
                <ChoiceButton
                  key={value}
                  active={settings.fpsProcessSource === value}
                  color={accent}
                  onClick={() => update({ fpsProcessSource: value })}
                >
                  {label}
                </ChoiceButton>
              ))}
            </div>
          </Section>

          <Section title="Order">
            <p className="-mt-1 text-[11px] text-white/40">Drag to reorder. FPS splash / big text placed between blocks splits the panel.</p>
            <BlockOrderList
              order={settings.blockOrder}
              labels={BLOCK_LABELS}
              isEnabled={(block) => isBlockEnabled(settings, block, gpuCount)}
              accentColor={accent}
              onChange={(blockOrder) => update({ blockOrder })}
            />
          </Section>
        </>
      )}

      {tab === "sensors" && (
        <Section title="Sensors">
          {SENSOR_GROUPS.map((group) => (
            <div key={group.title} className="flex flex-col gap-1.5">
              <h3 className="text-xs font-semibold text-white/70">{group.title}</h3>
              <div className="grid grid-cols-2 gap-x-3 gap-y-1">
                {group.toggles.map(({ key, label, parent }) => {
                  const disabled = parent != null && !settings[parent];
                  return (
                    <label
                      key={key}
                      className={`flex items-center gap-2 text-sm ${disabled ? "text-white/30" : "text-white/80"} ${parent ? "pl-4" : ""}`}
                    >
                      <input
                        type="checkbox"
                        checked={settings[key]}
                        disabled={disabled}
                        onChange={(e) => update({ [key]: e.target.checked })}
                      />
                      {label}
                    </label>
                  );
                })}
              </div>
              {group.title === "CPU" && systemInfo && (
                <p className="text-[11px] text-white/40">{SENSOR_SOURCE_TEXT[systemInfo.cpuSensorSource]}</p>
              )}
            </div>
          ))}
          {gpuGroups.map((title, index) => {
            const slot = gpuSlotAt(settings, index);
            return (
              <div key={title} className="flex flex-col gap-1.5">
                <h3 className="text-xs font-semibold text-white/70">{title}</h3>
                {systemInfo?.gpus[index] && (
                  <p className="text-[11px] text-white/40">{systemInfo.gpus[index].name}</p>
                )}
                <label className="flex items-center gap-2 text-sm text-white/80">
                  <input
                    type="checkbox"
                    checked={slot.enabled}
                    onChange={(e) => {
                      const slots = settings.gpuSlots.slice();
                      while (slots.length <= index) slots.push({ ...DEFAULT_GPU_SLOT });
                      slots[index] = { ...gpuSlotAt({ ...settings, gpuSlots: slots }, index), enabled: e.target.checked };
                      update({ gpuSlots: slots });
                    }}
                  />
                  Show
                </label>
                <div className="grid grid-cols-2 gap-x-3 gap-y-1">
                  {GPU_SLOT_TOGGLES.map(({ key, label, parent }) => {
                    const disabled = !slot.enabled || (parent != null && !slot[parent]);
                    return (
                      <label
                        key={key}
                        className={`flex items-center gap-2 text-sm ${disabled ? "text-white/30" : "text-white/80"} ${parent ? "pl-4" : ""}`}
                      >
                        <input
                          type="checkbox"
                          checked={slot[key]}
                          disabled={disabled}
                          onChange={(e) => {
                            const slots = settings.gpuSlots.slice();
                            while (slots.length <= index) slots.push({ ...DEFAULT_GPU_SLOT });
                            slots[index] = { ...gpuSlotAt({ ...settings, gpuSlots: slots }, index), [key]: e.target.checked };
                            update({ gpuSlots: slots });
                          }}
                        />
                        {label}
                      </label>
                    );
                  })}
                </div>
              </div>
            );
          })}
        </Section>
      )}
    </div>
  );
}
