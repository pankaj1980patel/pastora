import { useEffect, useRef, useState } from "react";
import { api, errorText, VaultStatus } from "../../lib/api";
import { strength } from "../../lib/password";
import { Icon } from "../Icon";
import { Button, inputClass } from "../ui";

export function SetupVault({ status, onDone }: { status: VaultStatus; onDone: () => void }) {
  const [pw, setPw] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [step, setStep] = useState<"password" | "biometric">("password");
  const s = strength(pw);

  const create = async () => {
    if (pw.length < 8) return setError("Use at least 8 characters.");
    if (pw !== confirm) return setError("Passwords don't match.");
    setBusy(true);
    try {
      await api.vaultSetup(pw);
      if (status.biometricAvailable) setStep("biometric");
      else onDone();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  if (step === "biometric") {
    return (
      <Center>
        <Icon name="fingerprint" size={34} className="text-indigo-500" strokeWidth={1.4} />
        <h2 className="text-[1.1rem] font-semibold">Unlock with {status.biometricName}?</h2>
        <p className="text-[0.9rem] text-neutral-500">Use your fingerprint instead of typing your master password every time.</p>
        <div className="flex gap-2">
          <Button onClick={onDone}>Not now</Button>
          <Button
            variant="primary"
            onClick={async () => {
              try {
                await api.vaultSetBiometric(true);
                onDone();
              } catch (e) {
                setError(errorText(e));
              }
            }}
          >
            Turn on {status.biometricName}
          </Button>
        </div>
        {error && <p className="text-[0.8rem] text-red-500">{error}</p>}
      </Center>
    );
  }

  return (
    <Center>
      <Icon name="shield" size={34} className="text-indigo-500" strokeWidth={1.4} />
      <h2 className="text-[1.1rem] font-semibold">Set up Secure Clips</h2>
      <p className="text-[0.9rem] text-neutral-500">
        Keep passwords, API keys and cards encrypted on this Mac. Choose a master password; it can't be recovered if you forget it.
      </p>
      <form
        className="flex w-full max-w-[17rem] flex-col gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          create();
        }}
      >
        <input autoFocus type="password" className={inputClass} placeholder="Master password" value={pw} onChange={(e) => setPw(e.target.value)} />
        <div className="flex gap-1">
          {[0, 1, 2, 3].map((i) => (
            <div
              key={i}
              className={`h-1 flex-1 rounded ${
                pw && i <= s - 1 ? ["bg-red-400", "bg-amber-400", "bg-lime-500", "bg-green-500"][Math.max(0, s - 1)] : "bg-black/10 dark:bg-white/10"
              }`}
            />
          ))}
        </div>
        <input type="password" className={inputClass} placeholder="Confirm password" value={confirm} onChange={(e) => setConfirm(e.target.value)} />
        {error && <p className="text-[0.8rem] text-red-500">{error}</p>}
        <Button variant="primary" type="submit" disabled={busy}>
          {busy ? "Creating…" : "Create vault"}
        </Button>
      </form>
    </Center>
  );
}

export function LockScreen({ status, onUnlocked, shownAt }: { status: VaultStatus; onUnlocked: () => void; shownAt: number }) {
  const [pw, setPw] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  const canBio = status.biometricEnabled && status.biometricAvailable && !status.passwordRequired;

  const bio = async () => {
    setError("");
    try {
      await api.vaultUnlockBiometric();
      onUnlocked();
    } catch (e) {
      const msg = errorText(e);
      if (msg !== "cancelled") setError(msg);
      input.current?.focus();
    }
  };

  // Offer Touch ID each time the popup opens on this tab. Wait until the popup is really
  // in front (after its fade-in): macOS cancels a prompt started from the background.
  useEffect(() => {
    if (!canBio) {
      input.current?.focus();
      return;
    }
    let timer: number | undefined;
    const start = () => {
      timer = window.setTimeout(bio, 200);
    };
    if (document.hasFocus()) start();
    else window.addEventListener("focus", start, { once: true });
    return () => {
      window.clearTimeout(timer);
      window.removeEventListener("focus", start);
    };
  }, [shownAt]);

  const unlock = async () => {
    if (!pw) return;
    setBusy(true);
    setError("");
    try {
      await api.vaultUnlock(pw);
      setPw("");
      onUnlocked();
    } catch (e) {
      setError(errorText(e));
      input.current?.select();
    } finally {
      setBusy(false);
    }
  };

  return (
    <Center>
      <Icon name="lock" size={32} className="text-indigo-500" strokeWidth={1.4} />
      <h2 className="text-[1.1rem] font-semibold">Secure Clips is locked</h2>
      {status.passwordRequired && status.biometricEnabled && (
        <p className="text-[0.85rem] text-neutral-500">Enter your password. It's required every so often, even with {status.biometricName}.</p>
      )}
      <form
        className="flex w-full max-w-[17rem] flex-col gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          unlock();
        }}
      >
        <input ref={input} type="password" className={inputClass} placeholder="Master password" value={pw} onChange={(e) => setPw(e.target.value)} />
        {error && <p className="text-[0.8rem] text-red-500">{error}</p>}
        <Button variant="primary" type="submit" disabled={busy || !pw}>
          {busy ? "Unlocking…" : "Unlock"}
        </Button>
        {canBio && (
          <Button onClick={bio}>
            <Icon name="fingerprint" size={15} /> Use {status.biometricName}
          </Button>
        )}
      </form>
      <button className="text-[0.8rem] text-neutral-500 hover:underline" onClick={() => api.openSettings("secure")}>
        Forgot password?
      </button>
    </Center>
  );
}

function Center({ children }: { children: React.ReactNode }) {
  return <div className="flex flex-1 flex-col items-center justify-center gap-3 overflow-y-auto p-6 text-center">{children}</div>;
}
