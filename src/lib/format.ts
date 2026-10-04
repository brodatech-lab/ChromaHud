const GIB = 1024 ** 3;

export function formatGiB(bytes: number | null | undefined, digits = 1): string {
  return bytes == null ? "--" : (bytes / GIB).toFixed(digits);
}

export function percentOf(used: number | null | undefined, total: number | null | undefined): number | null {
  return used == null || !total ? null : (used / total) * 100;
}

/** "AMD Ryzen 7 9800X3D 8-Core Processor" -> "AMD Ryzen 7 9800X3D". */
export function shortCpuName(name: string): string {
  return name
    .replace(/\((R|TM)\)/gi, "")
    .replace(/\s+\d+-Core Processor$/i, "")
    .replace(/\s+(CPU|Processor)$/i, "")
    .replace(/\s+@.*$/, "")
    .replace(/\s+/g, " ")
    .trim();
}

/** Joins the enabled detail parts with a middle dot; returns undefined when nothing is enabled. */
export function joinDetail(parts: (string | false | null | undefined)[]): string | undefined {
  const visible = parts.filter((part): part is string => Boolean(part));
  return visible.length ? visible.join(" · ") : undefined;
}

/** Bytes per second as decimal megabytes, one decimal below 10 MB/s. */
export function formatMBps(bytesPerSecond: number | null | undefined): string {
  if (bytesPerSecond == null) return "--";
  const mb = bytesPerSecond / 1e6;
  return mb.toFixed(mb < 10 ? 1 : 0);
}

export function orDash(value: number | null | undefined, digits = 0, suffix = ""): string {
  return value == null ? "--" : `${value.toFixed(digits)}${suffix}`;
}
