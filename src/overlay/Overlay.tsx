import {
  Children,
  Fragment,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import CpuCores from "./CpuCores";
import FpsBigText from "./FpsBigText";
import FpsSplash from "./FpsSplash";
import StatRow from "./StatRow";
import TempGauge from "./TempGauge";
import { useMetrics } from "../hooks/useMetrics";
import { useHudSettings } from "../hooks/useHudSettings";
import { useSystemInfo } from "../hooks/useSystemInfo";
import { formatGiB, formatMBps, joinDetail, orDash, percentOf, shortCpuName } from "../lib/format";
import { gpuSlotAt } from "../lib/settings";
import type { GpuMetrics, GpuSlotSettings, HudBlock } from "../types";

const ADMIN_HINT = "Run as admin for FPS";
/** Keep the horizontal HUD this far from every screen edge. */
const SAFE_MARGIN_PX = 10;

/** Gauge scales in °C: CPU up to Tjmax of current Ryzen parts, GPU up to typical throttle point. */
const CPU_TEMP_RANGE = { min: 30, max: 95 };
const GPU_TEMP_RANGE = { min: 30, max: 90 };

function ModelCaption({ name }: { name: string }) {
  return (
    <div className="-mb-1 truncate text-[0.65em] uppercase tracking-widest" style={{ color: "var(--hud-model)" }}>
      {name}
    </div>
  );
}

/** "2.9 / 24 GB"; integrated GPUs have small carve-outs (0.5 GB) and Windows counters give no total. */
function vramValue(used: number | null, total: number | null): string {
  if (total == null) return `${formatGiB(used)} GB`;
  const totalDigits = total < 2 * 1024 ** 3 ? 1 : 0;
  return `${formatGiB(used)} / ${formatGiB(total, totalDigits)} GB`;
}

interface RenderedBlock {
  /** Standalone blocks (FPS splash / big text) sit between panels instead of inside one. */
  standalone: boolean;
  node: ReactNode;
}

/** A table block made of the given rows, or null when every row is disabled. */
function tableBlock(...rows: ReactNode[]): RenderedBlock | null {
  const visible = Children.toArray(rows);
  return visible.length ? { standalone: false, node: visible } : null;
}

interface BlockGroup {
  key: string;
  standalone: boolean;
  nodes: ReactNode[];
}

export default function Overlay() {
  const metrics = useMetrics();
  const systemInfo = useSystemInfo();
  const { settings, update } = useHudSettings();
  const gpus = metrics?.gpus ?? [];
  const horizontal = settings.overlayLayout === "horizontal";
  const wrap = horizontal && settings.horizontalWrap;
  const contentRef = useRef<HTMLDivElement>(null);
  const fitBurst = useRef(0);
  const [viewport, setViewport] = useState(() => ({
    w: typeof window !== "undefined" ? Math.max(0, window.innerWidth - SAFE_MARGIN_PX * 2) : 0,
    h: typeof window !== "undefined" ? Math.max(0, window.innerHeight - SAFE_MARGIN_PX * 2) : 0,
  }));
  // Unscaled content size. Never written back onto the measured element.
  const [natural, setNatural] = useState({ w: 0, h: 0 });
  const processName =
    settings.fpsProcessSource === "game" ? (metrics?.fpsGame ?? metrics?.fpsProcess) : metrics?.fpsProcess;
  const fpsLabel =
    metrics && !metrics.fpsCapturing
      ? ADMIN_HINT
      : !settings.showFpsProcess
        ? null
        : metrics?.fpsPaused && processName
          ? `${processName} · paused`
          : processName;

  useEffect(() => {
    const unlisten = listen("toggle-cpu-cores", () => update((current) => ({ showCpuCores: !current.showCpuCores })));
    return () => {
      unlisten.then((stop) => stop());
    };
  }, [update]);

  useEffect(() => {
    void invoke("set_cpu_cores_checked", { checked: settings.showCpuCores });
  }, [settings.showCpuCores]);

  useEffect(() => {
    void invoke("set_keep_game", { enabled: settings.keepGameTracked });
  }, [settings.keepGameTracked]);

  useEffect(() => {
    if (!horizontal) return;
    const update = () => {
      const next = {
        w: Math.max(0, window.innerWidth - SAFE_MARGIN_PX * 2),
        h: Math.max(0, window.innerHeight - SAFE_MARGIN_PX * 2),
      };
      setViewport((prev) => (prev.w === next.w && prev.h === next.h ? prev : next));
    };
    update();
    window.addEventListener("resize", update);
    return () => window.removeEventListener("resize", update);
  }, [horizontal]);

  // Translating by the same percentage as the offset keeps the HUD fully on screen at 0% and 100%.
  const style = {
    opacity: settings.opacity,
    fontFamily: `"${settings.fontFamily}", system-ui, sans-serif`,
    fontSize: `${settings.fontSize}px`,
    gap: horizontal ? `${settings.blockGap}px` : "0.75em",
    "--hud-primary": settings.primaryColor,
    "--hud-secondary": settings.secondaryColor,
    "--hud-model": settings.modelColor,
    "--hud-value": settings.valueColor,
  } as CSSProperties;
  if (!horizontal) {
    style.left = `${settings.posX}%`;
    style.top = `${settings.posY}%`;
    style.transform = `translate(-${settings.posX}%, -${settings.posY}%)`;
  }

  const panelStyle: CSSProperties = {
    borderColor: "var(--hud-primary)",
    background: `color-mix(in srgb, ${settings.panelColor} ${Math.round(settings.panelOpacity * 100)}%, transparent)`,
  };

  const cpuDetail = metrics
    ? joinDetail([
        settings.showCpuClock && `${(metrics.cpuClockMhz / 1000).toFixed(2)} GHz`,
        settings.showCpuTemp && metrics.cpuTempC != null && `${orDash(metrics.cpuTempC, 0, "°C")}`,
        settings.showCpuPower && metrics.cpuPowerW != null && `${orDash(metrics.cpuPowerW, 0, " W")}`,
      ])
    : undefined;

  const latencyDetail = metrics
    ? joinDetail([
        metrics.bound && `${metrics.bound.toUpperCase()}-bound`,
        metrics.gpuBusyMs != null && `busy ${metrics.gpuBusyMs.toFixed(1)} ms`,
      ])
    : undefined;

  const cpuModel = systemInfo?.cpuModel ? shortCpuName(systemInfo.cpuModel) : null;
  const fps = metrics?.fps ?? null;
  const numbered = gpus.filter((_, index) => gpuSlotAt(settings, index).enabled).length > 1;

  const renderGpu = (gpu: GpuMetrics, slot: GpuSlotSettings, index: number): RenderedBlock | null => {
    if (!slot.enabled) return null;
    const label = numbered ? `GPU ${index + 1}` : "GPU";
    const detail = joinDetail([
      slot.showClock && gpu.coreClockMhz != null && `${gpu.coreClockMhz} MHz`,
      slot.showTemp && gpu.tempC != null && `${gpu.tempC}°C`,
      slot.showPower && gpu.powerW != null && `${Math.round(gpu.powerW)} W`,
    ]);
    const gpuPowerPercent = percentOf(gpu.powerW, gpu.powerLimitW);
    const gpuLimitValue =
      joinDetail([gpu.pstate != null && `P${gpu.pstate}`, gpuPowerPercent != null && `${Math.round(gpuPowerPercent)}% TDP`]) ??
      "--";
    const gpuLimitAvailable = gpu.pstate != null || gpu.powerLimitW != null || gpu.throttleReason != null;
    return tableBlock(
      slot.showModel && gpu.name && <ModelCaption name={gpu.name} />,
      slot.showUsage && <StatRow label={label} value={orDash(gpu.usage, 0, "%")} detail={detail} percent={gpu.usage} />,
      slot.showFan && (gpu.fanPercent != null || gpu.fanRpm != null) && (
        <StatRow
          label="FAN"
          value={gpu.fanPercent != null ? `${gpu.fanPercent}%` : `${gpu.fanRpm} RPM`}
          detail={gpu.fanPercent != null && gpu.fanRpm != null ? `${gpu.fanRpm} RPM` : undefined}
          percent={gpu.fanPercent}
        />
      ),
      slot.showLimit && gpuLimitAvailable && (
        <StatRow
          label="LIMIT"
          value={gpuLimitValue}
          detail={gpu.throttleReason ?? undefined}
          detailAccent={gpu.throttleReason != null}
          percent={gpuPowerPercent}
        />
      ),
      slot.showGauge && <TempGauge value={gpu.tempC} {...GPU_TEMP_RANGE} />,
    );
  };

  const renderVram = (gpu: GpuMetrics, slot: GpuSlotSettings, index: number): RenderedBlock | null => {
    if (!slot.enabled || !slot.showVram) return null;
    if (gpu.vramUsedBytes == null && gpu.vramTotalBytes == null) return null;
    return tableBlock(
      <StatRow
        label={numbered ? `VRAM ${index + 1}` : "VRAM"}
        value={vramValue(gpu.vramUsedBytes, gpu.vramTotalBytes)}
        detail={slot.showMemClock && gpu.memClockMhz != null ? `${gpu.memClockMhz} MHz` : undefined}
        percent={percentOf(gpu.vramUsedBytes, gpu.vramTotalBytes)}
      />,
    );
  };

  const renderBlock = (block: HudBlock): RenderedBlock | null => {
    switch (block) {
      case "fps":
        if (settings.fpsStyle === "splash") {
          return {
            standalone: true,
            node: (
              // Bottom padding leaves room for the process name drawn below the SVG box.
              <div className="pb-4">
                <FpsSplash
                  fps={fps}
                  process={fpsLabel}
                  primaryColor={settings.primaryColor}
                  secondaryColor={settings.secondaryColor}
                  fontFamily={settings.fontFamily}
                  size={settings.fpsSplashSize}
                />
              </div>
            ),
          };
        }
        if (settings.fpsStyle === "big") {
          return {
            standalone: true,
            node: (
              <FpsBigText
                fps={fps}
                process={fpsLabel}
                primaryColor={settings.primaryColor}
                secondaryColor={settings.secondaryColor}
                fontFamily={settings.fontFamily}
                size={settings.fpsBigSize}
              />
            ),
          };
        }
        return tableBlock(<StatRow label="FPS" value={orDash(fps, 0)} detail={fpsLabel ?? undefined} />);

      case "latency":
        return tableBlock(
          settings.showLatency && (
            <StatRow label="LAT" value={orDash(metrics?.displayLatencyMs, 1, " ms")} detail={latencyDetail} />
          ),
        );

      case "cpu":
        return tableBlock(
          settings.showCpuModel && cpuModel && <ModelCaption name={cpuModel} />,
          settings.showCpu && (
            <StatRow label="CPU" value={orDash(metrics?.cpuUsage, 0, "%")} detail={cpuDetail} percent={metrics?.cpuUsage} />
          ),
          settings.showCpuGauge && <TempGauge value={metrics?.cpuTempC} {...CPU_TEMP_RANGE} />,
          settings.showCpuCores && !horizontal && metrics && metrics.cores.length > 0 && (
            <CpuCores cores={metrics.cores} ccdTemps={settings.showCpuCcdTemp ? metrics.ccdTempsC : []} />
          ),
        );

      case "gpu1":
      case "gpu2":
      case "vram1":
      case "vram2":
        return null;

      case "ram":
        return tableBlock(
          settings.showRamManufacturer && systemInfo?.ramManufacturer && (
            <div className="-mb-1 break-words text-[0.65em] tracking-widest" style={{ color: "var(--hud-model)" }}>
              {systemInfo.ramManufacturer}
            </div>
          ),
          settings.showRam && (
            <StatRow
              label="RAM"
              value={`${formatGiB(metrics?.ramUsedBytes)} / ${formatGiB(metrics?.ramTotalBytes, 0)} GB`}
              detail={(settings.showRamSpeed && systemInfo?.ramSpeed) || undefined}
              percent={percentOf(metrics?.ramUsedBytes, metrics?.ramTotalBytes)}
            />
          ),
        );

      case "disk":
        return tableBlock(
          settings.showDisk && (
            <StatRow
              label="DISK"
              value={`R ${formatMBps(metrics?.diskReadBps)} · W ${formatMBps(metrics?.diskWriteBps)}`}
              detail="MB/s"
            />
          ),
        );

      case "display":
        return tableBlock(
          settings.showDisplay && (
            <StatRow
              label="RES"
              value={metrics?.screenWidth ? `${metrics.screenWidth}×${metrics.screenHeight}` : "--"}
              detail={metrics?.refreshHz ? `@ ${metrics.refreshHz} Hz` : undefined}
            />
          ),
        );
    }
  };

  // Vertical: consecutive table blocks share one panel. Horizontal: each block is its own panel.
  const groups: BlockGroup[] = [];
  const pushRendered = (key: string, rendered: RenderedBlock) => {
    const node = <Fragment key={key}>{rendered.node}</Fragment>;
    const last = groups[groups.length - 1];
    if (!horizontal && !rendered.standalone && last && !last.standalone) {
      last.nodes.push(node);
    } else {
      groups.push({ key, standalone: rendered.standalone, nodes: [node] });
    }
  };
  for (const block of settings.blockOrder) {
    const gpuIndex = block === "gpu1" || block === "vram1" ? 0 : block === "gpu2" || block === "vram2" ? 1 : null;
    if (gpuIndex != null) {
      const gpu = gpus[gpuIndex];
      if (!gpu) continue;
      const slot = gpuSlotAt(settings, gpuIndex);
      const rendered = block.startsWith("vram") ? renderVram(gpu, slot, gpuIndex) : renderGpu(gpu, slot, gpuIndex);
      if (rendered) pushRendered(block, rendered);
      continue;
    }
    const rendered = renderBlock(block);
    if (rendered) pushRendered(block, rendered);
  }

  const groupKey = groups.map((group) => group.key).join("|");

  // Scale lives on a parent of the measured row. Writing it back onto the row
  // (or sizing that row from its own offsetWidth) retriggers ResizeObserver forever
  // and can hang the desktop compositor.
  useLayoutEffect(() => {
    if (!horizontal) {
      setNatural((prev) => (prev.w === 0 && prev.h === 0 ? prev : { w: 0, h: 0 }));
      return;
    }
    const el = contentRef.current;
    if (!el) return;

    fitBurst.current = 0;
    let observer: ResizeObserver | null = null;
    const measure = () => {
      const w = Math.max(el.offsetWidth, el.scrollWidth);
      const h = Math.max(el.offsetHeight, el.scrollHeight);
      setNatural((prev) => {
        if (Math.abs(prev.w - w) <= 1 && Math.abs(prev.h - h) <= 1) return prev;
        if (fitBurst.current >= 8) {
          observer?.disconnect();
          return prev;
        }
        fitBurst.current += 1;
        return { w, h };
      });
    };

    measure();
    observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer?.disconnect();
  }, [horizontal, wrap, viewport.w, settings.fontSize, settings.blockGap, groupKey]);

  const scale =
    horizontal && natural.w > 1 && natural.h > 1 && viewport.w > 0 && viewport.h > 0
      ? Math.min(1, viewport.w / natural.w, viewport.h / natural.h)
      : 1;

  const align = horizontal
    ? settings.posY > 50
      ? "items-end"
      : "items-start"
    : settings.posX > 50
      ? "items-end"
      : "items-start";
  const justify = horizontal
    ? settings.posX > 50
      ? "justify-end"
      : settings.posX < 50
        ? "justify-start"
        : "justify-center"
    : "";

  const contentStyle: CSSProperties = horizontal
    ? { ...style, width: wrap && viewport.w > 0 ? viewport.w : "max-content" }
    : style;

  const hud = (
    <div
      ref={horizontal ? contentRef : undefined}
      className={`pointer-events-none flex p-[1.25em] ${horizontal ? "flex-row" : "absolute flex-col"} ${wrap ? "flex-wrap" : "flex-nowrap"} ${align} ${justify}`}
      style={contentStyle}
    >
      {groups.map((group) =>
        group.standalone ? (
          <div key={group.key} className="shrink-0">
            {group.nodes}
          </div>
        ) : (
          <div
            key={group.key}
            className="flex min-w-[17em] shrink-0 flex-col gap-[0.5em] rounded-lg border-l-4 px-[1em] py-[0.75em] shadow-lg"
            style={panelStyle}
          >
            {group.nodes}
          </div>
        ),
      )}
    </div>
  );

  if (!horizontal) {
    return hud;
  }

  return (
    <div className="pointer-events-none absolute overflow-hidden" style={{ inset: SAFE_MARGIN_PX }}>
      <div
        className="absolute"
        style={{
          left: `${settings.posX}%`,
          top: `${settings.posY}%`,
          transform: `translate(-${settings.posX}%, -${settings.posY}%) scale(${scale})`,
          transformOrigin: `${settings.posX}% ${settings.posY}%`,
        }}
      >
        {hud}
      </div>
    </div>
  );
}
