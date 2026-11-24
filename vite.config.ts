import react from '@vitejs/plugin-react';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';
import { frameworkFreeEntries } from './tooling/framework-free-entries.ts';

export default defineConfig({
  clearScreen: false,
  plugins: [react(), frameworkFreeEntries(['index', 'tabbar'])],
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ['**/src-tauri/**'] },
  },
  build: {
    target: 'safari16',
    rollupOptions: {
      input: {
        index: fileURLToPath(new URL('index.html', import.meta.url)),
        tabbar: fileURLToPath(new URL('tabbar.html', import.meta.url)),
        board: fileURLToPath(new URL('board.html', import.meta.url)),
      },
    },
  },
  test: {
    include: ['src/**/*.test.{ts,tsx}', 'tooling/**/*.test.ts'],
  },
});
