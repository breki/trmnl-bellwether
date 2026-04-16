import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { readFileSync, existsSync } from "fs";
import { resolve } from "path";

const rootDir = resolve(import.meta.dirname, "..");

// Read app version from Cargo.toml (single source of truth)
function getAppVersion() {
  const cargoToml = resolve(rootDir, "crates", "rustbase", "Cargo.toml");
  const content = readFileSync(cargoToml, "utf-8");
  const match = content.match(/^version\s*=\s*"([^"]+)"/m);
  if (!match) {
    throw new Error("Could not find version in Cargo.toml");
  }
  return match[1];
}

// Read backend port from ../.ports if it exists
function getBackendPort() {
  const portsFile = resolve(rootDir, ".ports");
  if (!existsSync(portsFile)) return 3000;

  const content = readFileSync(portsFile, "utf-8");
  for (const line of content.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const match = trimmed.match(/^backend_port\s*=\s*(\d+)/);
    if (match) return parseInt(match[1], 10);
  }
  return 3000;
}

const backendPort = getBackendPort();

export default defineConfig({
  plugins: [svelte()],
  define: {
    __APP_VERSION__: JSON.stringify(getAppVersion()),
  },
  server: {
    port: 5173,
    proxy: {
      "/api": {
        target: `http://127.0.0.1:${backendPort}`,
        changeOrigin: true,
      },
      "/health": {
        target: `http://127.0.0.1:${backendPort}`,
        changeOrigin: true,
      },
    },
  },
});
