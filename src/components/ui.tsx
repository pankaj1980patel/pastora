import { ButtonHTMLAttributes, ReactNode, useEffect, useState } from "react";
import { Icon } from "./Icon";

export function Button({
  variant = "secondary",
  className = "",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: "primary" | "secondary" | "ghost" | "danger" }) {
  const styles = {
    primary: "bg-indigo-500 text-white hover:bg-indigo-600 disabled:opacity-50",
    secondary:
      "bg-black/5 dark:bg-white/10 hover:bg-black/10 dark:hover:bg-white/15 text-neutral-800 dark:text-neutral-100 disabled:opacity-50",
    ghost: "hover:bg-black/5 dark:hover:bg-white/10 text-neutral-600 dark:text-neutral-300",
    danger: "bg-red-500/10 text-red-600 dark:text-red-400 hover:bg-red-500/20",
  }[variant];
  return (
    <button
      type="button"
      className={`inline-flex items-center justify-center gap-1.5 rounded-md px-2.5 py-1.5 text-[0.95rem] font-medium transition-colors ${styles} ${className}`}
      {...props}
    />
  );
}

export function IconButton({
  icon,
  title,
  onClick,
  active,
  className = "",
  size = 15,
}: {
  icon: string;
  title: string;
  onClick?: (e: React.MouseEvent) => void;
  active?: boolean;
  className?: string;
  size?: number;
}) {
  return (
    <button
      type="button"
      title={title}
      aria-label={title}
      tabIndex={-1}
      onClick={(e) => {
        e.stopPropagation();
        onClick?.(e);
      }}
      className={`rounded-md p-1 transition-colors hover:bg-black/10 dark:hover:bg-white/15 ${
        active ? "text-indigo-500" : "text-neutral-500 dark:text-neutral-400"
      } ${className}`}
    >
      <Icon name={icon} size={size} />
    </button>
  );
}

export function Toggle({ checked, onChange, disabled }: { checked: boolean; onChange: (v: boolean) => void; disabled?: boolean }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative h-[22px] w-[38px] shrink-0 rounded-full transition-colors disabled:opacity-40 ${
        checked ? "bg-indigo-500" : "bg-neutral-300 dark:bg-neutral-600"
      }`}
    >
      <span
        className={`absolute top-[2px] h-[18px] w-[18px] rounded-full bg-white shadow transition-all ${
          checked ? "left-[18px]" : "left-[2px]"
        }`}
      />
    </button>
  );
}

export function Kbd({ children }: { children: ReactNode }) {
  return (
    <kbd className="rounded border border-black/10 bg-black/5 px-1 py-px font-sans text-[0.75rem] text-neutral-500 dark:border-white/10 dark:bg-white/10 dark:text-neutral-400">
      {children}
    </kbd>
  );
}

export const inputClass =
  "w-full rounded-md border border-black/10 bg-white/70 px-2.5 py-1.5 text-[0.95rem] outline-none placeholder:text-neutral-400 focus:border-indigo-400 focus:ring-2 focus:ring-indigo-400/30 dark:border-white/10 dark:bg-black/30";

/** Small ring that empties over `seconds`. */
export function Countdown({ seconds, size = 16 }: { seconds: number; size?: number }) {
  const [left, setLeft] = useState(seconds);
  useEffect(() => {
    const start = Date.now();
    const t = setInterval(() => setLeft(Math.max(0, seconds - (Date.now() - start) / 1000)), 100);
    return () => clearInterval(t);
  }, [seconds]);
  const r = size / 2 - 2;
  const c = 2 * Math.PI * r;
  return (
    <svg width={size} height={size} className="-rotate-90">
      <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke="currentColor" strokeOpacity={0.25} strokeWidth={2} />
      <circle
        cx={size / 2}
        cy={size / 2}
        r={r}
        fill="none"
        stroke="currentColor"
        strokeWidth={2}
        strokeDasharray={c}
        strokeDashoffset={c * (1 - left / seconds)}
      />
    </svg>
  );
}

export interface ToastMsg {
  id: number;
  message: string;
  seconds?: number;
}

export function Toasts({ toasts }: { toasts: ToastMsg[] }) {
  return (
    <div className="pointer-events-none absolute inset-x-0 bottom-9 z-50 flex flex-col items-center gap-1.5">
      {toasts.map((t) => (
        <div
          key={t.id}
          className="flex items-center gap-2 rounded-full bg-neutral-900/90 px-3 py-1.5 text-[0.9rem] text-white shadow-lg dark:bg-white/90 dark:text-neutral-900"
        >
          {t.seconds ? <Countdown seconds={t.seconds} size={14} /> : <Icon name="check" size={13} />}
          {t.message}
        </div>
      ))}
    </div>
  );
}
