import logo from "../assets/logo.png";
import { ask, message, open, save } from "@tauri-apps/plugin-dialog";
import { ReactNode, useEffect, useState } from "react";
import { api, AppInfo, errorText, IgnoredApp, Settings, Shortcuts, UpdateStatus, VaultStatus } from "../lib/api";
import { useEvent } from "../lib/hooks";
import { acceleratorFromEvent, displayAccelerator, isMac, keys } from "../lib/shortcut";
import { Icon } from "./Icon";
import { Button, inputClass, Kbd, Toggle } from "./ui";

const SECTIONS = [
  { id: "general", label: "General", icon: "gear" },
  { id: "history", label: "History", icon: "clock" },
  { id: "shortcuts", label: "Shortcuts", icon: "key" },
  { id: "paste", label: "Paste", icon: "paste" },
  { id: "secure", label: "Secure Clips", icon: "shield" },
  { id: "appearance", label: "Appearance", icon: "eye" },
  { id: "data", label: "Data & Privacy", icon: "file" },
  { id: "about", label: "About", icon: "clipboard" },
] as const;
type SectionId = (typeof SECTIONS)[number]["id"];

type Update = (patch: Partial<Settings>) => Promise<boolean>;

export function SettingsWindow({ settings }: { settings: Settings }) {
  const initial = new URLSearchParams(location.search).get("section") as SectionId | null;
  const [section, setSection] = useState<SectionId>(initial ?? "general");
  const [error, setError] = useState("");
  useEvent<string>("settings-section", (s) => setSection(s as SectionId));

  const update: Update = async (patch) => {
    try {
      await api.setSettings({ ...settings, ...patch });
      setError("");
      return true;
    } catch (e) {
      setError(errorText(e));
      return false;
    }
  };

  return (
    <div className="flex h-screen text-[13px] text-neutral-900 dark:text-neutral-100">
      <nav className="w-52 shrink-0 border-r border-black/5 bg-black/[0.02] p-3 dark:border-white/10 dark:bg-white/[0.03]">
        <div className="mb-4 flex items-center gap-2 px-2 pt-1">
          <img src={logo} className="h-7 w-7" alt="" />
          <span className="text-[15px] font-semibold">Pastora</span>
        </div>
        {SECTIONS.map((s) => (
          <button
            key={s.id}
            onClick={() => setSection(s.id)}
            className={`mb-0.5 flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left ${
              section === s.id ? "bg-indigo-500 text-white" : "hover:bg-black/5 dark:hover:bg-white/10"
            }`}
          >
            <Icon name={s.icon} size={15} />
            {s.label}
          </button>
        ))}
      </nav>
      <main className="flex-1 overflow-y-auto p-6">
        <h1 className="mb-4 text-xl font-semibold">{SECTIONS.find((s) => s.id === section)?.label}</h1>
        {error && (
          <div className="mb-3 flex items-center gap-2 rounded-md bg-red-500/10 px-3 py-2 text-red-600 dark:text-red-400">
            <Icon name="x" size={14} /> {error}
          </div>
        )}
        {section === "general" && <General s={settings} update={update} />}
        {section === "history" && <History s={settings} update={update} />}
        {section === "shortcuts" && <ShortcutsSection s={settings} update={update} />}
        {section === "paste" && <Paste s={settings} update={update} />}
        {section === "secure" && <Secure s={settings} update={update} />}
        {section === "appearance" && <Appearance s={settings} update={update} />}
        {section === "data" && <Data />}
        {section === "about" && <About />}
      </main>
    </div>
  );
}

// ---------- building blocks ----------

function Group({ title, children }: { title?: string; children: ReactNode }) {
  return (
    <section className="mb-5">
      {title && <h2 className="mb-1.5 px-1 text-[12px] font-semibold uppercase tracking-wide text-neutral-500">{title}</h2>}
      <div className="divide-y divide-black/5 rounded-xl border border-black/5 bg-white dark:divide-white/10 dark:border-white/10 dark:bg-white/5">
        {children}
      </div>
    </section>
  );
}

function Row({ label, hint, children }: { label: ReactNode; hint?: ReactNode; children?: ReactNode }) {
  return (
    <div className="flex items-center gap-4 px-4 py-3">
      <div className="min-w-0 flex-1">
        <div className="font-medium">{label}</div>
        {hint && <div className="mt-0.5 text-[12px] text-neutral-500">{hint}</div>}
      </div>
      {children}
    </div>
  );
}

function Select<T extends string | number>({ value, options, onChange }: { value: T; options: [T, string][]; onChange: (v: T) => void }) {
  return (
    <select
      value={String(value)}
      onChange={(e) => {
        const raw = e.target.value;
        onChange((typeof value === "number" ? Number(raw) : raw) as T);
      }}
      className="rounded-md border border-black/10 bg-white px-2 py-1 outline-none dark:border-white/10 dark:bg-neutral-800"
    >
      {options.map(([v, l]) => (
        <option key={String(v)} value={String(v)}>
          {l}
        </option>
      ))}
    </select>
  );
}

// ---------- sections ----------

function General({ s, update }: { s: Settings; update: Update }) {
  return (
    <>
      <Group>
        <Row label="Start at login" hint="Open Pastora automatically when you log in to your Mac.">
          <Toggle checked={s.launchAtLogin} onChange={(v) => update({ launchAtLogin: v })} />
        </Row>
        <Row label="Show in Dock" hint="By default Pastora lives only in the menu bar.">
          <Toggle checked={s.showDockIcon} onChange={(v) => update({ showDockIcon: v })} />
        </Row>
        <Row label="Check for updates" hint="Asks GitHub once a day whether a newer version is out. Nothing else is sent.">
          <Toggle checked={s.checkUpdates} onChange={(v) => update({ checkUpdates: v })} />
        </Row>
        <Row label="Open popup" hint="Where the popup appears when you press the shortcut. Clicking the menu bar icon always drops it down below the icon.">
          <Select
            value={s.popupPosition}
            options={[["menubar", "Below the menu bar icon"], ["cursor", "Near the mouse pointer"], ["center", "Center of the screen"]]}
            onChange={(v) => update({ popupPosition: v })}
          />
        </Row>
      </Group>
      <Group title="Recording">
        <PauseRow s={s} />
      </Group>
    </>
  );
}

function PauseRow({ s }: { s: Settings }) {
  const paused = s.pausedUntil != null && s.pausedUntil > Date.now();
  const until =
    s.pausedUntil && s.pausedUntil < 8.64e15 && paused ? ` until ${new Date(s.pausedUntil).toLocaleTimeString([], { timeStyle: "short" })}` : "";
  return (
    <Row label={paused ? `Recording paused${until}` : "Recording clipboard"} hint="Pause when you're copying something you don't want saved.">
      {paused ? (
        <Button variant="primary" onClick={() => api.pauseRecording(0)}>
          <Icon name="play" size={13} /> Resume
        </Button>
      ) : (
        <div className="flex gap-1.5">
          <Button onClick={() => api.pauseRecording(15)}>15 min</Button>
          <Button onClick={() => api.pauseRecording(60)}>1 hour</Button>
          <Button onClick={() => api.pauseRecording(-1)}>Until resumed</Button>
        </div>
      )}
    </Row>
  );
}

const PATTERN_PRESETS: [string, string][] = [
  ["6-digit codes (OTP)", "^\\s*\\d{6}\\s*$"],
  ["Credit card numbers", "\\b(?:\\d[ -]?){13,19}\\b"],
  ["AWS access keys", "AKIA[0-9A-Z]{16}"],
  ["Private keys", "-----BEGIN [A-Z ]*PRIVATE KEY-----"],
  ["JSON Web Tokens", "eyJ[\\w-]+\\.[\\w-]+\\.[\\w-]+"],
];

function History({ s, update }: { s: Settings; update: Update }) {
  const [apps, setApps] = useState<IgnoredApp[]>([]);
  const [patterns, setPatterns] = useState(s.ignorePatterns.join("\n"));
  const [patternError, setPatternError] = useState("");
  useEffect(() => {
    api.runningApps().then(setApps);
  }, []);

  const savePatterns = (text: string) => {
    const list = text.split("\n").map((l) => l.trim()).filter(Boolean);
    for (const p of list) {
      try {
        new RegExp(p);
      } catch {
        return setPatternError(`Not a valid pattern: ${p}`);
      }
    }
    setPatternError("");
    update({ ignorePatterns: list });
  };

  const addable = apps.filter((a) => !s.ignoredApps.some((i) => i.bundleId === a.bundleId));

  return (
    <>
      <Group title="Size">
        <Row label="Keep up to" hint="Older items are removed when the limit is reached. Pins are never removed.">
          <Select
            value={s.historyLimit}
            options={[[50, "50 items"], [100, "100 items"], [250, "250 items"], [500, "500 items"], [1000, "1,000 items"], [5000, "5,000 items"], [0, "Unlimited"]]}
            onChange={(v) => update({ historyLimit: v })}
          />
        </Row>
        <Row label="Remove items older than">
          <Select
            value={s.historyDays}
            options={[[1, "1 day"], [7, "1 week"], [30, "1 month"], [90, "3 months"], [365, "1 year"], [0, "Never"]]}
            onChange={(v) => update({ historyDays: v })}
          />
        </Row>
        <Row label="Save images" hint="Screenshots and copied images.">
          <Toggle checked={s.storeImages} onChange={(v) => update({ storeImages: v })} />
        </Row>
        {s.storeImages && (
          <Row label="Skip images larger than">
            <Select value={s.maxImageMb} options={[[2, "2 MB"], [5, "5 MB"], [10, "10 MB"], [25, "25 MB"], [0, "No limit"]]} onChange={(v) => update({ maxImageMb: v })} />
          </Row>
        )}
        <Row label="Clear history" hint="Deletes all history items. Pins and Secure Clips are kept.">
          <Button
            variant="danger"
            onClick={async () => {
              if (await ask("Delete all clipboard history? Pins and Secure Clips are kept.", { title: "Clear history", kind: "warning" }))
                api.clearHistory();
            }}
          >
            Clear…
          </Button>
        </Row>
      </Group>

      <Group title="Don't record">
        <Row label="Skip passwords from password managers" hint="Ignore items that 1Password, Bitwarden, KeePassXC, etc. mark as concealed.">
          <Toggle checked={s.ignorePasswordManagers} onChange={(v) => update({ ignorePasswordManagers: v })} />
        </Row>
        <div className="px-4 py-3">
          <div className="font-medium">Ignored apps</div>
          <div className="mb-2 mt-0.5 text-[12px] text-neutral-500">Nothing copied while these apps are in front is saved.</div>
          <div className="flex flex-wrap gap-1.5">
            {s.ignoredApps.map((a) => (
              <span key={a.bundleId} title={a.bundleId} className="flex items-center gap-1 rounded-full bg-black/5 py-0.5 pl-2.5 pr-1 dark:bg-white/10">
                {a.name}
                <button
                  className="rounded-full p-0.5 hover:bg-black/10 dark:hover:bg-white/15"
                  onClick={() => update({ ignoredApps: s.ignoredApps.filter((x) => x.bundleId !== a.bundleId) })}
                >
                  <Icon name="x" size={11} />
                </button>
              </span>
            ))}
            <select
              value=""
              onChange={(e) => {
                const app = apps.find((a) => a.bundleId === e.target.value);
                if (app) update({ ignoredApps: [...s.ignoredApps, app] });
              }}
              className="rounded-full border border-dashed border-black/20 bg-transparent px-2 py-0.5 outline-none dark:border-white/20"
            >
              <option value="">+ Add running app…</option>
              {addable.map((a) => (
                <option key={a.bundleId} value={a.bundleId}>
                  {a.name}
                </option>
              ))}
            </select>
          </div>
        </div>
        <div className="px-4 py-3">
          <div className="font-medium">Ignore text that matches</div>
          <div className="mb-2 mt-0.5 text-[12px] text-neutral-500">One regular expression per line.</div>
          <textarea
            className={`${inputClass} h-24 font-mono text-[12px]`}
            value={patterns}
            onChange={(e) => setPatterns(e.target.value)}
            onBlur={() => savePatterns(patterns)}
            placeholder="^\d{6}$"
          />
          {patternError && <div className="mt-1 text-[12px] text-red-500">{patternError}</div>}
          <div className="mt-2 flex flex-wrap gap-1.5">
            {PATTERN_PRESETS.filter(([, p]) => !s.ignorePatterns.includes(p)).map(([name, p]) => (
              <button
                key={p}
                onClick={() => {
                  const next = [...s.ignorePatterns, p].join("\n");
                  setPatterns(next);
                  savePatterns(next);
                }}
                className="rounded-full border border-black/10 px-2 py-0.5 text-[12px] hover:bg-black/5 dark:border-white/15 dark:hover:bg-white/10"
              >
                + {name}
              </button>
            ))}
          </div>
        </div>
      </Group>
    </>
  );
}

function ShortcutRecorder({ value, onChange }: { value: string; onChange: (v: string) => Promise<boolean> }) {
  const [recording, setRecording] = useState(false);
  useEffect(() => {
    if (!recording) return;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") return setRecording(false);
      if ((e.key === "Backspace" || e.key === "Delete") && !e.metaKey && !e.ctrlKey && !e.altKey) {
        onChange("");
        return setRecording(false);
      }
      const accel = acceleratorFromEvent(e);
      if (accel) {
        onChange(accel);
        setRecording(false);
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [recording]);
  return (
    <button
      onClick={() => setRecording(!recording)}
      onBlur={() => setRecording(false)}
      className={`min-w-[8.5rem] rounded-md border px-3 py-1 text-center ${
        recording ? "border-indigo-500 bg-indigo-500/10 text-indigo-600 dark:text-indigo-300" : "border-black/10 bg-white dark:border-white/10 dark:bg-neutral-800"
      }`}
    >
      {recording ? "Type shortcut…" : value ? <span className="tracking-wider">{displayAccelerator(value)}</span> : <span className="text-neutral-400">Not set</span>}
    </button>
  );
}

const GLOBAL_SHORTCUTS: [keyof Shortcuts, string, string][] = [
  ["openPopup", "Open clipboard", "Shows the popup on the tab you used last. Press again to paste the password after “username, then password”."],
  ["openPins", "Open Pins", "Shows the popup on the Pins tab."],
  ["openSecure", "Open Secure Clips", "Shows the popup on the Secure Clips tab."],
  ["lockVault", "Lock Secure Clips", "Locks immediately, from anywhere."],
  ["togglePause", "Pause / resume recording", ""],
];

const POPUP_KEYS: [string, string][] = [
  ["↩", "Paste selected item"],
  [keys("⌘↩"), "Copy without pasting"],
  [keys("⌘1 … ⌘9"), "Paste item 1–9"],
  ["↑ ↓", "Move selection"],
  ["⇥ / ⇧⇥", "Next / previous tab"],
  [keys("⌘P"), "Pin / unpin (History)"],
  [keys("⌘S"), "Move to Secure Clips (History)"],
  [keys("⌘⌫"), "Delete item"],
  [keys("⌥↑ ⌥↓"), "Reorder pin"],
  [keys("⌘E"), "Edit pin or secure clip"],
  [keys("⌘N"), "New pin or secure clip"],
  [keys("⌘U / ⌘T"), "Paste username / one-time code (Secure)"],
  [keys("⌘L"), "Paste username, then password (Secure)"],
  ["→", "Open secure clip details"],
  [keys("⌘,"), "Open Settings"],
  ["⎋", "Clear search, go back or close"],
];

function ShortcutsSection({ s, update }: { s: Settings; update: Update }) {
  const all = Object.values(s.shortcuts).filter(Boolean);
  return (
    <>
      <Group title="Global shortcuts (work in any app)">
        {GLOBAL_SHORTCUTS.map(([key, label, hint]) => (
          <Row key={key} label={label} hint={hint || undefined}>
            {all.filter((v) => v === s.shortcuts[key]).length > 1 && <span className="text-[12px] text-red-500">Duplicate</span>}
            <ShortcutRecorder value={s.shortcuts[key]} onChange={(v) => update({ shortcuts: { ...s.shortcuts, [key]: v } })} />
          </Row>
        ))}
        <Row label="" hint="Click a shortcut and press new keys. Backspace removes it, Esc cancels.">
          <Button onClick={() => api.resetShortcuts()}>Reset to defaults</Button>
        </Row>
      </Group>
      <Group title="Inside the popup">
        <div className="grid grid-cols-2 gap-x-6 gap-y-1.5 px-4 py-3">
          {POPUP_KEYS.map(([k, d]) => (
            <div key={k} className="flex items-center justify-between gap-2">
              <span className="text-neutral-600 dark:text-neutral-300">{d}</span>
              <Kbd>{k}</Kbd>
            </div>
          ))}
        </div>
      </Group>
    </>
  );
}

function Paste({ s, update }: { s: Settings; update: Update }) {
  const [trusted, setTrusted] = useState<boolean | null>(null);
  useEffect(() => {
    const check = () => api.accessibilityStatus().then(setTrusted);
    check();
    const t = setInterval(check, 2000);
    return () => clearInterval(t);
  }, []);
  return (
    <Group>
      <Row label="Paste automatically" hint="After choosing an item, paste it into the app you were using. Otherwise it's only copied.">
        <Toggle checked={s.autoPaste} onChange={(v) => update({ autoPaste: v })} />
      </Row>
      {s.autoPaste && isMac && (
        <Row
          label="Accessibility permission"
          hint={
            trusted
              ? "Pastora can paste for you."
              : "macOS needs your OK before Pastora can press ⌘V for you. Already enabled? After an update macOS can keep an old entry. Click Reset, then turn Pastora on again."
          }
        >
          {trusted ? (
            <span className="flex items-center gap-1 text-green-600">
              <Icon name="check" size={14} /> Allowed
            </span>
          ) : (
            <div className="flex gap-2">
              <Button onClick={() => api.resetAccessibility()}>Reset</Button>
              <Button variant="primary" onClick={() => api.requestAccessibility()}>
                Open System Settings
              </Button>
            </div>
          )}
        </Row>
      )}
      <Row label="Move pasted item to the top" hint="Recently used items stay near the top of History.">
        <Toggle checked={s.moveToTopOnUse} onChange={(v) => update({ moveToTopOnUse: v })} />
      </Row>
    </Group>
  );
}

function Secure({ s, update }: { s: Settings; update: Update }) {
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const [err, setErr] = useState("");
  const [msg, setMsg] = useState("");
  const [pw, setPw] = useState({ old: "", next: "", confirm: "" });
  const [importPw, setImportPw] = useState("");
  const refresh = () => api.vaultStatus().then(setStatus);
  useEffect(() => {
    refresh();
  }, []);
  useEvent("vault-changed", refresh);
  useEvent("settings-changed", refresh);
  if (!status) return null;

  const run = async (f: () => Promise<unknown>, ok?: string) => {
    setErr("");
    setMsg("");
    try {
      await f();
      if (ok) setMsg(ok);
      refresh();
    } catch (e) {
      setErr(errorText(e));
    }
  };

  return (
    <>
      {(err || msg) && (
        <div className={`mb-3 rounded-md px-3 py-2 ${err ? "bg-red-500/10 text-red-600" : "bg-green-500/10 text-green-700 dark:text-green-400"}`}>{err || msg}</div>
      )}
      <Group title="Locking">
        <Row label="Lock after inactivity" hint="Secure Clips locks itself when you haven't used it for a while.">
          <Select
            value={s.autoLockMinutes}
            options={[[1, "1 minute"], [5, "5 minutes"], [15, "15 minutes"], [60, "1 hour"], [240, "4 hours"], [0, "Never"]]}
            onChange={(v) => update({ autoLockMinutes: v })}
          />
        </Row>
        <Row label="Lock when the Mac sleeps or the screen locks">
          <Toggle checked={s.lockOnSleep} onChange={(v) => update({ lockOnSleep: v })} />
        </Row>
        <Row label="Clear copied secrets from the clipboard after" hint="Only cleared if the clipboard still holds the secret.">
          <Select
            value={s.clearClipboardSeconds}
            options={[[10, "10 seconds"], [30, "30 seconds"], [60, "1 minute"], [90, "90 seconds"], [0, "Never"]]}
            onChange={(v) => update({ clearClipboardSeconds: v })}
          />
        </Row>
        <Row label="Hide Pastora from screenshots and screen sharing" hint="The popup appears blank in recordings and screen shares.">
          <Toggle checked={s.hideFromScreenCapture} onChange={(v) => update({ hideFromScreenCapture: v })} />
        </Row>
      </Group>

      {isMac && (
      <Group title={status.biometricName}>
        <Row
          label={`Unlock with ${status.biometricName}`}
          hint={
            !status.biometricAvailable
              ? `${status.biometricName} isn't available on this Mac.`
              : !status.initialized
                ? "Set up Secure Clips in the popup first."
                : !status.unlocked && !s.touchIdEnabled
                  ? "Unlock Secure Clips first to turn this on."
                  : "Your master password is still needed as a backup."
          }
        >
          <Toggle
            checked={s.touchIdEnabled}
            disabled={!status.biometricAvailable || !status.initialized || (!status.unlocked && !s.touchIdEnabled)}
            onChange={(v) => run(() => api.vaultSetBiometric(v))}
          />
        </Row>
        <Row label="Ask for the password every" hint="Even with Touch ID, re-enter the master password periodically so you don't forget it.">
          <Select
            value={s.requirePasswordDays}
            options={[[1, "Day"], [7, "Week"], [14, "2 weeks"], [30, "Month"], [0, "Never"]]}
            onChange={(v) => update({ requirePasswordDays: v })}
          />
        </Row>
      </Group>
      )}

      {status.initialized && (
        <Group title="Master password">
          <div className="grid grid-cols-3 gap-2 px-4 py-3">
            <input type="password" className={inputClass} placeholder="Current" value={pw.old} onChange={(e) => setPw({ ...pw, old: e.target.value })} />
            <input type="password" className={inputClass} placeholder="New" value={pw.next} onChange={(e) => setPw({ ...pw, next: e.target.value })} />
            <input type="password" className={inputClass} placeholder="Confirm new" value={pw.confirm} onChange={(e) => setPw({ ...pw, confirm: e.target.value })} />
          </div>
          <Row label="" hint="Your entries aren't re-encrypted; only the key that protects them changes.">
            <Button
              variant="primary"
              disabled={!pw.old || !pw.next}
              onClick={() =>
                pw.next !== pw.confirm
                  ? setErr("New passwords don't match.")
                  : run(async () => {
                      await api.vaultChangePassword(pw.old, pw.next);
                      setPw({ old: "", next: "", confirm: "" });
                    }, "Master password changed.")
              }
            >
              Change password
            </Button>
          </Row>
        </Group>
      )}

      {status.initialized && (
        <Group title="Backup">
          <Row label="Export encrypted backup" hint="A .pastoravault file, still protected by your master password.">
            <Button
              onClick={async () => {
                const path = await save({ defaultPath: "Pastora Secure Clips.pastoravault", filters: [{ name: "Pastora vault", extensions: ["pastoravault"] }] });
                if (path) run(() => api.vaultExport(path), "Backup saved.");
              }}
            >
              Export…
            </Button>
          </Row>
          <Row label="Import backup" hint={status.unlocked ? "Entries are added to your current Secure Clips." : "Unlock Secure Clips first."}>
            <input
              type="password"
              className={`${inputClass} w-44`}
              placeholder="Backup's password"
              value={importPw}
              onChange={(e) => setImportPw(e.target.value)}
              disabled={!status.unlocked}
            />
            <Button
              disabled={!status.unlocked || !importPw}
              onClick={async () => {
                const path = await open({ filters: [{ name: "Pastora vault", extensions: ["pastoravault", "json"] }] });
                if (typeof path === "string")
                  run(async () => {
                    const n = await api.vaultImport(path, importPw);
                    setImportPw("");
                    setMsg(`Imported ${n} item${n === 1 ? "" : "s"}.`);
                  });
              }}
            >
              Import…
            </Button>
          </Row>
        </Group>
      )}

      {status.initialized && (
        <Group title="Danger zone">
          <Row label="Erase Secure Clips" hint="Forgot your master password? This deletes every secure clip so you can start over. It can't be undone.">
            <Button
              variant="danger"
              onClick={async () => {
                if (await ask("Permanently delete all Secure Clips? This can't be undone.", { title: "Erase Secure Clips", kind: "warning" }))
                  run(() => api.vaultReset(), "Secure Clips erased.");
              }}
            >
              Erase…
            </Button>
          </Row>
        </Group>
      )}
    </>
  );
}

function Appearance({ s, update }: { s: Settings; update: Update }) {
  return (
    <Group>
      <Row label="Theme">
        <Select value={s.theme} options={[["system", "Match system"], ["light", "Light"], ["dark", "Dark"]]} onChange={(v) => update({ theme: v })} />
      </Row>
      <Row label="Popup size">
        <Select
          value={`${s.popupWidth}x${s.popupHeight}`}
          options={[["380x480", "Compact"], ["440x560", "Regular"], ["520x660", "Large"], ["620x760", "Extra large"]]}
          onChange={(v) => {
            const [w, h] = v.split("x").map(Number);
            update({ popupWidth: w, popupHeight: h });
          }}
        />
      </Row>
      <Row label="Text size">
        <Select value={s.fontSize} options={[[12, "Small"], [13, "Regular"], [14, "Large"], [16, "Extra large"]]} onChange={(v) => update({ fontSize: v })} />
      </Row>
      <Row label="Show preview of the selected item">
        <Toggle checked={s.showPreview} onChange={(v) => update({ showPreview: v })} />
      </Row>
      <Row label="Show which app it was copied from">
        <Toggle checked={s.showSourceApp} onChange={(v) => update({ showSourceApp: v })} />
      </Row>
      <Row label="Show relative times" hint="“5m ago” instead of the full date.">
        <Toggle checked={s.relativeTime} onChange={(v) => update({ relativeTime: v })} />
      </Row>
    </Group>
  );
}

function Data() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  useEffect(() => {
    api.appInfo().then(setInfo);
  }, []);
  return (
    <>
      <Group title="Where your data lives">
        <Row label="Stored only on this Mac" hint={info?.dataDir}>
          <Button onClick={() => api.openDataFolder()}>Show in Finder</Button>
        </Row>
      </Group>
      <Group title="Pins and settings">
        <Row label="Export" hint="Saves your settings and text pins to a file. History and Secure Clips aren't included.">
          <Button
            onClick={async () => {
              const path = await save({ defaultPath: "Pastora export.json", filters: [{ name: "JSON", extensions: ["json"] }] });
              if (path) {
                await api.exportData(path);
                await message("Export saved.", { title: "Pastora" });
              }
            }}
          >
            Export…
          </Button>
        </Row>
        <Row label="Import" hint="Adds the pins and replaces your settings.">
          <Button
            onClick={async () => {
              const path = await open({ filters: [{ name: "JSON", extensions: ["json"] }] });
              if (typeof path === "string") {
                try {
                  const n = await api.importData(path);
                  await message(`Imported ${n} pin${n === 1 ? "" : "s"}.`, { title: "Pastora" });
                } catch (e) {
                  await message(errorText(e), { title: "Import failed", kind: "error" });
                }
              }
            }}
          >
            Import…
          </Button>
        </Row>
      </Group>
      <Group title="Danger zone">
        <Row label="Reset Pastora" hint="Deletes history, pins, Secure Clips and settings, then restarts.">
          <Button
            variant="danger"
            onClick={async () => {
              if (await ask("Erase everything Pastora has stored, including Secure Clips? This can't be undone.", { title: "Reset Pastora", kind: "warning" }))
                api.resetAll();
            }}
          >
            Reset…
          </Button>
        </Row>
      </Group>
    </>
  );
}

function About() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [upd, setUpd] = useState<UpdateStatus | null>(null);
  const [checking, setChecking] = useState(false);
  const [checked, setChecked] = useState(false);
  const [updErr, setUpdErr] = useState("");
  useEffect(() => {
    api.appInfo().then(setInfo);
    api.updateStatus().then(setUpd);
  }, []);
  const checkNow = async () => {
    setChecking(true);
    setUpdErr("");
    try {
      setUpd(await api.checkUpdatesNow());
      setChecked(true);
    } catch (e) {
      setUpdErr(errorText(e));
    } finally {
      setChecking(false);
    }
  };
  return (
    <div className="flex flex-col items-center gap-2 py-8 text-center">
      <img src={logo} className="h-20 w-20" alt="Pastora" />
      <div className="text-xl font-semibold">Pastora</div>
      <div className="text-neutral-500">Version {info?.version}</div>
      <div className="flex flex-col items-center gap-1 text-[0.9rem]">
        {upd?.update ? (
          <div className="flex items-center gap-2">
            <span>Pastora {upd.update.version} is available.</span>
            <Button variant="primary" onClick={() => api.openUrl(upd.update!.url)}>
              <Icon name="download" size={13} /> Download
            </Button>
            <Button onClick={() => api.openUrl(upd.update!.notesUrl)}>What's new</Button>
          </div>
        ) : (
          <Button onClick={checkNow} disabled={checking}>
            {checking ? "Checking…" : "Check for updates"}
          </Button>
        )}
        {checked && upd && !upd.update && !checking && <span className="text-neutral-500">Checked: you're up to date.</span>}
        {updErr && <span className="text-red-500">{updErr}</span>}
      </div>
      <p className="max-w-sm text-neutral-500">
        Clipboard history, pins and encrypted Secure Clips. Everything stays on your device. The only request Pastora makes is the daily update check, which you can turn off in General.
      </p>
      <button className="text-indigo-500 hover:underline" onClick={() => api.openWebsite()}>
        github.com/pankaj1980patel/pastora
      </button>
    </div>
  );
}
