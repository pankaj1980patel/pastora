import { useEffect, useState } from "react";
import { api, Settings } from "../lib/api";
import { displayAccelerator } from "../lib/shortcut";
import { Icon } from "./Icon";
import { Button, Kbd } from "./ui";

export function Onboarding({ settings }: { settings: Settings }) {
  const [trusted, setTrusted] = useState(false);
  useEffect(() => {
    const check = () => api.accessibilityStatus().then(setTrusted);
    check();
    const t = setInterval(check, 1500);
    return () => clearInterval(t);
  }, []);
  const sc = settings.shortcuts;
  return (
    <div className="absolute inset-0 z-50 flex flex-col overflow-y-auto bg-white/95 p-5 dark:bg-neutral-900/95">
      <div className="flex items-center gap-2">
        <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-gradient-to-b from-indigo-500 to-violet-500 text-white">
          <Icon name="clipboard" size={18} />
        </div>
        <div>
          <div className="text-[1.15rem] font-semibold">Welcome to Pastora</div>
          <div className="text-[0.85rem] text-neutral-500">Your clipboard, with a memory and a safe.</div>
        </div>
      </div>
      <div className="mt-4 space-y-3 text-[0.92rem]">
        <Step icon="clipboard" title="History">
          Everything you copy is saved. Open it anytime with <Kbd>{displayAccelerator(sc.openPopup)}</Kbd>.
        </Step>
        <Step icon="pin" title="Pins">
          Keep snippets you use often. Drag to reorder. <Kbd>{displayAccelerator(sc.openPins)}</Kbd>
        </Step>
        <Step icon="shield" title="Secure Clips">
          Passwords, API keys and cards, encrypted and unlocked with Touch ID. <Kbd>{displayAccelerator(sc.openSecure)}</Kbd>
        </Step>
        <Step icon="paste" title="Paste automatically">
          To paste into the app you were using, Pastora needs Accessibility permission.
          <div className="mt-1.5">
            {trusted ? (
              <span className="inline-flex items-center gap-1 text-green-600">
                <Icon name="check" size={14} /> Allowed
              </span>
            ) : (
              <Button onClick={() => api.requestAccessibility()}>Allow in System Settings…</Button>
            )}
          </div>
        </Step>
      </div>
      <div className="mt-auto flex items-center justify-between pt-4">
        <button className="text-[0.85rem] text-neutral-500 hover:underline" onClick={() => api.openSettings()}>
          Open Settings
        </button>
        <Button variant="primary" onClick={() => api.finishOnboarding()}>
          Get started
        </Button>
      </div>
    </div>
  );
}

function Step({ icon, title, children }: { icon: string; title: string; children: React.ReactNode }) {
  return (
    <div className="flex gap-3">
      <Icon name={icon} size={18} className="mt-0.5 shrink-0 text-indigo-500" />
      <div>
        <div className="font-medium">{title}</div>
        <div className="text-neutral-600 dark:text-neutral-400">{children}</div>
      </div>
    </div>
  );
}
