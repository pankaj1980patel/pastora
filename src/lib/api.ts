import { invoke } from "@tauri-apps/api/core";

export type Kind = "text" | "image" | "files";

export interface Clip {
  id: number;
  kind: Kind;
  content: string;
  imagePath: string | null;
  sourceApp: string | null;
  width: number | null;
  height: number | null;
  createdAt: number;
  lastUsedAt: number;
  useCount: number;
  pinned: boolean;
}

export interface Pin {
  id: number;
  label: string | null;
  kind: Kind;
  content: string;
  imagePath: string | null;
  width: number | null;
  height: number | null;
  position: number;
  createdAt: number;
}

export type FieldKind = "text" | "email" | "url" | "secret" | "totp" | "note";

export interface Field {
  id: string;
  label: string;
  value: string;
  kind: FieldKind;
}

export interface Entry {
  id: string;
  template: string;
  favorite: boolean;
  updatedAt: number;
  title: string;
  fields: Field[];
  notes: string;
}

export interface FieldSummary {
  id: string;
  label: string;
  kind: FieldKind;
  hasValue: boolean;
  preview: string | null;
}

export interface EntrySummary {
  id: string;
  template: string;
  title: string;
  subtitle: string;
  favorite: boolean;
  updatedAt: number;
  fields: FieldSummary[];
  hasNotes: boolean;
}

export interface VaultStatus {
  initialized: boolean;
  unlocked: boolean;
  biometricAvailable: boolean;
  biometricName: string;
  biometricEnabled: boolean;
  passwordRequired: boolean;
}

export interface Shortcuts {
  openPopup: string;
  openPins: string;
  openSecure: string;
  lockVault: string;
  togglePause: string;
}

export interface IgnoredApp {
  bundleId: string;
  name: string;
}

export interface Settings {
  launchAtLogin: boolean;
  showDockIcon: boolean;
  theme: "system" | "light" | "dark";
  popupPosition: "menubar" | "cursor" | "center";
  historyLimit: number;
  historyDays: number;
  storeImages: boolean;
  maxImageMb: number;
  ignorePasswordManagers: boolean;
  ignoredApps: IgnoredApp[];
  ignorePatterns: string[];
  pausedUntil: number | null;
  shortcuts: Shortcuts;
  autoPaste: boolean;
  moveToTopOnUse: boolean;
  autoLockMinutes: number;
  lockOnSleep: boolean;
  clearClipboardSeconds: number;
  touchIdEnabled: boolean;
  requirePasswordDays: number;
  hideFromScreenCapture: boolean;
  popupWidth: number;
  popupHeight: number;
  fontSize: number;
  showPreview: boolean;
  showSourceApp: boolean;
  relativeTime: boolean;
  onboarded: boolean;
}

export interface AppInfo {
  version: string;
  dataDir: string;
  platform: string;
}

export const api = {
  listHistory: (query: string, limit = 100, offset = 0) =>
    invoke<Clip[]>("list_history", { query, limit, offset }),
  deleteClip: (id: number) => invoke<void>("delete_clip", { id }),
  clearHistory: () => invoke<void>("clear_history"),
  useItem: (source: "history" | "pin", id: number, paste: boolean, plain?: string) =>
    invoke<void>("use_item", { source, id, paste, plain: plain ?? null }),
  saveImage: (source: "history" | "pin", id: number, name: string) => invoke<boolean>("save_image", { source, id, name }),
  pinClip: (id: number) => invoke<number>("pin_clip", { id }),
  unpinClip: (id: number) => invoke<void>("unpin_clip", { id }),

  listPins: (query: string) => invoke<Pin[]>("list_pins", { query }),
  addPin: (label: string | null, content: string) => invoke<number>("add_pin", { label, content }),
  updatePin: (id: number, label: string | null, content: string | null) =>
    invoke<void>("update_pin", { id, label, content }),
  deletePin: (id: number) => invoke<void>("delete_pin", { id }),
  reorderPins: (ids: number[]) => invoke<void>("reorder_pins", { ids }),

  vaultStatus: () => invoke<VaultStatus>("vault_status"),
  vaultSetup: (password: string) => invoke<void>("vault_setup", { password }),
  vaultUnlock: (password: string) => invoke<void>("vault_unlock", { password }),
  vaultUnlockBiometric: () => invoke<void>("vault_unlock_biometric"),
  vaultLock: () => invoke<void>("vault_lock"),
  vaultSetBiometric: (enabled: boolean) => invoke<void>("vault_set_biometric", { enabled }),
  vaultChangePassword: (old: string, next: string) =>
    invoke<void>("vault_change_password", { old, new: next }),
  vaultList: () => invoke<EntrySummary[]>("vault_list"),
  vaultGet: (id: string) => invoke<Entry>("vault_get", { id }),
  vaultSave: (entry: Entry) => invoke<string>("vault_save", { entry }),
  vaultDelete: (id: string) => invoke<void>("vault_delete", { id }),
  vaultSetFavorite: (id: string, favorite: boolean) => invoke<void>("vault_set_favorite", { id, favorite }),
  vaultReorder: (ids: string[]) => invoke<void>("vault_reorder", { ids }),
  vaultCopyField: (id: string, fieldId: string, paste: boolean) =>
    invoke<{ clearSeconds: number }>("vault_copy_field", { id, fieldId, paste }),
  vaultRevealField: (id: string, fieldId: string) => invoke<string>("vault_reveal_field", { id, fieldId }),
  vaultTotp: (id: string, fieldId: string) =>
    invoke<{ code: string; remaining: number }>("vault_totp", { id, fieldId }),
  vaultPasteLogin: (id: string, userField: string, passField: string) =>
    invoke<void>("vault_paste_login", { id, userField, passField }),
  vaultExport: (path: string) => invoke<void>("vault_export", { path }),
  vaultImport: (path: string, password: string) => invoke<number>("vault_import", { path, password }),
  vaultReset: () => invoke<void>("vault_reset"),

  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (settings: Settings) => invoke<Settings>("set_settings", { settings }),
  resetShortcuts: () => invoke<Settings>("reset_shortcuts"),
  pauseRecording: (minutes: number) => invoke<void>("pause_recording", { minutes }),
  appInfo: () => invoke<AppInfo>("app_info"),
  runningApps: () => invoke<IgnoredApp[]>("running_apps"),
  accessibilityStatus: () => invoke<boolean>("accessibility_status"),
  requestAccessibility: () => invoke<boolean>("request_accessibility"),
  resetAccessibility: () => invoke<boolean>("reset_accessibility"),
  openDataFolder: () => invoke<void>("open_data_folder"),
  openWebsite: (page?: "support") => invoke<void>("open_website", { page: page ?? null }),
  openSettings: (section?: string) => invoke<void>("open_settings", { section: section ?? null }),
  hidePopup: () => invoke<void>("hide_popup"),
  exportData: (path: string) => invoke<void>("export_data", { path }),
  importData: (path: string) => invoke<number>("import_data", { path }),
  resetAll: () => invoke<void>("reset_all"),
  finishOnboarding: () => invoke<Settings>("finish_onboarding"),
};

export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
