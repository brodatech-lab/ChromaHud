import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { SystemInfo } from "../types";

/** Static hardware info; the backend emits `system-info` once sensors are probed, in case the first call is too early. */
export function useSystemInfo(): SystemInfo | null {
  const [info, setInfo] = useState<SystemInfo | null>(null);

  useEffect(() => {
    let active = true;
    invoke<SystemInfo | null>("get_system_info").then((result) => active && result && setInfo(result));
    const unlisten = listen<SystemInfo>("system-info", (event) => setInfo(event.payload));
    return () => {
      active = false;
      unlisten.then((stop) => stop());
    };
  }, []);

  return info;
}
