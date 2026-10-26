import { Entry, EntrySummary, Field, FieldKind } from "../../lib/api";

export interface Template {
  id: string;
  name: string;
  icon: string;
  fields: [string, FieldKind][];
}

export const TEMPLATES: Template[] = [
  { id: "login", name: "Login", icon: "user", fields: [["Username", "text"], ["Password", "secret"], ["Website", "url"], ["One-time code", "totp"]] },
  { id: "api", name: "API Key", icon: "key", fields: [["API key", "secret"], ["Secret", "secret"], ["Endpoint", "url"]] },
  { id: "card", name: "Card", icon: "card", fields: [["Cardholder", "text"], ["Card number", "secret"], ["Expiry", "text"], ["CVV", "secret"], ["PIN", "secret"]] },
  { id: "wifi", name: "Wi-Fi", icon: "wifi", fields: [["Network", "text"], ["Password", "secret"]] },
  { id: "ssh", name: "Server / SSH", icon: "terminal", fields: [["Host", "text"], ["User", "text"], ["Password", "secret"], ["Private key", "note"]] },
  { id: "note", name: "Secure Note", icon: "note", fields: [["Note", "note"]] },
  { id: "custom", name: "Custom", icon: "shield", fields: [["Value", "secret"]] },
];

export const FIELD_KINDS: { id: FieldKind; name: string }[] = [
  { id: "text", name: "Text" },
  { id: "email", name: "Email" },
  { id: "url", name: "URL" },
  { id: "secret", name: "Secret" },
  { id: "note", name: "Secret note" },
  { id: "totp", name: "One-time code (TOTP)" },
];

export function template(id: string): Template {
  return TEMPLATES.find((t) => t.id === id) ?? TEMPLATES[TEMPLATES.length - 1];
}

export function newEntry(templateId: string, prefill?: string): Entry {
  const t = template(templateId);
  const fields: Field[] = t.fields.map(([label, kind]) => ({ id: "", label, kind, value: "" }));
  if (prefill) {
    const target = fields.find((f) => f.kind === "secret" || f.kind === "note") ?? fields[0];
    target.value = prefill;
  }
  return { id: "", template: t.id, favorite: false, updatedAt: 0, title: "", fields, notes: "" };
}

type F = { kind: FieldKind; hasValue?: boolean; value?: string };
const has = (f: F) => f.hasValue ?? !!f.value;

/** The field that Enter copies: first secret, else first note, else first field with a value. */
export function primaryField<T extends F>(fields: T[]): T | undefined {
  return (
    fields.find((f) => f.kind === "secret" && has(f)) ??
    fields.find((f) => f.kind === "note" && has(f)) ??
    fields.find((f) => has(f))
  );
}

export function userField<T extends F>(fields: T[]): T | undefined {
  return fields.find((f) => (f.kind === "text" || f.kind === "email") && has(f));
}

export function totpField<T extends F>(fields: T[]): T | undefined {
  return fields.find((f) => f.kind === "totp" && has(f));
}

export function matches(e: EntrySummary, q: string): boolean {
  if (!q) return true;
  const s = q.toLowerCase();
  return (
    e.title.toLowerCase().includes(s) ||
    e.subtitle.toLowerCase().includes(s) ||
    template(e.template).name.toLowerCase().includes(s) ||
    e.fields.some((f) => f.preview?.toLowerCase().includes(s))
  );
}
