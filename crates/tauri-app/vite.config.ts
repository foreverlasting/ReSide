import { readFileSync } from "node:fs";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Single source of truth for the displayed version: package.json (which the
// release bump already updates alongside Cargo.toml / tauri.conf.json). Injected
// at build time so the sidebar badge never drifts from the real version again.
const pkg = JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf-8"));

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  define: { __APP_VERSION__: JSON.stringify(pkg.version) },
});
