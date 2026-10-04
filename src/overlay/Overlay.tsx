import { Children, Fragment, useEffect, type CSSProperties, type ReactNode } from "react";
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
import type { HudBlock } from "../types";

const ADMIN_HINT = "Run as admin for FPS";

/** Gauge scales in °C: CPU up to Tjmax of current Ryzen parts, GPU up to typical throttle point. */
const CPU_TEMP_RANGE = { min: 30, max: 95 };
const GPU_TEMP_RANGE = { min: 30, max: 90 };

function ModelCaption({ name }: { name: string }) {
  return <div className="-mb-1 truncate text-[0.65em] uppercase tracking-widest text-white/40">{name}</div>;
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
  const gpu = metrics?.gpu ?? null;
  const fpsLabel = metrics && !metrics.fpsCapturing ? ADMIN_HINT : metrics?.fpsProcess;

  useEffect(() => {
    const unlisten = listen("toggle-cpu-cores", () => update((current) => ({ showCpuCores: !current.showCpuCores })));
    return () => {
      unlisten.then((stop) => stop());
    };
  }, [update]);

  useEffect(() => {
    void invoke("set_cpu_cores_checked", { checked: settings.showCpuCores });
  }, [settings.showCpuCores]);

  // Translating by the same percentage as the offset keeps the HUD fully on screen at 0% and 100%.
  const style = {
    left: `${settings.posX}%`,
    top: `${settings.posY}%`,
    transform: `translate(-${settings.posX}%, -${settings.posY}%)`,
    opacity: settings.opacity,
    fontFamily: `"${settings.fontFamily}", system-ui, sans-serif`,
    fontSize: `${settings.fontSize}px`,
    "--hud-primary": settings.primaryColor,
    "--hud-secondary": settings.secondaryColor,
  } as CSSProperties;

  const panelStyle: CSSProperties = {
    borderColor: "var(--hud-primary)",
    background: `color-mix(in srgb, ${settings.panelColor} ${Math.round(settings.panelOpacity * 100)}%, transparent)`,
  };

  const cpuDetail = metrics
    ? joinDetail([
        settings.showCpuClock && `${(metrics.cpuClockMhz / 1000).toFixed(2)} GHz`,
        settings.showCpuTemp && orDash(metrics.cpuTempC, 0, "°C"),
        settings.showCpuPower && orDash(metrics.cpuPowerW, 0, " W"),
      ])
    : undefined;

  const gpuDetail = gpu
    ? joinDetail([
        settings.showGpuClock && orDash(gpu.coreClockMhz, 0, " MHz"),
        settings.showGpuTemp && orDash(gpu.tempC, 0, "°C"),
        settings.showGpuPower && orDash(gpu.powerW, 0, " W"),
      ])
    : undefined;

  const latencyDetail = metrics
    ? joinDetail([
        metrics.bound && `${metrics.bound.toUpperCase()}-bound`,
        metrics.gpuBusyMs != null && `busy ${metrics.gpuBusyMs.toFixed(1)} ms`,
      ])
    : undefined;

  const gpuPowerPercent = gpu ? percentOf(gpu.powerW, gpu.powerLimitW) : null;
  const gpuLimitValue = gpu
    ? (joinDetail([
        gpu.pstate != null && `P${gpu.pstate}`,
        gpuPowerPercent != null && `${Math.round(gpuPowerPercent)}% TDP`,
      ]) ?? "--")
    : "--";

  const cpuModel = systemInfo?.cpuModel ? shortCpuName(systemInfo.cpuModel) : null;
  const gpuModel = gpu?.name ?? systemInfo?.gpuModel ?? null;
  const fps = metrics?.fps ?? null;

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
          settings.showCpuCores && metrics && metrics.cores.length > 0 && (
            <CpuCores cores={metrics.cores} ccdTemps={settings.showCpuCcdTemp ? metrics.ccdTempsC : []} />
          ),
        );

      case "gpu":
        if (!gpu) return null;
        return tableBlock(
          settings.showGpuModel && gpuModel && <ModelCaption name={gpuModel} />,
          settings.showGpu && <StatRow label="GPU" value={orDash(gpu.usage, 0, "%")} detail={gpuDetail} percent={gpu.usage} />,
          settings.showGpu && (
            <StatRow
              label="VRAM"
              value={`${formatGiB(gpu.vramUsedBytes)} / ${formatGiB(gpu.vramTotalBytes, 0)} GB`}
              detail={settings.showGpuMemClock ? orDash(gpu.memClockMhz, 0, " MHz") : undefined}
              percent={percentOf(gpu.vramUsedBytes, gpu.vramTotalBytes)}
            />
          ),
          settings.showGpuFan && (
            <StatRow
              label="FAN"
              value={orDash(gpu.fanPercent, 0, "%")}
              detail={gpu.fanRpm != null ? `${gpu.fanRpm} RPM` : undefined}
              percent={gpu.fanPercent}
            />
          ),
          settings.showGpuLimit && (
            <StatRow
              label="LIMIT"
              value={gpuLimitValue}
              detail={gpu.throttleReason ?? undefined}
              detailAccent={gpu.throttleReason != null}
              percent={gpuPowerPercent}
            />
          ),
          settings.showGpuGauge && <TempGauge value={gpu.tempC} {...GPU_TEMP_RANGE} />,
        );

      case "ram":
        return tableBlock(
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

  // Consecutive table blocks share one panel; a standalone FPS block in between splits it.
  const groups: BlockGroup[] = [];
  for (const block of settings.blockOrder) {
    const rendered = renderBlock(block);
    if (!rendered) continue;
    const node = <Fragment key={block}>{rendered.node}</Fragment>;
    const last = groups[groups.length - 1];
    if (!rendered.standalone && last && !last.standalone) {
      last.nodes.push(node);
    } else {
      groups.push({ key: block, standalone: rendered.standalone, nodes: [node] });
    }
  }

  return (
    <div
      className={`pointer-events-none absolute flex flex-col gap-3 p-5 ${settings.posX > 50 ? "items-end" : "items-start"}`}
      style={style}
    >
      {groups.map((group) =>
        group.standalone ? (
          <Fragment key={group.key}>{group.nodes}</Fragment>
        ) : (
          <div
            key={group.key}
            className="flex min-w-[17em] flex-col gap-2 rounded-lg border-l-4 px-4 py-3 shadow-lg"
            style={panelStyle}
          >
            {group.nodes}
          </div>
        ),
      )}
    </div>
  );
}
