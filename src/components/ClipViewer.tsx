import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useRef } from "react";
import { absoluteTime, relativeTime } from "../lib/format";
import { isMod, keys } from "../lib/shortcut";
import { clipTitle, RowData } from "./ClipRow";
import { Icon } from "./Icon";
import { Button, IconButton } from "./ui";

const MAX_TEXT = 200_000;

/** Full view of one clip: all of its text, the whole image, or every file path. */
export function ClipViewer({
  d,
  onUse,
  onSave,
  onClose,
}: {
  d: RowData;
  onUse: (paste: boolean) => void;
  onSave?: () => void;
  onClose: () => void;
}) {
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => root.current?.focus(), []);

  const meta = [d.sourceApp, d.time ? `${relativeTime(d.time)} (${absoluteTime(d.time)})` : null].filter(Boolean).join(" · ");
  const long = d.kind === "text" && d.content.length > MAX_TEXT;

  return (
    <div
      ref={root}
      tabIndex={-1}
      className="absolute inset-0 z-40 flex flex-col bg-black/20 p-2.5 outline-none"
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === "Escape") return e.preventDefault(), onClose();
        if (e.key === "Enter") return e.preventDefault(), onUse(!isMod(e));
      }}
    >
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border border-black/10 bg-white shadow-xl dark:border-white/10 dark:bg-neutral-800">
        <div className="flex items-start gap-2 border-b border-black/5 px-3 py-2 dark:border-white/10">
          <div className="min-w-0 flex-1">
            <div className="truncate text-[1rem] font-semibold">{clipTitle(d)}</div>
            {meta && <div className="truncate text-[0.8rem] text-neutral-500">{meta}</div>}
          </div>
          <IconButton icon="x" title="Close (Esc)" onClick={onClose} />
        </div>

        <div className="min-h-0 flex-1 overflow-auto p-3">
          {d.kind === "image" && d.imagePath ? (
            <div className="flex h-full flex-col items-center justify-center gap-2">
              <img src={convertFileSrc(d.imagePath)} className="max-h-full max-w-full rounded object-contain" alt="" />
              <div className="text-[0.8rem] text-neutral-500">
                {d.width ?? "?"}×{d.height ?? "?"}
              </div>
            </div>
          ) : d.kind === "files" ? (
            <ul className="space-y-1 font-mono text-[0.85rem] [user-select:text]">
              {d.content.split("\n").map((p) => (
                <li key={p} className="flex items-start gap-2 break-all">
                  <Icon name="file" size={14} className="mt-0.5 shrink-0 text-neutral-400" />
                  {p}
                </li>
              ))}
            </ul>
          ) : (
            <>
              <pre className="whitespace-pre-wrap break-words font-mono text-[0.85rem] text-neutral-700 dark:text-neutral-200">
                {long ? d.content.slice(0, MAX_TEXT) : d.content}
              </pre>
              {long && (
                <div className="mt-2 text-[0.8rem] text-neutral-500">
                  Showing the first {MAX_TEXT.toLocaleString()} of {d.content.length.toLocaleString()} characters. Paste or Copy uses all of it.
                </div>
              )}
            </>
          )}
        </div>

        <div className="flex items-center gap-2 border-t border-black/5 px-3 py-2 dark:border-white/10">
          {onSave && (
            <Button onClick={onSave}>
              <Icon name="download" size={13} /> Save…
            </Button>
          )}
          <div className="flex-1" />
          <Button onClick={() => onUse(false)}>
            Copy <span className="opacity-60">{keys("⌘↩")}</span>
          </Button>
          <Button variant="primary" onClick={() => onUse(true)}>
            Paste <span className="opacity-70">↩</span>
          </Button>
        </div>
      </div>
    </div>
  );
}
