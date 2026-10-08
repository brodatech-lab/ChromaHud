/** Left margin that lines content up with the value column (past the label). */
export const VALUE_INDENT = "ml-[calc(3.4em*0.7+0.75rem)]";

interface StatRowProps {
  label: string;
  value: string;
  detail?: string;
  /** Draws the detail in the secondary color to flag a warning state. */
  detailAccent?: boolean;
  /** 0-100; renders a thin usage bar when provided. */
  percent?: number | null;
}

export default function StatRow({ label, value, detail, detailAccent, percent }: StatRowProps) {
  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-baseline gap-3">
        <span
          className="w-[3.4em] shrink-0 text-[0.7em] font-semibold uppercase tracking-[0.18em]"
          style={{ color: "var(--hud-secondary)" }}
        >
          {label}
        </span>
        <span className="font-semibold tabular-nums" style={{ color: "var(--hud-value)" }}>
          {value}
        </span>
        {detail && (
          <span
            className={`text-[0.8em] tabular-nums ${detailAccent ? "font-semibold" : ""}`}
            style={{
              color: detailAccent ? "var(--hud-secondary)" : "color-mix(in srgb, var(--hud-value) 60%, transparent)",
            }}
          >
            {detail}
          </span>
        )}
      </div>
      {percent != null && (
        <div className={`${VALUE_INDENT} h-[3px] overflow-hidden rounded-full bg-white/10`}>
          <div
            className="h-full rounded-full transition-[width] duration-300"
            style={{
              width: `${Math.min(100, Math.max(0, percent))}%`,
              background: "var(--hud-primary)",
            }}
          />
        </div>
      )}
    </div>
  );
}
