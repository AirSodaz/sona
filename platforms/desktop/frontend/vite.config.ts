/// <reference types="vitest" />

import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

const host = process.env.TAURI_DEV_HOST;

const ReactCompilerConfig = {
  target: '19',
};

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [
    react({
      babel: {
        plugins: [['babel-plugin-react-compiler', ReactCompilerConfig]],
      },
    }),
  ],
  test: {
    globals: true,
    exclude: ['**/node_modules/**', 'tests/e2e/**', 'playwright-report/**'],
    projects: [
      {
        test: {
          name: 'unit',
          environment: 'node',
          globals: true,
          include: [
            'src/locales/**/*.{test,spec}.?(c|m)[jt]s?(x)',
            'src/types/**/*.{test,spec}.?(c|m)[jt]s?(x)',
            'src/constants/**/*.{test,spec}.?(c|m)[jt]s?(x)',
            'src/utils/**/__tests__/**/*.{test,spec}.?(c|m)[jt]s?(x)',
            'src/stores/__tests__/**/*.{test,spec}.?(c|m)[jt]s?(x)',
            'src/services/llm/**/*.{test,spec}.?(c|m)[jt]s?(x)',
            'src/services/batch/**/*.{test,spec}.?(c|m)[jt]s?(x)',
            'src/services/pipeline/**/*.{test,spec}.?(c|m)[jt]s?(x)',
            'src/services/transcription/**/*.{test,spec}.?(c|m)[jt]s?(x)',
            'src/services/automation/**/*.{test,spec}.?(c|m)[jt]s?(x)',
            'src/services/voiceTyping/**/*.{test,spec}.?(c|m)[jt]s?(x)',
          ],
          exclude: ['**/node_modules/**', 'tests/e2e/**', 'playwright-report/**'],
        },
      },
      {
        test: {
          name: 'dom',
          environment: 'jsdom',
          globals: true,
          setupFiles: ['./src/__tests__/setup.ts'],
          include: [
            'src/components/**/*.{test,spec}.?(c|m)[jt]s?(x)',
            'src/hooks/**/*.{test,spec}.?(c|m)[jt]s?(x)',
            'src/__tests__/**/*.{test,spec}.?(c|m)[jt]s?(x)',
            'src/services/__tests__/**/*.{test,spec}.?(c|m)[jt]s?(x)',
            'src/services/startup/**/*.{test,spec}.?(c|m)[jt]s?(x)',
            'src/services/tauri/**/*.{test,spec}.?(c|m)[jt]s?(x)',
            'src/utils/*.{test,spec}.?(c|m)[jt]s?(x)',
          ],
          exclude: ['**/node_modules/**', 'tests/e2e/**', 'playwright-report/**'],
        },
      },
    ],
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
          protocol: 'ws',
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching the desktop host
      ignored: ['../src/**'],
    },
  },
  optimizeDeps: {
    entries: ['index.html', 'src/main.tsx'],
    exclude: ['@tauri-apps/api'],
  },
  build: {
    rollupOptions: {
      output: {
        strictExecutionOrder: true,
        codeSplitting: {
          includeDependenciesRecursively: false,
          groups: [
            {
              name: 'tauri-vendor',
              test: (id) => id.replace(/\\/g, '/').includes('/node_modules/@tauri-apps/'),
            },
            {
              name: 'dnd-vendor',
              test: (id) => id.replace(/\\/g, '/').includes('/node_modules/@dnd-kit/'),
            },
            {
              name: 'react-vendor',
              test: (id) => {
                const normalizedId = id.replace(/\\/g, '/');
                return (
                  normalizedId.includes('/node_modules/.pnpm/react@') ||
                  normalizedId.includes('/node_modules/.pnpm/react-dom@') ||
                  normalizedId.includes('/node_modules/.pnpm/scheduler@') ||
                  normalizedId.includes('/node_modules/react/') ||
                  normalizedId.includes('/node_modules/react-dom/') ||
                  normalizedId.includes('/node_modules/scheduler/') ||
                  normalizedId.includes('/node_modules/.vite/deps/react.js') ||
                  normalizedId.includes('/node_modules/.vite/deps/react-dom')
                );
              },
            },
            {
              name: 'virtual-list-vendor',
              test: (id) => {
                const normalizedId = id.replace(/\\/g, '/');
                return (
                  normalizedId.includes('/node_modules/.pnpm/react-virtuoso@') ||
                  normalizedId.includes('/node_modules/react-virtuoso/') ||
                  normalizedId.includes('/node_modules/.vite/deps/react-virtuoso')
                );
              },
            },
            {
              name: 'icons-vendor',
              test: (id) => id.replace(/\\/g, '/').includes('/node_modules/lucide-react/'),
            },
            {
              name: 'i18n-vendor',
              test: (id) => {
                const normalizedId = id.replace(/\\/g, '/');
                return (
                  normalizedId.includes('/node_modules/i18next/') ||
                  normalizedId.includes('/node_modules/react-i18next/') ||
                  normalizedId.includes('/node_modules/i18next-browser-languagedetector/') ||
                  normalizedId.includes('/src/i18n.ts')
                );
              },
            },
            {
              name: 'i18n-locales',
              test: (id) => id.replace(/\\/g, '/').includes('/src/locales/'),
            },

            {
              name: 'projects-surface',
              test: (id) => {
                const normalizedId = id.replace(/\\/g, '/');
                return (
                  normalizedId.includes('/src/components/projects/') ||
                  normalizedId.includes('/src/components/ProjectsView.tsx')
                );
              },
            },
          ],
        },
      },
    },
  },
}));
