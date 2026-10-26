import logo from "../assets/logo.png";
import { useEffect, useState } from "react";
import { api, Settings } from "../lib/api";
import { displayAccelerator, isMac } from "../lib/shortcut";
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
        <img src={logo} className="h-10 w-10" alt="" />
        <div>
          <div className="text-[1.15rem] font-semibold">Welcome to Pastora</div>
          <div className="text-[0.85rem] text-neutral-500">Your clipboard, with a memory and a safe.</div>
        </div>
      </div>
      <div className="mt-4 space-y-3 text-[0.92rem]">
        {isMac && (
          <div className={`rounded-lg border p-3 ${trusted ? "border-green-500/30 bg-green-500/5" : "border-indigo-500/30 bg-indigo-500/5"}`}>
            <Step icon="paste" title="1. Let Pastora paste for you">
              Pastora needs macOS <b>Accessibility</b> permission to press ⌘V in the app you were using. Without it, items are only copied.
              <div className="mt-2 flex flex-wrap items-center gap-2">
                {trusted ? (
                  <span className="inline-flex items-center gap-1 font-medium text-green-600">
                    <Icon name="check" size={14} /> Allowed
                  </span>
                ) : (
                  <>
                    <Button variant="primary" onClick={() => api.requestAccessibility()}>
                      Allow Accessibility…
                    </Button>
                    <span className="text-[0.8rem] text-neutral-500">
                      Turn on <b>Pastora</b> in the list. Already on?{" "}
                      <button className="text-indigo-500 hover:underline" onClick={() => api.resetAccessibility()}>
                        Reset
                      </button>{" "}
                      and turn it on again.
                    </span>
                  </>
                )}
              </div>
            </Step>
          </div>
        )}
        <Step icon="clipboard" title="History">
          Everything you copy is saved. Open it anytime with <Kbd>{displayAccelerator(sc.openPopup)}</Kbd>, then click an item to paste it.
        </Step>
        <Step icon="pin" title="Pins">
          Keep snippets you use often. Drag to reorder. <Kbd>{displayAccelerator(sc.openPins)}</Kbd>
        </Step>
        <Step icon="shield" title="Secure Clips">
          Passwords, API keys and cards, encrypted{isMac ? " and unlocked with Touch ID" : ""}. <Kbd>{displayAccelerator(sc.openSecure)}</Kbd>
        </Step>
      </div>
      <div className="mt-auto flex items-center justify-between pt-4">
        <button className="text-[0.85rem] text-neutral-500 hover:underline" onClick={() => api.openSettings()}>
          Open Settings
        </button>
        <div className="flex gap-2">
          {isMac && !trusted && <Button onClick={() => api.finishOnboarding()}>Skip for now</Button>}
          <Button variant="primary" disabled={isMac && !trusted} onClick={() => api.finishOnboarding()}>
            {isMac && !trusted ? "Allow paste first" : "Get started"}
          </Button>
        </div>
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
