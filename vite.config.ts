import { defineConfig } from 'vitest/config';

export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ['**/src-tauri/**'] },
  },
  build: {
    target: 'safari16',
  },
  test: {
    include: ['src/**/*.test.ts'],
  },
});
