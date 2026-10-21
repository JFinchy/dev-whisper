import { calls, expect, open, test } from "./fixtures";

test.use({ viewport: { width: 880, height: 720 } });

async function goTo(page: import("@playwright/test").Page, nav: string) {
  await page.locator(".dw-sidebar").getByRole("button", { name: nav, exact: true }).click();
}

test.describe("history keeps the original", () => {
  test("a refined entry shows a marker and expands to the original", async ({ page, consoleErrors: _ }) => {
    await open(page, "settings");
    await goTo(page, "History");

    const refined = page.getByRole("listitem").filter({ hasText: "Let's meet at three." });
    const plain = page.getByRole("listitem").filter({ hasText: "fix the crash" });
    await expect(refined.getByRole("button", { name: /Refined/ })).toBeVisible();
    await expect(plain.getByRole("button", { name: /Refined/ })).toHaveCount(0);
    await expect(page.getByTestId("original-text")).toHaveCount(0);

    await refined.getByRole("button", { name: /Refined/ }).click();
    await expect(page.getByTestId("original-text")).toContainText("um let's meet at three");
  });

  test("Use original swaps the entry's text back", async ({ page, consoleErrors: _ }) => {
    await open(page, "settings");
    await goTo(page, "History");

    const refined = page.getByRole("listitem").filter({ hasText: "Let's meet at three." });
    await refined.getByRole("button", { name: /Refined/ }).click();
    await page.getByRole("button", { name: "Use original" }).click();

    expect(await calls(page, "use_original_history_text")).toHaveLength(1);
    await expect(page.getByRole("listitem").filter({ hasText: "um let's meet at three" })).toBeVisible();
    await expect(page.getByRole("button", { name: /Refined/ })).toHaveCount(0);
  });

  test("editing an entry saves it as a learning edit", async ({ page, consoleErrors: _ }) => {
    await open(page, "settings");
    await goTo(page, "History");

    const entry = page.getByRole("listitem").filter({ hasText: "fix the crash" });
    await entry.hover();
    await entry.getByRole("button", { name: "Edit transcript" }).click();
    await page.getByLabel("Edit transcript text").fill("Look at @recording.rs and fix the crashes");
    await page.getByRole("button", { name: "Save" }).click();

    const saved = await calls(page, "update_history_entry_text");
    expect(saved).toHaveLength(1);
    expect(saved[0]).toMatchObject({ text: "Look at @recording.rs and fix the crashes", learn: true });
    await expect(page.getByText("fix the crashes")).toBeVisible();
  });
});

test.describe("learning from corrections", () => {
  const corrections = [{ from: "post gress", to: "Postgres", source: "edit" as const, count: 2 }];

  test("a repeated correction is offered and can be added", async ({ page, consoleErrors: _ }) => {
    await open(page, "settings", { corrections });
    await goTo(page, "Vocabulary");

    const card = page.getByTestId("correction-suggestions");
    await expect(card).toContainText("You've corrected 'post gress' to 'Postgres' 2 times. Add it?");
    await card.getByRole("button", { name: "Add" }).click();

    expect(await calls(page, "accept_correction_suggestion")).toEqual([
      { from: "post gress", to: "Postgres", source: "edit" },
    ]);
    await expect(card).toHaveCount(0);
    await expect(page.locator(".badge").filter({ hasText: /^Postgres/ })).toBeVisible();
    await expect(page.getByTestId("replacement-rules")).toContainText("post gress → Postgres");
  });

  test("a suggestion can be dismissed", async ({ page, consoleErrors: _ }) => {
    await open(page, "settings", { corrections });
    await goTo(page, "Vocabulary");

    await page.getByTestId("correction-suggestions").getByRole("button", { name: "Dismiss" }).click();
    expect(await calls(page, "dismiss_correction_suggestion")).toHaveLength(1);
    await expect(page.getByTestId("correction-suggestions")).toHaveCount(0);
  });

  test("no card without a repeated correction, and learning can be turned off", async ({ page, consoleErrors: _ }) => {
    await open(page, "settings");
    await goTo(page, "Vocabulary");

    await expect(page.getByTestId("correction-suggestions")).toHaveCount(0);
    const toggle = page.getByLabel("Learn from my corrections");
    await expect(toggle).toBeChecked();
    await toggle.uncheck();
    expect(await calls(page, "set_learn_corrections")).toEqual([{ enabled: false }]);
  });
});
