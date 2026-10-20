import { lazy, Suspense, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./App.css";
import WidgetView from "./WidgetView";

// Settings is most of the UI code; loading it lazily keeps the always-on
// widget window's bundle small.
const SettingsView = lazy(() => import("./SettingsView"));

function App() {
  const label = getCurrentWindow().label;
  useEffect(() => {
    invoke("ui_ready").catch((err) => console.error("ui_ready failed:", err));
  }, []);
  return label === "settings" ? (
    <Suspense fallback={null}>
      <SettingsView />
    </Suspense>
  ) : (
    <WidgetView />
  );
}

export default App;
