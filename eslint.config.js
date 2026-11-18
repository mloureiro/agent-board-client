import js from '@eslint/js';
import { defineConfig, globalIgnores } from 'eslint/config';
import tseslint from 'typescript-eslint';

export default defineConfig(
  globalIgnores(['dist/', 'src-tauri/target/', 'src-tauri/gen/']),
  js.configs.recommended,
  tseslint.configs.strictTypeChecked,
  {
    languageOptions: {
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
  },
  {
    files: ['src/board-notifications.js'],
    extends: [tseslint.configs.disableTypeChecked],
    languageOptions: {
      globals: { window: 'readonly', Event: 'readonly', EventTarget: 'readonly' },
    },
  },
);
