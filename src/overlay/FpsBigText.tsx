import { memo, type CSSProperties } from "react";
import { fpsAccent, hudFontFamily, shortProcessName } from "./FpsSplash";

interface FpsBigTextProps {
  fps: number | null;
  process?: string | null;
  primaryColor: string;
  secondaryColor: string;
  /** Face chosen in Look > Text. */
  fontFamily: string;
  /** Font size of the number, in px. */
  size: number;
}

/** Outlined comic lettering with a hard offset shadow, scaled from the font size. */
function comicText(color: string, size: number): CSSProperties {
  const stroke = Math.max(2, size * 0.07);
  const shadow = Math.max(2, size * 0.06);
  return {
    color,
    WebkitTextStroke: `${stroke}px #000`,
    paintOrder: "stroke fill",
    textShadow: `${shadow}px ${shadow}px 0 #000`,
  };
}

/** Large FPS number in the splash lettering style, without the starburst. */
function FpsBigText({ fps, process, primaryColor, secondaryColor, fontFamily, size }: FpsBigTextProps) {
  const value = fps == null ? "N/A" : Math.round(fps).toString();
  const labelSize = size * 0.32;

  return (
    <div className="flex flex-col items-center" style={{ transform: "rotate(-4deg)" }}>
      <div className="flex items-end gap-[0.15em] leading-none" style={{ fontFamily: hudFontFamily(fontFamily) }}>
        <span className="tracking-[0.04em]" style={{ fontSize: size, ...comicText(fpsAccent(fps, primaryColor), size) }}>
          {value}
        </span>
        <span
          className="tracking-[0.12em]"
          style={{ fontSize: labelSize, marginBottom: size * 0.12, ...comicText(secondaryColor, labelSize) }}
        >
          FPS
        </span>
      </div>
      {process && (
        <span
          className="mt-1 text-[13px] text-white"
          style={{ fontFamily: hudFontFamily(fontFamily), WebkitTextStroke: "3px #000", paintOrder: "stroke fill" }}
        >
          {shortProcessName(process)}
        </span>
      )}
    </div>
  );
}

export default memo(FpsBigText);
