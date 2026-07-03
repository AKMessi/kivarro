import { expect, test, type Page } from "@playwright/test";

const primaryViews = [
  { button: "Model Registry", content: "Model Registry" },
  { button: "Hardware Fit", content: "Hardware Fit" },
  { button: "Expert Tuning", content: "Expert Tuning" },
  { button: "Knowledge Base", content: "RAG Knowledge Bases" },
  { button: "Agents", content: "Controlled local autonomy" },
  { button: "Local API", content: "Local API Server" },
  { button: "Benchmarks", content: "Benchmarks" },
  { button: "System Logs", content: "System Logs" },
  { button: "Settings", content: "Settings" },
];

function failOnBrowserErrors(page: Page) {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") {
      errors.push(message.text());
    }
  });
  return errors;
}

test("renders the command center preview shell", async ({ page }) => {
  const errors = failOnBrowserErrors(page);
  await page.goto("/");

  await expect(page).toHaveTitle("Kivarro");
  await expect(page.getByRole("navigation", { name: "Primary navigation" })).toBeVisible();
  await expect(page.getByRole("main").getByText("Command Center").first()).toBeVisible();
  await expect(page.getByLabel("Prompt input")).toBeVisible();
  await expect(page.getByRole("button", { name: "Send prompt" })).toBeVisible();
  await expect(page.getByText("No model loaded").first()).toBeVisible();

  expect(errors).toEqual([]);
});

test("navigates every primary workspace", async ({ page }) => {
  const errors = failOnBrowserErrors(page);
  await page.goto("/");

  for (const view of primaryViews) {
    await page.getByRole("button", { name: view.button }).click();
    await expect(page.getByRole("main").getByText(view.content).first()).toBeVisible();
  }

  expect(errors).toEqual([]);
});

test("opens and closes the keyboard command palette", async ({ page }) => {
  const errors = failOnBrowserErrors(page);
  await page.goto("/");

  await page.keyboard.press("Control+K");
  await expect(page.getByLabel("Command palette search")).toBeVisible();

  await page.getByLabel("Command palette search").fill("logs");
  await expect(page.getByRole("button", { name: /Open System Logs/i })).toBeVisible();

  await page.keyboard.press("Escape");
  await expect(page.getByLabel("Command palette search")).toBeHidden();

  expect(errors).toEqual([]);
});

test("fits the minimum desktop window without horizontal overflow", async ({ page }) => {
  const errors = failOnBrowserErrors(page);
  await page.setViewportSize({ width: 900, height: 720 });
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Local inference workbench" })).toBeVisible();
  await expect(page.getByLabel("Prompt input")).toBeVisible();

  const layout = await page.evaluate(() => ({
    clientWidth: document.documentElement.clientWidth,
    scrollWidth: document.documentElement.scrollWidth,
    bodyScrollWidth: document.body.scrollWidth,
  }));

  expect(Math.max(layout.scrollWidth, layout.bodyScrollWidth)).toBeLessThanOrEqual(layout.clientWidth + 2);
  expect(errors).toEqual([]);
});
