import { useEffect, useState } from "react";
import { api, Settings, UpdateStatus } from "../lib/api";
import { useEvent } from "../lib/hooks";
import { isMac } from "../lib/shortcut";
import { Icon } from "./Icon";

/** Notices at the top of the popup: paste permission missing, and new release available. */
export function Banners({ settings }: { settings: Settings }) {
  return (
    <>
      <AccessBanner settings={settings} />
      <UpdateBanner />
    </>
  );
}

function Banner({ icon, tone, children, onClose }: { icon: string; tone: "amber" | "indigo"; children: React.ReactNode; onClose: () => void }) {
  const colors =
    tone === "amber"
      ? "bg-amber-400/15 text-amber-900 dark:text-amber-200"
      : "bg-indigo-500/12 text-indigo-900 dark:text-indigo-200";
  return (
    <div className={`mx-2.5 mb-1.5 flex items-center gap-2 rounded-lg px-2.5 py-1.5 text-[0.85rem] ${colors}`}>
      <Icon name={icon} size={14} className="shrink-0" />
      <div className="flex min-w-0 flex-1 flex-wrap items-center gap-x-2 gap-y-1">{children}</div>
      <button title="Hide" className="shrink-0 rounded p-0.5 opacity-60 hover:opacity-100" onClick={onClose}>
        <Icon name="x" size={12} />
      </button>
    </div>
  );
}

const linkBtn = "rounded px-1.5 py-0.5 font-medium underline-offset-2 hover:underline bg-black/5 dark:bg-white/10";

/** macOS: shown until Pastora may press Cmd+V, so paste never fails silently (e.g. after an update). */
function AccessBanner({ settings }: { settings: Settings }) {
  const [trusted, setTrusted] = useState(true);
  const [hidden, setHidden] = useState(false);
  const relevant = isMac && settings.onboarded && settings.autoPaste;
  const check = () => {
    if (relevant) api.accessibilityStatus().then(setTrusted);
  };
  useEffect(() => {
    check();
    if (!relevant) return;
    const t = setInterval(check, 2000);
    return () => clearInterval(t);
  }, [relevant]);
  useEvent("popup-shown", check);
  useEvent<boolean>("accessibility-changed", setTrusted);
  if (!relevant || trusted || hidden) return null;
  return (
    <Banner icon="paste" tone="amber" onClose={() => setHidden(true)}>
      <span>Pastora can't paste for you yet.</span>
      <button className={linkBtn} onClick={() => api.requestAccessibility()}>
        Allow
      </button>
      <button className={linkBtn} title="Already allowed? macOS may keep an old entry after an update." onClick={() => api.resetAccessibility()}>
        Reset
      </button>
    </Banner>
  );
}

function UpdateBanner() {
  const [status, setStatus] = useState<UpdateStatus | null>(null);
  const load = () => api.updateStatus().then(setStatus);
  useEffect(() => {
    load();
  }, []);
  useEvent("update-available", load);
  useEvent("popup-shown", load);
  const u = status?.update;
  if (!u || status?.dismissed) return null;
  return (
    <Banner
      icon="download"
      tone="indigo"
      onClose={() => {
        api.dismissUpdate(u.version);
        setStatus({ ...status!, dismissed: true });
      }}
    >
      <span>Pastora {u.version} is available.</span>
      <button className={linkBtn} onClick={() => api.openUrl(u.url)}>
        Download
      </button>
      <button className={linkBtn} onClick={() => api.openUrl(u.notesUrl)}>
        What's new
      </button>
    </Banner>
  );
}
