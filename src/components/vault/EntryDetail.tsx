import { useEffect, useState } from "react";
import { api, errorText, EntrySummary, FieldSummary } from "../../lib/api";
import { absoluteTime } from "../../lib/format";
import { Icon } from "../Icon";
import { Countdown, IconButton, Kbd } from "../ui";
import { primaryField, template, totpField, userField } from "./templates";

interface Props {
  entry: EntrySummary;
  onCopy: (field: FieldSummary, paste: boolean) => void;
  onBack: () => void;
  onEdit: () => void;
  onToast: (m: string) => void;
}

function TotpValue({ entryId, field }: { entryId: string; field: FieldSummary }) {
  const [code, setCode] = useState<{ code: string; remaining: number } | null>(null);
  const [err, setErr] = useState("");
  useEffect(() => {
    let alive = true;
    const tick = () =>
      api
        .vaultTotp(entryId, field.id)
        .then((c) => alive && setCode(c))
        .catch((e) => alive && setErr(errorText(e)));
    tick();
    const t = setInterval(tick, 1000);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, [entryId, field.id]);
  if (err) return <span className="text-red-500">{err}</span>;
  if (!code) return <span>…</span>;
  return (
    <span className="flex items-center gap-2 font-mono tracking-widest">
      {code.code.slice(0, 3)} {code.code.slice(3)}
      <span className={code.remaining <= 5 ? "text-red-500" : "text-neutral-400"}>
        <Countdown key={code.code} seconds={30} size={13} />
      </span>
    </span>
  );
}

function FieldRow({ entryId, field, onCopy, onToast }: { entryId: string; field: FieldSummary; onCopy: Props["onCopy"]; onToast: Props["onToast"] }) {
  const [revealed, setRevealed] = useState<string | null>(null);
  const secret = field.kind === "secret" || field.kind === "note";

  useEffect(() => {
    if (revealed === null) return;
    const t = setTimeout(() => setRevealed(null), 15000); // hide again after 15 s
    return () => clearTimeout(t);
  }, [revealed]);

  const reveal = async () => {
    if (revealed !== null) return setRevealed(null);
    try {
      setRevealed(await api.vaultRevealField(entryId, field.id));
    } catch (e) {
      onToast(errorText(e));
    }
  };

  let value: React.ReactNode;
  if (!field.hasValue) value = <span className="italic text-neutral-400">empty</span>;
  else if (field.kind === "totp") value = <TotpValue entryId={entryId} field={field} />;
  else if (secret) value = revealed !== null ? <span className="whitespace-pre-wrap break-all font-mono">{revealed}</span> : <span className="tracking-[0.2em]">••••••••••</span>;
  else if (field.kind === "url") value = <span className="text-indigo-500">{field.preview}</span>;
  else value = <span className="break-all">{field.preview}</span>;

  return (
    <div
      className="group flex items-center gap-2 rounded-lg px-2.5 py-1.5 hover:bg-black/5 dark:hover:bg-white/5"
      onDoubleClick={() => field.hasValue && onCopy(field, true)}
    >
      <div className="min-w-0 flex-1">
        <div className="text-[0.75rem] text-neutral-500">{field.label}</div>
        <div className="text-[0.95rem]">{value}</div>
      </div>
      {field.hasValue && (
        <div className="flex shrink-0 gap-0.5">
          {secret && <IconButton icon={revealed !== null ? "eyeOff" : "eye"} title={revealed !== null ? "Hide" : "Reveal"} onClick={reveal} />}
          <IconButton icon="copy" title="Copy" onClick={() => onCopy(field, false)} />
          <IconButton icon="paste" title="Paste into previous app" onClick={() => onCopy(field, true)} />
        </div>
      )}
    </div>
  );
}

export function EntryDetail({ entry, onCopy, onBack, onEdit, onToast }: Props) {
  const t = template(entry.template);
  const primary = primaryField(entry.fields);
  const user = userField(entry.fields);
  const totp = totpField(entry.fields);
  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      <div className="flex items-center gap-2 border-b border-black/5 px-2 py-2 dark:border-white/10">
        <IconButton icon="back" title="Back (Esc)" onClick={onBack} />
        <div className="flex h-7 w-7 items-center justify-center rounded-md bg-indigo-500/15 text-indigo-500">
          <Icon name={t.icon} size={15} />
        </div>
        <div className="min-w-0 flex-1">
          <div className="truncate text-[1rem] font-semibold">{entry.title}</div>
          <div className="text-[0.75rem] text-neutral-500">
            {t.name} · updated {absoluteTime(entry.updatedAt)}
          </div>
        </div>
        <IconButton icon="edit" title="Edit (⌘E)" onClick={onEdit} />
      </div>
      <div className="flex-1 overflow-y-auto px-1.5 py-1">
        {entry.fields.map((f) => (
          <FieldRow key={f.id} entryId={entry.id} field={f} onCopy={onCopy} onToast={onToast} />
        ))}
        {entry.hasNotes && <p className="px-2.5 py-1 text-[0.8rem] text-neutral-500">Has notes, open the editor to view them.</p>}
      </div>
      <div className="flex flex-wrap gap-x-3 gap-y-1 border-t border-black/5 px-3 py-1.5 text-[0.75rem] text-neutral-500 dark:border-white/10">
        {primary && (
          <span>
            <Kbd>↩</Kbd> paste {primary.label.toLowerCase()}
          </span>
        )}
        {user && (
          <span>
            <Kbd>⌘U</Kbd> {user.label.toLowerCase()}
          </span>
        )}
        {totp && (
          <span>
            <Kbd>⌘T</Kbd> code
          </span>
        )}
        {user && primary && (
          <span>
            <Kbd>⌘L</Kbd> username, then password
          </span>
        )}
      </div>
    </div>
  );
}
