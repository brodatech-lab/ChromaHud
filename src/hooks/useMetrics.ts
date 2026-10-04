import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { Metrics } from "../types";

export function useMetrics(): Metrics | null {
  const [metrics, setMetrics] = useState<Metrics | null>(null);

  useEffect(() => {
    const unlisten = listen<Metrics>("metrics", (event) => setMetrics(event.payload));
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  return metrics;
}
