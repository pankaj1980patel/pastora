import { useCallback, useEffect, useState } from "react";
import { api, Clip, Settings } from "../lib/api";
import { useEvent } from "../lib/hooks";
import { showContextMenu } from "../lib/contextMenu";
import { isMod, keys } from "../lib/shortcut";
import { useListNav } from "../lib/useListNav";
import { ClipRow, Empty, imageFileName, Preview } from "./ClipRow";
import { ClipViewer } from "./ClipViewer";
import { IconButton } from "./ui";

interface Props {
  query: string;
  settings: Settings;
  active: boolean;
  onToast: (m: string) => void;
  onMoveToSecure: (clip: Clip) => void;
  registerKeys: (h: (e: KeyboardEvent) => boolean) => void;
  setModal: (open: boolean) => void;
}

const PAGE = 100;

export function HistoryList({ query, settings, active, onToast, onMoveToSecure, registerKeys, setModal }: Props) {
  const [items, setItems] = useState<Clip[]>([]);
  const [hasMore, setHasMore] = useState(false);
  const { selected, setSelected, move, listRef } = useListNav(items.length, query);
  const [viewing, setViewing] = useState<Clip | null>(null);
  useEffect(() => setModal(viewing !== null), [viewing]);

  const load = useCallback(async () => {
    const rows = await api.listHistory(query, PAGE, 0);
    setItems(rows);
    setHasMore(rows.length === PAGE);
  }, [query]);

  const loadMore = async () => {
    const rows = await api.listHistory(query, PAGE, items.length);
    setItems((prev) => [...prev, ...rows]);
    setHasMore(rows.length === PAGE);
  };

  useEffect(() => {
    load();
  }, [load]);
  useEvent("history-changed", load);
  useEvent("popup-shown", () => {
    load();
    setSelected(0);
  });

  const use = (c: Clip, paste: boolean) => api.useItem("history", c.id, paste).catch((e) => onToast(String(e)));
  const togglePin = async (c: Clip) => {
    if (c.pinned) await api.unpinClip(c.id);
    else await api.pinClip(c.id);
    onToast(c.pinned ? "Unpinned" : "Pinned");
  };
  const remove = (c: Clip) => api.deleteClip(c.id);
  const saveImage = (c: Clip) =>
    api.saveImage("history", c.id, imageFileName({ ...c, time: c.lastUsedAt })).catch((e) => onToast(String(e)));

  useEffect(() => {
    if (!active) return;
    registerKeys((e) => {
      const cur = items[selected];
      if (e.key === "ArrowDown") return move(1), true;
      if (e.key === "ArrowUp") return move(-1), true;
      if (e.key === "PageDown") return move(8), true;
      if (e.key === "PageUp") return move(-8), true;
      if (isMod(e) && /^[1-9]$/.test(e.key)) {
        const c = items[Number(e.key) - 1];
        if (c) use(c, true);
        return true;
      }
      if (!cur) return false;
      if (isMod(e) && e.key.toLowerCase() === "y") return setViewing(cur), true;
      if (e.key === "Enter") return use(cur, !isMod(e)), true;
      if (isMod(e) && e.key.toLowerCase() === "c" && !window.getSelection()?.toString()) return use(cur, false), true;
      if (isMod(e) && e.key.toLowerCase() === "p") return togglePin(cur), true;
      if (isMod(e) && e.key.toLowerCase() === "s") return onMoveToSecure(cur), true;
      if (isMod(e) && (e.key === "Backspace" || e.key === "Delete")) return remove(cur), true;
      return false;
    });
  });

  if (!items.length) {
    return query ? (
      <Empty icon="search" title="No matches" hint={`Nothing in history matches “${query}”.`} />
    ) : (
      <Empty icon="clipboard" title="Nothing copied yet" hint="Copy some text, an image or a file and it will show up here." />
    );
  }

  const cur = items[selected];
  return (
    <>
      <div ref={listRef} className="flex-1 overflow-y-auto px-1.5 py-1">
        {items.map((c, i) => (
          <ClipRow
            key={c.id}
            d={{ ...c, time: c.lastUsedAt }}
            index={i}
            selected={i === selected}
            showSource={settings.showSourceApp}
            relative={settings.relativeTime}
            onClick={() => use(c, true)}
            onSelect={() => setSelected(i)}
            onContextMenu={() =>
              showContextMenu([
                { text: "View", accelerator: "CmdOrCtrl+Y", action: () => setViewing(c) },
                "-",
                { text: "Paste", accelerator: "Enter", action: () => use(c, true) },
                { text: "Copy", accelerator: "CmdOrCtrl+Enter", action: () => use(c, false) },
                ...(c.kind === "text"
                  ? [
                      { text: "Paste as Plain Text (trimmed)", action: () => api.useItem("history", c.id, true, c.content.trim()) },
                      { text: "Paste in UPPERCASE", action: () => api.useItem("history", c.id, true, c.content.toUpperCase()) },
                      { text: "Paste in lowercase", action: () => api.useItem("history", c.id, true, c.content.toLowerCase()) },
                    ]
                  : []),
                ...(c.kind === "image" ? ["-" as const, { text: "Save Image…", action: () => saveImage(c) }] : []),
                "-",
                { text: c.pinned ? "Unpin" : "Pin", accelerator: "CmdOrCtrl+P", action: () => togglePin(c) },
                ...(c.kind === "text" ? [{ text: "Move to Secure Clips…", accelerator: "CmdOrCtrl+S", action: () => onMoveToSecure(c) }] : []),
                "-",
                { text: "Delete", accelerator: "CmdOrCtrl+Backspace", action: () => remove(c) },
                { text: "Clear All History…", action: () => api.openSettings("history") },
              ])
            }
            actions={
              <>
                <IconButton icon="eye" title={keys("View (⌘Y)")} onClick={() => setViewing(c)} />
                {c.kind === "image" && <IconButton icon="download" title="Save Image…" onClick={() => saveImage(c)} />}
                <IconButton icon="pin" title={c.pinned ? keys("Unpin (⌘P)") : keys("Pin (⌘P)")} active={c.pinned} onClick={() => togglePin(c)} />
                {c.kind === "text" && (
                  <IconButton icon="shield" title={keys("Move to Secure Clips (⌘S)")} onClick={() => onMoveToSecure(c)} />
                )}
                <IconButton icon="trash" title={keys("Delete (⌘⌫)")} onClick={() => remove(c)} />
              </>
            }
          />
        ))}
        {hasMore && (
          <button onClick={loadMore} className="w-full py-2 text-[0.85rem] text-indigo-500 hover:underline">
            Load more
          </button>
        )}
      </div>
      {settings.showPreview && <Preview d={cur ?? null} onSave={cur?.kind === "image" ? () => saveImage(cur) : undefined} />}
      {viewing && (
        <ClipViewer
          d={{ ...viewing, time: viewing.lastUsedAt }}
          onUse={(paste) => {
            setViewing(null);
            use(viewing, paste);
          }}
          onSave={viewing.kind === "image" ? () => saveImage(viewing) : undefined}
          onClose={() => setViewing(null)}
        />
      )}
    </>
  );
}
