import { Menu, MenuItem, PredefinedMenuItem } from "@tauri-apps/api/menu";

export type CtxItem =
  | { text: string; action: () => void; enabled?: boolean; accelerator?: string }
  | "-";

/** Shows a native macOS context menu at the mouse pointer. */
export async function showContextMenu(items: CtxItem[]) {
  const built = await Promise.all(
    items.map((i) =>
      i === "-"
        ? PredefinedMenuItem.new({ item: "Separator" })
        : MenuItem.new({ text: i.text, enabled: i.enabled ?? true, accelerator: i.accelerator, action: i.action }),
    ),
  );
  const menu = await Menu.new({ items: built });
  await menu.popup();
}
