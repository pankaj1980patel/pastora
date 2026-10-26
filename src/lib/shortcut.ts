// Converts between keyboard events, accelerator strings understood by the
// global-shortcut plugin (e.g. "CommandOrControl+Shift+KeyV") and display text (⌘⇧V).

const MOD_ORDER = ["Control", "Alt", "Shift", "CommandOrControl"] as const;
const MOD_SYMBOL: Record<string, string> = {
  Control: "⌃",
  Ctrl: "⌃",
  Alt: "⌥",
  Option: "⌥",
  Shift: "⇧",
  CommandOrControl: "⌘",
  CmdOrCtrl: "⌘",
  Command: "⌘",
  Cmd: "⌘",
  Super: "⌘",
};

const KEY_LABEL: Record<string, string> = {
  Space: "Space",
  Enter: "↩",
  Backspace: "⌫",
  Delete: "⌦",
  Escape: "⎋",
  Tab: "⇥",
  ArrowUp: "↑",
  ArrowDown: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
  Backquote: "`",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
};

function keyLabel(code: string): string {
  if (code.startsWith("Key")) return code.slice(3);
  if (code.startsWith("Digit")) return code.slice(5);
  return KEY_LABEL[code] ?? code;
}

export function displayAccelerator(accel: string): string {
  if (!accel) return "";
  const parts = accel.split("+");
  const key = parts.pop()!;
  const mods = parts.map((p) => MOD_SYMBOL[p] ?? p);
  return mods.join("") + keyLabel(key.length === 1 ? key.toUpperCase() : key);
}

const MODIFIER_CODES = new Set([
  "ShiftLeft", "ShiftRight", "ControlLeft", "ControlRight", "AltLeft", "AltRight", "MetaLeft", "MetaRight",
]);

/** Returns an accelerator for a key event, or null if only modifiers are held / it's not allowed. */
export function acceleratorFromEvent(e: KeyboardEvent): string | null {
  if (MODIFIER_CODES.has(e.code)) return null;
  const mods: string[] = [];
  if (e.ctrlKey) mods.push("Control");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (e.metaKey) mods.push("CommandOrControl");
  const isFn = /^F\d{1,2}$/.test(e.code);
  // Global shortcuts need a modifier other than Shift, except function keys.
  if (!isFn && !mods.some((m) => m !== "Shift")) return null;
  mods.sort((a, b) => MOD_ORDER.indexOf(a as never) - MOD_ORDER.indexOf(b as never));
  return [...mods, e.code].join("+");
}

/** Whether the event matches "mod+key" for in-popup keys. */
export function isMod(e: KeyboardEvent | React.KeyboardEvent) {
  return e.metaKey || e.ctrlKey;
}
