import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { Popup } from "./components/Popup";
import { SettingsWindow } from "./components/Settings";
import { useAppearance, useSettings } from "./lib/hooks";

const label = getCurrentWebviewWindow().label;

export default function App() {
  const [settings] = useSettings();
  useAppearance(settings, label === "popup");
  if (!settings) return null;
  return label === "popup" ? <Popup settings={settings} /> : <SettingsWindow settings={settings} />;
}
