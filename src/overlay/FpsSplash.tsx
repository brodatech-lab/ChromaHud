import { memo, useId } from "react";

interface FpsSplashProps {
  fps: number | null;
  process?: string | null;
  primaryColor: string;
  secondaryColor: string;
  /** Face chosen in Look > Text. */
  fontFamily: string;
  size?: number;
}

/** CSS font stack for the HUD face, including names that contain spaces. */
export function hudFontFamily(family: string): string {
  return `"${family}", system-ui, sans-serif`;
}

const VIEW = 200;
const CENTER = VIEW / 2;

/** Deterministic pseudo-random value in [0, 1) so the burst shape is stable between renders. */
function noise(seed: number): number {
  const x = Math.sin(seed * 12.9898) * 43758.5453;
  return x - Math.floor(x);
}

function burstPoints(spikes: number, outer: number, inner: number, jitter: number, seed: number): string {
  const points: string[] = [];
  const total = spikes * 2;
  for (let i = 0; i < total; i++) {
    const isSpike = i % 2 === 0;
    const base = isSpike ? outer : inner;
    const radius = base * (1 - jitter + noise(seed + i) * jitter * 2);
    const angle = (i / total) * Math.PI * 2 - Math.PI / 2 + (noise(seed * 3 + i) - 0.5) * 0.12;
    points.push(`${(CENTER + Math.cos(angle) * radius).toFixed(1)},${(CENTER + Math.sin(angle) * radius).toFixed(1)}`);
  }
  return points.join(" ");
}

const OUTER_BURST = burstPoints(14, 96, 66, 0.12, 7);
const INNER_BURST = burstPoints(12, 74, 56, 0.08, 19);

/** FPS color thresholds shared by every FPS style. */
export function fpsAccent(fps: number | null, primary: string): string {
  if (fps == null) return "#6b7280";
  if (fps < 30) return "#ff3b30";
  if (fps < 60) return "#ffc400";
  return primary;
}

export function shortProcessName(process: string): string {
  return process.length > 24 ? `${process.slice(0, 22)}…` : process;
}

/** Comic-book "POW!" style starburst holding the FPS value. Pure inline SVG, no images. */
function FpsSplash({ fps, process, primaryColor, secondaryColor, fontFamily, size = 150 }: FpsSplashProps) {
  const id = useId().replace(/:/g, "");
  const accent = fpsAccent(fps, primaryColor);
  const value = fps == null ? "N/A" : Math.round(fps).toString();
  const valueSize = fps == null ? 46 : value.length >= 3 ? 58 : 72;

  return (
    <svg width={size} height={size} viewBox={`0 0 ${VIEW} ${VIEW}`} className="overflow-visible drop-shadow-lg">
      <defs>
        <pattern id={`halftone-${id}`} width="9" height="9" patternUnits="userSpaceOnUse" patternTransform="rotate(30)">
          <circle cx="4.5" cy="4.5" r="2" fill={secondaryColor} opacity="0.55" />
        </pattern>
      </defs>

      {/* Hard offset shadow, typical for comic panels. */}
      <polygon points={OUTER_BURST} transform="translate(6 7)" fill="#000" opacity="0.85" />

      <polygon points={OUTER_BURST} fill={secondaryColor} stroke="#000" strokeWidth="5" strokeLinejoin="round" />
      <polygon points={INNER_BURST} fill={accent} stroke="#000" strokeWidth="4" strokeLinejoin="round" />
      <polygon points={INNER_BURST} fill={`url(#halftone-${id})`} />

      <text
        x={CENTER}
        y={CENTER + 14}
        textAnchor="middle"
        fontFamily={hudFontFamily(fontFamily)}
        fontSize={valueSize}
        fill="#fff"
        stroke="#000"
        strokeWidth="8"
        strokeLinejoin="round"
        paintOrder="stroke"
        letterSpacing="2"
        transform={`rotate(-6 ${CENTER} ${CENTER})`}
      >
        {value}
      </text>

      <g transform={`rotate(-6 ${CENTER} ${CENTER})`}>
        <rect x={CENTER - 26} y={CENTER + 24} width="52" height="22" rx="3" fill="#000" />
        <text
          x={CENTER}
          y={CENTER + 41}
          textAnchor="middle"
          fontFamily={hudFontFamily(fontFamily)}
          fontSize="20"
          letterSpacing="3"
          fill={secondaryColor}
        >
          FPS
        </text>
      </g>

      {process && (
        <text
          x={CENTER}
          y={VIEW + 14}
          textAnchor="middle"
          fontFamily={hudFontFamily(fontFamily)}
          fontSize="13"
          fill="#fff"
          stroke="#000"
          strokeWidth="3"
          paintOrder="stroke"
        >
          {shortProcessName(process)}
        </text>
      )}
    </svg>
  );
}

export default memo(FpsSplash);
