import { calls, expect, open, test } from "./fixtures";

test.use({ viewport: { width: 880, height: 720 } });

async function goTo(page: import("@playwright/test").Page, nav: string) {
  await page.locator(".dw-sidebar").getByRole("button", { name: nav, exact: true }).click();
}

test.describe("privacy", () => {
  test("History is shown as usual while the lock is off @csp", async ({ page, consoleErrors: _ }) => {
    await open(page, "settings");
    await goTo(page, "History");
    await expect(page.getByTestId("history-privacy")).toBeVisible();
    await expect(page.getByTestId("history-lock-toggle")).not.toBeChecked();
    await expect(page.getByTestId("history-locked")).toHaveCount(0);
    await expect(page.getByText("fix the crash")).toBeVisible();
  });

  test("a locked History hides transcripts until unlocked", async ({ page, consoleErrors: _ }) => {
    await open(page, "settings", { historyLock: true });
    await goTo(page, "History");

    await expect(page.getByTestId("history-locked")).toBeVisible();
    await expect(page.getByText("fix the crash")).toHaveCount(0);
    expect(await calls(page, "list_history_entries")).toHaveLength(0);

    await page.getByRole("button", { name: "Unlock History" }).click();
    await expect(page.getByText("fix the crash")).toBeVisible();
    await expect(page.getByTestId("history-locked")).toHaveCount(0);
  });

  test("leaving History locks it again", async ({ page, consoleErrors: _ }) => {
    await open(page, "settings", { historyLock: true });
    await goTo(page, "History");
    await page.getByRole("button", { name: "Unlock History" }).click();
    await expect(page.getByText("fix the crash")).toBeVisible();

    await goTo(page, "Vocabulary");
    await expect.poll(() => calls(page, "lock_history").then((c) => c.length)).toBeGreaterThan(0);
    await goTo(page, "History");
    await expect(page.getByTestId("history-locked")).toBeVisible();
  });

  test("turning the lock on closes History right away", async ({ page, consoleErrors: _ }) => {
    await open(page, "settings");
    await goTo(page, "History");
    await page.getByTestId("history-lock-toggle").check();
    expect(await calls(page, "set_history_lock_enabled")).toEqual([{ enabled: true }]);
    await expect(page.getByTestId("history-locked")).toBeVisible();
  });

  test("transcript text in logs is off by default and can be turned on", async ({ page, consoleErrors: _ }) => {
    await open(page, "settings");
    await goTo(page, "History");
    const toggle = page.getByTestId("log-transcripts-toggle");
    await expect(toggle).not.toBeChecked();
    await toggle.check();
    expect(await calls(page, "set_log_transcripts")).toEqual([{ enabled: true }]);
  });
});
