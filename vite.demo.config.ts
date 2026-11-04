import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
// @ts-expect-error type error without @types/node package
import { fileURLToPath } from "node:url";

// Builds the real UI with sample data, for the website's live demo and screenshots.
export default defineConfig({
  plugins: [react()],
  base: "./",
  build: { outDir: "website/demo", emptyOutDir: true },
  resolve: {
    alias: {
      "@tauri-apps/api/core": fileURLToPath(new URL("./src/demo/tauri-mock.ts", import.meta.url)),
    },
  },
});
