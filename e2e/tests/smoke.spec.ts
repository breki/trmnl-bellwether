// @ts-check
import { test, expect } from "@playwright/test";

test.describe("smoke tests", () => {
  test("homepage loads", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator("h1")).toContainText(
      "rustbase",
    );
  });

  test("shows API status", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator("dd.ready")).toContainText(
      "ready",
    );
  });

  test("health endpoint returns OK", async ({ request }) => {
    const response = await request.get("/health");
    expect(response.ok()).toBeTruthy();
    const json = await response.json();
    expect(json.status).toBe("ok");
  });

  test("status API returns version", async ({ request }) => {
    const response = await request.get("/api/status");
    expect(response.ok()).toBeTruthy();
    const json = await response.json();
    expect(json.status).toBe("ready");
    expect(json.version).toBeTruthy();
  });

  test("greeting API returns message", async ({
    request,
  }) => {
    const response = await request.get("/api/greeting");
    expect(response.ok()).toBeTruthy();
    const json = await response.json();
    expect(json.message).toBeTruthy();
  });
});
