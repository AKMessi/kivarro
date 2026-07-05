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

  await expect(page.getByLabel("Prompt input")).toBeVisible();
  await page.getByLabel("Prompt input").focus();
  await page.keyboard.press("Control+K");
  await expect(page.getByLabel("Command palette search")).toBeVisible();

  await page.getByLabel("Command palette search").fill("logs");
  await expect(page.getByRole("button", { name: /Open System Logs/i })).toBeVisible();

  await page.keyboard.press("Escape");
  await expect(page.getByLabel("Command palette search")).toBeHidden();

  expect(errors).toEqual([]);
});

test("keeps titlebar controls clickable outside the drag region", async ({ page }) => {
  const errors = failOnBrowserErrors(page);
  await page.goto("/");

  await expect(page.locator(".titlebar")).not.toHaveAttribute("data-tauri-drag-region", /.*/);
  await expect(page.locator(".title-identity")).not.toHaveAttribute("data-tauri-drag-region", /.*/);
  await expect(page.locator(".title-actions")).not.toHaveAttribute("data-tauri-drag-region", /.*/);
  await expect(page.locator(".title-command")).not.toHaveAttribute("data-tauri-drag-region", /.*/);
  await expect(page.locator(".quick-actions")).not.toHaveAttribute("data-tauri-drag-region", /.*/);
  await expect(page.locator(".window-controls")).not.toHaveAttribute("data-tauri-drag-region", /.*/);

  await page.getByRole("button", { name: "Command palette" }).click();
  await expect(page.getByLabel("Command palette search")).toBeVisible();
  await page.keyboard.press("Escape");

  await page.getByRole("button", { name: "Minimize window" }).click();
  await page.getByRole("button", { name: "Maximize window" }).click();
  await page.getByRole("button", { name: "Close window" }).click();

  expect(errors).toEqual([]);
});

test("uses the app font stack and formats preview API URLs", async ({ page }) => {
  const errors = failOnBrowserErrors(page);
  await page.goto("/");

  const fontFamily = await page.locator("body").evaluate((node) => getComputedStyle(node).fontFamily);
  expect(fontFamily).toContain("JetBrains Mono");
  expect(fontFamily).toContain("Cascadia Code");

  await page.getByRole("button", { name: "Local API" }).click();
  await page.getByLabel("API host").fill("::1");
  await page.getByLabel("API port").fill("8081");
  await page.getByRole("button", { name: "Save endpoint" }).click();

  await expect(page.locator(".api-status-strip code")).toHaveText("http://[::1]:8081/v1");
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
