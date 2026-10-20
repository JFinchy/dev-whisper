import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { notifyError } from "../../ui/toast";

export function WebhookSection() {
  const [url, setUrl] = useState("");
  const [saved, setSaved] = useState<string | null>(null);
  const [testState, setTestState] = useState<"idle" | "sending" | "ok" | "error">("idle");
  const [testError, setTestError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    invoke<string | null>("get_webhook_url")
      .then((value) => {
        setUrl(value ?? "");
        setSaved(value ?? null);
      })
      .catch((err) => notifyError("get_webhook_url", err))
      .finally(() => setLoading(false));

    const unlisten = listen<{ ok: boolean; error?: string }>("webhook-test-result", (e) => {
      setTestState(e.payload.ok ? "ok" : "error");
      setTestError(e.payload.ok ? null : e.payload.error ?? "unknown error");
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  function save() {
    const trimmed = url.trim();
    invoke("set_webhook_url", { url: trimmed || null })
      .then(() => setSaved(trimmed || null))
      .catch((err) => notifyError("set_webhook_url", err));
  }

  function sendTest() {
    setTestState("sending");
    setTestError(null);
    invoke("send_test_webhook").catch((err) => {
      setTestState("error");
      setTestError(String(err));
    });
  }

  return (
    <div className="mb-4 border-t border-base-content/10 pt-3">
      <label className="mb-1 block text-xs font-medium opacity-70">Output webhook</label>
      <p className="mb-1.5 text-xs opacity-50">
        Sends each delivered dictation as a JSON POST to this URL — wire it into Zapier, n8n,
        Make.com, or a Notion/Slack incoming webhook. Off by default: dictated text only leaves
        the device once you set a URL here.
      </p>
      {loading ? (
        <span className="loading loading-spinner loading-xs" />
      ) : (
        <>
          <div className="flex gap-1.5">
            <input
              className="input input-sm flex-1"
              placeholder="https://hooks.example.com/…"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              onBlur={save}
              onKeyDown={(e) => {
                if (e.key === "Enter") save();
              }}
            />
            <button className="btn btn-sm" onClick={sendTest} disabled={!saved || testState === "sending"}>
              {testState === "sending" ? "Sending…" : "Send test event"}
            </button>
          </div>
          {testState === "ok" && <p className="mt-1 text-xs text-success">Test event delivered.</p>}
          {testState === "error" && (
            <p className="mt-1 text-xs text-error">Test event failed: {testError}</p>
          )}
        </>
      )}
    </div>
  );
}

export function DeliverySection() {
  const [copyOnly, setCopyOnly] = useState(false);
  const [pressEnterEnabled, setPressEnterEnabled] = useState(false);
  const [checked, setChecked] = useState(false);

  useEffect(() => {
    Promise.all([invoke<boolean>("get_copy_only"), invoke<boolean>("get_press_enter_enabled")])
      .then(([copyOnlyValue, pressEnterValue]) => {
        setCopyOnly(copyOnlyValue);
        setPressEnterEnabled(pressEnterValue);
      })
      .catch((err) => notifyError("Couldn't load delivery settings", err))
      .finally(() => setChecked(true));
  }, []);

  function toggleCopyOnly(enabled: boolean) {
    setCopyOnly(enabled);
    invoke("set_copy_only", { enabled }).catch((err) => {
      notifyError("set_copy_only", err);
      setCopyOnly(!enabled);
    });
  }

  function togglePressEnter(enabled: boolean) {
    setPressEnterEnabled(enabled);
    invoke("set_press_enter_enabled", { enabled }).catch((err) => {
      notifyError("set_press_enter_enabled", err);
      setPressEnterEnabled(!enabled);
    });
  }

  return (
    <div className="mb-4 border-b border-base-content/10 pb-3">
      <label className="mb-1 block text-xs font-medium opacity-70">Delivery</label>
      {!checked ? (
        <span className="loading loading-spinner loading-xs" />
      ) : (
        <div className="flex flex-col gap-2">
          <label className="flex items-center gap-1.5 text-xs">
            <input
              type="checkbox"
              className="checkbox checkbox-xs"
              checked={copyOnly}
              onChange={(e) => toggleCopyOnly(e.target.checked)}
            />
            Copy only — don't auto-paste into the active app
          </label>
          <label className="flex items-center gap-1.5 text-xs">
            <input
              type="checkbox"
              className="checkbox checkbox-xs"
              checked={pressEnterEnabled}
              disabled={copyOnly}
              onChange={(e) => togglePressEnter(e.target.checked)}
            />
            Say "press enter" to submit — presses Enter after pasting
            {copyOnly && " (disabled while copy only is on)"}
          </label>
        </div>
      )}
    </div>
  );
}
