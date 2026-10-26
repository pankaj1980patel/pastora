import { useCallback, useEffect, useState } from "react";
import { api, Clip, EntrySummary, errorText, FieldSummary, VaultStatus } from "../../lib/api";
import { useEvent } from "../../lib/hooks";
import { showContextMenu } from "../../lib/contextMenu";
import { isMod } from "../../lib/shortcut";
import { useListNav } from "../../lib/useListNav";
import { Empty } from "../ClipRow";
import { Icon } from "../Icon";
import { IconButton } from "../ui";
import { EntryDetail } from "./EntryDetail";
import { EntryEditor } from "./EntryEditor";
import { matches, primaryField, template, totpField, userField } from "./templates";
import { LockScreen, SetupVault } from "./Unlock";

interface Props {
  query: string;
  active: boolean;
  shownAt: number;
  pending: Clip | null;
  clearPending: () => void;
  onToast: (m: string, seconds?: number) => void;
  registerKeys: (h: (e: KeyboardEvent) => boolean) => void;
  setModal: (open: boolean) => void;
  onLockedChange: (locked: boolean) => void;
}

type View = { kind: "list" } | { kind: "detail"; id: string } | { kind: "edit"; id: string | null; prefill?: string };

export function SecurePanel({ query, active, shownAt, pending, clearPending, onToast, registerKeys, setModal, onLockedChange }: Props) {
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const [entries, setEntries] = useState<EntrySummary[]>([]);
  const [view, setView] = useState<View>({ kind: "list" });
  const visible = entries.filter((e) => matches(e, query.trim()));
  const { selected, setSelected, move, listRef } = useListNav(visible.length, query);

  const refresh = useCallback(async () => {
    const s = await api.vaultStatus();
    setStatus(s);
    onLockedChange(!s.unlocked);
    if (s.unlocked) setEntries(await api.vaultList());
    else {
      setEntries([]);
      setView({ kind: "list" });
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);
  useEvent("vault-changed", refresh);
  useEvent("settings-changed", refresh);
  useEffect(() => {
    if (active) refresh();
  }, [active, shownAt]);

  // "Move to Secure Clips" from history: open the editor once unlocked.
  useEffect(() => {
    if (pending && status?.unlocked) setView({ kind: "edit", id: null, prefill: pending.content });
  }, [pending, status?.unlocked]);

  useEffect(() => setModal(view.kind === "edit"), [view.kind]);

  const copy = async (entry: EntrySummary, field: FieldSummary | undefined, paste: boolean) => {
    if (!field) return;
    try {
      const r = await api.vaultCopyField(entry.id, field.id, paste);
      if (!paste) onToast(`${field.label} copied${r.clearSeconds ? `, clears in ${r.clearSeconds}s` : ""}`, r.clearSeconds || undefined);
    } catch (e) {
      onToast(errorText(e));
    }
  };

  const pasteLogin = async (entry: EntrySummary) => {
    const u = userField(entry.fields);
    const p = primaryField(entry.fields);
    if (!u || !p) return;
    await api.vaultPasteLogin(entry.id, u.id, p.id).catch((e) => onToast(errorText(e)));
  };

  const entryMenu = (e: EntrySummary) => {
    const p = primaryField(e.fields);
    const u = userField(e.fields);
    const t = totpField(e.fields);
    showContextMenu([
      ...(p ? [{ text: `Paste ${p.label}`, accelerator: "Enter", action: () => copy(e, p, true) }] : []),
      ...(u ? [{ text: `Paste ${u.label}`, accelerator: "CmdOrCtrl+U", action: () => copy(e, u, true) }] : []),
      ...(t ? [{ text: "Paste One-time Code", accelerator: "CmdOrCtrl+T", action: () => copy(e, t, true) }] : []),
      ...(u && p ? [{ text: `Paste ${u.label}, then ${p.label}`, accelerator: "CmdOrCtrl+L", action: () => pasteLogin(e) }] : []),
      "-",
      ...e.fields.filter((f) => f.hasValue).map((f) => ({ text: `Copy ${f.label}`, action: () => copy(e, f, false) })),
      "-",
      { text: "Show Details", accelerator: "Right", action: () => setView({ kind: "detail", id: e.id }) },
      { text: "Edit…", accelerator: "CmdOrCtrl+E", action: () => setView({ kind: "edit", id: e.id }) },
      { text: e.favorite ? "Remove from Favorites" : "Add to Favorites", action: () => api.vaultSetFavorite(e.id, !e.favorite) },
      "-",
      { text: "New Secure Clip…", accelerator: "CmdOrCtrl+N", action: () => setView({ kind: "edit", id: null }) },
      { text: "Lock Secure Clips", action: () => api.vaultLock() },
    ]);
  };

  const current: EntrySummary | undefined =
    view.kind === "detail" ? entries.find((e) => e.id === view.id) : visible[selected];

  useEffect(() => {
    if (!active || !status?.unlocked || view.kind === "edit") return;
    registerKeys((e) => {
      const k = e.key.toLowerCase();
      if (isMod(e) && k === "n") return setView({ kind: "edit", id: null }), true;
      if (view.kind === "list") {
        if (e.key === "ArrowDown") return move(1), true;
        if (e.key === "ArrowUp") return move(-1), true;
        if (e.key === "ArrowRight" && current) return setView({ kind: "detail", id: current.id }), true;
      } else if (e.key === "Escape" || (e.key === "ArrowLeft" && !query)) {
        return setView({ kind: "list" }), true;
      }
      if (!current) return false;
      if (e.key === "Enter") return copy(current, primaryField(current.fields), !isMod(e)), true;
      if (isMod(e) && k === "u") return copy(current, userField(current.fields), true), true;
      if (isMod(e) && k === "t") return copy(current, totpField(current.fields), true), true;
      if (isMod(e) && k === "l") return pasteLogin(current), true;
      if (isMod(e) && k === "e") return setView({ kind: "edit", id: current.id }), true;
      return false;
    });
  });

  if (!status) return null;
  if (!status.initialized) return <SetupVault status={status} onDone={refresh} />;
  if (!status.unlocked) return <LockScreen status={status} onUnlocked={refresh} shownAt={shownAt} />;

  if (view.kind === "edit") {
    return (
      <EntryEditor
        key={view.id ?? "new"}
        entryId={view.id}
        prefill={view.prefill}
        onClose={() => {
          if (view.prefill) clearPending();
          setView(view.id ? { kind: "detail", id: view.id } : { kind: "list" });
        }}
        onSaved={async (id) => {
          if (view.prefill && pending) {
            await api.deleteClip(pending.id); // it now lives (encrypted) in Secure Clips
            clearPending();
          }
          await refresh();
          setView({ kind: "detail", id });
          onToast("Saved to Secure Clips");
        }}
        onDeleted={async () => {
          await refresh();
          setView({ kind: "list" });
          onToast("Deleted");
        }}
      />
    );
  }

  if (view.kind === "detail" && current) {
    return (
      <EntryDetail
        entry={current}
        onCopy={(f, paste) => copy(current, f, paste)}
        onBack={() => setView({ kind: "list" })}
        onEdit={() => setView({ kind: "edit", id: current.id })}
        onToast={onToast}
      />
    );
  }

  return (
    <>
      {visible.length === 0 ? (
        query ? (
          <Empty icon="search" title="No matches" />
        ) : (
          <Empty
            icon="shield"
            title="No secure clips yet"
            hint={
              <>
                Store logins, API keys, cards and notes, encrypted.{" "}
                <button className="text-indigo-500 hover:underline" onClick={() => setView({ kind: "edit", id: null })}>
                  Add one
                </button>{" "}
                (⌘N)
              </>
            }
          />
        )
      ) : (
        <div ref={listRef} className="flex-1 overflow-y-auto px-1.5 py-1">
          {visible.map((e, i) => {
            const t = template(e.template);
            return (
              <div
                key={e.id}
                data-selected={i === selected}
                onClick={() => setSelected(i)}
                onDoubleClick={() => setView({ kind: "detail", id: e.id })}
                onContextMenu={(ev) => {
                  ev.preventDefault();
                  setSelected(i);
                  entryMenu(e);
                }}
                className={`group flex cursor-default select-none items-center gap-2.5 rounded-md px-2 py-1 ${
                  i === selected ? "bg-indigo-500/15 dark:bg-indigo-400/20" : "hover:bg-black/5 dark:hover:bg-white/5"
                }`}
              >
                <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-indigo-500/15 text-indigo-500">
                  <Icon name={t.icon} size={14} />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-1 truncate text-[1rem] leading-tight">
                    {e.title}
                    {e.favorite && <Icon name="star" size={11} className="shrink-0 text-amber-400" />}
                  </div>
                  <div className="truncate text-[0.8rem] leading-tight text-neutral-500">{e.subtitle || t.name}</div>
                </div>
                <div className={`flex items-center gap-0.5 ${i === selected ? "" : "opacity-0 group-hover:opacity-100"}`}>
                  {userField(e.fields) && <IconButton icon="user" title="Copy username (⌘U)" onClick={() => copy(e, userField(e.fields), false)} />}
                  {primaryField(e.fields) && <IconButton icon="key" title="Copy secret (↩ pastes)" onClick={() => copy(e, primaryField(e.fields), false)} />}
                  <IconButton icon="chevron" title="Details (→)" onClick={() => setView({ kind: "detail", id: e.id })} />
                </div>
              </div>
            );
          })}
        </div>
      )}
      <div className="flex items-center gap-2 border-t border-black/5 px-2 py-1 dark:border-white/10">
        <IconButton icon="plus" title="New secure clip (⌘N)" onClick={() => setView({ kind: "edit", id: null })} />
        <div className="flex-1 text-[0.75rem] text-neutral-500">↩ paste secret · ⌘U username · ⌘L both · → details</div>
        <IconButton icon="lock" title="Lock now" onClick={() => api.vaultLock()} />
      </div>
    </>
  );
}
