import { defineConfig, devices } from "@playwright/test";
import { readFileSync, existsSync } from "fs";
import { resolve } from "path";

/**
 * Walk up from process.cwd() until we find Cargo.toml.
 */
function findProjectRoot() {
  let dir = process.cwd();
  for (let i = 0; i < 10; i++) {
    if (existsSync(resolve(dir, "Cargo.toml")))
      return dir;
    const parent = resolve(dir, "..");
    if (parent === dir) break;
    dir = parent;
  }
  return process.cwd();
}

const projectRoot = findProjectRoot();

/**
 * Read port from .ports file if it exists.
 */
function getBackendPort() {
  const portsFile = resolve(projectRoot, ".ports");
  if (!existsSync(portsFile)) return 3100;

  const content = readFileSync(portsFile, "utf-8");
  for (const line of content.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const match = trimmed.match(
      /^backend_port\s*=\s*(\d+)/,
    );
    if (match) return parseInt(match[1], 10);
  }
  return 3100;
}

const backendPort = getBackendPort();
const frontendPort = 5173;

/**
 * @see https://playwright.dev/docs/test-configuration
 */
export default defineConfig({
  testDir: "./e2e/tests",

  timeout: 30 * 1000,
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: "html",

  use: {
    baseURL: `http://localhost:${frontendPort}`,
    trace: "on-first-retry",
    screenshot: "only-on-failure",
  },

  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],

  webServer: [
    {
      command: `cargo run -p bellwether-web -- --port ${backendPort}`,
      cwd: projectRoot,
      url: `http://localhost:${backendPort}/health`,
      reuseExistingServer: true,
      stdout: "pipe",
      timeout: 120 * 1000,
    },
    {
      command: "npm run dev",
      cwd: resolve(projectRoot, "frontend"),
      url: `http://localhost:${frontendPort}`,
      reuseExistingServer: true,
      stdout: "pipe",
      timeout: 120 * 1000,
    },
  ],
});
