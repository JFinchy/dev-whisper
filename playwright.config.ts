import { defineConfig, devices } from "@playwright/test";

// UI end-to-end tests against the real React app with the in-memory mock
// backend (src/testing/mockBackend.ts). WebKit is the engine that matters:
// Tauri renders through WKWebView on macOS.
//
// Two projects:
// - "webkit": the dev server, every spec.
// - "csp": a production build served with the exact CSP from
//   tauri.conf.json, running the specs tagged @csp. The fixtures fail a
//   test on any console error, which is how a CSP violation surfaces.
const DEV_PORT = 1430;
const CSP_PORT = 1431;

export default defineConfig({
  testDir: "e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [["github"], ["html", { open: "never" }]] : "list",
  use: {
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  projects: [
    {
      name: "webkit",
      use: { ...devices["Desktop Safari"], baseURL: `http://localhost:${DEV_PORT}` },
    },
    {
      name: "csp",
      grep: /@csp/,
      use: { ...devices["Desktop Safari"], baseURL: `http://localhost:${CSP_PORT}` },
    },
  ],
  webServer: [
    {
      command: `bunx vite --port ${DEV_PORT} --strictPort`,
      env: { VITE_MOCK_BACKEND: "1" },
      url: `http://localhost:${DEV_PORT}`,
      reuseExistingServer: !process.env.CI,
    },
    {
      command: `bunx vite build --outDir dist-e2e --emptyOutDir && bunx vite preview --outDir dist-e2e --port ${CSP_PORT} --strictPort`,
      env: { VITE_MOCK_BACKEND: "1", DW_CSP: "1" },
      url: `http://localhost:${CSP_PORT}`,
      reuseExistingServer: !process.env.CI,
      timeout: 120_000,
    },
  ],
});
