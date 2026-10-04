import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { DEFAULT_SETTINGS, SETTINGS_EVENT, loadSettings, saveSettings } from "../lib/settings";
import type { HudSettings } from "../types";

/** Current HUD settings, kept in sync across windows. `update` persists and broadcasts changes. */
export function useHudSettings() {
  const [settings, setSettings] = useState<HudSettings>(DEFAULT_SETTINGS);
  const latest = useRef(settings);

  const apply = useCallback((next: HudSettings) => {
    latest.current = next;
    setSettings(next);
  }, []);

  useEffect(() => {
    let active = true;
    loadSettings().then((loaded) => active && apply(loaded));
    const unlisten = listen<HudSettings>(SETTINGS_EVENT, (event) => apply(event.payload));
    return () => {
      active = false;
      unlisten.then((stop) => stop());
    };
  }, [apply]);

  const update = useCallback(
    (patch: Partial<HudSettings> | ((current: HudSettings) => Partial<HudSettings>)) => {
      const changes = typeof patch === "function" ? patch(latest.current) : patch;
      const next = { ...latest.current, ...changes };
      apply(next);
      void saveSettings(next);
    },
    [apply],
  );

  return { settings, update };
}
