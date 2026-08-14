import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

function SettingsView() {
  const [devices, setDevices] = useState<string[]>([]);
  const [activeDevice, setActiveDevice] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    Promise.all([
      invoke<string[]>("list_input_devices"),
      invoke<string | null>("get_active_input_device"),
    ])
      .then(([deviceList, active]) => {
        setDevices(deviceList);
        setActiveDevice(active);
      })
      .catch((err) => console.error("failed to load audio devices:", err))
      .finally(() => setLoading(false));
  }, []);

  function selectDevice(name: string) {
    setActiveDevice(name);
    invoke("set_input_device", { name }).catch((err) =>
      console.error("set_input_device failed:", err),
    );
  }

  return (
    <main className="min-h-screen bg-base-300 px-5 py-4 text-base-content">
      <h1 className="mb-4 text-base font-semibold">Settings</h1>

      <div className="mb-4">
        <label className="mb-1 block text-xs font-medium opacity-70">Microphone</label>
        {loading ? (
          <span className="loading loading-spinner loading-xs" />
        ) : devices.length === 0 ? (
          <p className="text-xs opacity-60">No input devices found.</p>
        ) : (
          <select
            className="select select-sm w-full"
            value={activeDevice ?? ""}
            onChange={(e) => selectDevice(e.target.value)}
          >
            {devices.map((name) => (
              <option key={name} value={name}>
                {name}
              </option>
            ))}
          </select>
        )}
      </div>

      <div className="mb-2 flex justify-between text-xs opacity-75">
        <span>Push-to-talk key</span>
        <span className="opacity-50">⌘⇧Space</span>
      </div>
      <div className="flex justify-between text-xs opacity-75">
        <span>Whisper model</span>
        <span className="opacity-50">base.en (q5_1)</span>
      </div>
    </main>
  );
}

export default SettingsView;
