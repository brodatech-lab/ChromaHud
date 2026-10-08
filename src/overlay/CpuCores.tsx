import { VALUE_INDENT } from "./StatRow";
import type { CoreMetrics } from "../types";

interface CpuCoresProps {
  cores: CoreMetrics[];
  /** Shown as a header line when non-empty. */
  ccdTemps: (number | null)[];
}

/** Compact two-column per-core view: usage with a thin bar and the active clock. */
export default function CpuCores({ cores, ccdTemps }: CpuCoresProps) {
  const fastest = cores.reduce<number | null>(
    (best, core) => (core.clockMhz != null && (best == null || core.clockMhz > best) ? core.clockMhz : best),
    null,
  );

  return (
    <div className={`${VALUE_INDENT} flex flex-col gap-1 text-[0.75em]`}>
      {ccdTemps.length > 0 && (
        <div className="flex gap-4 tabular-nums">
          {ccdTemps.map((temp, i) => (
            <span key={i}>
              <span className="font-semibold" style={{ color: "var(--hud-secondary)" }}>
                CCD{i + 1}
              </span>{" "}
              <span style={{ color: "var(--hud-value)" }}>{temp != null ? `${Math.round(temp)}°C` : "--"}</span>
            </span>
          ))}
        </div>
      )}
      <div className="grid grid-cols-2 gap-x-4 gap-y-1">
        {cores.map((core) => (
          <div key={core.index} className="flex flex-col gap-0.5">
            <div className="flex items-baseline gap-2 tabular-nums">
              <span
                className="w-[2.2em] font-semibold"
                style={{ color: core.clockMhz != null && core.clockMhz === fastest ? "var(--hud-primary)" : "var(--hud-secondary)" }}
              >
                C{core.index}
              </span>
              <span className="w-[2.6em]" style={{ color: "var(--hud-value)" }}>
                {Math.round(core.usage)}%
              </span>
              <span style={{ color: "color-mix(in srgb, var(--hud-value) 60%, transparent)" }}>
                {core.clockMhz != null ? `${(core.clockMhz / 1000).toFixed(2)} GHz` : "--"}
              </span>
            </div>
            <div className="h-[2px] overflow-hidden rounded-full bg-white/10">
              <div
                className="h-full rounded-full transition-[width] duration-300"
                style={{ width: `${Math.min(100, Math.max(0, core.usage))}%`, background: "var(--hud-primary)" }}
              />
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
