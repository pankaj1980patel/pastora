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
  if (d.kind === "image") return `Image ${d.width ?? "?"}×${d.height ?? "?"}`;
  if (d.kind === "files") {
    const files = d.content.split("\n");
    return files.length === 1 ? fileName(files[0]) : `${files.length} files`;
  }
  return firstLine(d.content) || "(whitespace)";
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
  onClick: () => void;
  onDoubleClick: () => void;
  onContextMenu?: (e: React.MouseEvent) => void;
  actions?: ReactNode;
  leading?: ReactNode;
  style?: React.CSSProperties;
  dragging?: boolean;
}

export const ClipRow = forwardRef<HTMLDivElement, Props>(function ClipRow(
  { d, selected, index, showSource, relative, onClick, onDoubleClick, onContextMenu, actions, leading, style, dragging },
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
      onDoubleClick={onDoubleClick}
      onContextMenu={(e) => {
        e.preventDefault();
        onClick();
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

export function Preview({ d }: { d: RowData | null }) {
  if (!d) return null;
  return (
    <div className="max-h-36 shrink-0 overflow-auto border-t border-black/5 px-3 py-2 dark:border-white/10">
      {d.kind === "image" && d.imagePath ? (
        <img src={convertFileSrc(d.imagePath)} className="mx-auto max-h-32 rounded object-contain" />
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
