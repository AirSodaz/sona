/// <reference types="vitest" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/__tests__/setup.ts'],
    exclude: ['**/node_modules/**', 'tests/e2e/**'],
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
        protocol: "ws",
        host,
        port: 1421,
      }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
  optimizeDeps: {
    entries: ["index.html", "src/main.tsx"],
    exclude: ["@tauri-apps/api", "@tauri-apps/plugin-shell"]
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          const normalizedId = id.replace(/\\/g, "/");

          if (normalizedId.includes("/node_modules/react-markdown/")
            || normalizedId.includes("/node_modules/remark-gfm/")
            || normalizedId.includes("/node_modules/remark-breaks/")) {
            return "markdown-vendor";
          }

          if (normalizedId.includes("/node_modules/@tauri-apps/")) {
            return "tauri-vendor";
          }

          if (normalizedId.includes("/node_modules/@dnd-kit/")) {
            return "dnd-vendor";
          }

          if (normalizedId.includes("/node_modules/lucide-react/")) {
            return "icons-vendor";
          }

          if (normalizedId.includes("/node_modules/i18next/")
            || normalizedId.includes("/node_modules/react-i18next/")
            || normalizedId.includes("/node_modules/i18next-browser-languagedetector/")) {
            return "i18n-vendor";
          }

          if (normalizedId.includes("/src/components/DiagnosticsModal.tsx")
            || normalizedId.includes("/src/components/RecoveryCenterModal.tsx")) {
            return "settings-surface";
          }

          if (normalizedId.includes("/src/components/projects/")
            || normalizedId.includes("/src/components/ProjectsView.tsx")) {
            return "projects-surface";
          }

          return undefined;
        },
      },
    },
  },
}));
