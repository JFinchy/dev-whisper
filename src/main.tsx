import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

async function boot() {
  // Browser-only runs (e2e tests, `bun run dev:mock`) swap the Rust backend
  // for an in-memory one. The env check is a build-time constant, so this
  // branch and the mock module are dropped from real builds.
  if (import.meta.env.VITE_MOCK_BACKEND === "1") {
    const { installMockBackend } = await import("./testing/mockBackend");
    installMockBackend();
  }

  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

void boot();
