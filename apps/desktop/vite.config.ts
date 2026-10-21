import { fileURLToPath, URL } from "node:url";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

const desktopRoot = fileURLToPath(new URL(".", import.meta.url));

export default defineConfig({
  root: desktopRoot,
  plugins: [react()],
  resolve: {
    alias: {
      "@sw/contracts": fileURLToPath(new URL("../../packages/contracts/src/index.ts", import.meta.url)),
    },
  },
  server: { host: "127.0.0.1", port: 1420, strictPort: true },
  clearScreen: false,
  build: { outDir: "dist", emptyOutDir: true },
  test: { environment: "jsdom", globals: true, setupFiles: ["./src/test-setup.ts"] },
});
