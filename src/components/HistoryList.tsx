import { useCallback, useEffect, useRef, useState } from "react";
import { api, Clip, errorText, Settings } from "../lib/api";
import { useDebounced, useEvent } from "../lib/hooks";
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
  /** False while the popup is hidden: background changes don't reload the list. */
  visible: boolean;
  onToast: (m: string) => void;
  onMoveToSecure: (clip: Clip) => void;
  registerKeys: (h: (e: KeyboardEvent) => boolean) => void;
  setModal: (open: boolean) => void;
}

const PAGE = 100;

export function HistoryList({ query, settings, active, visible, onToast, onMoveToSecure, registerKeys, setModal }: Props) {
  const [items, setItems] = useState<Clip[]>([]);
  const [hasMore, setHasMore] = useState(false);
  /** The query the shown results belong to (null until the first load). */
  const [loadedQuery, setLoadedQuery] = useState<string | null>(null);
  const q = useDebounced(query, 120, (v) => v === "");
  const { selected, setSelected, move, listRef, reset } = useListNav(items.length, query);
  const [viewing, setViewing] = useState<Clip | null>(null);
  useEffect(() => setModal(viewing !== null), [viewing]);

  const req = useRef(0); // only the newest request may update the list
  const itemsRef = useRef(items);
  itemsRef.current = items;
  const queryRef = useRef(query);
  queryRef.current = query;
  const loadingMore = useRef(false);
  /** Enter pressed before the results for the typed query arrived: paste (true) or copy. */
  const pendingUse = useRef<boolean | null>(null);

  const use = (c: Clip, paste: boolean) => api.useItem("history", c.id, paste).catch((e) => onToast(errorText(e)));

  /** `keepCount`: reload as many rows as are loaded now, so "Load more" pages survive a refresh. */
  const load = useCallback(
    async (keepCount = false) => {
      const id = ++req.current;
      const limit = keepCount ? Math.max(PAGE, itemsRef.current.length) : PAGE;
      try {
        const rows = await api.listHistory(q, limit, 0);
        if (id !== req.current) return;
        setItems(rows);
        setHasMore(rows.length === limit);
        setLoadedQuery(q);
        if (pendingUse.current !== null && q === queryRef.current) {
          const paste = pendingUse.current;
          pendingUse.current = null;
          if (rows[0]) use(rows[0], paste);
        }
      } catch (e) {
        if (id === req.current) onToast(errorText(e));
      }
    },
    [q],
  );

  const loadMore = async () => {
    if (loadingMore.current) return;
    loadingMore.current = true;
    const id = req.current;
    try {
      const rows = await api.listHistory(q, PAGE, itemsRef.current.length);
      if (id !== req.current) return; // the list was reloaded meanwhile
      setItems((prev) => {
        const seen = new Set(prev.map((c) => c.id));
        return [...prev, ...rows.filter((c) => !seen.has(c.id))];
      });
      setHasMore(rows.length === PAGE);
    } catch (e) {
      onToast(errorText(e));
    } finally {
      loadingMore.current = false;
    }
  };

  useEffect(() => {
    load();
  }, [load]);
  useEvent("history-changed", () => {
    if (visible) load(true);
  });
  useEvent("popup-shown", () => {
    pendingUse.current = null;
    reset();
    // A non-empty query is cleared by the popup, which reloads through `load` changing.
    if (!query) load();
  });

  /** Long text is only previewed in the list; fetch all of it before using the text itself. */
  const full = async (c: Clip) => (c.truncated ? api.getClip(c.id) : c);
  const pasteTransformed = (c: Clip, f: (s: string) => string) =>
    full(c)
      .then((x) => api.useItem("history", c.id, true, f(x.content)))
      .catch((e) => onToast(errorText(e)));
  const view = (c: Clip) => full(c).then(setViewing).catch((e) => onToast(errorText(e)));
  const moveToSecure = (c: Clip) => {
    if (c.kind === "text") full(c).then(onMoveToSecure).catch((e) => onToast(errorText(e)));
  };
  const togglePin = async (c: Clip) => {
    try {
      if (c.pinned) await api.unpinClip(c.id);
      else await api.pinClip(c.id);
      onToast(c.pinned ? "Unpinned" : "Pinned");
    } catch (e) {
      onToast(errorText(e));
    }
  };
  const remove = (c: Clip) => api.deleteClip(c.id).catch((e) => onToast(errorText(e)));
  const saveImage = (c: Clip) =>
    api.saveImage("history", c.id, imageFileName({ ...c, time: c.lastUsedAt })).catch((e) => onToast(errorText(e)));

  useEffect(() => {
    if (!active) return;
    // Results still belong to an earlier query (typing, debounce, request in flight).
    const stale = loadedQuery !== query;
    registerKeys((e) => {
      const cur = items[selected];
      if (e.key === "ArrowDown") return move(1), true;
      if (e.key === "ArrowUp") return move(-1), true;
      if (e.key === "PageDown") return move(8), true;
      if (e.key === "PageUp") return move(-8), true;
      if (isMod(e) && /^[1-9]$/.test(e.key)) {
        const c = items[Number(e.key) - 1];
        if (c && !stale) use(c, true);
        return true;
      }
      // Enter right after typing: use the best match once the new results are in.
      if (e.key === "Enter" && stale) return (pendingUse.current = !isMod(e)), true;
      if (!cur) return false;
      if (isMod(e) && e.key.toLowerCase() === "y") return view(cur), true;
      if (e.key === "Enter") return use(cur, !isMod(e)), true;
      if (isMod(e) && e.key.toLowerCase() === "c" && !window.getSelection()?.toString()) return use(cur, false), true;
      if (isMod(e) && e.key.toLowerCase() === "p") return togglePin(cur), true;
      if (isMod(e) && e.key.toLowerCase() === "s") return moveToSecure(cur), true;
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
                { text: "View", accelerator: "CmdOrCtrl+Y", action: () => view(c) },
                "-",
                { text: "Paste", accelerator: "Enter", action: () => use(c, true) },
                { text: "Copy", accelerator: "CmdOrCtrl+Enter", action: () => use(c, false) },
                ...(c.kind === "text"
                  ? [
                      { text: "Paste as Plain Text (trimmed)", action: () => pasteTransformed(c, (s) => s.trim()) },
                      { text: "Paste in UPPERCASE", action: () => pasteTransformed(c, (s) => s.toUpperCase()) },
                      { text: "Paste in lowercase", action: () => pasteTransformed(c, (s) => s.toLowerCase()) },
                    ]
                  : []),
                ...(c.kind === "image" ? ["-" as const, { text: "Save Image…", action: () => saveImage(c) }] : []),
                "-",
                { text: c.pinned ? "Unpin" : "Pin", accelerator: "CmdOrCtrl+P", action: () => togglePin(c) },
                ...(c.kind === "text" ? [{ text: "Move to Secure Clips…", accelerator: "CmdOrCtrl+S", action: () => moveToSecure(c) }] : []),
                "-",
                { text: "Delete", accelerator: "CmdOrCtrl+Backspace", action: () => remove(c) },
                { text: "Clear All History…", action: () => api.openSettings("history") },
              ])
            }
            actions={
              <>
                <IconButton icon="eye" title={keys("View (⌘Y)")} onClick={() => view(c)} />
                {c.kind === "image" && <IconButton icon="download" title="Save Image…" onClick={() => saveImage(c)} />}
                <IconButton icon="pin" title={c.pinned ? keys("Unpin (⌘P)") : keys("Pin (⌘P)")} active={c.pinned} onClick={() => togglePin(c)} />
                {c.kind === "text" && (
                  <IconButton icon="shield" title={keys("Move to Secure Clips (⌘S)")} onClick={() => moveToSecure(c)} />
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
