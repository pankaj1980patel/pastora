import {
  closestCenter,
  DndContext,
  DragEndEvent,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import { restrictToVerticalAxis, restrictToParentElement } from "@dnd-kit/modifiers";
import { arrayMove, SortableContext, sortableKeyboardCoordinates, useSortable, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { useCallback, useEffect, useState } from "react";
import { api, Pin, Settings } from "../lib/api";
import { useEvent } from "../lib/hooks";
import { showContextMenu } from "../lib/contextMenu";
import { isMod } from "../lib/shortcut";
import { useListNav } from "../lib/useListNav";
import { ClipRow, Empty, Preview } from "./ClipRow";
import { Icon } from "./Icon";
import { Button, IconButton, inputClass } from "./ui";

interface Props {
  query: string;
  settings: Settings;
  active: boolean;
  onToast: (m: string) => void;
  registerKeys: (h: (e: KeyboardEvent) => boolean) => void;
  setModal: (open: boolean) => void;
}

function SortablePin(props: {
  pin: Pin;
  index: number;
  selected: boolean;
  draggable: boolean;
  settings: Settings;
  onSelect: () => void;
  onUse: () => void;
  onEdit: () => void;
  onDelete: () => void;
  onContextMenu: () => void;
}) {
  const { pin, draggable } = props;
  const { attributes, listeners, setNodeRef, setActivatorNodeRef, transform, transition, isDragging } = useSortable({
    id: pin.id,
    disabled: !draggable,
  });
  return (
    <ClipRow
      ref={setNodeRef}
      style={{ transform: CSS.Transform.toString(transform), transition }}
      dragging={isDragging}
      d={{ ...pin, time: undefined }}
      index={props.index}
      selected={props.selected}
      showSource={false}
      relative={props.settings.relativeTime}
      onClick={props.onSelect}
      onDoubleClick={props.onUse}
      onContextMenu={props.onContextMenu}
      leading={
        <button
          ref={setActivatorNodeRef}
          {...attributes}
          {...listeners}
          tabIndex={-1}
          title={draggable ? "Drag to reorder (⌥↑ / ⌥↓)" : "Clear the search to reorder"}
          className={`-ml-1 rounded p-0.5 text-neutral-400 ${draggable ? "cursor-grab hover:bg-black/10 active:cursor-grabbing dark:hover:bg-white/10" : "opacity-30"}`}
        >
          <Icon name="grip" size={14} strokeWidth={3} />
        </button>
      }
      actions={
        <>
          <IconButton icon="edit" title="Rename / edit (⌘E)" onClick={props.onEdit} />
          <IconButton icon="trash" title="Unpin (⌘⌫)" onClick={props.onDelete} />
        </>
      }
    />
  );
}

export function PinList({ query, settings, active, onToast, registerKeys, setModal }: Props) {
  const [pins, setPins] = useState<Pin[]>([]);
  const [editing, setEditing] = useState<Pin | "new" | null>(null);
  const { selected, setSelected, move, listRef } = useListNav(pins.length, query);
  const draggable = query.trim() === "";

  const load = useCallback(() => api.listPins(query).then(setPins), [query]);
  useEffect(() => {
    load();
  }, [load]);
  useEvent("pins-changed", load);
  useEvent("popup-shown", load);
  useEffect(() => setModal(editing !== null), [editing]);

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );

  const saveOrder = (next: Pin[]) => {
    setPins(next);
    api.reorderPins(next.map((p) => p.id));
  };

  const onDragEnd = (e: DragEndEvent) => {
    const { active: a, over } = e;
    if (!over || a.id === over.id) return;
    const from = pins.findIndex((p) => p.id === a.id);
    const to = pins.findIndex((p) => p.id === over.id);
    saveOrder(arrayMove(pins, from, to));
    setSelected(to);
  };

  const moveTo = (from: number, to: number) => {
    if (!draggable || to < 0 || to >= pins.length || from === to) return;
    saveOrder(arrayMove(pins, from, to));
    setSelected(to);
  };
  const moveSelected = (delta: number) => moveTo(selected, selected + delta);

  const use = (p: Pin, paste: boolean) => api.useItem("pin", p.id, paste).catch((e) => onToast(String(e)));
  const remove = async (p: Pin) => {
    await api.deletePin(p.id);
    onToast("Unpinned");
  };

  useEffect(() => {
    if (!active || editing) return;
    registerKeys((e) => {
      const cur = pins[selected];
      if (e.altKey && e.key === "ArrowDown") return moveSelected(1), true;
      if (e.altKey && e.key === "ArrowUp") return moveSelected(-1), true;
      if (e.key === "ArrowDown") return move(1), true;
      if (e.key === "ArrowUp") return move(-1), true;
      if (isMod(e) && e.key.toLowerCase() === "n") return setEditing("new"), true;
      if (isMod(e) && /^[1-9]$/.test(e.key)) {
        const p = pins[Number(e.key) - 1];
        if (p) use(p, true);
        return true;
      }
      if (!cur) return false;
      if (e.key === "Enter") return use(cur, !isMod(e)), true;
      if (isMod(e) && e.key.toLowerCase() === "e") return setEditing(cur), true;
      if (isMod(e) && (e.key === "Backspace" || e.key === "Delete")) return remove(cur), true;
      return false;
    });
  });

  return (
    <>
      {pins.length === 0 ? (
        query ? (
          <Empty icon="search" title="No matching pins" />
        ) : (
          <Empty
            icon="pin"
            title="No pins yet"
            hint={
              <>
                Press ⌘P on a history item to pin it, or{" "}
                <button className="text-indigo-500 hover:underline" onClick={() => setEditing("new")}>
                  add a pin
                </button>
                . Drag pins to put them in any order.
              </>
            }
          />
        )
      ) : (
        <div ref={listRef} className="flex-1 overflow-y-auto overflow-x-hidden px-1.5 py-1">
          <DndContext
            sensors={sensors}
            collisionDetection={closestCenter}
            modifiers={[restrictToVerticalAxis, restrictToParentElement]}
            onDragEnd={onDragEnd}
          >
            <SortableContext items={pins.map((p) => p.id)} strategy={verticalListSortingStrategy}>
              {pins.map((p, i) => (
                <SortablePin
                  key={p.id}
                  pin={p}
                  index={i}
                  selected={i === selected}
                  draggable={draggable}
                  settings={settings}
                  onSelect={() => setSelected(i)}
                  onUse={() => use(p, true)}
                  onEdit={() => setEditing(p)}
                  onDelete={() => remove(p)}
                  onContextMenu={() =>
                    showContextMenu([
                      { text: "Paste", accelerator: "Enter", action: () => use(p, true) },
                      { text: "Copy", accelerator: "CmdOrCtrl+Enter", action: () => use(p, false) },
                      "-",
                      { text: "Rename / Edit…", accelerator: "CmdOrCtrl+E", action: () => setEditing(p) },
                      { text: "Move to Top", enabled: draggable && i > 0, action: () => moveTo(i, 0) },
                      { text: "Move Up", accelerator: "Alt+Up", enabled: draggable && i > 0, action: () => moveTo(i, i - 1) },
                      { text: "Move Down", accelerator: "Alt+Down", enabled: draggable && i < pins.length - 1, action: () => moveTo(i, i + 1) },
                      { text: "Move to Bottom", enabled: draggable && i < pins.length - 1, action: () => moveTo(i, pins.length - 1) },
                      "-",
                      { text: "New Pin…", accelerator: "CmdOrCtrl+N", action: () => setEditing("new") },
                      { text: "Unpin", accelerator: "CmdOrCtrl+Backspace", action: () => remove(p) },
                    ])
                  }
                />
              ))}
            </SortableContext>
          </DndContext>
          <button
            onClick={() => setEditing("new")}
            className="mt-1 flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-[0.9rem] text-neutral-500 hover:bg-black/5 dark:hover:bg-white/5"
          >
            <Icon name="plus" size={14} /> New pin <span className="ml-auto text-[0.75rem]">⌘N</span>
          </button>
        </div>
      )}
      {settings.showPreview && pins[selected] && !editing && <Preview d={pins[selected]} />}
      {editing && (
        <PinEditor
          pin={editing === "new" ? null : editing}
          onClose={() => setEditing(null)}
          onSaved={() => {
            setEditing(null);
            onToast("Saved");
          }}
        />
      )}
    </>
  );
}

function PinEditor({ pin, onClose, onSaved }: { pin: Pin | null; onClose: () => void; onSaved: () => void }) {
  const [label, setLabel] = useState(pin?.label ?? "");
  const [content, setContent] = useState(pin?.content ?? "");
  const [error, setError] = useState("");
  const textual = !pin || pin.kind === "text";

  const save = async () => {
    try {
      if (pin) await api.updatePin(pin.id, label || null, textual ? content : null);
      else await api.addPin(label || null, content);
      onSaved();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div
      className="absolute inset-0 z-40 flex items-start justify-center bg-black/20 p-4 pt-16"
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === "Escape") onClose();
        if (e.key === "Enter" && isMod(e)) save();
      }}
    >
      <div className="w-full rounded-xl border border-black/10 bg-white p-3 shadow-xl dark:border-white/10 dark:bg-neutral-800">
        <div className="mb-2 text-[1rem] font-semibold">{pin ? "Edit pin" : "New pin"}</div>
        <label className="mb-1 block text-[0.8rem] text-neutral-500">Label (optional)</label>
        <input autoFocus className={inputClass} value={label} onChange={(e) => setLabel(e.target.value)} placeholder="e.g. Office address" />
        {textual && (
          <>
            <label className="mb-1 mt-2 block text-[0.8rem] text-neutral-500">Content</label>
            <textarea
              className={`${inputClass} h-28 resize-none font-mono text-[0.85rem]`}
              value={content}
              onChange={(e) => setContent(e.target.value)}
            />
          </>
        )}
        {error && <div className="mt-1 text-[0.8rem] text-red-500">{error}</div>}
        <div className="mt-3 flex justify-end gap-2">
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" onClick={save}>
            Save <span className="opacity-70">⌘↩</span>
          </Button>
        </div>
      </div>
    </div>
  );
}
