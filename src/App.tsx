import { getCurrentWindow } from "@tauri-apps/api/window";
import "./App.css";
import WidgetView from "./WidgetView";
import SettingsView from "./SettingsView";

function App() {
  const label = getCurrentWindow().label;
  return label === "settings" ? <SettingsView /> : <WidgetView />;
}

export default App;
