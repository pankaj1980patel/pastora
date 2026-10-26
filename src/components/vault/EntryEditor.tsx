import { useEffect, useState } from "react";
import { api, Entry, errorText, Field, FieldKind } from "../../lib/api";
import { generatePassword, GenOptions, strength } from "../../lib/password";
import { isMod } from "../../lib/shortcut";
import { Icon } from "../Icon";
import { Button, IconButton, inputClass, Toggle } from "../ui";
import { FIELD_KINDS, newEntry, TEMPLATES } from "./templates";

interface Props {
  entryId: string | null; // null = new
  prefill?: string;
  onClose: () => void;
  onSaved: (id: string) => void;
  onDeleted: () => void;
}

function Generator({ onUse, onClose }: { onUse: (v: string) => void; onClose: () => void }) {
  const [opts, setOpts] = useState<GenOptions>(() => {
    try {
      return JSON.parse(localStorage.getItem("gen") ?? "") as GenOptions;
    } catch {
      return { mode: "random", length: 20, symbols: true, numbers: true };
    }
  });
  const [value, setValue] = useState(() => generatePassword(opts));
  useEffect(() => {
    setValue(generatePassword(opts));
    localStorage.setItem("gen", JSON.stringify(opts));
  }, [opts]);
  const words = opts.mode === "words";
  return (
    <div className="mt-1 rounded-lg border border-black/10 bg-black/[0.03] p-2.5 dark:border-white/10 dark:bg-white/5">
      <div className="flex items-center gap-2">
        <code className="min-w-0 flex-1 break-all rounded bg-white/70 px-2 py-1 font-mono text-[0.85rem] dark:bg-black/30">{value}</code>
        <IconButton icon="dice" title="Generate again" onClick={() => setValue(generatePassword(opts))} />
      </div>
      <div className="mt-2 flex items-center gap-2 text-[0.8rem]">
        <div className="flex overflow-hidden rounded-md border border-black/10 dark:border-white/10">
          {(["random", "words"] as const).map((m) => (
            <button
              key={m}
              className={`px-2 py-0.5 ${opts.mode === m ? "bg-indigo-500 text-white" : ""}`}
              onClick={() => setOpts({ ...opts, mode: m, length: m === "words" ? 5 : 20 })}
            >
              {m === "random" ? "Characters" : "Words"}
            </button>
          ))}
        </div>
        <input
          type="range"
          min={words ? 3 : 8}
          max={words ? 10 : 64}
          value={opts.length}
          onChange={(e) => setOpts({ ...opts, length: Number(e.target.value) })}
          className="flex-1 accent-indigo-500"
        />
        <span className="w-6 text-right tabular-nums">{opts.length}</span>
      </div>
      <div className="mt-2 flex items-center gap-4 text-[0.8rem]">
        <label className="flex items-center gap-1.5">
          <Toggle checked={opts.numbers} onChange={(v) => setOpts({ ...opts, numbers: v })} /> Numbers
        </label>
        <label className="flex items-center gap-1.5">
          <Toggle checked={opts.symbols} onChange={(v) => setOpts({ ...opts, symbols: v })} /> {words ? "Dashes" : "Symbols"}
        </label>
        <div className="ml-auto flex gap-1.5">
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="primary" onClick={() => onUse(value)}>
            Use
          </Button>
        </div>
      </div>
    </div>
  );
}

function FieldEditor({
  field,
  onChange,
  onRemove,
  autoFocus,
}: {
  field: Field;
  onChange: (f: Field) => void;
  onRemove: () => void;
  autoFocus?: boolean;
}) {
  const [show, setShow] = useState(false);
  const [gen, setGen] = useState(false);
  const isSecret = field.kind === "secret";
  const s = strength(field.value);
  return (
    <div className="rounded-lg border border-black/5 p-2 dark:border-white/10">
      <div className="mb-1 flex items-center gap-1.5">
        <input
          className="min-w-0 flex-1 bg-transparent text-[0.8rem] font-medium text-neutral-500 outline-none"
          value={field.label}
          onChange={(e) => onChange({ ...field, label: e.target.value })}
          placeholder="Field name"
        />
        <select
          className="rounded bg-transparent text-[0.75rem] text-neutral-500 outline-none"
          value={field.kind}
          onChange={(e) => onChange({ ...field, kind: e.target.value as FieldKind })}
        >
          {FIELD_KINDS.map((k) => (
            <option key={k.id} value={k.id}>
              {k.name}
            </option>
          ))}
        </select>
        <IconButton icon="x" title="Remove field" onClick={onRemove} size={13} />
      </div>
      {field.kind === "note" ? (
        <textarea
          autoFocus={autoFocus}
          className={`${inputClass} h-24 resize-none font-mono text-[0.85rem]`}
          value={field.value}
          onChange={(e) => onChange({ ...field, value: e.target.value })}
        />
      ) : (
        <div className="flex items-center gap-1">
          <input
            autoFocus={autoFocus}
            type={isSecret && !show ? "password" : "text"}
            className={`${inputClass} ${isSecret || field.kind === "totp" ? "font-mono" : ""}`}
            value={field.value}
            spellCheck={false}
            autoCapitalize="off"
            autoCorrect="off"
            placeholder={field.kind === "totp" ? "Base32 secret or otpauth:// link" : ""}
            onChange={(e) => onChange({ ...field, value: e.target.value })}
          />
          {isSecret && (
            <>
              <IconButton icon={show ? "eyeOff" : "eye"} title={show ? "Hide" : "Show"} onClick={() => setShow(!show)} />
              <IconButton icon="dice" title="Generate password" onClick={() => setGen(!gen)} active={gen} />
            </>
          )}
        </div>
      )}
      {isSecret && field.value && (
        <div className="mt-1 text-[0.7rem] text-neutral-500">
          Strength: {["Very weak", "Weak", "Fair", "Strong", "Very strong"][s]}
        </div>
      )}
      {gen && (
        <Generator
          onClose={() => setGen(false)}
          onUse={(v) => {
            onChange({ ...field, value: v });
            setGen(false);
          }}
        />
      )}
    </div>
  );
}

export function EntryEditor({ entryId, prefill, onClose, onSaved, onDeleted }: Props) {
  const [entry, setEntry] = useState<Entry | null>(entryId ? null : newEntry(prefill ? "custom" : "login", prefill));
  const [error, setError] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);
  const isNew = !entryId;

  useEffect(() => {
    if (entryId) api.vaultGet(entryId).then(setEntry).catch((e) => setError(errorText(e)));
  }, [entryId]);

  if (!entry) return <div className="flex-1 p-4 text-neutral-500">{error || "Loading…"}</div>;

  const setField = (i: number, f: Field) => setEntry({ ...entry, fields: entry.fields.map((x, j) => (j === i ? f : x)) });
  const save = async () => {
    try {
      const cleaned = { ...entry, fields: entry.fields.filter((f) => f.label.trim() || f.value) };
      onSaved(await api.vaultSave(cleaned));
    } catch (e) {
      setError(errorText(e));
    }
  };

  return (
    <div
      className="flex flex-1 flex-col overflow-hidden"
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          onClose();
        }
        if (e.key === "Enter" && isMod(e)) {
          e.preventDefault();
          save();
        }
      }}
    >
      <div className="flex items-center gap-2 border-b border-black/5 px-2 py-2 dark:border-white/10">
        <IconButton icon="back" title="Cancel (Esc)" onClick={onClose} />
        <div className="flex-1 text-[1rem] font-semibold">{isNew ? "New secure clip" : "Edit secure clip"}</div>
        <IconButton icon="star" title="Favorite" active={entry.favorite} onClick={() => setEntry({ ...entry, favorite: !entry.favorite })} />
      </div>
      <div className="flex-1 space-y-2 overflow-y-auto p-3">
        {isNew && (
          <div className="flex flex-wrap gap-1">
            {TEMPLATES.map((t) => (
              <button
                key={t.id}
                onClick={() => {
                  const keep = entry.fields.find((f) => f.value)?.value;
                  setEntry({ ...newEntry(t.id, keep), title: entry.title, favorite: entry.favorite });
                }}
                className={`flex items-center gap-1 rounded-full border px-2 py-0.5 text-[0.8rem] ${
                  entry.template === t.id
                    ? "border-indigo-500 bg-indigo-500/10 text-indigo-600 dark:text-indigo-300"
                    : "border-black/10 text-neutral-600 dark:border-white/10 dark:text-neutral-300"
                }`}
              >
                <Icon name={t.icon} size={12} />
                {t.name}
              </button>
            ))}
          </div>
        )}
        <input
          autoFocus
          className={`${inputClass} text-[1rem] font-medium`}
          placeholder="Title, e.g. GitHub or AWS prod"
          value={entry.title}
          onChange={(e) => setEntry({ ...entry, title: e.target.value })}
        />
        {entry.fields.map((f, i) => (
          <FieldEditor
            key={i}
            field={f}
            onChange={(nf) => setField(i, nf)}
            onRemove={() => setEntry({ ...entry, fields: entry.fields.filter((_, j) => j !== i) })}
          />
        ))}
        <Button
          variant="ghost"
          onClick={() => setEntry({ ...entry, fields: [...entry.fields, { id: "", label: "", value: "", kind: "text" }] })}
        >
          <Icon name="plus" size={14} /> Add field
        </Button>
        <div>
          <label className="mb-1 block text-[0.8rem] text-neutral-500">Notes (encrypted)</label>
          <textarea
            className={`${inputClass} h-16 resize-none text-[0.85rem]`}
            value={entry.notes}
            onChange={(e) => setEntry({ ...entry, notes: e.target.value })}
          />
        </div>
        {error && <div className="text-[0.8rem] text-red-500">{error}</div>}
      </div>
      <div className="flex items-center gap-2 border-t border-black/5 px-3 py-2 dark:border-white/10">
        {!isNew &&
          (confirmDelete ? (
            <Button
              variant="danger"
              onClick={async () => {
                await api.vaultDelete(entry.id);
                onDeleted();
              }}
            >
              Really delete?
            </Button>
          ) : (
            <Button variant="ghost" onClick={() => setConfirmDelete(true)}>
              <Icon name="trash" size={14} /> Delete
            </Button>
          ))}
        <div className="flex-1" />
        <Button onClick={onClose}>Cancel</Button>
        <Button variant="primary" onClick={save}>
          Save <span className="opacity-70">⌘↩</span>
        </Button>
      </div>
    </div>
  );
}
