// Minimal stroke icon set (Lucide-style paths).
const PATHS: Record<string, string> = {
  search: "M11 19a8 8 0 1 0 0-16 8 8 0 0 0 0 16Zm10 2-4.35-4.35",
  pin: "M12 17v5M9 10.76V6h6v4.76l2 2.24v2H7v-2l2-2.24ZM8 2h8",
  trash: "M3 6h18M8 6V4h8v2m3 0-1 14H6L5 6",
  copy: "M8 8h12v12H8zM4 16V4h12",
  eye: "M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12Zm10 3a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z",
  eyeOff: "M3 3l18 18M10.6 10.6a3 3 0 0 0 4.2 4.2M9.9 5.1A10 10 0 0 1 12 5c6.5 0 10 7 10 7a17 17 0 0 1-3.2 4.2M6.1 6.1A17 17 0 0 0 2 12s3.5 7 10 7a9.7 9.7 0 0 0 5.4-1.6",
  lock: "M5 11h14v10H5zM8 11V7a4 4 0 0 1 8 0v4",
  unlock: "M5 11h14v10H5zM8 11V7a4 4 0 0 1 7.8-1.3",
  fingerprint: "M12 11v3m-4-3a4 4 0 0 1 8 0v2a8 8 0 0 1-1 4M6.5 7.5A7 7 0 0 1 19 11v2M5 12v1a11 11 0 0 0 1.3 5M9 21a12 12 0 0 1-1-5v-5m8 9.5a14 14 0 0 0 .9-2.5",
  plus: "M12 5v14M5 12h14",
  gear: "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Zm7.4-3a7.4 7.4 0 0 0-.1-1.3l2-1.6-2-3.4-2.4 1a7.3 7.3 0 0 0-2.2-1.3L14.4 3h-4l-.4 2.5a7.3 7.3 0 0 0-2.2 1.3l-2.4-1-2 3.4 2 1.6a7.6 7.6 0 0 0 0 2.6l-2 1.6 2 3.4 2.4-1a7.3 7.3 0 0 0 2.2 1.3l.4 2.5h4l.4-2.5a7.3 7.3 0 0 0 2.2-1.3l2.4 1 2-3.4-2-1.6c.1-.4.1-.9.1-1.3Z",
  grip: "M9 5h.01M9 12h.01M9 19h.01M15 5h.01M15 12h.01M15 19h.01",
  key: "M15.5 7.5a3.5 3.5 0 1 1-7 0 3.5 3.5 0 0 1 7 0ZM12 11v10m0-4h3m-3 3h2",
  card: "M2 6h20v12H2zM2 10h20M6 15h4",
  wifi: "M5 12.5a10 10 0 0 1 14 0M8.5 16a5 5 0 0 1 7 0M2 9a15 15 0 0 1 20 0M12 20h.01",
  note: "M6 2h9l5 5v15H6zM14 2v6h6M9 13h7M9 17h5",
  globe: "M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20ZM2 12h20M12 2a15 15 0 0 1 0 20 15 15 0 0 1 0-20",
  star: "m12 3 2.8 5.7 6.2.9-4.5 4.4 1 6.2L12 17.3 6.5 20.2l1-6.2L3 9.6l6.2-.9Z",
  edit: "M4 20h4L19 9l-4-4L4 16v4Zm9-13 4 4",
  back: "M15 18l-6-6 6-6",
  chevron: "M9 18l6-6-6-6",
  image: "M3 5h18v14H3zM3 16l5-5 4 4 3-3 6 6M15.5 9.5h.01",
  file: "M6 2h9l5 5v15H6zM14 2v6h6",
  clipboard: "M9 4h6v3H9zM9 5H6v16h12V5h-3",
  shield: "M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10Z",
  pause: "M8 5v14M16 5v14",
  play: "M7 4l13 8-13 8z",
  check: "M5 12l5 5L20 7",
  x: "M6 6l12 12M18 6 6 18",
  dice: "M4 4h16v16H4zM8.5 8.5h.01M15.5 8.5h.01M12 12h.01M8.5 15.5h.01M15.5 15.5h.01",
  user: "M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8Zm-8 9a8 8 0 0 1 16 0",
  clock: "M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20Zm0-15v5l3 2",
  paste: "M9 4h6v3H9zM15 5h3v6M9 5H6v16h6M14 14h7v7h-7z",
  terminal: "M4 17l6-5-6-5M12 19h8",
  more: "M5 12h.01M12 12h.01M19 12h.01",
};

export function Icon({ name, size = 16, className = "", strokeWidth = 1.8 }: {
  name: keyof typeof PATHS | string;
  size?: number;
  className?: string;
  strokeWidth?: number;
}) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden
    >
      <path d={PATHS[name] ?? ""} />
    </svg>
  );
}
