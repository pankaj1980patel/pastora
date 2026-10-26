import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { api, Settings } from "./api";

/** Runs `handler` whenever the backend emits `event`. */
export function useEvent<T = unknown>(event: string, handler: (payload: T) => void) {
  const ref = useRef(handler);
  ref.current = handler;
  useEffect(() => {
    const un = listen<T>(event, (e) => ref.current(e.payload));
    return () => {
      un.then((f) => f());
    };
  }, [event]);
}

/** Current settings, kept in sync across windows. */
export function useSettings(): [Settings | null, (s: Settings) => void] {
  const [settings, setSettings] = useState<Settings | null>(null);
  useEffect(() => {
    api.getSettings().then(setSettings);
  }, []);
  useEvent<Settings>("settings-changed", setSettings);
  return [settings, setSettings];
}

/** Applies theme and font size to <html>. */
export function useAppearance(settings: Settings | null, scaleFont: boolean) {
  useEffect(() => {
    if (!settings) return;
    const root = document.documentElement;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const dark = settings.theme === "dark" || (settings.theme === "system" && mq.matches);
      root.classList.toggle("dark", dark);
    };
    apply();
    if (scaleFont) root.style.fontSize = `${settings.fontSize}px`;
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, [settings?.theme, settings?.fontSize, scaleFont]);
}
