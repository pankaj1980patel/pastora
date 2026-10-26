import { useEffect, useRef, useState } from "react";
import { api, Clip, Settings } from "../lib/api";
import { useEvent } from "../lib/hooks";
import { isMac, isMod, keys as keyText } from "../lib/shortcut";
import { HistoryList } from "./HistoryList";
import { Icon } from "./Icon";
import { Onboarding } from "./Onboarding";
import { PinList } from "./PinList";
import { IconButton, Kbd, ToastMsg, Toasts } from "./ui";
import { SecurePanel } from "./vault/SecurePanel";

type Tab = "history" | "pins" | "secure";
const TABS: { id: Tab; label: string; icon: string }[] = [
  { id: "history", label: "History", icon: "clock" },
  { id: "pins", label: "Pins", icon: "pin" },
  { id: "secure", label: "Secure", icon: "shield" },
];

export function Popup({ settings }: { settings: Settings }) {
  const [tab, setTab] = useState<Tab>("history");
  const [query, setQuery] = useState("");
  const [toasts, setToasts] = useState<ToastMsg[]>([]);
  const [shownAt, setShownAt] = useState(Date.now());
  const [pending, setPending] = useState<Clip | null>(null);
  const [vaultLocked, setVaultLocked] = useState(true);
  const modal = useRef(false);
  const keys = useRef<((e: KeyboardEvent) => boolean) | null>(null);
  const search = useRef<HTMLInputElement>(null);
  const paused = settings.pausedUntil != null && settings.pausedUntil > Date.now();
  const searchHidden = tab === "secure" && vaultLocked;

  const toast = (message: string, seconds?: number) => {
    const id = Date.now() + Math.random();
    setToasts((t) => [...t.slice(-2), { id, message, seconds }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), seconds ? Math.min(seconds, 4) * 1000 : 1800);
  };

  useEvent<{ message: string }>("toast", (p) => toast(p.message));
  useEvent("clipboard-cleared", () => toast("Clipboard cleared"));
  useEvent<{ tab: Tab | null }>("popup-shown", (p) => {
    if (p.tab) setTab(p.tab);
    setQuery("");
    setShownAt(Date.now());
    requestAnimationFrame(() => search.current?.focus());
  });

  const switchTab = (t: Tab) => {
    setTab(t);
    setQuery("");
    keys.current = null;
    requestAnimationFrame(() => search.current?.focus());
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (modal.current) return; // editors handle their own keys
      const input = document.activeElement as HTMLInputElement | null;
      if (e.key === "Escape") {
        e.preventDefault();
        if (keys.current?.(e)) return; // e.g. close a detail view
        if (query) setQuery("");
        else api.hidePopup();
        return;
      }
      if (e.key === "Tab") {
        e.preventDefault();
        const i = TABS.findIndex((t) => t.id === tab);
        switchTab(TABS[(i + (e.shiftKey ? TABS.length - 1 : 1)) % TABS.length].id);
        return;
      }
      if (isMod(e) && e.key === ",") {
        e.preventDefault();
        api.openSettings();
        return;
      }
      // Leave copy/select-all to the search box when it has a text selection.
      if (isMod(e) && ["c", "a", "x", "v"].includes(e.key.toLowerCase()) && input?.tagName === "INPUT" && input.selectionStart !== input.selectionEnd) return;
      if (input?.type === "password") return;
      if (keys.current?.(e)) e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [tab, query]);

  const registerKeys = (h: (e: KeyboardEvent) => boolean) => {
    keys.current = h;
  };
  const setModal = (open: boolean) => {
    modal.current = open;
  };

  return (
    <div
      className={`relative flex h-screen flex-col overflow-hidden rounded-[14px] border border-black/10 text-neutral-900 dark:border-white/10 dark:text-neutral-100 ${
        // Only macOS draws the frosted blur behind the window; elsewhere use a solid background.
        isMac ? "bg-white/75 dark:bg-neutral-900/70" : "bg-white dark:bg-neutral-900"
      }`}
    >
      {/* Search + tabs */}
      <div data-tauri-drag-region className="shrink-0 px-2.5 pb-1.5 pt-2.5">
        {!searchHidden && (
          <div className="flex items-center gap-2 rounded-lg bg-black/5 px-2.5 py-1.5 dark:bg-white/10">
            <Icon name="search" size={15} className="text-neutral-400" />
            <input
              ref={search}
              autoFocus
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={tab === "history" ? "Search history…" : tab === "pins" ? "Search pins…" : "Search secure clips…"}
              className="min-w-0 flex-1 bg-transparent text-[1.05rem] outline-none placeholder:text-neutral-400"
              spellCheck={false}
            />
            {paused && (
              <button
                title="Recording is paused. Click to resume."
                onClick={() => api.pauseRecording(0)}
                className="flex items-center gap-1 rounded bg-amber-400/20 px-1.5 py-0.5 text-[0.75rem] text-amber-700 dark:text-amber-300"
              >
                <Icon name="pause" size={11} /> Paused
              </button>
            )}
            <IconButton icon="gear" title={keyText("Settings (⌘,)")} onClick={() => api.openSettings()} />
          </div>
        )}
        <div className="mt-1.5 flex gap-1">
          {TABS.map((t) => (
            <button
              key={t.id}
              onClick={() => switchTab(t.id)}
              className={`flex flex-1 items-center justify-center gap-1.5 rounded-md py-1 text-[0.9rem] font-medium transition-colors ${
                tab === t.id
                  ? "bg-white text-neutral-900 shadow-sm dark:bg-white/15 dark:text-white"
                  : "text-neutral-500 hover:bg-black/5 dark:text-neutral-400 dark:hover:bg-white/5"
              }`}
            >
              <Icon name={t.icon} size={13} />
              {t.label}
            </button>
          ))}
        </div>
      </div>

      <div className="relative flex min-h-0 flex-1 flex-col border-t border-black/5 dark:border-white/10">
        {tab === "history" && (
          <HistoryList
            query={query}
            settings={settings}
            active
            onToast={toast}
            registerKeys={registerKeys}
            onMoveToSecure={(c) => {
              if (c.kind !== "text") return;
              setPending(c);
              switchTab("secure");
            }}
          />
        )}
        {tab === "pins" && (
          <PinList query={query} settings={settings} active onToast={toast} registerKeys={registerKeys} setModal={setModal} />
        )}
        {tab === "secure" && (
          <SecurePanel
            query={query}
            active
            shownAt={shownAt}
            pending={pending}
            clearPending={() => setPending(null)}
            onToast={toast}
            registerKeys={registerKeys}
            setModal={setModal}
            onLockedChange={setVaultLocked}
          />
        )}
      </div>

      {tab !== "secure" && (
        <div className="flex shrink-0 items-center gap-3 border-t border-black/5 px-3 py-1 text-[0.72rem] text-neutral-500 dark:border-white/10">
          <span>
            <Kbd>↩</Kbd> paste
          </span>
          <span>
            <Kbd>{keyText("⌘↩")}</Kbd> copy
          </span>
          {tab === "history" ? (
            <span>
              <Kbd>{keyText("⌘P")}</Kbd> pin
            </span>
          ) : (
            <span>
              <Kbd>{keyText("⌥↑↓")}</Kbd> reorder
            </span>
          )}
          <span className="ml-auto">
            <Kbd>⇥</Kbd> switch tab
          </span>
        </div>
      )}

      <Toasts toasts={toasts} />
      {!settings.onboarded && <Onboarding settings={settings} />}
    </div>
  );
}
