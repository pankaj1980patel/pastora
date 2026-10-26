import { convertFileSrc } from "@tauri-apps/api/core";
import { forwardRef, ReactNode } from "react";
import { Kind } from "../lib/api";
import { fileName, firstLine, relativeTime, absoluteTime } from "../lib/format";
import { Icon } from "./Icon";

export interface RowData {
  kind: Kind;
  content: string;
  imagePath: string | null;
  width: number | null;
  height: number | null;
  label?: string | null;
  sourceApp?: string | null;
  time?: number;
}

export function clipTitle(d: RowData): string {
  if (d.label) return d.label;
  if (d.kind === "image") {
    const size = `${d.width ?? "?"}×${d.height ?? "?"}`;
    // Copied image files keep their file name; clipboard images are stored as "<sha256>.png".
    return HASH_NAME.test(d.content) ? `Image ${size}` : `${d.content} · ${size}`;
  }
  if (d.kind === "files") {
    const files = d.content.split("\n");
    return files.length === 1 ? fileName(files[0]) : `${files.length} files`;
  }
  return firstLine(d.content) || "(whitespace)";
}

const HASH_NAME = /^[0-9a-f]{64}\.\w+$/;

/** Default file name when saving an image clip: its original name, or a dated one. */
export function imageFileName(d: RowData): string {
  if (d.content && !HASH_NAME.test(d.content)) return d.content;
  const ext = d.imagePath?.split(".").pop() ?? "png";
  const t = new Date(d.time ?? Date.now());
  const p = (n: number) => String(n).padStart(2, "0");
  return `Pastora Image ${t.getFullYear()}-${p(t.getMonth() + 1)}-${p(t.getDate())} at ${p(t.getHours())}.${p(t.getMinutes())}.${p(t.getSeconds())}.${ext}`;
}

export function ClipThumb({ d, selected }: { d: RowData; selected?: boolean }) {
  if (d.kind === "image" && d.imagePath) {
    return (
      <img
        src={convertFileSrc(d.imagePath)}
        className="h-6 w-6 shrink-0 rounded border border-black/10 object-cover dark:border-white/10"
        loading="lazy"
      />
    );
  }
  const icon = d.kind === "files" ? "file" : /^https?:\/\//.test(d.content.trim()) ? "globe" : "clipboard";
  return <Icon name={icon} size={15} className={`shrink-0 ${selected ? "text-white/80" : "text-neutral-400"}`} />;
}

interface Props {
  d: RowData;
  selected: boolean;
  index: number;
  showSource: boolean;
  relative: boolean;
  /** Single click: use the item (paste, or copy when auto-paste is off). */
  onClick: () => void;
  /** Selects the row without using it (right-click). */
  onSelect: () => void;
  onContextMenu?: (e: React.MouseEvent) => void;
  actions?: ReactNode;
  leading?: ReactNode;
  style?: React.CSSProperties;
  dragging?: boolean;
}

export const ClipRow = forwardRef<HTMLDivElement, Props>(function ClipRow(
  { d, selected, index, showSource, relative, onClick, onSelect, onContextMenu, actions, leading, style, dragging },
  ref,
) {
  const time = d.time ? (relative ? relativeTime(d.time) : absoluteTime(d.time)) : null;
  const tooltip = [d.label && d.kind === "text" ? firstLine(d.content, 120) : null, d.sourceApp, time].filter(Boolean).join(" · ");
  return (
    <div
      ref={ref}
      style={style}
      title={tooltip || undefined}
      data-selected={selected}
      onClick={onClick}
      onContextMenu={(e) => {
        e.preventDefault();
        onSelect();
        onContextMenu?.(e);
      }}
      className={`group relative flex h-[2.15rem] cursor-default select-none items-center gap-2 rounded-md px-2 ${
        selected ? "bg-indigo-500 text-white" : "hover:bg-black/5 dark:hover:bg-white/5"
      } ${dragging ? "z-10 bg-white shadow-lg dark:bg-neutral-800" : ""}`}
    >
      {leading}
      <ClipThumb d={d} selected={selected} />
      <div className={`min-w-0 flex-1 truncate text-[1rem] ${d.label ? "font-medium" : ""}`}>{clipTitle(d)}</div>
      {showSource && d.sourceApp && (
        <span className={`max-w-[6rem] shrink-0 truncate text-[0.75rem] ${selected ? "text-white/70" : "text-neutral-400"} group-hover:hidden`}>
          {d.sourceApp}
        </span>
      )}
      {time && (
        <span className={`shrink-0 text-[0.75rem] ${selected ? "text-white/70" : "text-neutral-400"} group-hover:hidden`}>{time}</span>
      )}
      <div className={`hidden items-center gap-0.5 group-hover:flex ${selected ? "[&_button]:text-white" : ""}`}>{actions}</div>
      {index < 9 && (
        <span className={`w-6 shrink-0 text-right text-[0.75rem] ${selected ? "text-white/70" : "text-neutral-400"}`}>⌘{index + 1}</span>
      )}
    </div>
  );
});

export function Preview({ d, onSave }: { d: RowData | null; onSave?: () => void }) {
  if (!d) return null;
  return (
    <div className="relative max-h-36 shrink-0 overflow-auto border-t border-black/5 px-3 py-2 dark:border-white/10">
      {d.kind === "image" && d.imagePath ? (
        <>
          <img src={convertFileSrc(d.imagePath)} className="mx-auto max-h-32 rounded object-contain" />
          {onSave && (
            <button
              type="button"
              onClick={onSave}
              title="Save Image…"
              className="absolute right-2 top-2 flex items-center gap-1 rounded-md bg-black/55 px-2 py-1 text-[0.75rem] text-white backdrop-blur hover:bg-black/70"
            >
              <Icon name="download" size={13} /> Save…
            </button>
          )}
        </>
      ) : (
        <pre className="whitespace-pre-wrap break-words font-mono text-[0.8rem] text-neutral-600 dark:text-neutral-300">
          {d.content.length > 4000 ? d.content.slice(0, 4000) + "…" : d.content}
        </pre>
      )}
    </div>
  );
}

export function Empty({ icon, title, hint }: { icon: string; title: string; hint?: ReactNode }) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-2 p-6 text-center text-neutral-500 dark:text-neutral-400">
      <Icon name={icon} size={28} strokeWidth={1.4} />
      <div className="text-[1rem] font-medium text-neutral-700 dark:text-neutral-200">{title}</div>
      {hint && <div className="max-w-[18rem] text-[0.85rem]">{hint}</div>}
    </div>
  );
}
