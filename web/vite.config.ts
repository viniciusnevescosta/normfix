import { defineConfig } from "vite";

export default defineConfig({
  base: "/",
  build: {
    assetsInlineLimit: 0,
    target: "baseline-widely-available",
  },
  server: {
    host: "127.0.0.1",
    port: 5173,
    strictPort: true,
  },
  preview: {
    host: "127.0.0.1",
    port: 4173,
    strictPort: true,
  },
});
