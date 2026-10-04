import { VALUE_INDENT } from "./StatRow";

interface TempGaugeProps {
  value: number | null | undefined;
  min: number;
  max: number;
}

const COOL = "#22c55e";
const SCALE = "linear-gradient(90deg, #22c55e, #eab308 55%, #ef4444)";

/** Temperature slider whose thumb shows the value and shifts from green to red across the scale. */
export default function TempGauge({ value, min, max }: TempGaugeProps) {
  const hasValue = value != null;
  const ratio = hasValue ? Math.min(1, Math.max(0, (value - min) / (max - min))) : 0;
  const percent = ratio * 100;
  const color = hasValue ? `hsl(${Math.round(120 * (1 - ratio))} 85% 50%)` : "#6b7280";

  return (
    <div className={`${VALUE_INDENT} relative h-[1.3em]`}>
      <div className="absolute inset-x-0 top-1/2 h-[0.4em] -translate-y-1/2 rounded-full opacity-25" style={{ background: SCALE }} />
      <div
        className="absolute left-0 top-1/2 h-[0.4em] -translate-y-1/2 rounded-full transition-[width] duration-300"
        style={{ width: `${percent}%`, background: `linear-gradient(90deg, ${COOL}, ${color})` }}
      />
      {/* Translating by the same percentage keeps the thumb inside the track at both ends. */}
      <div
        className="absolute top-1/2 rounded-full border border-black/40 px-[0.5em] text-[0.7em] font-bold leading-[1.5] tabular-nums text-black shadow transition-[left,background-color] duration-300"
        style={{ left: `${percent}%`, transform: `translate(-${percent}%, -50%)`, backgroundColor: color }}
      >
        {hasValue ? `${Math.round(value)}°` : "N/A"}
      </div>
    </div>
  );
}
